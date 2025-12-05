local M = {}

---@param opts? {}
function M.setup(opts)
	-- Find penview plugin root (lua/penview/websocket/init.lua -> plugin root)
	local current_path = debug.getinfo(1).source:match("@?(.*/)") or ""
	local plugin_root = vim.fn.fnamemodify(current_path, ":h:h:h:h")
	vim.opt.runtimepath:append(plugin_root .. "/rust/websocket-ffi")
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
