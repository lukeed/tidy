/**
 * Remember the choice, so the inline script in `<head>` can apply it before
 * paint.
 */
function theme() {
  let root = document.documentElement;
  let next = root.dataset.theme === "dark" ? "light" : "dark";
  root.dataset.theme = next;
  localStorage.setItem("theme", next);
}

/** Copy, then say so on the button that was pressed. */
async function copy(button: HTMLElement, text: string) {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    return;
  }
  let before = button.textContent;
  button.textContent = "copied";
  setTimeout(() => (button.textContent = before), 1400);
}

document.addEventListener("click", (event) => {
  let target = event.target as HTMLElement;

  let toggle = target.closest<HTMLElement>("[data-theme-toggle]");
  if (toggle) return theme();

  let copier = target.closest<HTMLElement>("[data-copy]");
  if (copier) return void copy(copier, copier.dataset.copy!);

  let block = target.closest<HTMLElement>("[data-copy-code]");
  if (block) {
    let code = block.closest("figure")?.querySelector("code");
    if (code) return void copy(block, code.textContent ?? "");
  }

  let menu = target.closest<HTMLElement>("[data-menu]");
  if (menu) {
    let open = menu.getAttribute("aria-expanded") === "true";
    menu.setAttribute("aria-expanded", String(!open));
    document.getElementById("side")?.classList.toggle("open", !open);
    return;
  }

  if (target.closest("[data-search]")) return void search();
});

// ───────────────────────────── on this page ─────────────────────────────

/** Mark the heading the reader is in, so the contents list follows the page. */
function spy() {
  let links = [...document.querySelectorAll<HTMLAnchorElement>(".toc a")];
  if (!links.length) return;

  let byId = new Map(
    links.map((link) => [decodeURIComponent(link.hash.slice(1)), link]),
  );
  let seen = new Set<string>();

  let observer = new IntersectionObserver(
    (entries) => {
      for (let entry of entries) {
        if (entry.isIntersecting) seen.add(entry.target.id);
        else seen.delete(entry.target.id);
      }
      let current = [...byId.keys()].find((id) => seen.has(id));
      for (let [id, link] of byId)
        link.toggleAttribute("aria-current", id === current);
    },
    { rootMargin: "-80px 0px -70% 0px" },
  );

  for (let id of byId.keys()) {
    let heading = document.getElementById(id);
    if (heading) observer.observe(heading);
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
      : index!.slice(0, 8).map((entry) => ({ entry }));

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

spy();
document.addEventListener("bongo:navigate", spy as EventListener);
