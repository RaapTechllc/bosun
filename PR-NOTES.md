# PR notes: `agent/claude/autonomy-trial-2` (2026-11-01)

Brief: `ORIENTATION.md`. Build Bosun forward from `docs/PRD.md` and the README, choose the next highest-value unbuilt slice(s), avoid overlap with open PR #8 (doc trim), and stay within `AGENTS.md` (M1 only).

## Where the project stood

- M1's software paths were already built and mock-tested: transport, mock, descriptor, codec, and `device list|info|watch|record|rgb|leds|lcd test`.
- M1 is blocked on hardware. AC-R1 measured **fail both** on 2026-09-20 (`docs/hardware-notes.md`): zero interrupt reports with LGS running and with LCore stopped. Only Kyle can run hardware sessions.
- So the remaining value is not new M1 surface. It is (a) making sure the hardware ACs cannot fail for software reasons, and (b) making the next hardware session decisive. Engine, LCD widgets, adapters, profiles and the like are M2+ and forbidden by `AGENTS.md`.

## What I built, in priority order

1. **AC-9 bug fix: `watch` and `record` survive a real unplug.**
   - **Bug.** Reconnect only triggered on `HidError::Disconnected`. hidapi never produces it: a pulled cable is an opaque backend error on every OS (Linux `poll` POLLHUP/POLLERR returns -1; Windows `ReadFile` fails). So on hardware, `watch` exited with an error on unplug. The mock test passed against behaviour no real backend shows.
   - **Fix.** Any read error now means the handle is dead. Watch prints one `disconnected:` line and releases held keys (`Decoder::release_all`, so nothing stays stuck). It then waits 250 ms before every reopen attempt and prints each distinct failure once as `waiting:`. Exactly one `reconnect` line is printed, at the first good read on the new handle.
   - The old retry was also a hot spin that re-enumerated every HID device. `MockTransport::push_error` now scripts the real failure mode.
   - Both loops release the dead handle before waiting for the replug. A handle to a surprise-removed device can keep the OS from tearing the old node down, most visibly on Windows.
   - **A device that opens but cannot be read is an error, not an endless loop.** On Windows, hidapi falls back to a no-access handle when it cannot open read/write, so the open succeeds and every read fails at once. A handle that fails before its first read prints `read failed:`. Three in a row end the command with the error and a hint (another program holds the device exclusively, or the OS reserves the collection). The reviewer found this regression; see Verification.
   - `record` gets the same recovery. Instead of aborting the capture, it keeps unexpected report shapes and the gap as `#` comments, which the fixture parser already skips.
   - `watch` now prints undecodable reports instead of silently dropping them.
2. **`bosunctl device probe`: a read-only R1 instrument.**
   - **Gap.** The 2026-09-20 run had one signal: zero interrupt reports, with "Kyle mash requested" as the only record of keypresses. It cannot tell a starved second reader from a window in which nothing was pressed.
   - **What the probe prints:**
     - every collection sharing the descriptor's VID/PID;
     - the OS's report descriptor for the interface;
     - a feature readback, proving the control pipe answers;
     - a window of interrupt reads interleaved with HID `GET_REPORT(Input)` polls of the same report.
   - The polls are the positive control: stock HID (`HidD_GetInputReport` / `HIDIOCGINPUT` / `IOHIDDeviceGetReport`) and independent of the interrupt ring buffer.
   - It ends with one verdict: `interrupt-live`, `interrupt-undecoded`, `polled-only`, `polled-static`, `read-failed`, or `silent`. Each comes with its meaning and next step. `read-failed` means the read itself failed before any input was seen. It is not an AC-R1 result.
   - It never writes an output or feature report, so never report 6.
   - **Supporting changes.** `Transport` gains `get_input_report` and `get_report_descriptor`.
3. **AC-8 hardening: `rgb` readback buffer.**
   - **Risk.** The readback used a 5-byte buffer. hidapi pads feature *sends* to Windows' `FeatureReportByteLength` (258 bytes on this device) but passes *get* buffers through unchanged. A 5-byte get is therefore likely to be refused on Kyle's workstation, after the backlight has already changed.
   - **Fix.** Reads now use a 1024-byte buffer (hidapi documents that longer is fine). The line ends `(matches)` or `(differs)`.
4. **Docs.**
   - `docs/hardware-notes.md` gains a next-session runbook: probe with LGS running and then closed, verdict to AC-R1 mapping, then the AC-9, K2/AC-10 and AC-8 checks. It is marked "not yet run" and claims no result.
   - `CONTEXT.md` defines "polled report".

## Considered and not done

- **Polling as the input path** if the probe says `polled-only`: a material plan change, so it is a decision for Kyle (`AGENTS.md`); see open decisions below.
- **Guided fixture capture** (prompt per control, write `G1.hex`, ...): useless until R1 passes. `watch` and `record` already cover K2/AC-10 once input arrives.
- **Descriptor-driven output report IDs.** `rgb_feature_report` and the LED/LCD builders use codec constants, while the descriptor's `report_id` fields go unread. This is D11 hygiene for device #2 (M3), with no M1 acceptance impact. Follow-up.
- **`bosunctl doctor` / LGS process detection**: M8 per the PRD.
- **README CLI list**: should gain `device probe`. README.md is edited by PR #8, so I left it alone; add the line after #8 merges.

## Verification

- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: clean. `--all-targets` type-checks every test.
- **Tests were written first but not executed locally.** Every form of `cargo test` (and `cargo build`, `cargo tree`, `cargo deny`) needs approval in this session and was refused, so "RED" was observed only as compile failures. Instead, I traced every new test by hand against the implementation and the mock. The first CI run is the real execution.
- **Independent review.** A read-only reviewer agent, briefed without my conclusions, acted as the test runner. It traced all 42 new or modified tests through commit `503cf1f` (plus the pre-existing tests that touch changed code) and found no failing assertion and no possible hang. It also found two real bugs, both fixed:
  - the busy reconnect loop on a device that opens but cannot be read (`c3d9ada`);
  - the probe calling a failed read `silent` (`e4d1063`).

  I traced the tests added or changed by those fixes myself; the reviewer did not re-review them.
- **AC-3 / AC-12.** No manifest, lockfile, `deny.toml` or CI change on this branch, so the dependency gates are unaffected. The AC-3 grep still matches only test data.
- **Building hidapi on this box.** The box has the `libudev1` runtime but no `libudev-dev` or `pkg-config`, and installing system packages was out of bounds. For clippy only, I passed per-invocation config: `cargo clippy --config 'env.PKG_CONFIG="/usr/bin/echo"' --config 'env.CFLAGS_x86_64_unknown_linux_gnu="-I<worktree>/target/localdeps/sysroot/include"' ...`. The include directory holds a prototype-only `libudev.h` I wrote, in the git-ignored `target/`. Nothing is committed and nothing persistent was configured. CI installs the real packages and is unaffected.

## Assumptions

- Adding `Transport` methods is not an HID redesign. Both are stock OS HID calls (ADR-0001), and the only implementors are `HidTransport` and `MockTransport`.
- A diagnostic subcommand is M1 scope: it is the S8/AC-R1 instrument. Nothing in it names the G13; IDs come from the descriptor and the codec.
- Any read error is treated as a dead handle. A persistent fault on a handle that never reads ends the command after three tries. A persistent fault on a handle that worked first is retried every 250 ms, and the `waiting:`/`disconnected:` lines say why.
- An explicit `--path` reopens the same path. If Linux renumbers the hidraw node on replug, matching by descriptor recovers and an explicit path may not.
- The Windows get-feature length requirement is inferred from the measured `SetFeature` behaviour (`docs/BOSUN-PLAN.md` §2.4) and the hidapi source, not observed. The larger buffer is correct either way.
- On Windows, a `GET_REPORT` IOCTL issued while an overlapped interrupt read is pending can rarely fail spuriously. Polling stops only after three failures in a row, and interrupt reports are counted regardless.
- `ORIENTATION.md`'s limits override the standing operating model where they conflict. Per the run instructions: commit only, no push, no merge.

## Open decisions for Kyle

1. **Run the runbook** in `docs/hardware-notes.md`: two probe runs (LGS on, then off). This is the M1 unblock.
2. **If the verdict is `polled-only`:** the keys work and the device reports them, but shared interrupt reads do not arrive. Decide whether Bosun may read input by `GET_REPORT` polling (stock HID, roughly 20–100 Hz). That is a plan change, so I did not build it.
3. **AC-12 note.** hidapi-rs 2.6.7's `build.rs` still carries a 2015 GPL-3 header, while the crate is declared and licensed MIT. The bundled C hidapi is tri-licensed, and Bosun can use the BSD or original terms. No action needed; flagging it so the M1 license attestation is not surprised.

## Commits

See `git log 45dcb32..HEAD`. Each commit message lists its own assumptions.
