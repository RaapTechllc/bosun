# Bosun

Bosun turns the Logitech G13 into a cross-platform command console for AI coding agents.

## Status

Phase 0 is complete on live Windows hardware. M1 software paths are mockable: a synchronous HID transport, a G13 TOML descriptor and codec, `MockTransport`, and `bosunctl device list|info|watch|record|rgb|leds|lcd test`.

AC-R1 measured **fail both** (0 reports with LGS running and with LCore stopped) on 2026-09-20; details in `docs/hardware-notes.md`. Per `docs/PRD.md` AC-R1 / R1, **fail both stops M1** for Kyle's review — no HID redesign hop from this potato.

## Non-negotiable boundaries

- Stock OS HID stack only: no libusb, WinUSB/Zadig, kexts, or kernel modules.
- Dual-licensed under MIT OR Apache-2.0; GPL-derived source is not accepted.
- Do not consult or copy GPL G13 driver source. The measured protocol is documented in `docs/BOSUN-PLAN.md`.
- Logitech Gaming Software may overwrite LCD/RGB output.

See `AGENTS.md`, `docs/PLAN-REVIEW.md`, and `docs/BOSUN-PLAN.md` before implementing. The product requirements, M1 acceptance criteria, and open owner decisions are in `docs/PRD.md`.

## CLI

Descriptor path defaults to `devices/logitech-g13.toml`.

```text
bosunctl device list --vid 0x046D --pid 0xC21C --usage-page 0xFF00
bosunctl device info
bosunctl device watch
bosunctl device record --output capture.hex
bosunctl device rgb 255 0 0
bosunctl device leds 0x0F
bosunctl device lcd test
```
