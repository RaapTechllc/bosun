# Hardware Notes

## AC-R1 outcome (2026-09-20 WINDOWS-CU-20260920-01)

- **fail both**
- Path: `\\?\HID#VID_046D&PID_C21C#7&1562afa0&0&0000#{4d1e55b2-f16f-11cf-88cb-001111000030}` (usage_page=ff00)
- Run 1: LGS/LCore running (pid 8668) — 0 input reports in 15s (Kyle mash requested)
- Run 2: LCore stopped — 0 input reports in 15s (Kyle mash requested)
- bosun tip: `706ecd8` main
- Non-HW suite: 38+1 pass same session

## Still pending (not AC-R1)

- Physical mapping for bitmap names LEFT, DOWN, and TOP: not yet verified.
- Joystick center, cardinal extremes, corners, and diagonal envelope: not yet recorded.

## Next session: R1 re-measure runbook (added 2026-11-01, not yet run)

The 2026-09-20 runs counted interrupt reports only. Zero reports cannot tell a starved second reader from a window in which no key was pressed ("mash requested" is the only record of the keypresses). `bosunctl device probe` adds a positive control from the same shared handle. While it reads the interrupt stream, it polls the same input report over HID `GET_REPORT` (`HidD_GetInputReport`, `HIDIOCGINPUT`, `IOHIDDeviceGetReport`), which does not depend on the interrupt ring buffer. It is read-only: it never sends an output report or any feature report (so never report 6).

Run from the repository root (the default descriptor path is relative) after the AC-13 check (`rustc -vV` reports `x86_64-pc-windows-msvc`; use `rustup run stable cargo ...` if Chocolatey's shim wins PATH):

1. `cargo run -p bosunctl -- device list --vid 0x046D --pid 0xC21C` lists every collection of the device, not only `0xFF00`.
2. With LGS running, run `cargo run -p bosunctl -- device probe --seconds 20`. During the window, press several G-keys, L1–L4 and the thumb controls, and sweep the stick. Keep the whole output.
3. Unless the verdict is `interrupt-live`, quit LGS, confirm `LCore.exe` is gone, and repeat step 2.
4. Record each run under a new dated heading with its LGS state, the `verdict:` line, and the `summary` block.

| Probe verdict | What it shows | AC-R1 reading for that LGS state |
|---|---|---|
| `interrupt-live` | Decodable input reports reach this reader | Pass |
| `interrupt-undecoded` | Reports arrive in a shape other than 8 bytes, ID `0x01` | Not a pass yet; compare the raw lines with `docs/BOSUN-PLAN.md` §2.4 |
| `polled-only` | `GET_REPORT` returns live key/stick state; the interrupt stream is silent | Fail, with positive evidence that the keys work. Reading input by polling would be a plan change for Kyle |
| `polled-static` | `GET_REPORT` answers but never changed | Inconclusive; rerun while pressing keys |
| `read-failed` | The read itself failed before any input was seen (on Windows, usually a handle opened without read access) | Not a result; fix the error and rerun |
| `silent` | Nothing on either channel | Fail (no positive control on this device) |

The two runs map onto the AC-R1 record. **pass with LGS running** means the first run is `interrupt-live`. **pass only with LGS closed** means only the second is. **fail both** means neither is; attach both summaries. Rerun any `polled-static` or `read-failed` run before recording. If the probe stalls inside the window, the device is blocking on `GET_REPORT`. Stop it and rerun with `--no-poll`, which still prints the inventory, the report descriptor, and the feature readback.

Once input reports arrive:

- **AC-9:** run `device watch`, then unplug and replug. Expect one `disconnected: …` line, `key … up` for any held key, at most one `waiting: …` line per distinct reason, exactly one `reconnect` line; the first report after it prints the stick position again (`axis …`). The reopen interval is 250 ms, so replug to `reconnect` should be well under 2 s.
- **K2 / AC-10:** while `device watch` runs, press each thumb control and note which of `LEFT`, `DOWN`, `TOP` it prints; update "Still pending" above. Capture fixtures with `device record --output <name>.hex`. A capture survives an unplug and keeps any unexpected report shape as a `#` comment, so the file stays a valid fixture.
- **AC-8:** `device rgb 255 0 0` now ends with `(matches)` or `(differs)`. The readback buffer covers Windows' `FeatureReportByteLength` (258 bytes for this device) instead of 5 bytes.
