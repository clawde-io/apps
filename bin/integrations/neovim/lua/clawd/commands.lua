--- clawd.commands — :ClawdChat, :ClawdAsk, :ClawdSessions implementations.
---
--- Sprint KK NV.2
--- Depends: clawd.rpc, nui.nvim (Popup + Input), plenary.nvim (async)

local M = {}
local rpc = require("clawd.rpc")
local Popup = require("nui.popup")
local Input = require("nui.input")
local event = require("nui.utils.autocmd").event

-- ── Config (set by setup) ────────────────────────────────────────────────────

M._config = {}

function M.setup(cfg)
  M._config = cfg
end

-- ── Shared helpers ────────────────────────────────────────────────────────────

--- Append a line to a nui Popup buffer.
local function append_line(popup, line)
  vim.schedule(function()
    local buf = popup.bufnr
    if not vim.api.nvim_buf_is_valid(buf) then return end
    local line_count = vim.api.nvim_buf_line_count(buf)
    vim.api.nvim_buf_set_lines(buf, line_count, -1, false, { line })
    -- Auto-scroll
    local wins = vim.fn.win_findbuf(buf)
    for _, win in ipairs(wins) do
      vim.api.nvim_win_set_cursor(win, { vim.api.nvim_buf_line_count(buf), 0 })
    end
  end)
end

--- Render a message (role + content) into a popup buffer.
local function render_message(popup, role, content)
  local prefix = role == "user" and "You: " or "ClawDE: "
  local lines = vim.split(content or "", "\n", { plain = true })
  append_line(popup, "")
  append_line(popup, prefix .. lines[1])
  for i = 2, #lines do
    append_line(popup, "  " .. lines[i])
  end
end

--- Create or reuse a daemon session, then stream a message.
local function send_to_session(session_id, message, on_token, on_done)
  local function do_send(sid)
    rpc.call("message.send", { session_id = sid, content = message }, function(err, result)
      if err then
        on_done(err)
        return
      end
      -- Stream: listen for push events
      local done_received = false
      rpc.on("session.messageDelta", function(params)
        if params.session_id == sid then
          on_token(params.delta or "")
        end
      end)
      rpc.on("session.messageComplete", function(params)
        if params.session_id == sid and not done_received then
          done_received = true
          on_done(nil)
        end
      end)
      -- Fallback: result may contain full content if streaming not supported
      if result and result.content then
        on_token(result.content)
        on_done(nil)
      end
    end)
  end

  if session_id then
    do_send(session_id)
  else
    rpc.call("session.create", {
      provider = M._config.provider or "claude",
      repo_path = vim.fn.getcwd(),
    }, function(err, result)
      if err then
        on_done(err)
        return
      end
      do_send(result.session_id or result.id)
    end)
  end
end

-- ── :ClawdChat ───────────────────────────────────────────────────────────────

local chat_state = {
  session_id = nil,
  popup = nil,
  input = nil,
}

local function close_chat()
  if chat_state.popup then
    chat_state.popup:unmount()
    chat_state.popup = nil
  end
  if chat_state.input then
    chat_state.input:unmount()
    chat_state.input = nil
  end
end

local function open_chat_window()
  local cfg = M._config.window or {}
  local width = cfg.width or 80
  local height = cfg.height or 28
  local border = cfg.border or "rounded"

  -- Output popup (top 80% of height)
  local out_height = height - 4
  local out_popup = Popup({
    enter = false,
    focusable = false,
    border = { style = border, text = { top = " ClawDE Chat ", top_align = "center" } },
    position = { row = math.floor((vim.o.lines - height) / 2), col = math.floor((vim.o.columns - width) / 2) },
    size = { width = width, height = out_height },
    buf_options = { modifiable = false, readonly = true },
    win_options = { wrap = true, linebreak = true },
  })

  -- Input popup (bottom)
  local inp = Input({
    border = { style = border, text = { top = " Message ", top_align = "left" } },
    position = {
      row = math.floor((vim.o.lines - height) / 2) + out_height + 2,
      col = math.floor((vim.o.columns - width) / 2),
    },
    size = { width = width, height = 1 },
  }, {
    prompt = "> ",
    default_value = "",
    on_submit = function(value)
      if value == "" then return end
      render_message(out_popup, "user", value)

      -- Accumulate response
      local response_buf = {}
      send_to_session(chat_state.session_id, value,
        function(token)
          table.insert(response_buf, token)
        end,
        function(err)
          if err then
            append_line(out_popup, "[error] " .. tostring(err))
          else
            local full = table.concat(response_buf)
            render_message(out_popup, "assistant", full)
          end
        end
      )
    end,
  })

  out_popup:mount()
  inp:mount()
  inp:on(event.BufLeave, close_chat)

  -- Keybinds: q / <Esc> close
  out_popup:map("n", "q", close_chat, { noremap = true })
  out_popup:map("n", "<Esc>", close_chat, { noremap = true })

  chat_state.popup = out_popup
  chat_state.input = inp

  -- Focus input
  vim.api.nvim_set_current_win(inp.winid)
end

--- Open (or reopen) the ClawDE chat window.
--- @param initial_prompt string optional first message
function M.open_chat(initial_prompt)
  if not rpc.is_connected() then
    rpc.connect(function(err)
      if err then
        vim.notify("[clawd] Cannot connect to daemon: " .. tostring(err), vim.log.levels.ERROR)
        return
      end
      vim.schedule(function()
        open_chat_window()
        if initial_prompt and initial_prompt ~= "" then
          -- wait a tick for window to render
          vim.defer_fn(function()
            render_message(chat_state.popup, "user", initial_prompt)
            local response_buf = {}
            send_to_session(nil, initial_prompt,
              function(token) table.insert(response_buf, token) end,
              function(err2)
                if not err2 then
                  render_message(chat_state.popup, "assistant", table.concat(response_buf))
                end
              end)
          end, 100)
        end
      end)
    end)
    return
  end
  open_chat_window()
end

-- ── :ClawdAsk ────────────────────────────────────────────────────────────────

--- Ask ClawDE about selected code.
--- @param question string optional question (may be empty — will prompt)
function M.ask_with_selection(question)
  -- Grab visual selection
  local start_line = vim.fn.line("'<")
  local end_line = vim.fn.line("'>")
  local lines = vim.api.nvim_buf_get_lines(0, start_line - 1, end_line, false)
  local selection = table.concat(lines, "\n")
  local fname = vim.fn.expand("%:t")

  local prompt_fn = function(q)
    local context_message = string.format(
      "File: %s (lines %d-%d)\n```\n%s\n```\n\n%s",
      fname, start_line, end_line, selection, q
    )

    -- Quick floating result popup
    local cfg = M._config.window or {}
    local width = cfg.width or 80
    local height = cfg.height or 20

    local result_popup = Popup({
      enter = true,
      focusable = true,
      border = { style = cfg.border or "rounded", text = { top = " ClawDE Answer ", top_align = "center" } },
      position = "50%",
      size = { width = width, height = height },
      buf_options = { modifiable = false },
      win_options = { wrap = true, linebreak = true },
    })
    result_popup:mount()
    result_popup:map("n", "q", function() result_popup:unmount() end, { noremap = true })
    result_popup:map("n", "<Esc>", function() result_popup:unmount() end, { noremap = true })

    append_line(result_popup, "Question: " .. q)
    append_line(result_popup, "")
    append_line(result_popup, "ClawDE: (thinking…)")

    local response_parts = {}
    local thinking_cleared = false

    if not rpc.is_connected() then
      rpc.connect(function(err)
        if err then
          vim.schedule(function()
            vim.notify("[clawd] Cannot connect: " .. tostring(err), vim.log.levels.ERROR)
            result_popup:unmount()
          end)
          return
        end
        send_to_session(nil, context_message,
          function(token)
            if not thinking_cleared then
              -- Clear "thinking…" line
              vim.schedule(function()
                local buf = result_popup.bufnr
                if vim.api.nvim_buf_is_valid(buf) then
                  local lc = vim.api.nvim_buf_line_count(buf)
                  vim.api.nvim_buf_set_option(buf, "modifiable", true)
                  vim.api.nvim_buf_set_lines(buf, lc - 1, lc, false, {})
                  vim.api.nvim_buf_set_option(buf, "modifiable", false)
                end
              end)
              thinking_cleared = true
            end
            table.insert(response_parts, token)
          end,
          function(done_err)
            vim.schedule(function()
              if done_err then
                append_line(result_popup, "[error] " .. tostring(done_err))
              else
                local full = table.concat(response_parts)
                for _, line in ipairs(vim.split(full, "\n", { plain = true })) do
                  append_line(result_popup, line)
                end
              end
            end)
          end
        )
      end)
      return
    end

    send_to_session(nil, context_message,
      function(token) table.insert(response_parts, token) end,
      function(done_err)
        vim.schedule(function()
          if done_err then
            append_line(result_popup, "[error] " .. tostring(done_err))
          else
            local full = table.concat(response_parts)
            for _, line in ipairs(vim.split(full, "\n", { plain = true })) do
              append_line(result_popup, line)
            end
          end
        end)
      end
    )
  end

  if question and question ~= "" then
    prompt_fn(question)
  else
    -- Ask for question via vim.ui.input
    vim.ui.input({ prompt = "Ask ClawDE: " }, function(input)
      if input and input ~= "" then
        prompt_fn(input)
      end
    end)
  end
end

-- ── :ClawdSessions ──────────────────────────────────────────────────────────

--- List active clawd sessions in a floating window.
function M.list_sessions()
  if not rpc.is_connected() then
    vim.notify("[clawd] Not connected to daemon", vim.log.levels.WARN)
    return
  end

  rpc.call("session.list", {}, function(err, result)
    vim.schedule(function()
      if err then
        vim.notify("[clawd] session.list error: " .. tostring(err), vim.log.levels.ERROR)
        return
      end
      local sessions = result and result.sessions or {}
      if #sessions == 0 then
        vim.notify("[clawd] No active sessions", vim.log.levels.INFO)
        return
      end

      local lines = { "Active Sessions:", "" }
      for _, s in ipairs(sessions) do
        table.insert(lines, string.format(
          "  [%s] %s — %s (%s)",
          s.id:sub(1, 8), s.provider or "?", s.status or "?", s.repo_path or ""
        ))
      end
      table.insert(lines, "")
      table.insert(lines, "Press q or <Esc> to close")

      local popup = Popup({
        enter = true,
        focusable = true,
        border = { style = "rounded", text = { top = " Sessions ", top_align = "center" } },
        position = "50%",
        size = { width = 70, height = math.min(#lines + 2, 20) },
        buf_options = { modifiable = false },
      })
      popup:mount()
      popup:map("n", "q", function() popup:unmount() end, { noremap = true })
      popup:map("n", "<Esc>", function() popup:unmount() end, { noremap = true })

      vim.api.nvim_buf_set_option(popup.bufnr, "modifiable", true)
      vim.api.nvim_buf_set_lines(popup.bufnr, 0, -1, false, lines)
      vim.api.nvim_buf_set_option(popup.bufnr, "modifiable", false)
    end)
  end)
end

return M
