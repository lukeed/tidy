import { describe, expect, test } from "bun:test";

import { check, format } from "./index.js";

let markdown = { parser: "markdown" } as const;

describe("markdown", () => {
	test("uses the Prettier preset by default", () => {
		expect(format("* one\n* two\n", markdown)).toBe("- one\n- two\n");
		expect(format("* one\n", { filepath: "notes/a.md" })).toBe("- one\n");
	});

	test("applies a preset and overrides", () => {
		expect(format("* one\n", { ...markdown, markdown: { preset: "preserve" } })).toBe("* one\n");
		expect(format("* one\n", { ...markdown, markdown: { preset: "preserve", bullet: "plus" } })).toBe("+ one\n");
		expect(format("one\ntwo\n", { ...markdown, markdown: { wrap: "never" } })).toBe("one two\n");
	});

	test("rejects unknown option values", () => {
		expect(() => format("x\n", { ...markdown, markdown: { wrap: "sometimes" } })).toThrow("unknown wrap `sometimes`");
	});

	test("hands code blocks in a language Prettier knows to the hook", () => {
		let seen: [string, string, number][] = [];
		let output = format("```js\nlet  x\n```\n\n```txt\nkeep  me\n```\n", markdown, (request) => {
			seen.push([request.language, request.code, request.width]);
			return request.code.replace("  ", " ");
		});
		expect(output).toBe("```js\nlet x\n```\n\n```txt\nkeep  me\n```\n");
		expect(seen).toEqual([["babel", "let  x\n", 80]]);
	});

	test("formats style sheets itself when the hook declines", () => {
		expect(format("```css\na{color:red}\n```\n", markdown)).toBe("```css\na {\n  color: red;\n}\n```\n");
		expect(format("```css\na{color:red}\n```\n", { ...markdown, embedded: false })).toBe("```css\na{color:red}\n```\n");
	});

	test("throws what the hook throws", () => {
		expect(() =>
			format("```js\nx\n```\n", markdown, () => {
				throw new Error("boom");
			}),
		).toThrow("boom");
	});
});

describe("check", () => {
	test("tells whether a file is formatted", () => {
		expect(check("- one\n", markdown)).toBe(true);
		expect(check("* one\n", markdown)).toBe(false);
		expect(check("* one\n", { ...markdown, markdown: { preset: "preserve" } })).toBe(true);
		expect(check("<p>x</p>\n", { parser: "html" })).toBe(true);
	});
});
