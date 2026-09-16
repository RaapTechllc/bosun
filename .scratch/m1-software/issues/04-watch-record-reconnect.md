# 04: watch, record, and mock reconnect

**What to build:** `watch` prints decoded named key and axis events over `MockTransport`. `record` writes raw 8-byte reports that decode back to the same events. After a scripted disconnect, watch prints one reconnect line and resumes within 2 s of mock time.

**Blocked by:** 02, 03

**Status:** ready-for-agent

- [ ] `watch` over mock prints the expected named events (AC-7)
- [ ] `record` output round-trips through the decoder (AC-7)
- [ ] Scripted disconnect then reports: one reconnect line, events resume, elapsed ≤ 2 s (AC-9)
- [ ] Existing `device list` filters still pass (AC-2)
