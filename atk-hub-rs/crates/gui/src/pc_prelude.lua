-- ATK HUB PC script prelude, ported from the original Hub runtime.
VK = {
  A = 0x41,
  B = 0x42,
  C = 0x43,
  D = 0x44,
  E = 0x45,
  F = 0x46,
  G = 0x47,
  H = 0x48,
  I = 0x49,
  J = 0x4a,
  K = 0x4b,
  L = 0x4c,
  M = 0x4d,
  N = 0x4e,
  O = 0x4f,
  P = 0x50,
  Q = 0x51,
  R = 0x52,
  S = 0x53,
  T = 0x54,
  U = 0x55,
  V = 0x56,
  W = 0x57,
  X = 0x58,
  Y = 0x59,
  Z = 0x5a,
  ["0"] = 0x30,
  ["1"] = 0x31,
  ["2"] = 0x32,
  ["3"] = 0x33,
  ["4"] = 0x34,
  ["5"] = 0x35,
  ["6"] = 0x36,
  ["7"] = 0x37,
  ["8"] = 0x38,
  ["9"] = 0x39,
  F1 = 0x70,
  F2 = 0x71,
  F3 = 0x72,
  F4 = 0x73,
  F5 = 0x74,
  F6 = 0x75,
  F7 = 0x76,
  F8 = 0x77,
  F9 = 0x78,
  F10 = 0x79,
  F11 = 0x7a,
  F12 = 0x7b,
  SPACE = 0x20,
  SHIFT = 0x10,
  CONTROL = 0x11,
  MENU = 0x12,
  ESCAPE = 0x1b,
  BACK = 0x8,
  TAB = 0x9,
  RETURN = 0xd,
  LEFT = 0x25,
  UP = 0x26,
  RIGHT = 0x27,
  DOWN = 0x28,
  LBUTTON = 0x1,
  RBUTTON = 0x2,
  MBUTTON = 0x4,
  XBUTTON1 = 0x5,
  XBUTTON2 = 0x6,
}
do
  if input then
    for k, v in pairs(VK) do
      if type(k) == "string" and type(v) == "number" then
        local nk
        if #k == 1 and k:match("[A-Z]") then
          nk = "VK_" .. k
        elseif k:match("^%d$") then
          nk = "VK_" .. k
        elseif k:match("^F%d+$") then
          nk = "VK_" .. k
        else
          nk = "VK_" .. k
        end
        pcall(function() input[nk] = v end)
      end
    end
  end
end
do
  local raw = input_log_native
  if input and raw then
    input.log = function(...)
      return raw(...)
    end
  end
end
function move_abs(x, y)
  move_abs_native(x, y)
end
function move_rel(dx, dy)
  move_rel_native(dx, dy)
end
do
  if input then
    input.move_rel = function(dx, dy)
      move_rel_native(dx, dy)
    end
  end
end
function mouse_click(button)
  local b = tonumber(button)
  if b == nil then return end
  mouse_click_native(math.floor(b))
end
function mouse_down(button)
  local b = tonumber(button)
  if b == nil then return end
  mouse_down_native(math.floor(b))
end
function mouse_up(button)
  local b = tonumber(button)
  if b == nil then return end
  mouse_up_native(math.floor(b))
end
function send_unicode_utf16(code)
  if code == nil then return end
  local n = code
  if type(n) ~= "number" then n = tonumber(code) end
  if n == nil then return end
  send_unicode_utf16_native(math.floor(n))
end
function send_text(str)
  if str == nil then return end
  if type(str) ~= "string" then str = tostring(str) end
  send_text_native(str)
end
do
  if input then
    input.send_text = function(str)
      send_text(str)
    end
  end
end
function send_key_tap(vk)
  if vk == nil then return end
  local code = vk
  if type(code) ~= "number" then code = tonumber(code) end
  if code == nil then return end
  send_key_tap_native(math.floor(code))
end
function send_key_down(vk)
  if vk == nil then return end
  local code = vk
  if type(code) ~= "number" then code = tonumber(code) end
  if code == nil then return end
  send_key_down_native(math.floor(code))
end
function send_key_up(vk)
  if vk == nil then return end
  local code = vk
  if type(code) ~= "number" then code = tonumber(code) end
  if code == nil then return end
  send_key_up_native(math.floor(code))
end
do
  if input then
    input.send_key_down = function(vk) send_key_down(vk) end
    input.send_key_up = function(vk) send_key_up(vk) end
    input.mouse_down = function(button) mouse_down(button) end
    input.mouse_up = function(button) mouse_up(button) end
  end
end
function chord_tap(main_vk, ...)
  return chord_tap_native(main_vk, ...)
end
function shift_key_tap(vk)
  if vk == nil then return end
  local code = vk
  if type(code) ~= "number" then code = tonumber(code) end
  if code == nil then return end
  shift_key_tap_native(math.floor(code))
end
function pc_key_pressed(vk)
  if vk == nil then return false end
  local code = vk
  if type(code) ~= "number" then
    code = tonumber(code)
  end
  if code == nil then return false end
  return pc_key_pressed_native(math.floor(code)) == 1
end
function pc_lmb_pressed()
  return input.mouse_lmb_raw() == 1
end
function pc_rmb_pressed()
  return input.mouse_rmb_raw() == 1
end
function pc_mmb_pressed()
  return input.mouse_mmb_raw() == 1
end
function pc_xbutton1_pressed()
  return input.mouse_xbutton1_raw() == 1
end
function pc_xbutton2_pressed()
  return input.mouse_xbutton2_raw() == 1
end
function pc_digit_or_numpad_pressed(n)
  local d = math.floor(tonumber(n) or -1)
  if d < 0 or d > 9 then return false end
  return pc_key_pressed(0x30 + d) or pc_key_pressed(0x60 + d)
end
