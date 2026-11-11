---
title: Editors
summary: Format on save in VS Code, Zed, Neovim and JetBrains, by running the binary.
group: Usage
order: 6
---

<!-- prettier-ignore -->
> There is no tidy editor extension and no language server yet. Every recipe
> below runs the binary on the file, which every editor can do.

Because tidy is fast, running the whole binary on save costs a few milliseconds — about the same as an extension would, without one existing.

## VS Code

Install an extension that can run a command on save, such as **Run on Save** (`emeraldwalk.RunOnSave`), then:

```json [.vscode/settings.json]
{
  "emeraldwalk.runonsave": {
    "commands": [
      {
        "match": "\\.(js|jsx|mjs|cjs|ts|tsx|mts|cts|css|scss|less|html|vue|hbs|md|json|jsonc|yaml|yml|toml|xml|svg|graphql)$",
        "cmd": "tidy up ${file}"
      }
    ]
  }
}
```

Turn off the built-in formatter for those languages so the two do not both write:

```json [.vscode/settings.json]
{
  "editor.formatOnSave": false
}
```

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

Repeat the block for each language. `--stdin=<path>` is what makes it work: tidy reads the buffer from stdin and uses the path only to pick the language and find the config files.

## Neovim

```lua
vim.api.nvim_create_autocmd("BufWritePre", {
	pattern = { "*.ts", "*.tsx", "*.js", "*.jsx", "*.css", "*.scss", "*.html", "*.vue", "*.md", "*.json", "*.yaml", "*.yml", "*.toml" },
	callback = function(args)
		local view = vim.fn.winsaveview()
		vim.cmd("silent %!tidy --stdin=" .. vim.fn.shellescape(args.file))
		if vim.v.shell_error ~= 0 then
			vim.cmd("silent undo")
		end
		vim.fn.winrestview(view)
	end,
})
```

The `shell_error` check matters: without it, a file that does not parse is replaced by tidy's error message. Saving the view and restoring it keeps the cursor where it was.

With **conform.nvim**:

```lua
require("conform").setup({
	formatters = {
		tidy = {
			command = "tidy",
			args = { "--stdin=$FILENAME" },
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
