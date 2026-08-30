# Bosun

Bosun turns the Logitech G13 into a cross-platform command console for AI coding agents.

## Status

Phase 0 is complete on live Windows hardware. The repository is prepared for **M1 only**: a synchronous HID transport, descriptor-driven G13 codec, mock transport, and `bosunctl device` commands.

## Non-negotiable boundaries

- Stock OS HID stack only: no libusb, WinUSB/Zadig, kexts, or kernel modules.
- Dual-licensed under MIT OR Apache-2.0; GPL-derived source is not accepted.
- Do not consult or copy GPL G13 driver source. The measured protocol is documented in `docs/BOSUN-PLAN.md`.
- Logitech Gaming Software may overwrite LCD/RGB output. The first M1 hardware test must determine whether shared input reads work while LGS is running.

See `AGENTS.md`, `docs/PLAN-REVIEW.md`, and `docs/BOSUN-PLAN.md` before implementing.
