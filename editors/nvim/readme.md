# tidy for Neovim

Formats buffers with [tidy](https://tidy.lukeed.com) on save, and with `:Tidy`. Needs Neovim 0.10 or later and the `tidy` binary.

Each buffer uses the same `tidy.json`, `tidy.toml` and `.editorconfig` that `tidy up` would use for its file. Only the lines that change are replaced, so the cursor and marks stay. Before it formats, the plugin checks that the binary is tidy 0.4.0 or later: macOS ships an unrelated HTML Tidy as `/usr/bin/tidy`, which it refuses to run.

## Install

With lazy.nvim:

```lua
{
	"lukeed/tidy",
	config = function(plugin)
		vim.opt.rtp:append(plugin.dir .. "/editors/nvim")
		require("tidy").setup()
	end,
}
```

## Options

```lua
require("tidy").setup({
	cmd = "tidy", -- the binary to run
	format_on_save = true,
	timeout_ms = 2000,
	filetypes = { "css", "html", "markdown", "typescript" }, -- default: every filetype tidy formats
})
```

See `:help tidy`.
