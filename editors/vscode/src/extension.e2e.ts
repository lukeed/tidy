import * as assert from "node:assert/strict";
import * as fs from "node:fs";
import * as path from "node:path";

import * as vscode from "vscode";

const LONG = '<p class="aaaa bbbb">hi there friend</p>';
const NARROW = '<p\n  class="aaaa bbbb"\n>\n  hi there friend\n</p>\n';
const WIDE = '<p class="aaaa bbbb">hi there friend</p>\n';

const workspace = process.env.TIDY_WORKSPACE || "";

/** Writes `text` to a new file of the workspace and opens it in an editor. */
async function open(name: string, text: string): Promise<vscode.TextEditor> {
  let file = path.join(workspace, name);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, text);
  let document = await vscode.workspace.openTextDocument(file);
  let editor = await vscode.window.showTextDocument(document);
  await vscode.commands.executeCommand(
    "workbench.action.focusActiveEditorGroup",
  );
  return editor;
}

/** Waits until `check` passes, for what VS Code does after a command returns. */
async function until(check: () => boolean): Promise<void> {
  for (let tries = 0; tries < 100 && !check(); tries++) {
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
}

const tests: Record<string, () => Promise<void>> = {
  async "formats a document with the config next to it"() {
    let narrow = await open("narrow/a.html", LONG);
    await vscode.commands.executeCommand("editor.action.formatDocument");
    assert.equal(narrow.document.getText(), NARROW);

    let wide = await open("wide/a.html", LONG);
    await vscode.commands.executeCommand("editor.action.formatDocument");
    assert.equal(wide.document.getText(), WIDE);
  },

  async "formats on save"() {
    let editor = await open("wide/save.css", "a{color:red}\n");
    await editor.edit((edit) =>
      edit.insert(new vscode.Position(1, 0), "b{color:blue}\n"),
    );
    await editor.document.save();
    let saved = fs.readFileSync(path.join(workspace, "wide/save.css"), "utf8");
    assert.equal(saved, "a {\n  color: red;\n}\nb {\n  color: blue;\n}\n");
  },

  async "formats the whole document on paste"() {
    let editor = await open("narrow/paste.html", "");
    await vscode.env.clipboard.writeText(LONG);
    await vscode.commands.executeCommand("editor.action.clipboardPasteAction");
    await until(() => editor.document.getText() === NARROW);
    assert.equal(editor.document.getText(), NARROW);
  },

  async "formats an untitled document by its language"() {
    let document = await vscode.workspace.openTextDocument({
      language: "css",
      content: "a{color:red}",
    });
    await vscode.window.showTextDocument(document);
    await vscode.commands.executeCommand(
      "workbench.action.focusActiveEditorGroup",
    );
    await vscode.commands.executeCommand("editor.action.formatDocument");
    assert.equal(document.getText(), "a {\n  color: red;\n}\n");
  },

  async "converts line endings as tidy does"() {
    let editor = await open(
      "wide/crlf.css",
      "a{color:red}\r\nb{color:blue}\r\n",
    );
    await vscode.commands.executeCommand("editor.action.formatDocument");
    assert.equal(editor.document.eol, vscode.EndOfLine.LF);
    assert.equal(
      editor.document.getText(),
      "a {\n  color: red;\n}\nb {\n  color: blue;\n}\n",
    );
  },

  async "leaves a name tidy doesn't know as it is"() {
    let editor = await open("wide/page.ejs", "<p   ><%= a %></p>\n");
    assert.equal(editor.document.languageId, "html");
    await vscode.commands.executeCommand("editor.action.formatDocument");
    assert.equal(editor.document.getText(), "<p   ><%= a %></p>\n");
  },

  async "leaves text that doesn't parse as it is"() {
    let editor = await open("wide/broken.css", "a {");
    await vscode.commands.executeCommand("editor.action.formatDocument");
    assert.equal(editor.document.getText(), "a {");
  },

  async "leaves text as it is when the binary is missing"() {
    let config = vscode.workspace.getConfiguration("tidy");
    let before = config.inspect<string>("path")?.workspaceValue;
    await config.update(
      "path",
      "bin/missing",
      vscode.ConfigurationTarget.Workspace,
    );
    try {
      let editor = await open("wide/missing.css", "a{color:red}");
      await vscode.commands.executeCommand("editor.action.formatDocument");
      assert.equal(editor.document.getText(), "a{color:red}");
    } finally {
      await config.update("path", before, vscode.ConfigurationTarget.Workspace);
    }
  },
};

/** The entry that `@vscode/test-electron` calls inside VS Code. */
export async function run(): Promise<void> {
  await vscode.extensions.getExtension("lukeed.tidy")?.activate();
  let failed = 0;
  for (let [name, check] of Object.entries(tests)) {
    try {
      await check();
      console.log(`  ✓ ${name}`);
    } catch (error) {
      failed++;
      console.log(
        `  ✗ ${name}\n${error instanceof Error ? error.stack : error}`,
      );
    } finally {
      await vscode.commands.executeCommand(
        "workbench.action.revertAndCloseActiveEditor",
      );
    }
  }
  if (failed)
    throw new Error(`${failed} of ${Object.keys(tests).length} tests failed`);
}
