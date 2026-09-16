# Spec: M1 remaining SOFTWARE acceptance criteria

**Status:** ready-for-agent
**Parent:** Heavy Lift brief — Bosun M1 remaining SOFTWARE ACs (mockable paths)
**Authoritative ACs:** `docs/PRD.md` §5, software paths only

## Problem Statement

M1 already has a synchronous HID transport, a scripted mock, device match, and `bosunctl device list`. What is still missing is everything that turns those reports into named device behavior on CI: loading the G13 descriptor, decoding kept key bits into edge events, driving `info|watch|record|rgb|leds|lcd test` over `MockTransport`, recovering from a scripted disconnect, and committing golden fixtures — without touching live hardware or PARKED milestones.

## Solution

A generic descriptor loader and codec live in `bosun-hid` as data-driven types. `bosunctl` becomes a thin library so each device command can be exercised with `MockTransport`. Golden 8-byte reports, derived from the measured bitmap in `docs/BOSUN-PLAN.md` §2.4, drive decode tests. Physical LEFT/DOWN/TOP mapping and AC-R1 stay pending.

## User Stories

1. As a CI runner, I want descriptor load tests to pass without a G13 attached, so that AC-5 is checkable on every OS.
2. As a CI runner, I want a malformed descriptor to return `Err` rather than panic, so that bad device data cannot crash the process.
3. As a CI runner, I want range-syntax key lists rejected, so that only literal key arrays ship.
4. As an operator, I want the committed G13 descriptor to name 34 keys plus match, axes, screen, rgb, and leds, so that the device is data rather than compiled policy.
5. As a decoder caller, I want bits 0..=21 and 24..=35 mapped to those 34 names, so that a pressed bit becomes a named key.
6. As a decoder caller, I want down/up events from a diff against the previous report, so that watchers see edges rather than raw bitmaps.
7. As a decoder caller, I want toggling bits 22, 23, 36, 37, 38, and 39 to emit no key event, so that device-state flags stay silent.
8. As a decoder caller, I want stick bytes 1 and 2 as 0–255 axes, so that joystick motion is named and numeric.
9. As a CI runner, I want golden fixtures to replay through the decoder, so that AC-6 does not depend on a finger on a key.
10. As an operator, I want `bosunctl device info` to print the loaded descriptor, so that I can confirm match and capabilities before opening a handle.
11. As an operator, I want `bosunctl device watch` to print named key and axis events, so that I can see decoded input.
12. As a test, I want watch to run over `MockTransport`, so that AC-7 is mockable.
13. As an operator, I want `bosunctl device record` to write raw 8-byte reports to a file, so that later hardware captures have a stable format.
14. As a test, I want a recorded file to decode back to the same events, so that record round-trips.
15. As an operator, I want `bosunctl device rgb R G B` to send feature report 7 and read it back, so that backlight writes are exercisable.
16. As an operator, I want `bosunctl device leds MASK` to send feature report 5, so that M-key LEDs are exercisable.
17. As an operator, I want `bosunctl device lcd test` to send one 992-byte report 3 with the measured header and hidden rows zero, so that the LCD path is proven without widgets.
18. As a reviewer, I want no CLI command to write feature report 6, so that the LGS identity channel stays untouched.
19. As a test, I want mock writes compared to exact expected bytes, so that AC-8 does not need a glowing keypad.
20. As an operator, I want watch to print one reconnect line after a disconnect and then resume events, so that hot-plug recovery is visible.
21. As a test, I want that recovery to finish within 2 s on a scripted mock without sleeping, so that AC-9 is mockable.
22. As a reviewer, I want one raw fixture per named control plus stick center, cardinals, corners, and diagonals, so that AC-10 has committed goldens.
23. As Kyle, I want `docs/hardware-notes.md` to keep LEFT/DOWN/TOP as pending until I measure them, so that agents do not invent physical mapping.
24. As a CI runner, I want fmt, clippy `-D warnings`, workspace tests, and cargo-deny green without `BOSUN_HW=1`, so that AC-11 and AC-12 hold.
25. As a Windows operator, I want the MSVC host triple documented and asserted on `windows-latest`, so that AC-13 is a software path.
26. As a contributor, I want hardware tests to stay ignored and to skip unless `BOSUN_HW=1`, so that `--ignored` on a bare machine stays honest.
27. As a license reviewer, I want no GPL identifiers or dependencies, so that AC-12 holds.
28. As a maintainer, I want existing device-list and transport tests unchanged in behavior, so that AC-2, AC-3, and AC-4 do not regress.

## Implementation Decisions

- Descriptor and codec live in `bosun-hid`. Types are generic. G13 VID/PID literals stay in descriptor data, fixtures, and tests — not crate policy.
- The G13 TOML gains an `ignored_key_bits` list so the ignore set is data. Keys remain a literal array in listed order; the codec maps kept bits in ascending index onto that array.
- `keys.len() + ignored_key_bits.len()` must equal 40 (bits 0..=39). Duplicate names, empty names, and any key token containing `..` are rejected.
- `InputEvent` is `KeyDown`, `KeyUp`, or `Axis { name, value }`. The first report diffs against "no previous keys / no previous axes": pressed keys emit down; current axis values emit.
- LCD, RGB, and LED builders live next to the codec. RGB is `[0x07, r, g, b, 0x00]`. LEDs are `[0x05, mask, 0, 0, 0]`. LCD test is 992 bytes, byte 0 = `0x03`, bytes 1..=31 zero, a fixed visible-row pattern, rows 43..=47 zero. Packing uses `offset = col + (row >> 3) * 160` and `bit = 1 << (row & 7)`.
- `bosunctl` exposes a library API. Commands take a writer and, where I/O is needed, an opener that returns a `Transport`. Production `main` opens `HidTransport` from the descriptor match (or an explicit path). Tests inject `MockTransport`.
- `watch` treats `Disconnected` as "drop the handle and open again". It prints exactly one `reconnect` line per successful reopen after a drop. Reopen is synchronous; tests assert elapsed time ≤ 2 s with no sleep.
- `record` writes one report per line as eight space-separated lowercase hex bytes. That file is the round-trip fixture format.
- `info` prints the descriptor (id, match, keys, axes, screen, rgb, leds) without requiring a live handle.
- `--descriptor` defaults to `devices/logitech-g13.toml`.
- Feature report 6 has no builder and no CLI path.
- Golden fixtures are synthetic protocol reports under `crates/bosun-hid/tests/fixtures/g13/`. They do not claim physical thumb-button identity or a measured stick envelope.
- Hardware tests stay `#[ignore]` and keep the `BOSUN_HW=1` gate. CI on Windows asserts `x86_64-pc-windows-msvc`. CONTRIBUTING documents the rustup-vs-Chocolatey PATH rule.
- No new ADR: D11 and ADR-0001 already lock descriptor-driven HID. No Tokio. No engine, adapters, Tauri, or empty crates.

## Testing Decisions

- Test behavior at public seams: `load_descriptor` / `parse_descriptor`, `Decoder::decode`, CLI command functions, and `MockTransport` write logs.
- Expected bytes and event names come from `docs/BOSUN-PLAN.md` §2.4 and committed fixture files — not from re-encoding the implementation.
- Prefer existing test style: table-driven unit tests next to the module, CLI parse tests like the current `device list` tests.
- One dedicated test flips each ignored bit after a rest report and expects an empty event stream.
- One CLI-surface test inspects every command's mock feature writes and asserts none begin with `0x06`.
- Do not add a live-hardware pass/fail to `docs/hardware-notes.md`.

## Out of Scope

- AC-R1 live shared-input while LGS runs
- Physical LEFT/DOWN/TOP mapping and measured joystick envelope
- Engine, adapters, JSON-RPC, HTTP/WS, `bosund`, Tauri, profiles, LCD widgets
- libusb, WinUSB, Zadig, kexts, `EVIOCGRAB`, exclusive opens
- Reading or translating GPL G13 sources
- Merging the PR

## Further Notes

Wayfinder is not required: the hop is one session of mockable software. Existing transport and list command stay asserted, not rewritten.
