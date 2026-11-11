# tidy for VS Code

Formats your files with [tidy](https://tidy.lukeed.com), on save and on paste.

The extension runs the `tidy` binary, so [install it](https://tidy.lukeed.com/docs/install/) first. Each file uses the same `tidy.json`, `tidy.toml` and `.editorconfig` that `tidy up` would use for it.

## Setup

```json
{
  "editor.defaultFormatter": "lukeed.tidyup",
  "editor.formatOnSave": true,
  "editor.formatOnPaste": true
}
```

Format on paste and **Format Selection** format the whole file. If the file was already formatted, only the pasted text changes.

## Settings

| Setting     | Default          | What it does                                                                                        |
| ----------- | ---------------- | --------------------------------------------------------------------------------------------------- |
| `tidy.path` | `tidy` on `PATH` | The binary to run. A relative path, such as `node_modules/.bin/tidy`, is relative to the workspace. |

In an untrusted workspace, the extension ignores the `tidy.path` of the workspace.

## Errors

When tidy can't format a file, for example on a syntax error, the file stays as it is. The error shows in the language status item and in the **tidy** output channel.
