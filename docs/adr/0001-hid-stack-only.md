# ADR-0001: Use the stock HID stack only

- Status: Accepted
- Date: 2026-08-30

## Context

Live Windows measurements found the Logitech G13 bound to Microsoft's stock `HidUsb` driver. Its vendor-defined usage page (`0xFF00`) produces no keyboard or mouse events. Shared userspace access succeeded while Logitech Gaming Software was running, including feature reports for RGB and mode LEDs and a 992-byte LCD output report.

## Decision

Bosun uses `hidapi` over the native OS HID facilities: Windows HID API, macOS IOHIDManager, and Linux hidraw. The Rust dependency disables default features and explicitly selects hidraw on Linux.

Bosun will not require or support libusb, WinUSB/Zadig, a kext, or a kernel module. Device selection matches VID, PID, and usage page, then opens the enumerated path.

## Consequences

One userspace transport model serves all supported platforms. The G13 creates no stray system input and requires no exclusive-grab mechanism. Linux needs a udev permission rule; macOS needs bundle-scoped Input Monitoring. The first M1 test must still verify that a second reader receives input reports while LGS owns another shared handle.

## Provenance

Protocol facts come from direct hardware measurements recorded in `docs/BOSUN-PLAN.md`. GPL G13 source is not an implementation reference and must not be read, copied, translated, or adapted.
