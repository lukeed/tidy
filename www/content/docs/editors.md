---
title: Editors
summary: Format on save in VS Code, Neovim, Zed and JetBrains.
group: Usage
order: 6
---

Each editor below runs the `tidy` binary with the buffer on stdin and its path in `--stdin=<path>`. tidy uses the path to pick the language and find the config files, so the result is the same as `tidy up`. A run takes a few milliseconds.

## VS Code

Install **tidy up** (`lukeed.tidyup`) from the Marketplace or Open VSX:

```sh
$ code --install-extension lukeed.tidyup
```

Then make it the formatter, for every language or for the ones you choose:

```json [.vscode/settings.json]
{
  "editor.defaultFormatter": "lukeed.tidyup",
  "editor.formatOnSave": true,
  "editor.formatOnPaste": true
}
```

The extension runs `tidy` from your `PATH`. To run another binary, set `tidy.path`. A relative path, such as `node_modules/.bin/tidy`, is relative to the workspace folder. In an untrusted workspace, the extension ignores the `tidy.path` of the workspace.

Format on paste and **Format Selection** format the whole file, not only the selection. If the file was already formatted, only the pasted text changes.

When tidy fails, for example on a syntax error, the file stays as it is. The error shows in the language status item and in the **tidy** output channel.

## Zed

Zed can format with an external command, per language:

```json [.zed/settings.json]
{
  "languages": {
    "TypeScript": {
      "formatter": {
        "external": {
          "command": "tidy",
          "arguments": ["--stdin={buffer_path}"]
        }
      }
    }
  }
}
```

Repeat the block for each language. Zed formats on save by default. There is no Zed extension: Zed extensions can add a language server, but not a formatter.

## Neovim

The plugin is in `editors/nvim` of the repository. With lazy.nvim:

```lua
{
	"lukeed/tidy",
	config = function(plugin)
		vim.opt.rtp:append(plugin.dir .. "/editors/nvim")
		require("tidy").setup()
	end,
}
```

It formats before each write, and `:Tidy` formats the current buffer. It replaces only the lines that change, so the cursor and marks stay. When tidy fails, the buffer stays as it is and the error shows with `vim.notify`. See `:help tidy` for the options: `cmd`, `format_on_save`, `timeout_ms` and `filetypes`.

With **conform.nvim** instead:

```lua
require("conform").setup({
	formatters = {
		tidy = {
			command = "tidy",
			args = { "--stdin=$FILENAME", "--skip-unknown" },
			stdin = true,
		},
	},
	formatters_by_ft = {
		typescript = { "tidy" },
		javascript = { "tidy" },
		css = { "tidy" },
		html = { "tidy" },
		markdown = { "tidy" },
		yaml = { "tidy" },
		json = { "tidy" },
	},
	format_on_save = { timeout_ms = 2000 },
})
```

## JetBrains

Settings → Tools → File Watchers → **+** → `<custom>`:

| Field                   | Value                               |
| ----------------------- | ----------------------------------- |
| File type               | the type you want, one watcher each |
| Program                 | `tidy`                              |
| Arguments               | `up $FilePath$`                     |
| Output paths to refresh | `$FilePath$`                        |
| Auto-save edited files  | off                                 |

Turn off "Auto-save edited files to trigger the watcher", or the watcher runs while you type.

## Which flag to use

| Form                  | When                                                                          |
| --------------------- | ----------------------------------------------------------------------------- |
| `tidy up <path>`      | The editor can run a command on a saved file.                                 |
| `tidy --stdin=<path>` | The editor pipes the buffer in and takes stdout. Formats unsaved buffers too. |

Prefer the stdin form where the editor supports it: it never writes a half-formatted file if something goes wrong, and it works before the first save.
