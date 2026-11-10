/**
 * The home page's moving parts: the hero card, where every token of
 * `routes.ts` hops into the place tidy puts it, and the two bars below it,
 * which race in real time.
 */

type Mode = "mess" | "tidy";

type Pos = {
  /** The token's text. */
  t: string;
  row: number;
  col: number;
  /** What makes two tokens the same token on both sides of the diff. */
  k: string;
  cls: string;
};

type Tok = {
  /** Where it sits before tidy runs, and after. `null` means it is not there. */
  a: Pos | null;
  b: Pos | null;
  el: HTMLElement;
  /** The two quote marks of a string, which flip as the string lands. */
  qs?: HTMLElement[];
  kind: "enter" | "exit" | "move";
  x: number;
  y: number;
  vx: number;
  vy: number;
  sx: number;
  sy: number;
  vsx: number;
  vsy: number;
  tx: number;
  ty: number;
  tsx: number;
  tsy: number;
  dy: number;
  dyT: number;
  /** When to crouch, and when to jump. `0` is never. */
  antAt: number;
  goAt: number;
};

const KEYWORDS = new Set([
  "import",
  "from",
  "type",
  "const",
  "export",
  "function",
  "return",
  "async",
  "await",
  "new",
  "null",
]);

const TOKEN = /'[^'\n]*'|"[^"\n]*"|=>|===|\?\?|[A-Za-z_$][\w$]*|\S/g;

let quoted = (text: string) => text[0] === "'" || text[0] === '"';

/**
 * Split source into tokens with a row and column, and give each one a class.
 * `k` ignores which quote mark a string uses, so `'ok'` and `"ok"` are the
 * same token on both sides and the diff pairs them up.
 */
function tokenize(source: string): Pos[] {
  let out: Pos[] = [];

  source.split("\n").forEach((line, row) => {
    TOKEN.lastIndex = 0;
    let match: RegExpExecArray | null;
    while ((match = TOKEN.exec(line))) {
      let t = match[0];
      out.push({
        t,
        row,
        col: match.index,
        k: quoted(t) ? "S" + t.slice(1, -1) : t,
        cls: "",
      });
    }
  });

  out.forEach((token, i) => {
    let next = out[i + 1];
    token.cls = quoted(token.t)
      ? "str"
      : KEYWORDS.has(token.t)
        ? "kw"
        : /^[A-Z]/.test(token.t)
          ? "ty"
          : /^[A-Za-z_$]/.test(token.t)
            ? next && next.t === "("
              ? "fn"
              : "id"
            : "pun";
  });

  return out;
}

/**
 * Pair the tokens of the two files by their longest common subsequence, so a
 * token that survives tidy keeps one element and moves, and only the ones
 * that do not have to appear or leave.
 */
function pair(a: Pos[], b: Pos[]): { a: Pos | null; b: Pos | null }[] {
  let n = a.length;
  let m = b.length;
  let best = Array.from({ length: n + 1 }, () => new Uint16Array(m + 1));

  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      best[i][j] =
        a[i].k === b[j].k
          ? best[i + 1][j + 1] + 1
          : Math.max(best[i + 1][j], best[i][j + 1]);
    }
  }

  let out: { a: Pos | null; b: Pos | null }[] = [];
  let i = 0;
  let j = 0;

  while (i < n || j < m) {
    if (i < n && j < m && a[i].k === b[j].k) {
      out.push({ a: a[i++], b: b[j++] });
    } else if (j >= m || (i < n && best[i + 1][j] >= best[i][j + 1])) {
      out.push({ a: a[i++], b: null });
    } else {
      out.push({ a: null, b: b[j++] });
    }
  }

  return out;
}

/** The hero card: `routes.ts`, tidying itself up. */
function stage(host: HTMLElement) {
  let board = host.querySelector<HTMLElement>("[data-stage]");
  let label = host.querySelector<HTMLElement>("[data-state]");
  let replay = host.querySelector<HTMLElement>("[data-replay]");
  let before = board?.querySelector<HTMLElement>("[data-mess]");
  let after = board?.querySelector<HTMLElement>("[data-tidy]");
  if (!board || !label || !replay || !before || !after) return;

  let reduce = matchMedia("(prefers-reduced-motion: reduce)");
  let messy = tokenize(before.textContent ?? "");
  let tidied = tokenize(after.textContent ?? "");
  let lines = (before.textContent ?? "").split("\n");
  let tidyLines = (after.textContent ?? "").split("\n");

  let columns =
    Math.max(...lines.concat(tidyLines).map((line) => line.length)) + 1;
  let rows = Math.max(lines.length, tidyLines.length);

  let toks: Tok[] = pair(messy, tidied).map(({ a, b }) => {
    let source = (b || a)!;
    let el = document.createElement("span");
    el.className = "glyph c-" + source.cls + (a ? "" : " c-new");

    let tok: Tok = {
      a,
      b,
      el,
      kind: "move",
      x: 0,
      y: 0,
      vx: 0,
      vy: 0,
      sx: 1,
      sy: 1,
      vsx: 0,
      vsy: 0,
      tx: 0,
      ty: 0,
      tsx: 1,
      tsy: 1,
      dy: 0,
      dyT: 0,
      antAt: 0,
      goAt: 0,
    };

    if (source.cls === "str") {
      let open = document.createElement("span");
      let body = document.createElement("span");
      let close = document.createElement("span");
      open.className = close.className = "q";
      open.textContent = close.textContent = (a || b)!.t[0];
      body.textContent = source.t.slice(1, -1);
      el.append(open, body, close);
      tok.qs = [open, close];
    } else {
      el.textContent = source.t;
    }

    return tok;
  });

  before.remove();
  after.remove();
  for (let tok of toks) board.append(tok.el);

  let cell = 9;
  let height = 22;

  function measure() {
    let probe = document.createElement("span");
    probe.style.cssText =
      "position:absolute;visibility:hidden;white-space:pre;font:400 100px var(--mono)";
    probe.textContent = "x".repeat(50);
    board!.append(probe);
    let ratio = probe.getBoundingClientRect().width / 5000;
    probe.remove();

    let size = Math.max(
      8.5,
      Math.min(15.5, board!.clientWidth / (columns * ratio)),
    );
    board!.style.fontSize = size + "px";
    cell = size * ratio;
    height = Math.round(size * 1.62);
    board!.style.height = rows * height + "px";
    board!.style.lineHeight = height + "px";
  }

  let at = (pos: Pos) => ({ x: pos.col * cell, y: pos.row * height });

  function draw(tok: Tok, speed: number) {
    let stretch = Math.min(0.14, speed / 5000);
    let tilt = Math.max(-10, Math.min(10, tok.vx / 90));
    let scale = Math.min(tok.sx, tok.sy);
    tok.el.style.transform =
      `translate(${tok.x.toFixed(2)}px,${(tok.y + tok.dy).toFixed(2)}px)` +
      ` rotate(${tilt.toFixed(2)}deg)` +
      ` scale(${(tok.sx * (1 + stretch)).toFixed(3)},${(tok.sy * (1 - stretch)).toFixed(3)})`;
    tok.el.style.opacity = Math.max(0, Math.min(1, scale * 1.6)).toFixed(3);
  }

  let mode: Mode = "mess";
  let generation = 0;
  let running = false;
  let last = 0;

  function snap() {
    for (let tok of toks) {
      let here = mode === "tidy" ? tok.b : tok.a;
      let { x, y } = at((here || tok.a || tok.b)!);
      Object.assign(tok, {
        x,
        y,
        tx: x,
        ty: y,
        vx: 0,
        vy: 0,
        vsx: 0,
        vsy: 0,
        dy: 0,
        dyT: 0,
        antAt: 0,
        goAt: 0,
      });
      tok.sx = tok.sy = tok.tsx = tok.tsy = here ? 1 : 0;
      let quote = mode === "tidy" ? '"' : (tok.a || tok.b)!.t[0];
      for (let q of tok.qs ?? []) q.textContent = quote;
      draw(tok, 0);
    }
  }

  function settle(mine: number) {
    if (mine !== generation) return;
    label!.classList.remove("tidy", "pop");
    if (mode !== "tidy") {
      label!.textContent = "as written";
      return;
    }
    label!.textContent = "✓ tidy";
    label!.classList.add("tidy");
    void label!.offsetWidth;
    if (!reduce.matches) label!.classList.add("pop");
  }

  function flip(tok: Tok, quote: string) {
    tok.qs?.forEach((q, i) => {
      setTimeout(() => {
        q.classList.add("flip");
        setTimeout(() => {
          q.textContent = quote;
          q.classList.remove("flip");
        }, 90);
      }, i * 70);
    });
  }

  /**
   * The tokens that do not belong pop out first. Everything that stays
   * crouches, hops along an arc, overshoots and settles, top to bottom. The
   * new tokens land last.
   */
  function run(next: Mode) {
    mode = next;
    let mine = ++generation;
    label!.classList.remove("tidy", "pop");
    label!.textContent = mode === "tidy" ? "tidying…" : "as written";

    if (reduce.matches) {
      snap();
      settle(mine);
      return;
    }

    let now = performance.now();
    let tidying = mode === "tidy";
    let slowest = 0;

    for (let tok of toks) {
      let target = tidying ? tok.b : tok.a;
      let from = tidying ? tok.a : tok.b;
      tok.antAt = tok.goAt = 0;

      if (!target) {
        tok.kind = "exit";
        tok.goAt = now + (from || tok.a || tok.b)!.row * 18;
      } else if (from) {
        tok.kind = "move";
        let delay = 110 + target.row * 46 + target.col * 2.2;
        tok.goAt = now + delay;
        tok.antAt = tok.goAt - 85;
        slowest = Math.max(slowest, delay);
      }
    }

    for (let tok of toks) {
      let target = tidying ? tok.b : tok.a;
      if (!target || (tidying ? tok.a : tok.b)) continue;
      tok.kind = "enter";
      tok.goAt = now + slowest + 300 + target.row * 32 + target.col * 1.5;
      let { x, y } = at(target);
      if (tok.sx < 0.05) Object.assign(tok, { x, y, vx: 0, vy: 0 });
      tok.tx = x;
      tok.ty = y;
    }

    setTimeout(() => settle(mine), slowest + 1050);
    kick();
  }

  function fire(tok: Tok, now: number) {
    if (tok.antAt && now >= tok.antAt) {
      tok.antAt = 0;
      tok.tsx = 1.1;
      tok.tsy = 0.84;
      tok.dyT = 3;
    }
    if (!tok.goAt || now < tok.goAt) return;

    tok.goAt = 0;
    tok.dyT = 0;
    let tidying = mode === "tidy";

    if (tok.kind === "exit") {
      tok.tsx = tok.tsy = 0;
      tok.vy -= 140;
      return;
    }
    if (tok.kind === "enter") {
      tok.tsx = tok.tsy = 1;
      tok.vsy += 6;
      tok.vsx -= 2;
      return;
    }

    let { x, y } = at((tidying ? tok.b : tok.a)!);
    let far = Math.hypot(x - tok.x, y - tok.y);
    tok.tx = x;
    tok.ty = y;
    tok.tsx = tok.tsy = 1;
    tok.vy -= 150 + Math.min(far, 380) * 1.15;

    let quote = tidying ? '"' : (tok.a || tok.b)!.t[0];
    if (tok.qs && tok.qs[0].textContent !== quote) {
      setTimeout(() => flip(tok, quote), 240);
    }
  }

  function step(dt: number): boolean {
    let busy = false;
    let now = performance.now();

    for (let tok of toks) {
      if (tok.antAt || tok.goAt) {
        fire(tok, now);
        busy = true;
      }

      for (let i = 0; i < 4; i++) {
        let h = dt / 4;
        tok.vx += (210 * (tok.tx - tok.x) - 19 * tok.vx) * h;
        tok.vy += (210 * (tok.ty - tok.y) - 19 * tok.vy) * h;
        tok.x += tok.vx * h;
        tok.y += tok.vy * h;
        tok.dy += (tok.dyT - tok.dy) * Math.min(1, h * 30);
        tok.vsx += (520 * (tok.tsx - tok.sx) - 21 * tok.vsx) * h;
        tok.vsy += (300 * (tok.tsy - tok.sy) - 13 * tok.vsy) * h;
        tok.sx += tok.vsx * h;
        tok.sy += tok.vsy * h;
      }

      if (tok.tsx === 0 && tok.sx < 0.02) {
        tok.sx = tok.sy = tok.vsx = tok.vsy = 0;
      }

      let speed = Math.hypot(tok.vx, tok.vy);
      if (
        speed > 2 ||
        Math.abs(tok.tx - tok.x) > 0.3 ||
        Math.abs(tok.ty - tok.y) > 0.3 ||
        Math.abs(tok.vsx) > 0.02 ||
        Math.abs(tok.vsy) > 0.02 ||
        Math.abs(tok.tsx - tok.sx) > 0.005 ||
        Math.abs(tok.dy - tok.dyT) > 0.05
      ) {
        busy = true;
      }

      draw(tok, speed);
    }

    return busy;
  }

  function kick() {
    if (running) return;
    running = true;
    last = 0;
    requestAnimationFrame(function loop(time) {
      if (!host.isConnected) return void (running = false);
      let dt = last ? (time - last) / 1000 : 0.016;
      last = time;
      if (step(Math.min(0.034, dt > 0 ? dt : 0.016))) {
        requestAnimationFrame(loop);
      } else {
        running = false;
      }
    });
  }

  /** Put the mess back, let it be read for a beat, then tidy it again. */
  function again() {
    mode = "mess";
    generation++;
    snap();
    settle(generation);
    replay!.classList.remove("spin");
    void replay!.offsetWidth;
    replay!.classList.add("spin");
    setTimeout(() => mode === "mess" && run("tidy"), reduce.matches ? 0 : 500);
  }

  replay.addEventListener("click", again);
  board.addEventListener("click", again);

  let resized: ReturnType<typeof setTimeout>;
  addEventListener("resize", () => {
    clearTimeout(resized);
    resized = setTimeout(() => {
      if (!host.isConnected) return;
      measure();
      generation++;
      snap();
      settle(generation);
    }, 80);
  });

  void (document.fonts ? document.fonts.ready : Promise.resolve()).then(() => {
    if (!host.isConnected) return;
    measure();
    snap();
    setTimeout(() => mode === "mess" && generation === 0 && run("tidy"), 700);
  });
}

/** tidy and Prettier, over the whole corpus, at the speed it really happens. */
function race(root: HTMLElement) {
  let fills = {
    tidy: root.querySelector<HTMLElement>('[data-fill="tidy"]'),
    prettier: root.querySelector<HTMLElement>('[data-fill="prettier"]'),
  };
  let times = {
    tidy: root.querySelector<HTMLElement>('[data-time="tidy"]'),
    prettier: root.querySelector<HTMLElement>('[data-time="prettier"]'),
  };
  let badge = root.querySelector<HTMLElement>("[data-badge]");
  let note = root.querySelector<HTMLElement>("[data-race-note]");
  let rerun = root.querySelector<HTMLElement>("[data-rerun]");
  if (!fills.tidy || !fills.prettier || !times.tidy || !times.prettier) return;
  if (!badge || !note || !rerun) return;

  let reduce = matchMedia("(prefers-reduced-motion: reduce)");
  let settled = note.textContent ?? "";
  let quick = Number(root.dataset.tidy) || 280;
  let slow = Number(root.dataset.prettier) || 18548;
  let files = root.dataset.files ?? "";
  let show = (ms: number) =>
    ms < 1000 ? Math.round(ms) + "ms" : (ms / 1000).toFixed(1) + "s";

  let current = 0;

  function go() {
    if (reduce.matches) return;
    let mine = ++current;
    let start = performance.now();
    let won = false;

    badge!.hidden = true;
    badge!.classList.remove("pop");
    note!.textContent = `Formatting ${files} files…`;

    requestAnimationFrame(function tick(time) {
      if (mine !== current || !root.isConnected) return;
      let gone = time - start;
      fills.tidy!.style.transform = `scaleX(${Math.min(1, gone / quick)})`;
      fills.prettier!.style.transform = `scaleX(${Math.min(1, gone / slow)})`;
      times.tidy!.textContent = show(Math.min(gone, quick));
      times.prettier!.textContent = show(Math.min(gone, slow));

      if (gone >= quick && !won) {
        won = true;
        badge!.hidden = false;
        badge!.classList.add("pop");
        note!.textContent = "tidy is done. Prettier is still going.";
      }

      if (gone < slow) requestAnimationFrame(tick);
      else note!.textContent = settled;
    });
  }

  rerun.addEventListener("click", go);
  return go;
}

/** Each set of language bars grows once, when it first scrolls into view. */
function reveal(race: (() => void) | undefined, bars: HTMLElement | null) {
  let reduce = matchMedia("(prefers-reduced-motion: reduce)");
  let watcher = new IntersectionObserver(
    (entries) => {
      for (let entry of entries) {
        if (!entry.isIntersecting) continue;
        watcher.unobserve(entry.target);
        if (entry.target === bars) entry.target.classList.add("go");
        else race?.();
      }
    },
    { threshold: 0.4 },
  );

  let board = document.querySelector("[data-race]");
  if (board && race) watcher.observe(board);
  if (bars && !reduce.matches) watcher.observe(bars);
}

export function mount(host: HTMLElement) {
  stage(host);
  let board = document.querySelector<HTMLElement>("[data-race]");
  reveal(
    board ? race(board) : undefined,
    document.querySelector<HTMLElement>("[data-langs]"),
  );
}
