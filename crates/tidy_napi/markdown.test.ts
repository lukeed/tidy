import { describe, expect, test } from "bun:test";

import { check, format } from "./index.js";

describe("format", () => {
	test("uses the Prettier preset by default", () => {
		expect(format("* one\n* two\n")).toBe("- one\n- two\n");
	});

	test("applies a preset and overrides", () => {
		expect(format("* one\n", { preset: "preserve" })).toBe("* one\n");
		expect(format("* one\n", { preset: "preserve", bullet: "plus" })).toBe("+ one\n");
		expect(format("one\ntwo\n", { wrap: "never" })).toBe("one two\n");
	});

	test("rejects unknown option values", () => {
		// @ts-expect-error — not a wrap mode
		expect(() => format("x\n", { wrap: "sometimes" })).toThrow("unknown wrap `sometimes`");
	});

	test("hands code blocks to the callback", () => {
		let seen: [string, string, number][] = [];
		let output = format("```js\nlet  x\n```\n\n```txt\nkeep  me\n```\n", {}, (lang, text, width) => {
			seen.push([lang, text, width]);
			if (lang === "js") return text.replace("  ", " ");
		});
		expect(output).toBe("```js\nlet x\n```\n\n```txt\nkeep  me\n```\n");
		expect(seen).toEqual([
			["js", "let  x\n", 80],
			["txt", "keep  me\n", 80],
		]);
	});

	test("throws what the callback throws", () => {
		expect(() =>
			format("```js\nx\n```\n", {}, () => {
				throw new Error("boom");
			}),
		).toThrow("boom");
	});
});

describe("check", () => {
	test("tells whether a document is formatted", () => {
		expect(check("- one\n")).toBe(true);
		expect(check("* one\n")).toBe(false);
		expect(check("* one\n", { preset: "preserve" })).toBe(true);
	});
});
