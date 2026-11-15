import { expect, test } from "bun:test";

import * as version from "./version";

test("accepts tidy 0.2.0 and later", () => {
  expect(version.problem("tidy 0.2.0\n")).toBeUndefined();
  expect(version.problem("tidy 0.10.3\n")).toBeUndefined();
  expect(version.problem("tidy 1.0.0\n")).toBeUndefined();
});

test("refuses HTML Tidy", () => {
  let output =
    "HTML Tidy for Mac OS X released on 31 October 2006 - Apple Inc. build 4474\n";
  expect(version.problem(output)).toBe(
    "is HTML Tidy, which macOS ships as /usr/bin/tidy, not tidy",
  );
});

test("refuses another program", () => {
  expect(version.problem("something 1.2.3\nmore\n")).toBe(
    'is not tidy: it printed "something 1.2.3"',
  );
});

test("refuses a tidy without --stdin=<path>", () => {
  expect(version.problem("tidy 0.1.0\n")).toBe(
    "is tidy 0.1.0, and this extension needs 0.2.0 or later",
  );
});
