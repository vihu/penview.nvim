-- UUID v4 generator (inlined from utils.nvim to remove dependency)
local M = {}

local random = math.random

---@return string
function M.v4()
	local template = "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx"
	local result = string.gsub(template, "[xy]", function(c)
		local v = (c == "x") and random(0, 0xf) or random(8, 0xb)
		return string.format("%x", v)
	end)
	return result
end

return M
