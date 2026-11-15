import * as os from "node:os";
import * as path from "node:path";

import * as vscode from "vscode";

import * as languages from "./languages";
import * as tidy from "./tidy";
import * as version from "./version";

/**
 * Registers tidy as a formatter of every language it knows, for whole documents
 * and for ranges. A range, as format on paste sends, formats the whole document
 * too.
 */
export function activate(context: vscode.ExtensionContext): void {
  let selector = Object.keys(languages.EXTENSIONS).flatMap((language) => [
    { language, scheme: "file" },
    { language, scheme: "untitled" },
  ]);
  let log = vscode.window.createOutputChannel("tidy", { log: true });
  let status = vscode.languages.createLanguageStatusItem("tidy", selector);
  status.name = "tidy";
  status.text = "tidy";
  status.command = { title: "Show Output", command: "tidy.output" };
  // Binaries that passed the version check. One that failed is checked again, so a fix needs no reload.
  let checked = new Set<string>();
  // Problems already shown, so each pops up once.
  let shown = new Set<string>();
  let never = new vscode.CancellationTokenSource().token;

  /** Why `bin` can't format, or `undefined` once `tidy --version` shows it can. */
  async function refused(
    bin: string,
    cwd: string,
  ): Promise<string | undefined> {
    if (checked.has(bin)) return undefined;
    let result = await tidy.run(bin, ["--version"], undefined, cwd, never);
    let problem = result.ok
      ? version.problem(result.output)
      : result.missing
        ? "can't be found"
        : `fails: ${result.error}`;
    if (!problem) checked.add(bin);
    return problem;
  }

  function fail(error: string, popup?: string): void {
    log.error(error);
    status.severity = vscode.LanguageStatusSeverity.Error;
    status.detail = error;
    if (!popup || shown.has(popup)) return;
    shown.add(popup);
    vscode.window.showErrorMessage(popup, "Open Settings").then((choice) => {
      if (choice)
        vscode.commands.executeCommand(
          "workbench.action.openSettings",
          "tidy.path",
        );
    });
  }

  async function format(
    document: vscode.TextDocument,
    token: vscode.CancellationToken,
  ): Promise<vscode.TextEdit[]> {
    let folder =
      vscode.workspace.getWorkspaceFolder(document.uri) ??
      vscode.workspace.workspaceFolders?.[0];
    let cwd =
      folder?.uri.fsPath ??
      (document.isUntitled ? os.homedir() : path.dirname(document.uri.fsPath));
    let file = document.isUntitled
      ? path.join(
          cwd,
          `untitled.${languages.EXTENSIONS[document.languageId] ?? "txt"}`,
        )
      : document.uri.fsPath;
    let bin = binary(folder);
    let input = document.getText();

    let problem = await refused(bin, cwd);
    if (token.isCancellationRequested) return [];
    if (problem) {
      let message = `tidy: \`${bin}\` ${problem}. Install tidy, or set \`tidy.path\`.`;
      fail(message, message);
      return [];
    }

    log.trace(`${bin} --stdin=${file} --skip-unknown`);
    let result = await tidy.run(
      bin,
      [`--stdin=${file}`, "--skip-unknown"],
      input,
      cwd,
      token,
    );
    if (token.isCancellationRequested) return [];

    if (!result.ok) {
      checked.delete(bin);
      fail(result.error);
      return [];
    }

    status.severity = vscode.LanguageStatusSeverity.Information;
    status.detail = undefined;
    if (result.output === input) return [];

    let all = new vscode.Range(
      document.positionAt(0),
      document.positionAt(input.length),
    );
    let edits = [vscode.TextEdit.replace(all, result.output)];
    let eol = result.output.includes("\r\n")
      ? vscode.EndOfLine.CRLF
      : vscode.EndOfLine.LF;
    if (result.output.includes("\n") && eol !== document.eol) {
      edits.push(vscode.TextEdit.setEndOfLine(eol));
    }
    return edits;
  }

  context.subscriptions.push(
    log,
    status,
    vscode.commands.registerCommand("tidy.output", () => log.show()),
    vscode.languages.registerDocumentFormattingEditProvider(selector, {
      provideDocumentFormattingEdits: (document, _options, token) =>
        format(document, token),
    }),
    vscode.languages.registerDocumentRangeFormattingEditProvider(selector, {
      provideDocumentRangeFormattingEdits: (
        document,
        _range,
        _options,
        token,
      ) => format(document, token),
    }),
  );
}

/** The binary that `tidy.path` names, else `tidy` on the PATH. */
function binary(folder: vscode.WorkspaceFolder | undefined): string {
  let setting = vscode.workspace
    .getConfiguration("tidy", folder)
    .get<string>("path", "")
    .trim();
  if (!setting) return "tidy";
  if (setting.startsWith("~/"))
    return path.join(os.homedir(), setting.slice(2));
  if (path.isAbsolute(setting) || !/[\\/]/.test(setting) || !folder)
    return setting;
  return path.join(folder.uri.fsPath, setting);
}
