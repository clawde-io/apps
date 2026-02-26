--- clawd.rpc — WebSocket/JSON-RPC 2.0 client for the clawd daemon.
---
--- Uses vim.loop (libuv) for async TCP + a minimal WebSocket upgrade.
--- Handles framing for both text and binary frames (sends text frames only).

local M = {}
local uv = vim.loop
local json = vim.json

-- ── State ────────────────────────────────────────────────────────────────────

M._config = {}
M._tcp = nil
M._connected = false
M._next_id = 1
M._pending = {}   -- id → callback(err, result)
M._push_handlers = {}  -- method → [callback, ...]
M._recv_buf = ""
M._ws_upgraded = false

-- ── WebSocket helpers ─────────────────────────────────────────────────────────

local function b64(s)
  local chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
  local result = {}
  local padding = (3 - #s % 3) % 3
  s = s .. string.rep("\0", padding)
  for i = 1, #s, 3 do
    local a, b, c = s:byte(i), s:byte(i + 1), s:byte(i + 2)
    local n = a * 65536 + b * 256 + c
    table.insert(result, chars:sub(math.floor(n / 262144) + 1, math.floor(n / 262144) + 1))
    table.insert(result, chars:sub(math.floor(n / 4096) % 64 + 1, math.floor(n / 4096) % 64 + 1))
    table.insert(result, chars:sub(math.floor(n / 64) % 64 + 1, math.floor(n / 64) % 64 + 1))
    table.insert(result, chars:sub(n % 64 + 1, n % 64 + 1))
  end
  for i = 1, padding do result[#result - i + 1] = "=" end
  return table.concat(result)
end

local function random_key()
  local bytes = {}
  for i = 1, 16 do bytes[i] = string.char(math.random(0, 255)) end
  return b64(table.concat(bytes))
end

local function ws_upgrade_request(host, port)
  local key = random_key()
  return string.format(
    "GET / HTTP/1.1\r\n"
    .. "Host: %s:%d\r\n"
    .. "Upgrade: websocket\r\n"
    .. "Connection: Upgrade\r\n"
    .. "Sec-WebSocket-Key: %s\r\n"
    .. "Sec-WebSocket-Version: 13\r\n"
    .. "\r\n",
    host, port, key
  )
end

--- Encode a text WebSocket frame (opcode 0x1, masked).
local function ws_frame(payload)
  local len = #payload
  local header
  if len < 126 then
    header = string.char(0x81, 0x80 | len)
  elseif len < 65536 then
    header = string.char(0x81, 0xFE, math.floor(len / 256), len % 256)
  else
    -- large frames: simplified 8-byte length
    header = string.char(0x81, 0xFF, 0, 0, 0, 0,
      math.floor(len / 16777216),
      math.floor(len / 65536) % 256,
      math.floor(len / 256) % 256,
      len % 256)
  end
  -- masking key (4 random bytes)
  local mask = { math.random(0, 255), math.random(0, 255), math.random(0, 255), math.random(0, 255) }
  local mask_str = string.char(mask[1], mask[2], mask[3], mask[4])
  local masked = {}
  for i = 1, #payload do
    masked[i] = string.char(payload:byte(i) ~ mask[(i - 1) % 4 + 1])
  end
  return header .. mask_str .. table.concat(masked)
end

--- Parse one complete WebSocket text frame from buf.
--- Returns (payload, rest) or nil if incomplete.
local function ws_parse_frame(buf)
  if #buf < 2 then return nil end
  local b1, b2 = buf:byte(1), buf:byte(2)
  local masked = (b2 & 0x80) ~= 0
  local len = b2 & 0x7F
  local offset = 3
  if len == 126 then
    if #buf < 4 then return nil end
    len = buf:byte(3) * 256 + buf:byte(4)
    offset = 5
  elseif len == 127 then
    if #buf < 10 then return nil end
    -- simplified: only handle lower 32 bits
    len = buf:byte(7) * 16777216 + buf:byte(8) * 65536 + buf:byte(9) * 256 + buf:byte(10)
    offset = 11
  end
  if masked then offset = offset + 4 end
  if #buf < offset - 1 + len then return nil end
  local payload = buf:sub(offset, offset + len - 1)
  return payload, buf:sub(offset + len)
end

-- ── Internal dispatch ─────────────────────────────────────────────────────────

local function dispatch_message(raw)
  local ok, msg = pcall(json.decode, raw)
  if not ok then return end

  if msg.id then
    -- Response to a pending call
    local cb = M._pending[msg.id]
    if cb then
      M._pending[msg.id] = nil
      if msg.error then
        cb(msg.error.message or "RPC error", nil)
      else
        cb(nil, msg.result)
      end
    end
  elseif msg.method then
    -- Push notification
    local handlers = M._push_handlers[msg.method] or {}
    for _, h in ipairs(handlers) do
      pcall(h, msg.params)
    end
  end
end

local function on_data(data)
  M._recv_buf = M._recv_buf .. data

  if not M._ws_upgraded then
    -- Wait for HTTP upgrade response
    local header_end = M._recv_buf:find("\r\n\r\n")
    if header_end then
      M._ws_upgraded = true
      M._recv_buf = M._recv_buf:sub(header_end + 4)
      -- Authenticate immediately
      M.call("daemon.auth", { token = M._config.auth_token }, function(err)
        if err then
          vim.schedule(function()
            vim.notify("[clawd] Auth failed: " .. tostring(err), vim.log.levels.WARN)
          end)
        else
          M._connected = true
          vim.schedule(function()
            vim.notify("[clawd] Connected to daemon", vim.log.levels.INFO)
          end)
        end
      end)
    end
    return
  end

  -- Parse WebSocket frames
  while true do
    local payload, rest = ws_parse_frame(M._recv_buf)
    if not payload then break end
    M._recv_buf = rest or ""
    dispatch_message(payload)
  end
end

-- ── Public API ────────────────────────────────────────────────────────────────

--- Setup the RPC client with a config table (daemon_url, auth_token).
function M.setup(cfg)
  M._config = cfg
end

--- Connect to the daemon WebSocket.
function M.connect(callback)
  local url = M._config.daemon_url or "ws://127.0.0.1:4300"
  local host = url:match("ws://([^:/]+)")
  local port = tonumber(url:match(":(%d+)")) or 4300

  M._tcp = uv.new_tcp()
  M._tcp:connect(host, port, function(err)
    if err then
      callback(err)
      return
    end
    M._tcp:read_start(function(read_err, data)
      if read_err or not data then
        M._connected = false
        return
      end
      on_data(data)
    end)
    -- Send WebSocket upgrade
    M._tcp:write(ws_upgrade_request(host, port))
    callback(nil)
  end)
end

--- Send a JSON-RPC call. callback(err, result).
function M.call(method, params, callback)
  local id = M._next_id
  M._next_id = M._next_id + 1
  M._pending[id] = callback or function() end

  local req = json.encode({
    jsonrpc = "2.0",
    id = id,
    method = method,
    params = params or {},
  })
  M._tcp:write(ws_frame(req))
end

--- Register a handler for push notifications.
function M.on(method, callback)
  if not M._push_handlers[method] then
    M._push_handlers[method] = {}
  end
  table.insert(M._push_handlers[method], callback)
end

--- Close the connection.
function M.close()
  M._connected = false
  if M._tcp then
    M._tcp:close()
    M._tcp = nil
  end
  M._pending = {}
end

--- Returns true when connected and authenticated.
function M.is_connected()
  return M._connected
end

return M
