/**
 * Why the program that printed `output` for `--version` can't format here, or
 * `undefined` when it is tidy 0.2.0 or later, the first with `--stdin=<path>`
 * and `--skip-unknown`.
 *
 * @example
 *   version.problem("tidy 0.2.1\n"); //=> undefined
 *   version.problem("tidy 0.1.0\n"); //=> "is tidy 0.1.0, and this extension needs 0.2.0 or later"
 */
export function problem(output: string): string | undefined {
  let first = output.trim().split("\n")[0] ?? "";
  if (first.startsWith("HTML Tidy"))
    return "is HTML Tidy, which macOS ships as /usr/bin/tidy, not tidy";
  let match = /^tidy (\d+)\.(\d+)\.(\d+)/.exec(first);
  if (!match) return `is not tidy: it printed "${first}"`;
  let [major, minor] = [Number(match[1]), Number(match[2])];
  if (major === 0 && minor < 2)
    return `is tidy ${match[1]}.${match[2]}.${match[3]}, and this extension needs 0.2.0 or later`;
  return undefined;
}
