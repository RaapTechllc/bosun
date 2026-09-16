# 03: info, rgb, leds, and lcd test over mock

**What to build:** `bosunctl device info` prints the loaded descriptor. `rgb`, `leds`, and `lcd test` send the measured reports over a transport. Mock tests assert exact bytes. No command writes feature report 6.

**Blocked by:** 01, 02

**Status:** ready-for-agent

- [ ] `info` prints id, match, 34 keys, axes, screen, rgb, and leds (AC-7 software path)
- [ ] `rgb R G B` sends feature report 7 and reads it back (AC-8)
- [ ] `leds MASK` sends feature report 5 (AC-8)
- [ ] `lcd test` sends one 992-byte report 3: byte 0 = `0x03`, bytes 1–31 zero, rows 43–47 zero (AC-8)
- [ ] Across the CLI surface, no feature write has report ID 6
