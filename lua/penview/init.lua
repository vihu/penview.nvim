-- penview.nvim - Real-time Markdown preview for Neovim
-- Based on tatum https://github.com/elijah-potter/tatum
-- WebSocket support from websocket.nvim (https://github.com/samsze0/websocket.nvim)

local M = {}

-- Get the plugin's root directory
local function get_plugin_root()
	local source = debug.getinfo(1, "S").source:sub(2)
	-- source is /path/to/penview/lua/penview/init.lua
	-- we want /path/to/penview
	return vim.fn.fnamemodify(source, ":h:h:h")
end

-- Get the path to the penview binary
local function get_binary_path()
	local root = get_plugin_root()
	local binary = root .. "/target/release/penview"
	if vim.fn.executable(binary) == 1 then
		return binary
	end
	-- Fallback to PATH
	if vim.fn.executable("penview") == 1 then
		return "penview"
	end
	return nil
end

-- Configuration
M.browser = nil
M.debounce_ms = 100
M.port = 0
M.client = nil
M.timer = nil
M.server_addr = nil
M.server_job = nil
M.debug = false
M.sync_scroll = true

function M.setup(opts)
	opts = opts or {}
	if not opts.browser then
		error("[penview] 'browser' is required in setup(). Example: require('penview').setup({ browser = 'firefox' })")
	end
	M.browser = opts.browser
	M.debounce_ms = opts.debounce or 100
	M.port = opts.port or 0
	M.debug = opts.debug or false
	M.sync_scroll = opts.sync_scroll ~= false -- default true
end

local function log(msg)
	if M.debug then
		print("[penview] " .. msg)
	end
end

function M.start()
	local path = vim.fn.expand("%:p")
	if not path:match("%.md$") then
		print("[penview] Not a markdown file")
		return
	end

	if not M.browser then
		print("[penview] Browser not configured. Call setup() first with browser option.")
		return
	end

	local binary = get_binary_path()
	if not binary then
		print("[penview] Binary not found. Run 'make build' in the plugin directory.")
		return
	end

	log("Binary: " .. binary)
	log("Path: " .. path)
	log("Browser: " .. M.browser)

	-- Build command
	local cmd = { binary, "serve", "-q", "-p", tostring(M.port), "--open", path, "--browser", M.browser }

	log("Command: " .. table.concat(cmd, " "))

	-- Store path for use in callback
	local file_path = path

	-- Start server
	M.server_job = vim.fn.jobstart(cmd, {
		stdout_buffered = false, -- Don't buffer - we need the address immediately
		on_stdout = function(_, data)
			log("stdout received: " .. vim.inspect(data))
			for _, line in ipairs(data) do
				if line and line ~= "" then
					M.server_addr = line:gsub("%s+", "")
					print("[penview] Server started at " .. M.server_addr)
					vim.schedule(function()
						M._connect(file_path)
					end)
					return -- Only need the first line (the address)
				end
			end
		end,
		on_stderr = function(_, data)
			for _, line in ipairs(data) do
				if line and line ~= "" then
					print("[penview] stderr: " .. line)
				end
			end
		end,
		on_exit = function(_, code)
			log("Server exited with code " .. code)
			if code ~= 0 then
				print("[penview] Server exited with code " .. code)
			end
			M.server_job = nil
			M.client = nil
		end,
	})

	log("Job ID: " .. tostring(M.server_job))

	if M.server_job <= 0 then
		print("[penview] Failed to start server (job ID: " .. tostring(M.server_job) .. ")")
	end
end

function M._connect(path)
	log("Connecting to WebSocket...")

	-- Initialize vendored websocket module
	local ws_ok, websocket = pcall(require, "penview.websocket")
	if not ws_ok then
		print("[penview] Failed to load websocket module: " .. tostring(websocket))
		return
	end

	-- Initialize websocket if not already done
	if _G["_WEBSOCKET_NVIM"] == nil then
		log("Initializing websocket...")
		websocket.setup()
	end

	-- Now require the client module
	local client_ok, client_module = pcall(require, "penview.websocket.client")
	if not client_ok then
		print("[penview] Failed to load websocket client: " .. tostring(client_module))
		return
	end

	local WebsocketClient = client_module.WebsocketClient
	if not WebsocketClient then
		print("[penview] WebsocketClient not found in websocket.client module")
		return
	end

	log("websocket loaded successfully")

	local ws_url = "ws://" .. M.server_addr .. "/api/preview?path=" .. vim.fn.fnameescape(path)
	log("WebSocket URL: " .. ws_url)

	M.client = WebsocketClient.new({
		connect_addr = ws_url,
		on_connect = function(self)
			print("[penview] Connected to preview")
			vim.schedule(function()
				M._send_buffer()
				M._setup_autocmds()
			end)
		end,
		on_disconnect = function(self)
			print("[penview] Disconnected")
			M.client = nil
		end,
		on_message = function(self, msg)
			log("Received message: " .. tostring(msg))
		end,
		on_error = function(self, err)
			print("[penview] WebSocket error: " .. vim.inspect(err))
		end,
	})

	log("Calling try_connect...")
	local connect_ok, connect_err = pcall(function()
		M.client:try_connect()
	end)
	if not connect_ok then
		print("[penview] Connection failed: " .. tostring(connect_err))
	else
		log("try_connect called successfully")
	end
end

function M._send_buffer()
	if M.client then
		local lines = vim.api.nvim_buf_get_lines(0, 0, -1, false)
		local total_lines = #lines
		local cursor_line = vim.fn.line(".")

		local data = vim.fn.json_encode({
			content = table.concat(lines, "\n"),
			cursor_line = cursor_line,
			total_lines = total_lines,
			sync_scroll = M.sync_scroll,
		})
		M.client:try_send_data(data)
	end
end

function M._setup_autocmds()
	local bufnr = vim.api.nvim_get_current_buf()

	vim.api.nvim_create_autocmd({ "TextChanged", "TextChangedI", "CursorMoved", "CursorMovedI" }, {
		buffer = bufnr,
		callback = function()
			if M.timer then
				vim.fn.timer_stop(M.timer)
			end
			M.timer = vim.fn.timer_start(M.debounce_ms, function()
				vim.schedule(function()
					M._send_buffer()
				end)
			end)
		end,
	})

	-- Clean up when buffer is closed
	vim.api.nvim_create_autocmd("BufUnload", {
		buffer = bufnr,
		callback = function()
			M.stop()
		end,
	})
end

function M.stop()
	if M.client then
		M.client:try_disconnect()
		M.client = nil
	end
	if M.server_job then
		vim.fn.jobstop(M.server_job)
		M.server_job = nil
	end
	if M.timer then
		vim.fn.timer_stop(M.timer)
		M.timer = nil
	end
	M.server_addr = nil
	print("[penview] Stopped")
end

-- Commands
vim.api.nvim_create_user_command("PenviewStart", M.start, {})
vim.api.nvim_create_user_command("PenviewStop", M.stop, {})

return M
