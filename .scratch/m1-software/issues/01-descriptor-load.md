# 01: Load and reject G13 descriptors

**What to build:** The committed G13 TOML loads as a descriptor with 34 literal key names, a match block, axes, screen, rgb, leds, and ignored key bits. Missing fields, wrong types, and range syntax return `Err` and never panic.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] Valid `devices/logitech-g13.toml` loads with 34 keys and the required blocks (AC-5)
- [ ] Missing field, wrong type, and `G1..=G22`-style range syntax each return `Err`
- [ ] G13 VID/PID literals are not compiled into non-test `bosun-hid` policy
