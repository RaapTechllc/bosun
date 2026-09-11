# Bosun PRD

| | |
|---|---|
| Status | Draft for Kyle's review — doc only, no product code |
| Date | 2026-09-11 |
| Author | Spec Desk (Fable 5.1) |
| Owner | Kyle, RaapTech LLC |
| Scope | M1 as defined in `AGENTS.md`; nothing later is committed here |
| Authoritative inputs | `AGENTS.md`, `docs/BOSUN-PLAN.md`, `docs/PLAN-REVIEW.md`, `docs/M1-KICKOFF.md`, `docs/adr/0001-hid-stack-only.md`, `docs/hardware-notes.md`, `docs/MARKET-WATCH.md`, `docs/CODEX-MICRO-COMPLAINTS.md` |

This document does not reopen D1–D11. Where it conflicts with `AGENTS.md`, `AGENTS.md` wins.

---

## 1. Problem

AI coding agents run in the background and hide their state. Approving, interrupting, or refocusing one means finding a window. Running several means finding several.

OpenAI's Codex Micro ($230, Work Louder OEM) proved the fix is physical: glanceable status, a tactile approve/interrupt, an effort dial ([OpenAI docs](https://developers.openai.com/codex/features/codex-micro), [TechCrunch, 2026-07-15](https://techcrunch.com/2026/07/15/amid-hardware-legal-battle-openai-releases-a-230-keyboard-for-codex/)). Hands-on review found the weak point is software: setup split across two apps, one desktop integration, six visible tasks, no text display ([PCMag](https://ca.pcmag.com/ai/17313/i-got-my-hands-on-openais-sold-out-codex-micro-who-is-this-230-vibe-coding-keyboard-even-for); repo brief `docs/CODEX-MICRO-COMPLAINTS.md`).

The Logitech G13 (USB `046D:C21C`) has 34 buttons, an analog stick, a 160×43 LCD, and a global RGB backlight. It is discontinued, unsupported by G HUB, and kept alive on Windows 11 only by Logitech Gaming Software 9.04.x (Spec Desk notes, 2026-09-11). Every existing third-party driver is a gaming macro tool, most are libusb forks that need a driver swap, and the most complete ones are GPL.

Phase 0 (2026-08-30, `desktop-2QF5HUN`) measured that the whole G13 feature set — input, LCD, RGB, M-LEDs — is reachable from userspace through the stock OS HID stack, concurrently with LGS, with no driver change (`docs/BOSUN-PLAN.md` §2). The hardware problem is solved. The software does not exist.

**Bosun is that software.** A permissively licensed, cross-platform, userspace command console for AI coding agents, starting on a $20–40 used G13. Not a gaming macro product (R7).

### Landscape

| Category | Examples | What it proves | Where Bosun differs |
|---|---|---|---|
| Purpose-built agent pad | Codex Micro ([docs](https://developers.openai.com/codex/features/codex-micro)) | Demand for tactile approve/interrupt/effort and glanceable state | Multi-adapter, LCD text, more keys, dual license, Win/macOS/Linux, cheap used hardware |
| Headless agent control surfaces | Cursor CLI + hooks ([overview](https://cursor.com/docs/cli/overview), [hooks](https://cursor.com/docs/hooks.md)); Claude Code hooks + headless mode ([hooks](https://code.claude.com/docs/en/hooks), [headless](https://code.claude.com/docs/en/headless.md)) | Agents already expose programmatic state and control; a device layer can bind to them without vendor cooperation | Bosun's later adapters consume these; M1 does not |
| Software-only fleet dashboards | AgentCtl, Argus, ADHDev (AGPL), ARC, CodePal (Spec Desk notes; names only, not independently linked) | Multi-agent status/approve demand exists without hardware | Bosun adds the hardware surface; the AGPL entry is a license trap, not a source |
| Stream Deck agent plugins | TerminalDeck, elChango, AgentDeck (Spec Desk notes; names only); Elgato official HID API ([docs](https://docs.elgato.com/streamdeck/hid/intro/)) | Class A vendor-HID devices are a valid, documented path | Bosun owns its daemon and descriptor-driven device layer; it is not an Elgato Marketplace plugin |
| Open-firmware pads | QMK Raw HID, usage page `0xFF60` ([docs](https://docs.qmk.fm/features/rawhid)) | New-buy hardware with real encoders exists on a G13-like side channel | This is device #2 (Class C), locked before M3 |

Claims above marked "Spec Desk notes" are discovery leads per `docs/MARKET-WATCH.md`, not verified product facts. Do not promote them without a source.

---

## 2. Buyer

### Primary binding surface: the agents

The keys, LCD, and RGB exist to be bound to coding agents — Cursor, Claude Code, Codex, Kyle's `clawdbot` fleet, and whatever comes next. An agent is the thing that raises an approval, reports `thinking`/`error`, and receives `interrupt`. If a feature does not help a person command an agent fleet, it is out (R7).

### Operator: Kyle

One human at the keypad, approving, rejecting, and refocusing across a fleet. Kyle is also the only person who can run the live-hardware acceptance tests; the G13 is on his desk, LGS is installed, and R1 needs a finger on a key.

### Agent-First intent

Bosun is built by agents (Cursor, Claude Code) under `AGENTS.md`. The contract is therefore written for an agent reader: measured protocol facts, locked decisions, forbidden sources, and a test-first gate. This PRD keeps that discipline — every acceptance criterion below is something an agent can run or a human can tick.

### What M1 is to the buyer

M1 is not agent UX. It is **foundational HID ownership of the G13 under LGS coexistence**: prove the device can be read and written from Bosun while Logitech's software still holds a handle, and ship that as a policy-free Rust transport plus a CLI to exercise it. The gaming profile mentioned in the plan is onboarding bait for the existing G13 community, not the roadmap center (R7).

---

## 3. MVP: M1 scope

M1 is exactly the mission in `docs/M1-KICKOFF.md` and the acceptance sequence in `docs/PLAN-REVIEW.md`.

### Deliver

| # | Deliverable | Constraint |
|---|---|---|
| S1 | `bosun-hid::Transport` — synchronous trait for open/read/write/feature I/O | No Tokio. Zero product knowledge. Only backend is `hidapi` with default features off and `linux-static-hidraw` (ADR-0001) |
| S2 | `HidTransport` — stock OS HID via `hidapi` | Match by VID + PID + usage page, then open the enumerated path. Never open by VID/PID alone |
| S3 | `MockTransport` — scripted reports, timeouts, disconnects | Everything above the transport is testable with no hardware attached |
| S4 | G13 TOML descriptor (`devices/logitech-g13.toml`) + codec | Literal key arrays. Nothing above `bosun-hid` names the G13 (D11) |
| S5 | Input decode | 8-byte report ID `0x01`; stick at bytes 1–2; key bitmap bytes 3–7; decode bits `0..=21` and `24..=35` only; ignore 22, 23, 36–39. Never use Appendix B's mask |
| S6 | `bosunctl device list \| info \| watch \| record \| rgb \| leds \| lcd test` | `record` captures raw 8-byte reports and the joystick envelope with a human present |
| S7 | Hot-plug recovery | Unplug/replug resumes within 2 s |
| S8 | R1 live test | Shared reads while LGS runs plus physical keypresses; first hardware action of M1 |
| S9 | Committed hardware fixtures | Raw reports for every control; LEFT/DOWN/TOP physical mapping in `docs/hardware-notes.md` |
| S10 | Gates | `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test` green on Windows, macOS, Linux; hardware tests `#[ignore]` and gated on `BOSUN_HW=1` |

### Output paths in scope

- RGB backlight: feature report 7, `[07, R, G, B, 00]`.
- M1/M2/M3/MR LEDs: feature report 5, `[05, mask, 0, 0, 0]`.
- LCD: 992-byte output report 3 — 31 padding bytes then a 960-byte framebuffer; `offset = col + (row >> 3) * 160`, `bit = 1 << (row & 7)`; rows 0–42 visible, rows 43–47 always zero.
- Feature report 6 (LGS identity channel): **never written**.

### Current repository state (2026-09-11)

Already on `main`: `Transport`, `HidTransport` (enumerate, `open_path`), `MockTransport`, `DeviceMatch`/`select*`, the `g13_probe` example, `bosunctl device list`, and the hardware-gated R1 test in `crates/bosun-hid/tests/hardware.rs`. Not yet: descriptor loading and codec, `info|watch|record|rgb|leds|lcd test`, hot-plug recovery, fixtures, R1 result.

---

## 4. Non-goals

Out of M1, and out of this PRD's commitments:

- Engine (layers, gestures, radial, macro capture), profiles, profile schema, LGS importer.
- Adapters, adapter host, JSON-RPC, `clawdbot` (locked in v1, outside M1).
- Keystroke/mouse injection.
- HTTP/WS control plane, `bosund` daemon, Tauri GUI, tray.
- LCD widgets, `embedded-graphics`, simulator, golden images. `lcd test` writes a fixed pattern to prove the report path, nothing more.
- Empty future crates. The workspace stays `bosun-hid` + `bosunctl` + `devices/`.
- Any device other than the G13. The descriptor format must not preclude device #2, but device #2 is not built.
- libusb, WinUSB/Zadig, kexts, kernel modules, HidHide, `EVIOCGRAB`, exclusive opens (D1, ADR-0001).
- GPL code or dependencies. GPL G13 repositories are not read, copied, translated, or adapted (D9, R6, `CONTRIBUTING.md`).
- Gaming as a feature area (R7).
- Market claims beyond `docs/CODEX-MICRO-COMPLAINTS.md` and the Spec Desk notes cited here.

---

## 5. Acceptance criteria

Each criterion is pass/fail. "HW" means run on Kyle's workstation with the G13 attached and `BOSUN_HW=1`. Everything else runs in CI on all three OSes with no hardware.

| ID | Criterion | How to check |
|---|---|---|
| **AC-R1** | **Shared input while LGS runs.** With `LCore.exe` running and holding its shared handle, a second reader opened by Bosun receives ≥1 input report (8 bytes, ID `0x01`) during 15 s of physical keypresses. | HW: `BOSUN_HW=1 BOSUN_HW_PATH=<path from device list> cargo test -p bosun-hid --test hardware -- --ignored r1_shared_input_reports_reach_a_second_reader --nocapture`. Record the outcome in `docs/hardware-notes.md` as one of: **pass with LGS running**, **pass only with LGS closed**, **fail both**. If zero reports with LGS running, quit LGS and rerun before any redesign. "Fail both" stops M1 for Kyle's review. |
| AC-2 | Device selection uses VID + PID + usage page and opens the enumerated path. The keyboard-page collection of the same VID/PID is never selected. | Unit tests in `bosun-hid::device` (present). HW: `bosunctl device list --vid 0x046D --pid 0xC21C --usage-page 0xFF00` prints exactly the `0xFF00` collection. |
| AC-3 | `bosun-hid` is synchronous and policy-free: no Tokio, no G13 identifiers compiled in, `hidapi` pinned with default features off and `linux-static-hidraw`. | `cargo tree -p bosun-hid` shows no `tokio`; `grep -ri "c21c\|046d" crates/bosun-hid/src` matches only test data; `Cargo.toml` inspection. |
| AC-4 | `MockTransport` scripts reports, timeouts, and disconnects; all decode and CLI tests pass with no hardware attached. | `cargo test --workspace --all-features` green on `ubuntu-latest`, `windows-latest`, `macos-latest`. |
| AC-5 | The G13 descriptor loads from `devices/logitech-g13.toml` with 34 literal key names, a `[match]` block, axes, screen, rgb, and leds. A malformed descriptor is rejected with an error, not a panic. | Unit tests over the loader: valid file, missing key, wrong type, range syntax rejected. |
| AC-6 | Decode maps bits `0..=21` and `24..=35` to named keys with pressed = 1 and emits edge events (down/up) by diffing against the previous report. Toggling bits 22, 23, 36, 37, 38, 39 emits **no** key event. Stick bytes decode to 0–255 axes. | Golden fixture tests replaying captured 8-byte reports; a dedicated test flips each ignored bit and asserts an empty event stream. |
| AC-7 | `bosunctl device watch` prints decoded named key/axis events. `record` writes raw 8-byte reports to a fixture file. | HW: press each control; every one of 34 names appears. Mock: `watch` over `MockTransport` emits the expected events; `record` output round-trips through the decoder. |
| AC-8 | `bosunctl device rgb R G B` sends feature report 7 and reads it back; `leds MASK` sends feature report 5; `lcd test` sends one 992-byte report 3 with byte 0 = `0x03`, bytes 1–31 zero, rows 43–47 zero. Feature report 6 is never written by any command. | Mock: assert exact bytes written per command; assert no write with report ID 6 across the whole CLI surface. HW: backlight visibly changes and readback matches; M-LEDs match the mask; LCD shows the pattern. |
| AC-9 | Hot-plug: after unplug/replug, `watch` resumes delivering events within 2 s and prints one reconnect line. | Mock: scripted disconnect followed by reports; assert recovery. HW: stopwatch or timestamp diff ≤ 2 s. |
| AC-10 | Hardware fixtures are committed: at least one raw report per control, plus stick center, four cardinals, four corners, and diagonals. `docs/hardware-notes.md` names the physical thumb button for each of `LEFT`, `DOWN`, `TOP` and replaces its "Pending M1 capture" section. | File presence in the repo; fixture files parse and drive AC-6. |
| AC-11 | Gates are green: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features`, `cargo-deny`. Hardware tests are `#[ignore]` and skip with a message unless `BOSUN_HW=1`. | CI on all three OSes; `cargo test -- --ignored` without `BOSUN_HW` prints the skip message and passes. |
| AC-12 | License hygiene: `deny.toml` passes; no GPL/AGPL dependency; no code, comments, or identifiers traceable to `khampf/g13`, `cavefish-dev/g13-driver`, or kernel `hid-lg-*`. | `cargo-deny` in CI; reviewer attestation on the M1 PR. |
| AC-13 | Windows toolchain: `rustc -vV` reports `x86_64-pc-windows-msvc` before any HW run. | Shell check per `PLAN-REVIEW` item 16; note in the HW run log. |

**M1 is done** when AC-R1 has a recorded outcome other than "fail both", AC-2 through AC-13 pass, and CI is green on Windows, macOS, and Linux (`docs/M1-KICKOFF.md`, Done condition).

---

## 6. Risks

| # | Risk | Severity | Evidence | Mitigation in M1 | Later |
|---|---|---|---|---|---|
| R1 | Input reports may not reach a second reader while LGS holds the device. | Medium | Phase 0 read window saw 0 reports with nobody at the keypad; unverified either way | **AC-R1 is the first hardware action.** Quit LGS and retry before redesign. Cost of the bad outcome is a README paragraph | — |
| R3 | LGS / G HUB fight Bosun for the LCD and RGB (both paint). | Low–Medium | Phase 0 writes succeeded with LGS running; LGS + G HUB device-stealing is a recurring community complaint (Reddit 2024–2025, Spec Desk notes; not independently linked) | Document in README that LGS may overwrite output (already there). No detection code in M1 | `bosunctl doctor` detects `LCore.exe` / `lghub_agent.exe` (M8) |
| R6 | GPL contamination. `cavefish-dev/g13-driver` is GPL-3; `khampf/g13` is banned by `CONTRIBUTING.md`; `vividnightmare/g510s` is GPL-3; kernel `hid-lg-*` is GPL. Permissive siblings exist (`RunicLuke/logitech-g13`, `golgote/G13`, both MIT) but are still not code sources. | Medium | Metadata only; no source consulted | AC-12. Implementation source is `docs/BOSUN-PLAN.md` §2 exclusively. Reject GPL-derived contributions | Same rule forever |
| R5 | G13 supply is finite and discontinued. | Low by design | G HUB does not support it; only used units exist (Spec Desk notes) | D11 descriptor-driven layer from M1; AC-5 ensures the G13 is data | Device #2 is a VIA/QMK pad with encoder (Class C, QMK Raw HID `0xFF60`), bought before M3 |
| R2 | HID-only on macOS means Input Monitoring (TCC) gates all HID access and binds to the bundle, not the binary. A bare-terminal daemon grants the permission to Terminal. | Medium | `docs/BOSUN-PLAN.md` §6; Work Louder warns Karabiner/Logitech Options+ interfere on macOS ([brief](CODEX-MICRO-COMPLAINTS.md) P1) | M1 CLI runs from a terminal and accepts the Terminal grant; document it. CI compiles and runs mock tests on macOS | `Bosun.app` bundle with daemon inside, Input Monitoring + Accessibility explained (M8) |
| — | Linux hidraw needs a udev rule; without it enumeration returns nothing for the G13. | Low | `docs/BOSUN-PLAN.md` §6 | Hardware test message already points at the udev rule; ship the rule text in docs | Installer drops `70-bosun.rules` (M8) |
| — | Stale Chocolatey GNU Rust shims on the Windows workstation break `hidapi` builds. | Low | `PLAN-REVIEW` item 16 | AC-13 | — |
| R7 | Scope creep toward a general macro tool or into M2+ features during M1. | Medium | Four existing G13 drivers are all gaming macro tools | §4 non-goals; `AGENTS.md` forbids future crates; a review never expands M1 silently (`MARKET-WATCH.md`) | Every feature request asks: does this help a person command an agent fleet? |
| — | Inventing market facts. | Low | `MARKET-WATCH.md` hard exclusions (e.g. resale price, on-device radial claims) | This PRD cites only the sources listed; unlinked names are labeled as such | Weekly sweep until M4 |

---

## 7. Open Kyle decisions

Only unresolved items. Already closed and not reopened: public repo; `clawdbot` in v1, outside M1; device #2 class = VIA/QMK with encoder, before M3; name Bosun; D1–D11.

| # | Decision | Needed by | Default if silent |
|---|---|---|---|
| K1 | **R1 outcome.** Does a second reader get input while LGS holds a shared handle? Requires a finger on the keypad. | First M1 hardware session | None — this is a measurement, not a choice. Outcome is recorded in `docs/hardware-notes.md` |
| K2 | **Physical LEFT / DOWN / TOP mapping and the joystick envelope.** Which case button each bitmap name is; center/cardinal/corner/diagonal values. | Same session as K1, via `bosunctl device record` | None — measurement |
| K3 | **Exact VIA/QMK SKU for device #2.** Class is locked; the part is not. | Before M3 | Any VIA-compatible pad with ≥1 rotary encoder and QMK Raw HID |
| K4 | **Does the LGS importer (D10) stay a day-one v1 priority?** It is M8 in the plan and a day of work; it buys the existing G13 community but is not agent value. | Before M8 planning | Stays in v1 per D10 |
| K5 | **Any market-watch promotions since the 2026-08-30 baseline?** Signals are advisory until promoted (`MARKET-WATCH.md`). | Next weekly sweep | No promotions; M1 unchanged |

---

## 8. After M1

Brief, from `docs/BOSUN-PLAN.md` §11. Not committed by this PRD.

- **M2** `bosun-lcd`: `embedded-graphics` `DrawTarget`, widgets, headless golden images in CI; simulator is local-only.
- **M3** `bosun-engine` + profile schema: tap/hold/double-tap/chord/detent FSM on injected timestamps; layers on M1–M3 with LEDs; device #2 arrives to keep the `Device` abstraction honest.
- **M4** `bosund` + loopback control plane: random per-install bearer token, Host/Origin validation, no permissive CORS; `shell` and `inject` actions; the device becomes daily-useful.
- **M5** adapter host + `clawdbot` + `claude-code`: JSON-RPC over stdio (stdout reserved, logs to stderr), agent state on the LCD, physical approvals.
- **M6–M9** radial menu and reasoning dial, `vscode`-family adapter, packaging/signing/LGS importer, registry and launch.

The G13 is the first device, not the product. The product is the agent-control layer above a descriptor-driven transport — which is why M1 has to get the transport boundary right.
