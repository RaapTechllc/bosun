# Bosun

Userspace HID ownership of a descriptor-driven command pad, starting with the Logitech G13. M1 is foundational transport, descriptor, codec, and CLI — not agent UX.

## Language

**Descriptor**:
A TOML document that names a device's match criteria, keys, axes, screen, RGB, and LEDs. The G13 is data in `devices/logitech-g13.toml`, not compiled crate policy.
_Avoid_: profile, driver, device definition

**Match**:
The VID, PID, and usage page that identify one HID interface. Selection then opens the enumerated path.
_Avoid_: VID/PID-only identity, product name lookup

**Codec**:
The mapping from an 8-byte input report onto named key and axis events, and the packing of LCD, RGB, and M-LED output reports.
_Avoid_: parser, translator, driver logic

**Edge event**:
A key down, key up, or axis change produced by diffing the current report against the previous one.
_Avoid_: poll state, held-key stream, raw bitmap

**Ignored bit**:
A bit in the 40-bit key field that is device state, not a user control. Toggling it emits no key event.
_Avoid_: mask (the incomplete Appendix B mask), filter byte

**Transport**:
A synchronous, policy-free HID channel that moves reports. `MockTransport` scripts those reports for tests with no hardware.
_Avoid_: async HID, exclusive open, libusb backend
