# clawd.nvim

Neovim plugin for [ClawDE](https://clawde.io). Connects to the local `clawd` daemon and brings AI-assisted development directly into Neovim.

## Requirements

- Neovim 0.10+
- [plenary.nvim](https://github.com/nvim-lua/plenary.nvim)
- [nui.nvim](https://github.com/MunifTanjim/nui.nvim)
- `clawd` daemon running on `localhost:4300`

## Installation

### lazy.nvim

```lua
{
  "clawde-io/clawd.nvim",
  dependencies = {
    "nvim-lua/plenary.nvim",
    "MunifTanjim/nui.nvim",
  },
  config = function()
    require("clawd").setup({
      -- All fields are optional
      daemon_url = "ws://127.0.0.1:4300",
      auth_token_path = vim.fn.expand("~/.claw/auth.token"),
      provider = "claude",  -- or "codex"
      window = {
        width = 80,
        height = 30,
        border = "rounded",
      },
    })
  end,
}
```

### packer.nvim

```lua
use {
  "clawde-io/clawd.nvim",
  requires = {
    "nvim-lua/plenary.nvim",
    "MunifTanjim/nui.nvim",
  },
  config = function()
    require("clawd").setup()
  end,
}
```

## Commands

| Command | Description |
| --- | --- |
| `:ClawdChat [message]` | Open the ClawDE chat window. Optional initial message. |
| `:'<,'>ClawdAsk [question]` | Ask ClawDE about the visually selected code. |
| `:ClawdSessions` | List active daemon sessions. |

## Usage

**Chat window**

Open with `:ClawdChat`. Type your message at the `>` prompt and press `<CR>` to send. Responses stream in real time. Press `q` or `<Esc>` to close.

**Ask about selected code**

Select code in visual mode, then run `:ClawdAsk`. You will be prompted for your question. The selected code is sent as context alongside your question.

Or pass the question directly: `:'<,'>ClawdAsk What does this function do?`

**Keyboard shortcuts (suggested)**

Add to your config:

```lua
vim.keymap.set("n", "<leader>cc", ":ClawdChat<CR>", { desc = "ClawDE chat" })
vim.keymap.set("v", "<leader>ca", ":ClawdAsk<CR>", { desc = "Ask ClawDE" })
vim.keymap.set("n", "<leader>cs", ":ClawdSessions<CR>", { desc = "ClawDE sessions" })
```

## Configuration

All configuration fields with defaults:

```lua
require("clawd").setup({
  daemon_url = "ws://127.0.0.1:4300",
  auth_token_path = vim.fn.expand("~/.claw/auth.token"),
  provider = "claude",
  window = {
    width = 80,
    height = 30,
    border = "rounded",  -- or "single", "double", "shadow", "none"
  },
})
```

The auth token is read from `auth_token_path` at startup. Start the `clawd` daemon before loading Neovim, or it will reconnect on the first command.

## License

MIT
