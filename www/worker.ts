import { crowne } from "crowne/server";

// @ts-expect-error — wrangler's Text module rule turns this into a string.
import script from "./install.sh";

type Stored = {
  body?: ReadableStream;
  httpEtag: string;
  size: number;
};

type Env = {
  CROWNE_SITE: string;
  CROWNE_KEY: string;
  ASSETS: {
    fetch(request: Request): Promise<Response>;
  };
  /**
   * The `tidy-releases` bucket: `latest` names the newest tag, and
   * `<tag>/<archive>` holds each file the release built.
   */
  RELEASES: {
    head(key: string): Promise<Stored | null>;
    get(key: string): Promise<Required<Stored> | null>;
  };
};

/**
 * Anchored, so a browser whose user agent merely contains "curl" still gets the
 * page. Only these ask for the script at `/`.
 */
const CLI = /^(curl|wget|libfetch|httpie|python-requests|powershell|fetch)/i;

/** `tidy-<target>.tar.gz`, `tidy-<target>.zip`, and the `.sha256` of each. */
const RELEASE =
  /^\/download\/(v\d+\.\d+\.\d+[\w.+-]*)\/(tidy-([a-z\d_-]+)\.(tar\.gz|zip)(\.sha256)?)$/;

const TEXT = "text/plain; charset=utf-8";

const SCRIPT_HEADERS = {
  "content-type": "text/x-shellscript; charset=utf-8",
  "cache-control": "public, max-age=300",
};

/**
 * The site is static assets, except for these paths:
 *
 * - `/x/script.js` and `/x` proxy crowne.io, so an ad blocker is less likely to
 *   drop them. @see https://crowne.io/docs/proxy/#cloudflare-workers
 * - `/` answers with `install.sh` when a command line asks for it, so that `curl
 *   -fsSL https://tidy.lukeed.com | sh` works and the browser still gets the
 *   home page.
 * - `/install.sh` always answers with the script.
 * - `/download/latest` names the newest tag, and `/download/<tag>/<archive>`
 *   serves that file out of R2, which the release workflow fills from the same
 *   artifacts it attaches to the GitHub release.
 *
 * Serving the script counts a `download` event, and serving an archive counts
 * an `archive` event with its version and target.
 */
export default {
  async fetch(
    req: Request,
    env: Env,
    ctx: ExecutionContext,
  ): Promise<Response> {
    let { pathname } = new URL(req.url);

    let isEvent = req.method === "POST" && pathname === "/x";
    let isScript = req.method === "GET" && pathname === "/x/script.js";
    if (isScript || isEvent) {
      return crowne({ key: env.CROWNE_KEY, site: env.CROWNE_SITE }).proxy(req);
    }

    let isRead = req.method === "GET" || req.method === "HEAD";

    if (isRead && pathname === "/download/latest") {
      return release(req, env, "latest", {
        "content-type": TEXT,
        // Short, so a new release shows up without a purge.
        "cache-control": "public, max-age=60",
      });
    }

    let found = isRead && RELEASE.exec(pathname);
    if (found) {
      let [, version, file, target, kind, sum] = found;
      let res = await release(req, env, `${version}/${file}`, {
        "content-type": sum
          ? TEXT
          : kind === "zip"
            ? "application/zip"
            : "application/gzip",
        // A tag never changes, so the archive under it never does either.
        "cache-control": "public, max-age=31536000, immutable",
      });
      if (res.ok && !sum && req.method === "GET") {
        count(req, env, ctx, "archive", { version, target });
      }
      return res;
    }

    let agent = req.headers.get("user-agent") ?? "";
    let wantsScript =
      pathname === "/install.sh" || (pathname === "/" && CLI.test(agent));

    if (wantsScript && (req.method === "GET" || req.method === "HEAD")) {
      // Serving the script must never depend on analytics answering.
      if (env.CROWNE_KEY && env.CROWNE_SITE) {
        let analytics = crowne({ key: env.CROWNE_KEY, site: env.CROWNE_SITE });
        let event = analytics.track(req, "download", {
          // `curl | sh` versus someone reading it first.
          piped: pathname === "/",
          platform: platform(agent),
        });
        ctx.waitUntil(
          event.catch((error: unknown) =>
            console.warn("crowne: download event", error),
          ),
        );
      }
      return new Response(req.method === "HEAD" ? null : script, {
        headers: SCRIPT_HEADERS,
      });
    }

    return env.ASSETS.fetch(req);
  },
};

/** One file from the bucket, or a 404 that says so in plain text. */
async function release(
  req: Request,
  env: Env,
  key: string,
  headers: Record<string, string>,
): Promise<Response> {
  let object =
    req.method === "HEAD"
      ? await env.RELEASES.head(key)
      : await env.RELEASES.get(key);

  if (!object) {
    return new Response("not found\n", {
      status: 404,
      headers: { "content-type": TEXT, "cache-control": "no-store" },
    });
  }

  return new Response(object.body ?? null, {
    headers: {
      ...headers,
      etag: object.httpEtag,
      "content-length": String(object.size),
    },
  });
}

/** Counts in the background, so a slow crowne never holds up a download. */
function count(
  req: Request,
  env: Env,
  ctx: ExecutionContext,
  name: string,
  props: Record<string, string>,
) {
  if (!env.CROWNE_KEY || !env.CROWNE_SITE) return;
  let event = crowne({ key: env.CROWNE_KEY, site: env.CROWNE_SITE }).track(
    req,
    name,
    props,
  );
  ctx.waitUntil(
    event.catch((error: unknown) =>
      console.warn(`crowne: ${name} event`, error),
    ),
  );
}

/**
 * What the user agent says about the machine, for the event. `curl/8.7.1` says
 * nothing.
 */
function platform(agent: string): string {
  if (/darwin|mac/i.test(agent)) return "macos";
  if (/linux/i.test(agent)) return "linux";
  if (/win/i.test(agent)) return "windows";
  return "unknown";
}
