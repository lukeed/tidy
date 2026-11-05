import { crowne } from "crowne/server";

// @ts-expect-error — wrangler's Text module rule turns this into a string.
import script from "./install.sh";

type Env = {
  CROWNE_SITE: string;
  CROWNE_KEY: string;
  ASSETS: {
    fetch(request: Request): Promise<Response>;
  };
};

/**
 * Anchored, so a browser whose user agent merely contains "curl" still gets the
 * page. Only these ask for the script at `/`.
 */
const CLI = /^(curl|wget|libfetch|httpie|python-requests|powershell|fetch)/i;

const SCRIPT_HEADERS = {
  "content-type": "text/x-shellscript; charset=utf-8",
  "cache-control": "public, max-age=300",
};

/**
 * The site is static assets, except for three paths:
 *
 * - `/x/script.js` and `/x` proxy crowne.io, so an ad blocker is less likely to
 *   drop them. @see https://crowne.io/docs/proxy/#cloudflare-workers
 * - `/` answers with `install.sh` when a command line asks for it, so that `curl
 *   -fsSL https://tidy.lukeed.com | sh` works and the browser still gets the
 *   home page.
 * - `/install.sh` always answers with the script.
 *
 * Serving the script counts a `download` event, which is the only thing we can
 * count: the archives themselves come from GitHub releases.
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
