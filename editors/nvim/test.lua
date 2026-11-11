-- Run with: nvim --headless --clean -l editors/nvim/test.lua
local here = vim.fs.dirname(vim.fs.abspath(debug.getinfo(1, "S").source:sub(2)))
local repo = vim.fs.dirname(vim.fs.dirname(here))
vim.opt.runtimepath:prepend(here)

local bin = os.getenv("TIDY_BIN")
if not bin then
	local build = vim.system({ "cargo", "build", "-q", "-p", "tidy_cli" }, { cwd = repo }):wait()
	assert(build.code == 0, build.stderr)
	local metadata = vim.system({ "cargo", "metadata", "--format-version=1", "--no-deps" }, { cwd = repo }):wait()
	bin = vim.fs.joinpath(vim.json.decode(metadata.stdout).target_directory, "debug", "tidy")
end

local LONG = '<p class="aaaa bbbb">hi there friend</p>'
local NARROW = { "<p", '  class="aaaa bbbb"', ">", "  hi there friend", "</p>" }
local WIDE = { LONG }

-- Two sibling configs and none above them, so a lookup from the wrong folder gives a different result.
local root = vim.fn.tempname()
for name, text in pairs({ ["narrow/tidy.json"] = '{ "width": 20 }\n', ["wide/tidy.json"] = "{}\n" }) do
	vim.fn.mkdir(vim.fs.dirname(vim.fs.joinpath(root, name)), "p")
	vim.fn.writefile(vim.split(text, "\n", { trimempty = true }), vim.fs.joinpath(root, name))
end

local errors = {}
vim.notify = function(message)
	table.insert(errors, message)
end

local tidy = require("tidy")
tidy.setup({ cmd = bin })

local function open(name, lines)
	vim.cmd("silent edit " .. vim.fn.fnameescape(vim.fs.joinpath(root, name)))
	vim.api.nvim_buf_set_lines(0, 0, -1, false, lines)
	return vim.api.nvim_get_current_buf()
end

local function buffer()
	return vim.api.nvim_buf_get_lines(0, 0, -1, false)
end

local function saved(name)
	return vim.fn.readfile(vim.fs.joinpath(root, name))
end

local tests = {
	["formats on save with the config next to the file"] = function()
		open("narrow/a.html", { LONG })
		vim.cmd("silent write")
		assert(vim.deep_equal(saved("narrow/a.html"), NARROW), vim.inspect(saved("narrow/a.html")))

		open("wide/a.html", { LONG })
		vim.cmd("silent write")
		assert(vim.deep_equal(saved("wide/a.html"), WIDE), vim.inspect(saved("wide/a.html")))
	end,

	["formats with :Tidy, without saving"] = function()
		open("wide/command.css", { "a{color:red}" })
		vim.cmd.Tidy()
		assert(vim.deep_equal(buffer(), { "a {", "  color: red;", "}" }), vim.inspect(buffer()))
		assert(vim.bo.modified)
	end,

	["keeps the cursor and marks outside a change"] = function()
		open("wide/marks.css", { "a {", "  color: red;", "}", "b{color:blue}" })
		vim.api.nvim_win_set_cursor(0, { 2, 4 })
		local ns = vim.api.nvim_create_namespace("tidy-test")
		local mark = vim.api.nvim_buf_set_extmark(0, ns, 1, 2, {})
		vim.cmd.Tidy()
		assert(vim.deep_equal(vim.api.nvim_win_get_cursor(0), { 2, 4 }), vim.inspect(vim.api.nvim_win_get_cursor(0)))
		local at = vim.api.nvim_buf_get_extmark_by_id(0, ns, mark, {})
		assert(vim.deep_equal(at, { 1, 2 }), vim.inspect(at))
	end,

	["formats an unnamed buffer by its filetype"] = function()
		vim.cmd.enew()
		vim.bo.filetype = "css"
		vim.api.nvim_buf_set_lines(0, 0, -1, false, { "a{color:red}" })
		vim.cmd.Tidy()
		assert(vim.deep_equal(buffer(), { "a {", "  color: red;", "}" }), vim.inspect(buffer()))
	end,

	["leaves a name tidy doesn't know as it is"] = function()
		open("wide/page.ejs", { "<p   ><%= a %></p>" })
		vim.bo.filetype = "html"
		vim.cmd("silent write")
		assert(vim.deep_equal(saved("wide/page.ejs"), { "<p   ><%= a %></p>" }))
		assert(#errors == 0, vim.inspect(errors))
	end,

	["leaves other filetypes alone"] = function()
		open("wide/main.lua", { "local   a=1" })
		vim.cmd("silent write")
		assert(vim.deep_equal(saved("wide/main.lua"), { "local   a=1" }))
		assert(#errors == 0, vim.inspect(errors))
	end,

	["saves text that doesn't parse as it is, and says why"] = function()
		open("wide/broken.css", { "a {" })
		vim.cmd("silent write")
		assert(vim.deep_equal(saved("wide/broken.css"), { "a {" }))
		assert(#errors == 1 and errors[1]:find("broken.css:1:1: Unclosed block", 1, true), vim.inspect(errors))
	end,

	["says when the binary is missing"] = function()
		tidy.setup({ cmd = vim.fs.joinpath(root, "missing") })
		open("wide/missing.css", { "a{color:red}" })
		vim.cmd("silent write")
		tidy.setup({ cmd = bin })
		assert(vim.deep_equal(saved("wide/missing.css"), { "a{color:red}" }))
		assert(#errors == 1 and errors[1]:find("can't run", 1, true), vim.inspect(errors))
	end,

	["can leave saving alone"] = function()
		tidy.setup({ cmd = bin, format_on_save = false })
		open("wide/manual.css", { "a{color:red}" })
		vim.cmd("silent write")
		tidy.setup({ cmd = bin, format_on_save = true })
		assert(vim.deep_equal(saved("wide/manual.css"), { "a{color:red}" }))
	end,
}

local names = vim.tbl_keys(tests)
table.sort(names)
local failed = 0
for _, name in ipairs(names) do
	errors = {}
	local ok, error = pcall(tests[name])
	vim.cmd("silent! %bwipeout!")
	if ok then
		io.stdout:write("  ✓ ", name, "\n")
	else
		failed = failed + 1
		io.stdout:write("  ✗ ", name, "\n", tostring(error), "\n")
	end
end
vim.fn.delete(root, "rf")
if failed > 0 then
	io.stdout:write(("%d of %d tests failed\n"):format(failed, #names))
	os.exit(1)
end
