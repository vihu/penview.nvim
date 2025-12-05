# penview.nvim

Real-time Markdown preview for Neovim with GitHub Flavored Markdown styling.

## Features

- Live preview updates as you type (no save required)
- GitHub Flavored Markdown rendering
- Light/dark theme toggle
- Syntax highlighting for code blocks
- KaTeX math support
- Self-contained HTML export

## Requirements

- Neovim 0.9+
- Rust toolchain (for building)

## Installation

### lazy.nvim

```lua
{
  "vihu/penview.nvim",
  build = "make build",
  ft = "markdown",
  config = function()
    require("penview").setup({
      browser = "firefox",      -- Required: your browser command
      -- debounce = 100,        -- Optional: ms to wait before updating (default: 100)
      -- port = 0,              -- Optional: server port (default: random)
      -- debug = false,         -- Optional: enable debug logging
      -- sync_scroll = true,    -- Optional: sync scroll with nvim (default: true)
    })
  end,
  keys = {
    { "<leader>po", "<cmd>PenviewStart<cr>", desc = "[P]review [O]pen" },
    { "<leader>pc", "<cmd>PenviewStop<cr>", desc = "[P]review [C]lose" },
  },
}
```

## Usage

1. Open a markdown file
2. Run `:PenviewStart` or press `<leader>po`
3. Browser opens with live preview
4. Edit your markdown - preview updates in real-time
5. Run `:PenviewStop` or press `<leader>pc` to stop

## Commands

| Command         | Description                               |
| --------------- | ----------------------------------------- |
| `:PenviewStart` | Start the preview server and open browser |
| `:PenviewStop`  | Stop the preview server                   |

## Credits

This plugin basically is a combination of the following original works:

- [tatum](https://github.com/elijah-potter/tatum): Original Markdown renderer
- [websocket.nvim](https://github.com/samsze0/websocket.nvim): WebSocket client for Neovim

## License

MIT
