# Plan — M1 remaining SOFTWARE ACs (mockable)

Scope is locked to mockable paths. This plan does **not** unpark AC-R1, engine, adapters, Tauri, or live finger-on-key work.

## Files / modules

### Add

- `bosun-hid` descriptor loader (generic TOML types + validation)
- `bosun-hid` codec (input edge events, LCD/RGB/LED report builders)
- `crates/bosun-hid/tests/fixtures/g13/` golden 8-byte reports
- `bosunctl` library surface so commands run over `impl Transport`
- CLI subcommands: `info`, `watch`, `record`, `rgb`, `leds`, `lcd test`
- CONTRIBUTING + CI notes for Windows MSVC (AC-13)

### Do not rewrite

- `Transport`, `HidTransport`, `MockTransport`
- device match/select
- `g13_probe`
- `bosunctl device list` filters
- `crates/bosun-hid/tests/hardware.rs` (keep `#[ignore]` + `BOSUN_HW=1`)

## Interfaces

- `load_descriptor(path) -> Result<DeviceDescriptor>`
- `DeviceDescriptor` exposes match, literal keys, axes, screen, rgb, leds, ignored key bits
- `Decoder::from_descriptor` + `decode(&[u8]) -> Result<Vec<InputEvent>>`
- `rgb_feature_report` / `leds_feature_report` / `lcd_test_report`
- CLI commands accept an opener `FnMut() -> Result<impl Transport>` so tests inject `MockTransport`
- `watch` reprints after `Disconnected` by re-opening; one `reconnect` line; no 2 s sleep in mock

## Test list

- AC-5: valid G13 file; missing field; wrong type; range syntax rejected; no panic
- AC-6: golden replay of 34 keys + stick envelope; ignored bits 22, 23, 36–39 emit no key events
- AC-7: `watch` prints named events over mock; `record` file round-trips through decoder
- AC-8: mock exact bytes for RGB (report 7), LEDs (report 5), LCD (992-byte report 3); no report-ID-6 write
- AC-9: scripted disconnect then reports → one reconnect line, events resume, elapsed ≤ 2 s
- AC-10: fixture files present and drive AC-6; hardware-notes LEFT/DOWN/TOP stay pending
- AC-11/12/13: fmt, clippy `-D warnings`, workspace tests, cargo-deny; ignored HW tests skip without `BOSUN_HW=1`; Windows MSVC documented/CI-asserted

## Protocol source

`docs/BOSUN-PLAN.md` §2.4 and `AGENTS.md` Measured G13 protocol only.

## Stop condition

If a ticket would require live hardware, physical LEFT/DOWN/TOP mapping, or any PARKED crate — stop.
