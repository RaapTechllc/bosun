# 02: Decode edges from golden reports

**What to build:** An 8-byte report ID `0x01` decodes to named key downs/ups and 0–255 axis values. Bits 22, 23, and 36–39 emit no key event. Committed goldens cover every named control plus stick center, cardinals, corners, and diagonals. Physical LEFT/DOWN/TOP mapping stays pending.

**Blocked by:** 01

**Status:** ready-for-agent

- [ ] Bits `0..=21` and `24..=35` map to the 34 descriptor names; pressed = 1 (AC-6)
- [ ] Events are edges against the previous report
- [ ] Flipping each ignored bit yields an empty event stream
- [ ] Fixture files exist and drive the decode tests (AC-10)
- [ ] `docs/hardware-notes.md` still says LEFT/DOWN/TOP are pending
