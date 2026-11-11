import { spawn } from "node:child_process";

import type { CancellationToken } from "vscode";

export type Result =
  { ok: true; output: string } | { ok: false; error: string; missing: boolean };

/**
 * Runs the tidy binary with `input` on its stdin, and gives back its stdout or
 * its error.
 *
 * @example
 *   let result = await tidy.run("tidy", ["--stdin=/repo/a.ts"], "let a=1", "/repo", token);
 *   if (result.ok) console.log(result.output); //=> "let a = 1;\n"
 */
export function run(
  bin: string,
  args: string[],
  input: string,
  cwd: string,
  token: CancellationToken,
): Promise<Result> {
  return new Promise((resolve) => {
    let child = spawn(bin, args, { cwd, windowsHide: true });
    let cancel = token.onCancellationRequested(() => child.kill());
    let stdout: Buffer[] = [];
    let stderr: Buffer[] = [];

    child.stdout.on("data", (chunk: Buffer) => stdout.push(chunk));
    child.stderr.on("data", (chunk: Buffer) => stderr.push(chunk));
    // tidy exits before it reads all of stdin when it fails early; `close` reports why.
    child.stdin.on("error", () => {});
    child.on("spawn", () => child.stdin.end(input));

    child.on("error", (error: NodeJS.ErrnoException) => {
      cancel.dispose();
      resolve({
        ok: false,
        error: error.message,
        missing: error.code === "ENOENT",
      });
    });

    child.on("close", (code) => {
      cancel.dispose();
      if (code === 0) {
        return resolve({
          ok: true,
          output: Buffer.concat(stdout).toString("utf8"),
        });
      }
      let error = Buffer.concat(stderr)
        .toString("utf8")
        .trim()
        .replace(/^tidy: /, "");
      resolve({
        ok: false,
        error: error || `${bin} exited with code ${code}`,
        missing: false,
      });
    });
  });
}
