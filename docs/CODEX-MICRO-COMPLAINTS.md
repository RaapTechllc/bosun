# Codex Micro complaint brief

**Research date:** 2026-08-30

**Scope:** One bounded review round covering OpenAI Codex Micro, its ChatGPT/Codex desktop integration, and clearly labeled Work Louder Creator Micro 2 base-hardware reports.[1][2][3]
**Purpose:** Extract product lessons for Bosun. This brief does not change M1 scope.

## Executive summary

Codex Micro validates Bosun's core premise: the strongest feature is not generic macro execution but **glanceable agent state plus tactile intervention**. Hands-on reviewers praised the six status keys, single/double-tap task navigation, push-to-talk, remappable controls, and the joystick/dial as useful physicalizations of otherwise hidden software state.[2][15][16]

The recurring weakness is software architecture and integration, not the mechanical concept.[1][4][16]

Configuration is split between ChatGPT and Work Louder Input; reviewers found setup non-intuitive and one owner reported confusing navigation, model-selection trouble, and a device freeze.[1][16]

Multiple independent Windows reports associate the optional Codex Micro integration with severe desktop-app stalls—even with no Micro connected.[4][5][6]

A separate macOS report describes the controller becoming unresponsive over both USB and Bluetooth.[7]

The other repeated criticism is narrow value: $230 buys deep integration with one desktop experience, only six visible tasks, and no on-device text display.[2][15][18]

Reviewers and community commenters repeatedly questioned the price and preferred broader or display-equipped alternatives.[13][14][15]

Bosun already answers most of this through a multi-adapter architecture, a 160×43 LCD, seven agent keys plus fleet/queue views, descriptor-driven hardware, and data-only profiles.

## Findings

Recurrence labels are conservative:

- **Repeated** means at least two independent sources or reports support the same issue class.
- **OEM repeated** means multiple Creator Micro 2 users/releases support a base-platform issue; it is not proof that every Codex Micro has it.
- **Anecdote** means one first-hand report or one vendor warning.
- **Design fact** means official behavior that creates a limitation, not necessarily a reported defect.

| Priority | Category / product | Complaint or quirk | Recurrence | Current status | Bosun response |
|---|---|---|---|---|---|
| P0 | App / Codex Micro | Optional hardware discovery can stall the entire Windows desktop app. Reports describe startup, typing, task switching, and restore freezes; one profile attributed 95.4% of samples to synchronous HID enumeration. Some reports reproduce with no Codex Micro connected.[4][5][6] | **Repeated** (3 issue reports, with linked related reports) | Open in the cited reports | Preserve process isolation: `bosun-hid` remains synchronous but must never run on a GUI/control-plane thread. M4 daemon/API and later GUI must degrade safely when HID enumeration hangs. Add timeout/worker-boundary acceptance tests in M4, not M1. |
| P0 | App/device integration | The device or task keys can freeze or stop responding during use. One owner saw a mid-run freeze; a macOS report reproduced unresponsive task keys over USB and Bluetooth and found that opening device settings temporarily restored response.[7][16] | **Repeated** (2 independent first-hand reports) | Open/early software | M4 hot-plug/recovery must be observable and automatic; M5 adapter failure must not block input; M8 `doctor` should expose transport, adapter, and permission state. Keep device I/O and agent state as separately restartable paths. |
| P1 | Configuration / Codex Micro | Setup and customization are fragmented across ChatGPT and Work Louder Input. PCMag called the process non-intuitive and attributed friction to software split between two apps; official setup also directs Codex controls to ChatGPT and general layers to Input.[1][2][3] | **Repeated** (review + official workflow) | By design | Bosun should have one canonical profile model and one control plane. The GUI and CLI must edit the same files/API. M3 schema, M4 control plane, and M9 registry must not create separate configuration authorities. |
| P1 | Lock-in / Codex Micro | Live status is centered on the ChatGPT desktop integration, while general-purpose mappings use the OEM app. A hands-on review says the deepest integration is not available for every coding tool.[2][3][15] | **Repeated** | By design | D5 and M5/M7 are the direct answer: vendor-neutral adapters, JSON-RPC isolation, and no vendor knowledge in `bosun-core`. Keep `clawdbot` in v1 so multi-vendor value exists before launch. |
| P1 | Information density / Codex Micro | Only six tasks can be represented at once. Official docs define six Agent Keys; a reviewer managing a larger workload describes the device as a priority board rather than a complete operations console.[2][15] | **Design fact + hands-on limitation** | Hardware-limited | Preserve Bosun's seven direct agent keys, but rely on LCD fleet, queue, notification, and focused-detail views rather than pretending seven keys equal the whole fleet. M2 widgets and M5 target paging should make overflow explicit. |
| P1 | Status UI / Codex Micro | RGB communicates state but not task identity, approval text, error detail, or why input is needed. Official docs define a color/status mapping and four directional stick actions; the product specification lists RGB controls but no display.[2][18] | **Design fact** | Hardware-unfixable | D8, M2, and M6 already provide the counter: LCD text, approval body, queue counts, breadcrumbs, and a visible radial selection. Never encode critical meaning by color alone; pair RGB severity with text/glyphs. |
| P1 | Safety / Codex Micro | High-impact actions sit beside routine commands. PCMag identifies modes that bypass approvals and a command that stages, commits, pushes, and opens a pull request.[1] | **Design fact; risk inference** | By design | Preserve D6: profiles are data, `shell.run` requires explicit consent, and dangerous operations show a diff. Add an M3/M4 action-risk tier so destructive/broadcast actions can require hold or confirmation. G22 stop-all remains reserved and unoverrideable. |
| P1 | macOS coexistence | Work Louder warns that Karabiner and Logitech Options+ can interfere with Codex Micro communication when those apps hold Input Monitoring permission.[3] | **Vendor-confirmed anecdote** | Vendor says a fix is in progress | M8 `doctor` should enumerate likely permission/input-tool conflicts and explain Input Monitoring vs Accessibility. Do not claim exclusive-open assumptions on macOS without testing. |
| P2 | Navigation / Codex Micro app | One owner could not find a back action and reported that model selection "fought" the workflow.[16] | **Anecdote** | Unknown | M2/M6 LCD flows need an invariant cancel/back gesture, visible breadcrumbs for nested radials, and no modal state without an escape. Test every interaction with cancel, timeout, disconnect, and focus-loss paths. |
| P2 | Price/value/availability | Reviewers and community discussion repeatedly question whether a $230 specialized pad provides enough value versus software UI, a generic macropad, or a Stream Deck. OpenAI described it as a limited-run collaboration.[13][14][15] | **Repeated** | Product positioning; hardware sold out during review round | Position Bosun as software that recovers existing or inexpensive hardware, works with multiple agents, and remains useful with zero adapters through injection. Do not imitate luxury/collector positioning. |
| P2 | Platform support | Official Codex Micro documentation and product copy focus on macOS/Windows; PCMag says Linux support is community-maintained and mobile devices cannot perform configuration.[1][2][18] | **Repeated** | Limited by support model | Keep the three-OS matrix and headless daemon. M8 installers must be tested on fresh Windows/macOS/Linux systems; configuration must not require a vendor cloud or a second supported desktop OS. |
| P2 | Recovery ergonomics / Codex Micro | Work Louder's documented soft reset requires removing four PCB screws and pressing an internal reset button.[3] | **Vendor-documented quirk** | Hardware design | Bosun cannot fix OEM reset hardware, but `doctor`, reconnect, state restore, and transport restart should resolve software faults without unplugging or disassembly. Never make reset the normal recovery path. |
| P2 | OEM-inherited / Creator Micro 2 | Users report the Input app losing the device, charging problems, and lag or delayed reversal when an encoder emits repeated keys.[8][9][11] Firmware releases subsequently emphasize Bluetooth/USB reliability, pairing, power transitions, charging feedback, and recovery after moving between computers.[12] | **OEM repeated**, but not all reports are Codex Micro-specific | Mixed: charging megathread marked completed; reliability work continued in firmware | Park as device-#2 test cases. Before M3, validate reconnect, sleep/wake, USB/Bluetooth transitions, update interruption, encoder burst rate, and direction reversal on the chosen VIA/QMK device. Do not expand G13 M1 for unrelated wireless firmware behavior. |
| P2 | Compatibility/version negotiation / Creator Micro 2 | Creator Micro 2 can be detected as Codex-compatible yet enter a reconnect loop when an expected RPC method is missing.[17] | **Anecdote** | Open in cited feedback | M5 adapter initialization must negotiate protocol/capability versions and fail once with an actionable incompatibility state—never reconnect-loop indefinitely. Unknown methods or capabilities must degrade gracefully. |

A separate Creator Micro 2 owner reported that a required firmware update repeatedly ended with a generic retry error.[10] This is only one report, but it supports testing interrupted updates and actionable recovery on the second-device path rather than expanding G13 M1.

## What to copy, not fix

1. **Glanceable state is the product.** The status keys consistently receive the strongest praise because they remove tab hunting and make background work visible.[2][15]
2. **Single tap vs double tap is useful.** Background focus on tap and foreground activation on double tap is small but valuable; Bosun's agent-key behavior should preserve it and expose its timing in tests.[2][15]
3. **Physical approval and interruption are high-value verbs.** Codex Micro assigns dedicated command keys to approval and decline rather than hiding them in a menu.[2] Keep the tactile home cluster and approval queue; do not bury these common verbs in a radial.
4. **Reasoning effort benefits from a continuous-feeling control.** Codex Micro offers a dial mode dedicated to reasoning effort.[2] Keep Bosun's detent emulation, but always show the value on the LCD and require adapter capability discovery.
5. **The joystick invites exploration.** Reviewers liked it, but Bosun can improve it with a visible radial menu, deadzone feedback, cancel-at-center, breadcrumbs, and a confirmation flash.[16]
6. **Remapping matters.** Codex Micro's controls and extra layers make it useful beyond one default layout.[2][3] Bosun should retain declarative, inspectable, versioned profiles rather than hard-coded workflows.

## Requirements to carry forward

These are later-milestone acceptance requirements, not M1 changes:

- **M2/M6 — accessible feedback:** every RGB state also has a text/glyph representation; radial and reasoning selection are visible; every modal interaction has cancel/back.[2][16][18]
- **M3 — input safety:** debounce, hold thresholds, double-tap windows, and destructive-action confirmation are deterministic and replay-tested.[1][2]
- **M4 — fault containment:** HID discovery/read/write cannot block the API or GUI; absence, timeout, unplug, sleep/wake, and a stuck HID call have bounded recovery behavior.[4][5][7]
- **M5 — compatibility containment:** adapter protocol and capabilities are negotiated; malformed output, missing methods, and version mismatch produce one actionable offline state rather than a restart loop.[17]
- **M8 — diagnostics:** `bosunctl doctor` reports device-path match, permissions, conflicting software, adapter state, and recovery steps without requiring disassembly.[3][8]
- **M9 — coherent configuration:** CLI, GUI, registry, and importer all operate on one profile model; no split-brain settings across separate apps.[1][2][3]

## Claims not established in this round

The search did **not** produce adequate evidence for key chatter, capacitive-sensor false triggers, accidental joystick sector firing, or a specific Codex Micro joystick deadzone defect.[unverified] Those remain hypotheses, not requirements. The plan's $1,850 resale figure was not independently established in this round and is intentionally omitted.[unverified]

No source demonstrated a true on-device radial menu in Codex Micro; official documentation describes four thresholded stick directions. Bosun's eight-sector LCD radial should therefore be treated as a differentiated design, not described as a direct bug fix for a verified Codex Micro radial implementation.[2]

## Sources

[1] https://ca.pcmag.com/ai/17313/i-got-my-hands-on-openais-sold-out-codex-micro-who-is-this-230-vibe-coding-keyboard-even-for — PCMag hands-on review
[2] https://developers.openai.com/codex/features/codex-micro — Official Codex Micro documentation
[3] https://worklouder.cc/openai-micro-setup — Work Louder Codex Micro setup
[4] https://github.com/openai/codex/issues/33912 — OpenAI Codex issue #33912
[5] https://github.com/openai/codex/issues/33606 — OpenAI Codex issue #33606
[6] https://github.com/openai/codex/issues/34327 — OpenAI Codex issue #34327
[7] https://github.com/openai/codex/issues/34099 — OpenAI Codex issue #34099
[8] https://feedback.worklouder.cc/en/p/no-device-found-error-creator-micro-v2 — Creator Micro 2 no-device report
[9] https://feedback.worklouder.cc/en/p/creator-micro-v2-knob1-battery-charging-issue-megathread — Creator Micro 2 battery megathread
[10] https://feedback.worklouder.cc/en/p/new-creator-micro-v2-update-available-fails — Creator Micro 2 update failure
[11] https://feedback.worklouder.cc/en/p/scrolling-lag — Creator Micro 2 encoder lag report
[12] https://github.com/worklouder/cm-v2-fw-releases/releases — Creator Micro 2 firmware releases
[13] https://news.ycombinator.com/item?id=48923079 — Hacker News Codex Micro discussion
[14] https://techcrunch.com/2026/07/15/amid-hardware-legal-battle-openai-releases-a-230-keyboard-for-codex — TechCrunch Codex Micro launch coverage
[15] https://kingy.ai/blog/codex-micro-review — Kingy AI hands-on review
[16] https://www3.skool.com/start-my-ai/do-not-buy-openais-new-keyboard-im-keeping-mine — Start My AI owner first look
[17] https://feedback.worklouder.cc/p/creator-micro-2-codex-compatibility-needs-clarification-and-graceful-fallback-2 — Creator Micro 2 Codex compatibility report
[18] https://openai.com/supply/co-lab/work-louder — OpenAI Supply Co Codex Micro product page
