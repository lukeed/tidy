import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";

import { runTests } from "@vscode/test-electron";

let repo = path.resolve(import.meta.dir, "../..");
let inherit: ["inherit", "inherit", "inherit"] = [
  "inherit",
  "inherit",
  "inherit",
];

// VS Code needs a display, so it runs on a virtual one, in a Linux container when the host isn't Linux.
if (process.platform !== "linux") {
  let image = "tidy-vscode-test";
  let dockerfile = Bun.file(path.join(import.meta.dir, "test.dockerfile"));
  let build = Bun.spawnSync(["docker", "build", "-q", "-t", image, "-"], {
    stdin: dockerfile,
    stdout: "ignore",
    stderr: "inherit",
  });
  if (!build.success) process.exit(1);
  let run = Bun.spawnSync(
    [
      "docker",
      "run",
      "--rm",
      "--init",
      "--shm-size=1g",
      "-v",
      `${repo}:/repo`,
      "-v",
      "tidy-vscode-target:/target",
      "-v",
      "tidy-vscode-cargo:/usr/local/cargo/registry",
      "-v",
      "tidy-vscode-cache:/cache",
      "-e",
      "CARGO_TARGET_DIR=/target",
      "-e",
      "VSCODE_TEST_CACHE=/cache",
      "-w",
      "/repo/editors/vscode",
      image,
      "bun",
      "test.ts",
    ],
    { stdio: inherit },
  );
  process.exit(run.exitCode ?? 1);
}
if (!process.env.DISPLAY) {
  let run = Bun.spawnSync(
    ["xvfb-run", "-a", process.execPath, ...process.argv.slice(1)],
    { stdio: inherit },
  );
  process.exit(run.exitCode ?? 1);
}

let cargo = Bun.spawnSync(["cargo", "build", "-q", "-p", "tidy_cli"], {
  cwd: repo,
  stdio: inherit,
});
if (!cargo.success) process.exit(1);

let metadata = Bun.spawnSync(
  ["cargo", "metadata", "--format-version=1", "--no-deps"],
  { cwd: repo },
);
let target: unknown = JSON.parse(metadata.stdout.toString()).target_directory;
if (typeof target !== "string")
  throw new Error("cargo metadata has no target_directory");
let bin = path.join(target, "debug", "tidy");

let built = await Bun.build({
  entrypoints: ["src/extension.ts", "src/extension.e2e.ts"],
  root: "src",
  outdir: "dist",
  target: "node",
  format: "cjs",
  external: ["vscode"],
});
if (!built.success) throw new AggregateError(built.logs, "bundle failed");

let unit = Bun.spawnSync([process.execPath, "test", "src"], {
  env: { ...process.env, TIDY_BIN: bin },
  stdio: inherit,
});
if (!unit.success) process.exit(1);

// Two sibling configs and none above them, so a lookup from the wrong folder gives a different result.
let workspace = fs.mkdtempSync(path.join(os.tmpdir(), "tidy-vscode-"));
let user = fs.mkdtempSync(path.join(os.tmpdir(), "tidy-vscode-user-"));
let files: Record<string, string> = {
  "narrow/tidy.json": '{ "width": 20 }\n',
  "wide/tidy.json": "{}\n",
  ".vscode/settings.json": JSON.stringify({
    "tidy.path": "bin/tidy",
    "editor.defaultFormatter": "lukeed.tidy",
    "editor.formatOnSave": true,
    "editor.formatOnPaste": true,
    "chat.disableAIFeatures": true,
    "workbench.startupEditor": "none",
  }),
};
for (let [name, text] of Object.entries(files)) {
  fs.mkdirSync(path.join(workspace, path.dirname(name)), { recursive: true });
  fs.writeFileSync(path.join(workspace, name), text);
}
fs.mkdirSync(path.join(workspace, "bin"));
fs.copyFileSync(bin, path.join(workspace, "bin", "tidy"));

try {
  await runTests({
    version: process.env.VSCODE_VERSION,
    cachePath: process.env.VSCODE_TEST_CACHE,
    vscodeExecutablePath: process.env.VSCODE_PATH,
    extensionDevelopmentPath: import.meta.dir,
    extensionTestsPath: path.join(import.meta.dir, "dist", "extension.e2e.js"),
    extensionTestsEnv: { TIDY_WORKSPACE: workspace },
    launchArgs: [
      workspace,
      "--disable-extensions",
      "--disable-workspace-trust",
      "--disable-gpu",
      "--no-sandbox",
      "--user-data-dir",
      user,
    ],
  });
} catch (error) {
  console.error(error);
  process.exitCode = 1;
} finally {
  fs.rmSync(workspace, { recursive: true, force: true });
  fs.rmSync(user, { recursive: true, force: true });
}
