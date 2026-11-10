/**
 * The paint pots. Picking one writes `data-scheme` on `<html>`, which every
 * colour on the site reads, and remembers it for the inline script in `<head>`
 * to apply before the next page paints.
 */

const SCHEMES = [
  "cobalt",
  "bubblegum",
  "tangerine",
  "sprout",
  "candy",
  "glow",
  "crayon",
];

let root = document.documentElement;
let current = () => root.dataset.scheme || SCHEMES[0];

/** Squash the pot, as if it were just set down. */
function splat(pot: Element | null) {
  if (!pot) return;
  pot.classList.remove("splat");
  void (pot as HTMLElement).offsetWidth;
  pot.classList.add("splat");
}

/**
 * A named element is captured on its own, so it would cross-fade to the new
 * scheme while the rest of the page waits for the splash to reach it. Drop the
 * names for the one transition, so the whole page wipes together.
 */
function unname(): () => void {
  let named = [
    ...document.querySelectorAll<HTMLElement>(
      '[style*="view-transition-name"]',
    ),
  ];
  let names = named.map((el) => el.style.viewTransitionName);
  for (let el of named) el.style.viewTransitionName = "none";
  return () =>
    named.forEach((el, i) => (el.style.viewTransitionName = names[i]));
}

/**
 * A press during a splash starts a second one, and the browser drops the first.
 * Two things have to follow it out.
 *
 * The clip runs on a pseudo-element of the transition that was dropped, and an
 * animation left on one of those takes the renderer down with it, so it is
 * cancelled before the next begins. And the cleanup of a dropped transition
 * must not run: it would take the names back while the live one still needs
 * them gone, and `data-wiping` with them, which is what tells the new page to
 * clip in over the old rather than above it.
 */
let splash = 0;
let rename: (() => void) | undefined;
let clip: Animation | undefined;

function wipe(from: Element, apply: () => void) {
  let mine = ++splash;
  clip?.cancel();
  // Only the first of a run reads the real names: by the second they are
  // already gone, and restoring what it read would lose them for good.
  rename ??= unname();
  root.dataset.wiping = "";

  let done = () => {
    if (mine !== splash) return;
    delete root.dataset.wiping;
    rename?.();
    rename = undefined;
    clip = undefined;
  };

  let box = from.getBoundingClientRect();
  let x = box.left + box.width / 2;
  let y = box.top + box.height / 2;
  let reach = Math.hypot(
    Math.max(x, innerWidth - x),
    Math.max(y, innerHeight - y),
  );

  let transition = document.startViewTransition(apply);
  void transition.finished.then(done, done);
  void transition.ready
    .then(() => {
      if (mine !== splash) return;
      clip = root.animate(
        {
          clipPath: [
            `circle(0px at ${x}px ${y}px)`,
            `circle(${reach * 0.12}px at ${x}px ${y}px)`,
            `circle(${reach}px at ${x}px ${y}px)`,
          ],
          offset: [0, 0.18, 1],
        },
        {
          duration: 560,
          easing: "cubic-bezier(.3,.7,.2,1)",
          pseudoElement: "::view-transition-new(root)",
        },
      );
    })
    .catch(() => {});
}

export function paint() {
  let button = document.querySelector<HTMLElement>("[data-paint]");
  let tray = document.getElementById("tray");
  if (!button || !tray) return;

  let options = [...tray.querySelectorAll<HTMLElement>(".opt")];
  let mark = () => {
    for (let option of options) {
      option.setAttribute(
        "aria-pressed",
        String(option.dataset.scheme === current()),
      );
    }
  };

  function open(on: boolean) {
    tray!.hidden = !on;
    button!.setAttribute("aria-expanded", String(on));
    tray!.classList.remove("open");
    if (!on) return;
    void tray!.offsetWidth;
    tray!.classList.add("open");
  }

  function pick(name: string, from: Element) {
    if (!SCHEMES.includes(name) || name === current()) return;
    let apply = () => {
      root.dataset.scheme = name;
      mark();
    };

    try {
      localStorage.setItem("scheme", name);
    } catch {
      // A browser that refuses storage still gets the scheme for this page.
    }

    if (
      matchMedia("(prefers-reduced-motion: reduce)").matches ||
      !document.startViewTransition
    ) {
      apply();
      return;
    }

    wipe(from, apply);
  }

  button.addEventListener("click", (event) => {
    event.stopPropagation();
    open(Boolean(tray.hidden));
  });

  document.addEventListener("click", (event) => {
    let target = event.target as HTMLElement;
    if (!tray.hidden && !target.closest(".paint")) open(false);
  });

  /** Where a keyboard pick splashes from, since no pot was pressed. */
  let here = () => button!.querySelector(".pot");

  /**
   * Whether a bare letter belongs to the page rather than to whoever is typing.
   * A field, a dialog, a held modifier or an IME all mean no.
   */
  function loose(event: KeyboardEvent): boolean {
    if (event.metaKey || event.ctrlKey || event.altKey) return false;
    if (event.repeat || event.isComposing) return false;
    if (document.querySelector("dialog[open]")) return false;
    let at = event.target as HTMLElement | null;
    return !at?.closest("input, textarea, select, [contenteditable]");
  }

  addEventListener("keydown", (event) => {
    if (event.key === "Escape" && !tray.hidden) {
      open(false);
      button.focus();
      return;
    }

    if (!loose(event)) return;

    // `c` walks the pots in order, and wraps.
    if (event.key === "c") {
      let next = SCHEMES[(SCHEMES.indexOf(current()) + 1) % SCHEMES.length];
      let pot = here();
      splat(pot);
      if (pot) pick(next, pot);
      return;
    }

    // A number takes its pot, while the tray is open to show which is which.
    if (tray.hidden) return;
    let nth = Number(event.key);
    let option = Number.isInteger(nth) ? options[nth - 1] : undefined;
    if (!option) return;
    event.preventDefault();
    let pot = option.querySelector(".pot");
    splat(pot);
    if (pot) pick(option.dataset.scheme!, pot);
  });

  for (let option of options) {
    option.addEventListener("click", () => {
      let pot = option.querySelector(".pot");
      splat(pot);
      if (pot) pick(option.dataset.scheme!, pot);
    });
  }

  mark();
}
