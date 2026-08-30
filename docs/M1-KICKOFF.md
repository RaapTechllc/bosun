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

1. Use strict RED-GREEN-REFACTOR for each behavior.
2. Implement the smallest probe/watch path needed to test R1.
3. With LGS running, physically press keys and confirm whether shared input reports arrive. If not, close LGS and repeat before changing the architecture.
4. Capture raw 8-byte reports as fixtures for every control and the joystick envelope.
5. Resolve physical LEFT/DOWN/TOP names and update `docs/hardware-notes.md`.
6. Complete mock-backed tests, hot-plug recovery, and the M1 CLI.
7. Run format, clippy with warnings denied, all tests, and hardware-gated tests.

## Prohibited

Do not add Tokio, adapters, engine code, profiles, injection, HTTP/WS, Tauri, embedded-graphics, or LCD widgets. Do not consult GPL G13 repositories or copy GPL-derived code.

## Done condition

M1 is done only when the documented acceptance sequence passes, raw hardware fixtures are committed, reconnect occurs within two seconds, and CI is green on Windows, macOS, and Linux.
