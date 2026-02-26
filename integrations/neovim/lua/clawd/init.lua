--- clawd.nvim — ClawDE Neovim plugin
--- Sprint KK NV.1
---
--- Connects to the local clawd daemon (ws://127.0.0.1:4300) and provides
--- `:ClawdChat` and `:ClawdAsk` commands for AI-assisted development.
---
--- Dependencies: plenary.nvim (async), nui.nvim (UI)
---
--- Usage (lazy.nvim):
---   { "clawde-io/clawd.nvim", dependencies = { "nvim-lua/plenary.nvim", "MunifTanjim/nui.nvim" } }

local M = {}
local rpc = require("clawd.rpc")
local commands = require("clawd.commands")

M._config = {
  daemon_url = "ws://127.0.0.1:4300",
  auth_token_path = vim.fn.expand("~/.claw/auth.token"),
  provider = "claude",
  window = {
    width = 80,
    height = 30,
    border = "rounded",
  },
}

--- Setup the plugin with user config.
---@param opts table optional config overrides
function M.setup(opts)
  M._config = vim.tbl_deep_extend("force", M._config, opts or {})

  -- Read auth token from file
  local token_path = M._config.auth_token_path
  local ok, lines = pcall(vim.fn.readfile, token_path)
  M._config.auth_token = (ok and lines[1]) or ""

  rpc.setup(M._config)
  commands.setup(M._config)

  -- Register user commands
  vim.api.nvim_create_user_command("ClawdChat", function(args)
    commands.open_chat(args.args)
  end, { nargs = "?", desc = "Open ClawDE chat window" })

  vim.api.nvim_create_user_command("ClawdAsk", function(args)
    commands.ask_with_selection(args.args)
  end, { nargs = "?", range = true, desc = "Ask ClawDE about selected code" })

  vim.api.nvim_create_user_command("ClawdSessions", function()
    commands.list_sessions()
  end, { desc = "List active clawd sessions" })
end

return M
