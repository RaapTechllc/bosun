# Bosun Agent Contract

## Current scope

Implement **M1 only**. Do not add the engine, adapters, Tauri GUI, HTTP/WS control plane, injection, profiles, or LCD widgets. Empty future crates are forbidden.

## Locked constraints

- Bosun is dual-licensed `MIT OR Apache-2.0`; never introduce GPL code or dependencies.
- HID stack only. No libusb, WinUSB, Zadig, kext, or kernel module.
- Use `hidapi` with default features disabled and Linux `hidraw`; `bosun-hid` is synchronous and contains zero policy. Do not add Tokio to it.
- Nothing above `bosun-hid` knows the G13 exists. A device is a TOML descriptor plus a codec where required.
- Profiles are declarative data. Future `shell.run` requires explicit consent.
- Do not consult or paste from `khampf/g13`, `cavefish-dev/g13-driver`, or other GPL G13 source. Use only measured protocol facts in `docs/BOSUN-PLAN.md`.
- Implement behavior test-first. Run fmt, clippy with warnings denied, and all tests before commit.

## Measured G13 protocol

- VID `0x046D`, PID `0xC21C`, usage page `0xFF00`.
- Input: 8 bytes, report ID `0x01`, joystick at bytes 1–2, key bitmap at bytes 3–7.
- Keep only key bits `0..=21` and `24..=35`; ignore bits 22, 23, and 36–39. Never copy the incomplete mask from Appendix B.
- LCD: 992-byte output report ID `0x03`, 31 padding bytes, then 960 framebuffer bytes.
- LCD packing: `offset = col + (row >> 3) * 160`; `bit = 1 << (row & 7)`. Visible rows are 0–42; rows 43–47 must remain zero.
- RGB is feature report 7. M1/M2/M3/MR LEDs are feature report 5. Leave feature report 6 alone.
- Match and open the enumerated HID path by VID/PID and usage page, not VID/PID alone.
- First hardware action: run shared reads while LGS is active and physically press keys. If empty, quit LGS and retry before redesigning anything.

## Later invariants to preserve

- M4 loopback HTTP/WS requires a random per-install bearer token, Host and Origin validation, and no permissive CORS.
- Adapter stdout is JSON-RPC only; logs go to stderr; malformed stdout is a health violation.
- Engine events carry injected monotonic timestamps; engine code must not read the clock.
- CI LCD goldens use a headless image target; the SDL simulator is local-development-only.
