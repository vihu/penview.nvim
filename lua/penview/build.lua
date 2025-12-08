-- penview build module
-- Downloads pre-compiled binaries or falls back to source compilation

local M = {}

-- GitHub repository for releases
M.repo = "vihu/penview.nvim"

-- Get the plugin's root directory
local function get_plugin_root()
	local source = debug.getinfo(1, "S").source:sub(2)
	return vim.fn.fnamemodify(source, ":h:h:h")
end

-- Detect current platform and architecture
function M.get_platform()
	local uname = vim.loop.os_uname()
	local os_name = uname.sysname:lower()
	local arch = uname.machine

	-- Normalize OS name
	if os_name == "darwin" then
		os_name = "macos"
	elseif os_name:match("linux") then
		os_name = "linux"
	else
		return nil, nil, "Unsupported OS: " .. os_name
	end

	-- Normalize architecture
	if arch == "arm64" then
		arch = "aarch64"
	elseif arch == "x86_64" or arch == "amd64" then
		arch = "x86_64"
	else
		return nil, nil, "Unsupported architecture: " .. arch
	end

	return os_name, arch, nil
end

-- Get the latest release version from GitHub
local function get_latest_version(callback)
	local url = string.format("https://api.github.com/repos/%s/releases/latest", M.repo)

	vim.system({ "curl", "-sL", url }, { text = true }, function(result)
		if result.code ~= 0 then
			callback(nil, "Failed to fetch latest release: " .. (result.stderr or "unknown error"))
			return
		end

		local ok, data = pcall(vim.json.decode, result.stdout)
		if not ok or not data or not data.tag_name then
			callback(nil, "Failed to parse release info")
			return
		end

		callback(data.tag_name, nil)
	end)
end

-- Construct download URL for a specific version and platform
function M.get_download_url(version, os_name, arch)
	local filename = string.format("penview-%s-%s-%s.tar.gz", version, os_name, arch)
	return string.format("https://github.com/%s/releases/download/%s/%s", M.repo, version, filename)
end

-- Download file to destination
local function download_file(url, dest, callback)
	vim.system({ "curl", "-sL", "-o", dest, url }, {}, function(result)
		if result.code ~= 0 then
			callback(false, "Download failed: " .. (result.stderr or "unknown error"))
			return
		end
		callback(true, nil)
	end)
end

-- Get checksums file URL for a version
function M.get_checksums_url(version)
	return string.format("https://github.com/%s/releases/download/%s/SHA256SUMS.txt", M.repo, version)
end

-- Verify SHA256 checksum of a file
local function verify_checksum(file_path, expected_checksum, callback)
	-- Use sha256sum on Linux, shasum -a 256 on macOS
	local cmd
	local uname = vim.loop.os_uname().sysname:lower()
	if uname == "darwin" then
		cmd = { "shasum", "-a", "256", file_path }
	else
		cmd = { "sha256sum", file_path }
	end

	vim.system(cmd, { text = true }, function(result)
		if result.code ~= 0 then
			callback(false, "Failed to compute checksum: " .. (result.stderr or "unknown error"))
			return
		end

		-- Both sha256sum and shasum output format: "checksum  filename"
		local computed = result.stdout:match("^(%x+)")
		if not computed then
			callback(false, "Failed to parse checksum output")
			return
		end

		if computed:lower() == expected_checksum:lower() then
			callback(true, nil)
		else
			callback(false, string.format("Checksum mismatch: expected %s, got %s", expected_checksum, computed))
		end
	end)
end

-- Parse SHA256SUMS.txt and find checksum for a specific file
local function parse_checksums(checksums_content, filename)
	for line in checksums_content:gmatch("[^\r\n]+") do
		-- Format: "checksum  filename" (two spaces)
		local checksum, name = line:match("^(%x+)%s+(.+)$")
		if checksum and name and name:match(vim.pesc(filename) .. "$") then
			return checksum
		end
	end
	return nil
end

-- Extract tarball to destination directory
local function extract_tarball(tarball_path, dest_dir, callback)
	-- Create destination directory
	vim.fn.mkdir(dest_dir, "p")

	-- Extract with --strip-components=1 to avoid nested directory
	vim.system({ "tar", "-xzf", tarball_path, "-C", dest_dir, "--strip-components=1" }, {}, function(result)
		if result.code ~= 0 then
			callback(false, "Extraction failed: " .. (result.stderr or "unknown error"))
			return
		end

		-- Make binary executable
		local binary = dest_dir .. "/penview"
		vim.system({ "chmod", "+x", binary }, {}, function(chmod_result)
			if chmod_result.code ~= 0 then
				callback(false, "Failed to set executable permission")
				return
			end
			callback(true, nil)
		end)
	end)
end

-- Get installed version from VERSION file
function M.get_installed_version()
	local root = get_plugin_root()
	local version_file = root .. "/bin/VERSION"
	local f = io.open(version_file, "r")
	if f then
		local version = f:read("*l")
		f:close()
		return version
	end
	return nil
end

-- Check if pre-compiled binaries exist
function M.binaries_exist()
	local root = get_plugin_root()
	local binary = root .. "/bin/penview"
	local ffi = root .. "/bin/websocket_ffi.so"
	return vim.fn.filereadable(binary) == 1 and vim.fn.filereadable(ffi) == 1
end

-- Install pre-compiled binaries
function M.install(opts)
	opts = opts or {}
	local force = opts.force or false
	local root = get_plugin_root()
	local bin_dir = root .. "/bin"

	-- Check if already installed
	if not force and M.binaries_exist() then
		local version = M.get_installed_version()
		print("[penview] Binaries already installed" .. (version and (" (" .. version .. ")") or ""))
		return true
	end

	-- Detect platform
	local os_name, arch, err = M.get_platform()
	if err then
		print("[penview] " .. err)
		print("[penview] Falling back to source compilation...")
		return M.build_from_source()
	end

	print(string.format("[penview] Detected platform: %s-%s", os_name, arch))
	print("[penview] Fetching latest release...")

	-- Get latest version and download (synchronous for plugin manager compatibility)
	local version = nil
	local version_err = nil
	local done = false

	get_latest_version(function(v, e)
		version = v
		version_err = e
		done = true
	end)

	-- Wait for async operation (with timeout)
	local timeout = 30000 -- 30 seconds
	local start = vim.loop.now()
	while not done and (vim.loop.now() - start) < timeout do
		vim.wait(100, function()
			return done
		end, 100)
	end

	if version_err or not version then
		print("[penview] " .. (version_err or "Timeout fetching release"))
		print("[penview] Falling back to source compilation...")
		return M.build_from_source()
	end

	print("[penview] Latest version: " .. version)

	-- Download
	local url = M.get_download_url(version, os_name, arch)
	local tmp_file = vim.fn.tempname() .. ".tar.gz"

	print("[penview] Downloading from: " .. url)

	local download_ok = nil
	local download_err = nil
	done = false

	download_file(url, tmp_file, function(ok, e)
		download_ok = ok
		download_err = e
		done = true
	end)

	start = vim.loop.now()
	while not done and (vim.loop.now() - start) < timeout do
		vim.wait(100, function()
			return done
		end, 100)
	end

	if not download_ok then
		print("[penview] " .. (download_err or "Download timeout"))
		print("[penview] Falling back to source compilation...")
		os.remove(tmp_file)
		return M.build_from_source()
	end

	print("[penview] Download complete, verifying checksum...")

	-- Download checksums file
	local checksums_url = M.get_checksums_url(version)
	local checksums_content = nil
	local checksums_err = nil
	done = false

	vim.system({ "curl", "-sL", checksums_url }, { text = true }, function(result)
		if result.code ~= 0 then
			checksums_err = "Failed to download checksums: " .. (result.stderr or "unknown error")
		else
			checksums_content = result.stdout
		end
		done = true
	end)

	start = vim.loop.now()
	while not done and (vim.loop.now() - start) < timeout do
		vim.wait(100, function()
			return done
		end, 100)
	end

	if checksums_err or not checksums_content then
		print("[penview] [WARN] " .. (checksums_err or "Timeout downloading checksums"))
		print("[penview] [WARN] Skipping checksum verification")
	else
		-- Parse checksums and find expected checksum for our file
		local tarball_name = string.format("penview-%s-%s-%s.tar.gz", version, os_name, arch)
		local expected_checksum = parse_checksums(checksums_content, tarball_name)

		if not expected_checksum then
			print("[penview] [WARN] Checksum not found for " .. tarball_name)
			print("[penview] [WARN] Skipping checksum verification")
		else
			-- Verify checksum
			local verify_ok = nil
			local verify_err = nil
			done = false

			verify_checksum(tmp_file, expected_checksum, function(ok, e)
				verify_ok = ok
				verify_err = e
				done = true
			end)

			start = vim.loop.now()
			while not done and (vim.loop.now() - start) < timeout do
				vim.wait(100, function()
					return done
				end, 100)
			end

			if not verify_ok then
				print("[penview] [ERROR] " .. (verify_err or "Checksum verification timeout"))
				print("[penview] Falling back to source compilation...")
				os.remove(tmp_file)
				return M.build_from_source()
			end

			print("[penview] Checksum verified")
		end
	end

	print("[penview] Extracting...")

	-- Extract
	local extract_ok = nil
	local extract_err = nil
	done = false

	extract_tarball(tmp_file, bin_dir, function(ok, e)
		extract_ok = ok
		extract_err = e
		done = true
	end)

	start = vim.loop.now()
	while not done and (vim.loop.now() - start) < timeout do
		vim.wait(100, function()
			return done
		end, 100)
	end

	-- Cleanup temp file
	os.remove(tmp_file)

	if not extract_ok then
		print("[penview] " .. (extract_err or "Extraction timeout"))
		print("[penview] Falling back to source compilation...")
		return M.build_from_source()
	end

	-- Create lua/ subdirectory in bin/ for runtimepath compatibility
	-- Neovim looks for lua/ subdirectory when resolving require()
	local ffi_src = bin_dir .. "/websocket_ffi.so"
	local ffi_lua_dir = bin_dir .. "/lua"
	vim.fn.mkdir(ffi_lua_dir, "p")
	vim.fn.system({ "cp", ffi_src, ffi_lua_dir .. "/websocket_ffi.so" })

	print("[penview] [OK] Installation complete (" .. version .. ")")
	return true
end

-- Build from source using make
function M.build_from_source()
	local root = get_plugin_root()

	-- Check for cargo
	if vim.fn.executable("cargo") ~= 1 then
		print("[penview] [ERROR] Rust toolchain not found")
		print("[penview] Install Rust from https://rustup.rs/ and try again")
		return false
	end

	print("[penview] Building from source (this may take a while)...")

	local result = vim.fn.system({ "make", "-C", root, "build" })
	if vim.v.shell_error ~= 0 then
		print("[penview] [ERROR] Build failed:")
		print(result)
		return false
	end

	print("[penview] [OK] Build complete")
	return true
end

-- Update to latest version
function M.update()
	local current = M.get_installed_version()
	if current then
		print("[penview] Current version: " .. current)
	end
	return M.install({ force = true })
end

-- Clean installed binaries
function M.clean()
	local root = get_plugin_root()
	local bin_dir = root .. "/bin"
	if vim.fn.isdirectory(bin_dir) == 1 then
		vim.fn.delete(bin_dir, "rf")
		print("[penview] Cleaned bin/ directory")
	end
end

return M
