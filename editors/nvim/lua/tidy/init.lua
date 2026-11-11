local M = {}

-- The extension of the name an unnamed buffer gets, so that tidy can tell its language.
local extensions = {
	css = "css",
	dockerfile = "dockerfile",
	dosini = "ini",
	gitconfig = "gitconfig",
	graphql = "graphql",
	handlebars = "hbs",
	html = "html",
	javascript = "js",
	javascriptreact = "jsx",
	json = "json",
	json5 = "json5",
	jsonc = "jsonc",
	less = "less",
	markdown = "md",
	scss = "scss",
	svg = "svg",
	toml = "toml",
	typescript = "ts",
	typescriptreact = "tsx",
	vue = "vue",
	xml = "xml",
	yaml = "yaml",
}

local config = {
	cmd = "tidy",
	format_on_save = true,
	timeout_ms = 2000,
	filetypes = vim.tbl_keys(extensions),
}

local function notify(message)
	vim.notify("tidy: " .. message, vim.log.levels.ERROR)
end

--- Replaces the lines of `bufnr` with `lines`, hunk by hunk, so that marks and the cursor outside a change stay.
local function apply(bufnr, before, lines)
	local diff = vim.text and vim.text.diff or vim.diff
	local old = table.concat(before, "\n") .. "\n"
	local new = table.concat(lines, "\n") .. "\n"
	local hunks = diff(old, new, { result_type = "indices", algorithm = "histogram" })
	for i = #hunks, 1, -1 do
		local start, count, from, size = unpack(hunks[i])
		local replacement = vim.list_slice(lines, from, from + size - 1)
		local first = count == 0 and start or start - 1
		vim.api.nvim_buf_set_lines(bufnr, first, first + count, false, replacement)
	end
end

--- Formats a buffer with tidy, with the config files of its path. Returns whether it worked.
---@param bufnr? integer The buffer, the current one by default
---@return boolean
function M.format(bufnr)
	bufnr = (bufnr == nil or bufnr == 0) and vim.api.nvim_get_current_buf() or bufnr
	if not vim.bo[bufnr].modifiable then
		return false
	end

	local name = vim.api.nvim_buf_get_name(bufnr)
	local path = name ~= ""
			and name
		or vim.fs.joinpath(vim.fn.getcwd(), "untitled." .. (extensions[vim.bo[bufnr].filetype] or "txt"))
	local dir = vim.fs.dirname(path)
	local lines = vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)
	local input = table.concat(lines, "\n") .. "\n"

	local ok, job = pcall(vim.system, { config.cmd, "--stdin=" .. path, "--skip-unknown" }, {
		stdin = input,
		text = true,
		cwd = vim.fn.isdirectory(dir) == 1 and dir or nil,
	})
	if not ok then
		notify(("can't run `%s`: %s"):format(config.cmd, job))
		return false
	end

	local result = job:wait(config.timeout_ms)
	if result.code ~= 0 then
		local error = vim.trim(result.stderr or ""):gsub("^tidy: ", "")
		notify(error ~= "" and error or ("`%s` exited with code %d"):format(config.cmd, result.code))
		return false
	end

	local output = result.stdout:gsub("\r\n", "\n")
	if output ~= input then
		local formatted = vim.split(output, "\n", { plain = true })
		if formatted[#formatted] == "" then
			table.remove(formatted)
		end
		apply(bufnr, lines, formatted)
	end
	return true
end

--- Sets up tidy: the `:Tidy` command, and formatting on save.
---@param opts? { cmd?: string, format_on_save?: boolean, timeout_ms?: integer, filetypes?: string[] }
function M.setup(opts)
	config = vim.tbl_extend("force", config, opts or {})
	local filetypes = {}
	for _, filetype in ipairs(config.filetypes) do
		filetypes[filetype] = true
	end

	vim.api.nvim_create_user_command("Tidy", function()
		M.format(0)
	end, { desc = "Format the buffer with tidy" })

	local group = vim.api.nvim_create_augroup("tidy", { clear = true })
	if config.format_on_save then
		vim.api.nvim_create_autocmd("BufWritePre", {
			group = group,
			callback = function(args)
				if filetypes[vim.bo[args.buf].filetype] then
					M.format(args.buf)
				end
			end,
		})
	end
end

return M
