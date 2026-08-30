# Cursor Kickoff — M1 Only

Implement M1 only, following `AGENTS.md`, `docs/BOSUN-PLAN.md`, `docs/PLAN-REVIEW.md`, and ADR-0001.

## Mission

Implement:

- synchronous `bosun-hid::Transport`;
- `HidTransport` and scripted `MockTransport`;
- G13 TOML descriptor loading;
- input decoding with the corrected button-bit filtering;
- `bosunctl device list|info|watch|record|rgb|leds|lcd test`.

## Required order

1. Verify `rustc -vV` reports `x86_64-pc-windows-msvc`; if Chocolatey's GNU shim wins PATH, run Cargo through rustup stable.
2. Use strict RED-GREEN-REFACTOR for each behavior.
3. Implement the smallest probe/watch path needed to test R1.
4. With LGS running, physically press keys and confirm whether shared input reports arrive. If not, close LGS and repeat before changing the architecture.
5. Capture raw 8-byte reports as fixtures for every control and the joystick envelope.
6. Resolve physical LEFT/DOWN/TOP names and update `docs/hardware-notes.md`.
7. Complete mock-backed tests, hot-plug recovery, and the M1 CLI.
8. Run format, clippy with warnings denied, all tests, and hardware-gated tests.

## Prohibited

Do not add Tokio, adapters, engine code, profiles, injection, HTTP/WS, Tauri, embedded-graphics, or LCD widgets. Do not consult GPL G13 repositories or copy GPL-derived code.

## Done condition

M1 is done only when the documented acceptance sequence passes, raw hardware fixtures are committed, reconnect occurs within two seconds, and CI is green on Windows, macOS, and Linux.
