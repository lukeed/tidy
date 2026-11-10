import { on } from "bongo/client";

import { paint } from "./paint";

/** Copy, then say so on the button that was pressed. */
async function copy(button: HTMLElement, text: string) {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    return;
  }
  let before = button.textContent;
  button.textContent = "copied";
  button.classList.add("ok");
  setTimeout(() => {
    button.textContent = before;
    button.classList.remove("ok");
  }, 1400);
}

/** The docs sidebar is a drawer on a phone, over a scrim. */
function drawer(open: boolean) {
  let root = document.documentElement;
  if (open) root.dataset.open = "";
  else delete root.dataset.open;
  document
    .querySelector("[data-menu]")
    ?.setAttribute("aria-expanded", String(open));
}

document.addEventListener("click", (event) => {
  let target = event.target as HTMLElement;

  let copier = target.closest<HTMLElement>("[data-copy]");
  if (copier) return void copy(copier, copier.dataset.copy!);

  let block = target.closest<HTMLElement>("[data-copy-code]");
  if (block) {
    let code = block.closest("figure")?.querySelector("code");
    if (code) return void copy(block, code.textContent ?? "");
  }

  if (target.closest("[data-menu]")) {
    return drawer(!("open" in document.documentElement.dataset));
  }

  if (target.closest(".scrim")) return drawer(false);
  if (target.closest("[data-search]")) return void search();
});

addEventListener("keydown", (event) => {
  if (event.key === "Escape") drawer(false);
});

// ───────────────────────────── on this page ─────────────────────────────

/**
 * Mark the heading the reader has reached, and spring the marker to it. The
 * marker is one sticker that moves, rather than a border on each link, so the
 * list reads as a single place you are in.
 */
let marks: { links: HTMLAnchorElement[]; heads: (HTMLElement | null)[] } = {
  links: [],
  heads: [],
};
let at = -2;

function place(index: number) {
  if (index === at) return;
  at = index;

  let { links } = marks;
  for (let [i, link] of links.entries()) {
    link.toggleAttribute("aria-current", i === index);
  }

  let marker = document.querySelector<HTMLElement>(".marker");
  if (!marker) return;

  let link = links[index];
  if (!link) {
    marker.style.height = "0px";
    marker.style.borderWidth = "0";
    return;
  }

  marker.style.borderWidth = "";
  marker.style.transform = `translateY(${link.offsetTop}px)`;
  marker.style.height = link.offsetHeight + "px";

  // A long list scrolls on its own, so keep the marker in sight.
  let box = link.closest<HTMLElement>(".toc");
  let list = link.offsetParent as HTMLElement | null;
  if (!box || !list) return;
  let top = list.offsetTop + link.offsetTop;
  if (top >= box.scrollTop && top <= box.scrollTop + box.clientHeight - 60) {
    return;
  }
  box.scrollTo({
    top: top - 80,
    behavior: matchMedia("(prefers-reduced-motion: reduce)").matches
      ? "auto"
      : "smooth",
  });
}

/**
 * The last heading above a line near the top of the window, rather than
 * whichever one a band happens to hold: past the final heading the reader is
 * still in its section, and a long table or code block leaves a band empty.
 */
function spy() {
  let index = -1;
  for (let [i, head] of marks.heads.entries()) {
    if (head && head.getBoundingClientRect().top <= 140) index = i;
  }
  place(Math.max(0, index));
}

/** Read the list the page just rendered, then mark where the reader is. */
function watch() {
  let links = [...document.querySelectorAll<HTMLAnchorElement>(".toc a")];
  marks = {
    links,
    heads: links.map((link) =>
      document.getElementById(decodeURIComponent(link.hash.slice(1))),
    ),
  };
  at = -2;
  spy();
}

let pending = 0;
addEventListener(
  "scroll",
  () => {
    if (pending) return;
    pending = requestAnimationFrame(() => {
      pending = 0;
      spy();
    });
  },
  { passive: true },
);

addEventListener("resize", () => {
  at = -2;
  spy();
});

// ──────────────────────── sidebar and header ────────────────────────

/**
 * The sidebar and the header are `transition:persist`, so the client keeps the
 * elements the server rendered for the first page and their `aria-current`
 * never moves. Set it from the URL after every navigation instead.
 */
function here(path: string) {
  for (let link of document.querySelectorAll<HTMLAnchorElement>(".side a")) {
    let href = link.getAttribute("href") ?? "";
    let state =
      path === href
        ? "page"
        : href !== "/" && path.startsWith(href)
          ? "true"
          : null;
    if (state) link.setAttribute("aria-current", state);
    else link.removeAttribute("aria-current");
  }

  // Only the two sections, as the server renders them. The shortcuts beside
  // them live under the docs, and would else mark a second link.
  for (let link of document.querySelectorAll<HTMLAnchorElement>(".top nav a")) {
    let href = link.getAttribute("href") ?? "";
    let mine =
      (href === "/docs/" && path.startsWith("/docs/")) ||
      (href === "/playground/" && path === "/playground/");
    link.toggleAttribute("aria-current", mine);
  }
}

// ───────────────────────────── search ─────────────────────────────

type Entry = {
  title: string;
  path: string;
  group: string;
  summary: string;
  headings: [string, string][];
  text: string;
};

type Hit = { entry: Entry; heading?: [string, string] };

let index: Entry[] | undefined;

/**
 * How many times `query` appears in `text`, capped so one long page cannot win
 * on length alone.
 */
function count(text: string, query: string): number {
  let total = 0;
  let at = text.indexOf(query);
  while (at !== -1 && total < 20) {
    total++;
    at = text.indexOf(query, at + query.length);
  }
  return total;
}

/**
 * A title match beats a heading, which beats the summary, which beats the body.
 * Within the body, the page that says it most often wins — which is why
 * "ignore" finds "Ignoring code" even though its title does not contain it.
 */
function rank(entries: Entry[], query: string): Hit[] {
  let hits: (Hit & { score: number })[] = [];

  for (let entry of entries) {
    let titled = entry.title.toLowerCase().includes(query);
    // A sub-heading link only helps when the title itself did not match.
    let heading = titled
      ? undefined
      : entry.headings.find(([, text]) => text.toLowerCase().includes(query));
    let score =
      (titled ? 60 : 0) +
      (heading ? 25 : 0) +
      (entry.summary.toLowerCase().includes(query) ? 15 : 0) +
      count(entry.text.toLowerCase(), query);

    if (score > 0) hits.push({ entry, heading, score });
  }

  return hits.sort((a, b) => b.score - a.score);
}

const ESCAPE: Record<string, string> = {
  "&": "&amp;",
  "<": "&lt;",
  ">": "&gt;",
  '"': "&quot;",
};

let escape = (text: string) => text.replace(/[&<>"]/g, (char) => ESCAPE[char]);

async function search() {
  let dialog = document.getElementById("search") as HTMLDialogElement | null;
  if (!dialog) return;

  index ??= await fetch("/search.json").then(
    (response) => response.json() as Promise<Entry[]>,
  );

  let input = dialog.querySelector<HTMLInputElement>("[data-search-input]")!;
  let results = dialog.querySelector<HTMLElement>("[data-search-results]")!;

  function draw() {
    let query = input.value.trim().toLowerCase();
    let hits = query
      ? rank(index!, query).slice(0, 12)
      : index!.slice(0, 8).map((entry): Hit => ({ entry }));

    results.innerHTML = hits.length
      ? hits
          .map(({ entry, heading }) => {
            let href = heading ? `${entry.path}#${heading[0]}` : entry.path;
            let title = heading
              ? `${escape(entry.title)} &rsaquo; ${escape(heading[1])}`
              : escape(entry.title);
            return `<a href="${href}"><em>${escape(entry.group)}</em><b>${title}</b><span>${escape(entry.summary)}</span></a>`;
          })
          .join("")
      : '<p class="empty">Nothing matches that.</p>';
  }

  input.oninput = draw;
  draw();
  dialog.showModal();
  input.focus();
  input.select();
}

document.addEventListener("keydown", (event) => {
  if ((event.metaKey || event.ctrlKey) && event.key === "k") {
    event.preventDefault();
    void search();
  }
});

/**
 * Custom elements, so the browser mounts them whenever they enter the document.
 * The client runtime replaces `<body>`, and a script inside replaced markup
 * never runs, but an upgrade does.
 */
class TidyPlayground extends HTMLElement {
  connectedCallback() {
    void import("./play").then((module) => module.mount(this));
  }
}

class TidyHero extends HTMLElement {
  connectedCallback() {
    void import("./home").then((module) => module.mount(this));
  }
}

customElements.define("tidy-playground", TidyPlayground);
customElements.define("tidy-hero", TidyHero);

paint();
watch();
here(location.pathname);

on("bongo:navigated", ({ to }) => {
  here(to.pathname);
  drawer(false);
  watch();
});
