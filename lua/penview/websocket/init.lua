local M = {}

---@param opts? {}
function M.setup(opts)
	-- Find penview plugin root (lua/penview/websocket/init.lua -> plugin root)
	local current_path = debug.getinfo(1).source:match("@?(.*/)") or ""
	local plugin_root = vim.fn.fnamemodify(current_path, ":h:h:h:h")

	-- Check for FFI library locations in order of preference:
	-- 1. Pre-compiled in bin/lua/
	-- 2. Source-built in rust/websocket-ffi/lua/
	local precompiled_path = plugin_root .. "/bin"
	local source_path = plugin_root .. "/rust/websocket-ffi"

	if vim.fn.filereadable(precompiled_path .. "/lua/websocket_ffi.so") == 1 then
		vim.opt.runtimepath:append(precompiled_path)
	else
		-- Fall back to source-built location (also used by make build)
		vim.opt.runtimepath:append(source_path)
	end

	_G["_WEBSOCKET_NVIM"] = {
		clients = {
			callbacks = {},
		},
		servers = {
			callbacks = {},
		},
	}
end

return M
