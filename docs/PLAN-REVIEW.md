# Bosun Plan Review

**Verdict:** build from the plan without rewriting it. Decisions D1–D11 remain locked. M1 is the only implementation scope for the initial Cursor work.

## Owner decisions — 2026-08-30

- Repository: public from the start.
- RaapTech fleet (`clawdbot`) adapter: remains in v1, but is outside M1.
- Second device: buy a small VIA/QMK pad with an encoder before M3.
- Local repository: `C:\Users\Kyle\CC\bosun`.

## Corrections and additions

1. Appendix A is illustrative, not a bootstrap script. This repository is a two-member workspace for M1; do not create a single root crate or add Tokio to `bosun-hid`.
2. `hidapi` defaults can select libusb on Linux. Pin it with default features disabled and `linux-static-hidraw`.
3. Appendix B's hexadecimal ignore mask is incomplete. Decode only button bits 0–21 and 24–35; ignore 22, 23, and 36–39. Add a test proving state-flag transitions emit no key events.
4. `bosun-hid` is synchronous. The daemon can bridge blocking I/O to async infrastructure in M4.
5. Do not scaffold the complete §5.2 tree. M1 uses only `bosun-hid`, `bosunctl`, and device data.
6. R1 is the first live M1 test: press keys while LGS is running, then repeat with LGS closed if necessary.
7. Add `bosunctl device record` to M1 so raw report fixtures and the joystick travel envelope can be captured while a human is at the hardware.
8. Open devices by enumerated path after matching VID/PID and usage page.
9. The real TOML descriptor must use literal key arrays; range syntax in §14 is pseudocode.
10. M4 must authenticate the loopback control plane with a random per-install token, reject untrusted Host/Origin values, and avoid permissive CORS.
11. Adapter stdout is reserved for NDJSON JSON-RPC; logs go to stderr.
12. Engine FSMs accept timestamped events and never read the clock directly.
13. CI LCD golden rendering must be headless; use the simulator only for local interactive development.
14. The LCD draw surface is 160×43 even though packing storage is 160×48; clear hidden rows on every report.
15. macOS packaging must explain both Input Monitoring and Accessibility grants.
16. The workstation has stale Chocolatey GNU Rust shims ahead of rustup on some shells. Cursor must use the rustup stable MSVC toolchain; verify `rustc -vV` reports `x86_64-pc-windows-msvc` before hardware work.

## M1 acceptance sequence

1. Prove shared key input with LGS running and capture raw 8-byte reports.
2. Build `Transport`, `HidTransport`, and scripted `MockTransport` test-first.
3. Load the G13 descriptor and decode named key/axis events with corrected filtering.
4. Implement `bosunctl device list|info|watch|record|rgb|leds|lcd test`.
5. Characterize center, cardinal extremes, corners, and diagonals; resolve physical LEFT/DOWN/TOP names in `docs/hardware-notes.md`.
6. Verify reconnect within two seconds, then pass fmt, clippy, tests, and hardware-gated checks.
