# BOSUN — Execution Plan

**Turning a discontinued Logitech G13 into a cross-platform command console for AI coding agents.**

| | |
|---|---|
| Codename | **Bosun** — locked 2026-08-30 (the crew member who relays the captain's orders) |
| Target hardware | Logitech G13 Advanced Gameboard, USB `046D:C21C` |
| Platforms | Windows 10/11, macOS 12+, Linux (X11 + Wayland) |
| Core language | Rust |
| License | MIT or Apache-2.0 (dual) — **not** GPL, see D9 and R6 |
| Author | RaapTech LLC |
| Plan date | 2026-08-30 |
| Status | Phase 0 **complete and verified on live hardware** (see §2) |

---

## 0. What this is

The G13 is a 34-button, analog-stick, 160×43-LCD, RGB-backlit input device that Logitech abandoned. Logitech Gaming Software 9.04 is the only thing keeping it alive on Windows, it has never worked properly on macOS, and every Linux driver is a libusb fork of a 2010 codebase.

OpenAI shipped the **Codex Micro** in July 2026: 13 keys, a rotary encoder, a joystick, a capacitive sensor, RGB status LEDs, $230, sold out in 12 hours, reselling at $1,850. It is a reskinned Work Louder Creator Micro 2 whose entire value is the *interaction model*: agent-status-at-a-glance, a joystick radial menu for workflows, and a dial for reasoning level.

The G13 has more buttons, an actual screen, and an analog stick. The hardware is strictly superior for this job. What's missing is software.

Bosun is that software: a cross-platform userspace driver plus an agent-control layer, with a community profile ecosystem.

**Non-goal:** gaming. There are four half-working G13 gaming drivers already. Bosun's default profiles target agent orchestration; keyboard emulation exists but is a fallback action type, not the point.

---

## 1. Decisions locked

These are settled. Do not re-litigate during implementation.

| # | Decision | Rationale |
|---|---|---|
| D1 | **HID stack only. No libusb, no WinUSB/Zadig, no kext, no kernel module.** | Measured working on Windows in §2. One code path for all three OSes. Every competing project got this wrong. |
| D2 | **Rust core, `hidapi` crate** | `hidapi` wraps Windows HID API, macOS IOHIDManager, Linux hidraw behind one API. Single static binary, no runtime for end users. |
| D3 | **`embedded-graphics` for LCD rendering** | The G13 LCD is exactly an `embedded-graphics` `DrawTarget`: 1bpp, 160×43. Free fonts, primitives, text layout, and a simulator for host-side testing. |
| D4 | **Daemon + CLI + tray GUI, three binaries, one workspace** | Daemon runs headless (server/homelab friendly). GUI is optional Tauri v2 shell over the daemon's local HTTP/WS API. |
| D5 | **Adapters are out-of-process, JSON-RPC 2.0** | Kyle's ecosystem is Python/n8n/TypeScript. Forcing adapters into Rust would kill contribution. Any language, declared by a manifest. |
| D6 | **Profiles are declarative data, never code** | Community profiles from strangers must not be able to execute arbitrary commands. `shell` actions require explicit per-profile consent with a diff shown. Non-negotiable — this is the TopG gate. |
| D7 | **Layer model: M1/M2/M3 × (base, L1-shift) = 6 layers × 22 G-keys = 132 bindings** | Uses hardware that already exists instead of inventing chords. |
| D8 | **LCD is the status surface; global RGB is the severity aggregate** | The G13 backlight is a single global RGB, not per-key. Codex Micro's per-key status colors are not reproducible. The screen is better anyway. |
| D9 | **Dual MIT/Apache-2.0** | Permissive so adapters and forks can be commercial. Do not copy GPL code from `cavefish-dev/g13-driver` or `khampf/g13`. Protocol facts are not copyrightable; their source is. |
| D10 | **LGS profile importer ships in v1** | Kyle already has an LGS profile on disk. Migration path is a day of work and buys the entire existing G13 community. |
| D11 | **Descriptor-driven device layer from M1 — hardware-agnostic core** | Nothing above `bosun-hid` knows the G13 exists; new Class A/C devices are a TOML descriptor plus at most a small codec impl. See §14. |

---

## 2. Verified hardware facts

Everything in this section was **measured on `desktop-2QF5HUN` on 2026-08-30** against the physically attached G13, with Logitech Gaming Software (`LCore.exe`) running. This is not documentation-derived.

### 2.1 Device topology as Windows sees it

```
USB\VID_046D&PID_C21C\6&2CE11939&0&1
  BusReportedDeviceDesc : "G13"
  Service               : HidUsb          <-- Microsoft's generic HID driver
  Driver                : Microsoft 10.0.26100.9278
  └── HID\VID_046D&PID_C21C\7&1562AFA0&0&0000
        \\?\hid#vid_046d&pid_c21c#7&1562afa0&0&0000#{4d1e55b2-...}
```

The G13 is bound to **Microsoft's stock `HidUsb` driver**, not a Logitech driver. LGS talks to it as a plain HID device from userspace. So can we.

### 2.2 Report descriptor, read from the device

```
Usage page 0xFF00 (vendor-defined), usage 0x0000, 1 link collection

  INPUT   report 1 :   7 data bytes  (8 with ID)    usage 0xFF00:0x0001
  OUTPUT  report 3 : 991 data bytes  (992 with ID)  usage 0xFF00:0x0002
  FEATURE report 4 :   4 data bytes                 usage 0xFF00:0x0004
  FEATURE report 5 :   4 data bytes                 usage 0xFF00:0x0005
  FEATURE report 6 : 257 data bytes                 usage 0xFF00:0x0006
  FEATURE report 7 :   4 data bytes                 usage 0xFF00:0x0003
```

Because the usage page is **vendor-defined (0xFF00), the OS generates no keyboard or mouse events from this device.** When LGS is removed, an unconfigured G13 produces zero system input. That means:

- **No HidHide / Interception / grab needed on Windows.**
- **No `EVIOCGRAB` or udev blacklist needed on Linux.**
- **No exclusive-open fight on macOS.**

This removes what is normally the single ugliest part of building a macropad driver.

### 2.3 Live read/write results

| Operation | Method | Result |
|---|---|---|
| Open device (shared R/W) | `CreateFileW(..., FILE_SHARE_READ\|FILE_SHARE_WRITE)` | **OK — while `LCore.exe` was running** |
| Open device (exclusive) | `CreateFileW(..., 0)` | FAIL, `ERROR_SHARING_VIOLATION (32)` — LGS holds it |
| Read RGB backlight | `HidD_GetFeature`, id 7 | `00 6e ff 5a` → R=0x6E G=0xFF B=0x5A (Kyle's current green) |
| Set backlight red | `HidD_SetFeature [07 FF 00 00 00]` | **OK**, readback `07 ff 00 00` |
| Set backlight green | `HidD_SetFeature [07 00 FF 00 00]` | **OK**, readback `07 00 ff 00` |
| Set backlight blue | `HidD_SetFeature [07 00 00 FF 00]` | **OK**, readback `07 00 00 ff` |
| Set M-key LEDs | `HidD_SetFeature [05 02 00 00 00]` | **OK** |
| Write LCD frame (border + X + bar) | `WriteFile`, 992 bytes | **OK, 992 bytes written** |
| Write LCD frame (stripes) | `WriteFile`, 992 bytes | **OK, 992 bytes written** |
| Restore original backlight | `HidD_SetFeature [07 6E FF 5A 00]` | **OK** |
| Read input reports | overlapped `ReadFile`, 8 bytes, 20 s window | 0 reports — **nobody was at the keypad**. Unverified, low risk (see §13 R1) |

**Conclusion: the entire G13 feature set is reachable from userspace through the stock HID stack, on Windows, concurrently with LGS, with zero driver changes.** Phase 0 is done.

### 2.4 Protocol reference (authoritative)

**Input report — 8 bytes, interrupt IN, report ID 1**

```
byte 0 : 0x01           report ID
byte 1 : joystick X     0..255, ~127 centred
byte 2 : joystick Y     0..255, ~127 centred
byte 3 : keys bits 0-7   G1  G2  G3  G4  G5  G6  G7  G8
byte 4 : keys bits 8-15  G9  G10 G11 G12 G13 G14 G15 G16
byte 5 : keys bits 16-23 G17 G18 G19 G20 G21 G22 --  LIGHT_STATE
byte 6 : keys bits 24-31 BD  L1  L2  L3  L4  M1  M2  M3
byte 7 : keys bits 32-39 MR  LEFT DOWN TOP --  LIGHT LIGHT2 MISC_TOGGLE
```

Bit `n` of byte `3 + (n>>3)` is `1 << (n & 7)`. Pressed = 1.
`LIGHT_STATE`, `LIGHT`, `LIGHT2`, `MISC_TOGGLE` and the two `--` bits are device state flags, not user buttons — mask them out.

**Output report — LCD framebuffer, 992 bytes, report ID 3**

```
byte 0        : 0x03      report ID
bytes 1..31   : 0x00      31 bytes of padding (must be present)
bytes 32..991 : 960-byte framebuffer
```

Framebuffer addressing — column-major within 8-row bands:

```
offset = col + (row >> 3) * 160          // 0 <= col < 160, 0 <= row < 48
bit    = 1 << (row & 7)                  // 1 = pixel on
```

6 bands × 160 bytes = 960 bytes. **Rows 0–42 are visible**; rows 43–47 exist in the buffer and are off-screen. Write them as zero.

**Feature reports**

| ID | Length (with ID) | Payload | Purpose |
|---|---|---|---|
| 4 | 5 | `[04, ?, 0, 0, 0]` | Device state. Reads back `04 81 00 00`. Not needed for v1. |
| 5 | 5 | `[05, ledmask, 0, 0, 0]` | M1/M2/M3/MR LEDs. Bit 0 = M1, bit 1 = M2, bit 2 = M3, bit 3 = MR. |
| 6 | 258 | ASCII string block | LGS handshake/name channel. Reads back `06 00 "SGL" 00 ... "EMAN"` — 4-byte-reversed `LGS` / `NAME`. **Leave alone in v1**; document as the LGS identity channel. |
| 7 | 5 | `[07, R, G, B, 00]` | Global RGB key backlight, 0–255 per channel. |

On Windows, `HidD_SetFeature` requires a buffer of exactly `FeatureReportByteLength` (**258**) regardless of which report ID you're sending; pad with zeros. `hidapi` handles this for you.

**Initialization**: none required. Older libusb drivers issue a `SET_CONFIGURATION` control transfer before LCD writes; through the HID stack this is unnecessary — the first `WriteFile` of a report-3 frame worked with no prior setup.

### 2.5 Physical inventory

| Group | Count | Names |
|---|---|---|
| Programmable macro keys | 22 | `G1`–`G22`, rows of 7 / 7 / 7 / 1 |
| LCD soft keys | 4 | `L1`–`L4` (below the screen) |
| Mode keys (with LEDs) | 4 | `M1` `M2` `M3` `MR` |
| Backlight/display button | 1 | `BD` |
| Thumb buttons | 3 | `TOP`, `LEFT`, `DOWN` (two mouse-style + stick click) |
| Analog axes | 2 | X, Y, 8-bit each |
| Display | 1 | 160×43 monochrome, backlit |
| Backlight | 1 | global RGB, 24-bit |
| Mode LEDs | 4 | M1/M2/M3/MR, on/off |

**34 buttons + 2 axes + a screen.** Logitech's "25 programmable keys" marketing number is the 22 G-keys plus the 3 thumb buttons.

**The single most important ergonomic fact:** `G4`, `G10`, `G11`, `G12` have **circular tactile indents** — Logitech's WASD cluster. They are the only four keys you can find by touch without looking. The default profile must put its four highest-frequency actions there. §4.1 does.

*To confirm in M1:* which physical thumb button each of `LEFT` / `DOWN` / `TOP` corresponds to. The names come from the report bitmap, not from the case. Five minutes with `bosunctl device watch`.

---

## 3. Bosun vs Codex Micro

| | Codex Micro ($230, sold out) | G13 + Bosun ($20–40 used) |
|---|---|---|
| Keys | 13 | **35** |
| Analog stick | yes | yes |
| Rotary encoder | yes | no — emulated on stick Y with detents (§4.5) |
| Display | none (RGB LEDs only) | **160×43 LCD** |
| Status signalling | per-key RGB, 5 states | LCD status strip + global RGB severity + 4 M-LEDs |
| Radial menu | yes, no visual | **yes, drawn on screen** |
| Agents supported | Codex only | **any, via adapters** |
| Platforms | ChatGPT desktop app | **Win / macOS / Linux** |
| Profiles | OpenAI's | **community registry** |
| Open | no | yes |

The one genuine hardware loss is the rotary encoder. The one genuine hardware win is the screen, and it is a much bigger win.

Codex Micro's status colors — white idle, green unread, blue thinking, peach question, red error — are a good vocabulary. Bosun adopts them as the canonical `AgentState` enum so profiles are portable and the semantics are already familiar.

---

## 4. The interaction model

This is the product. Everything in §5 onward exists to serve this section.

### 4.1 Layout (default profile: `raaptech/fleet`)

Built around the tactile indents, not around key numbering. `G4/G10/G11/G12` (marked `◉`) are the only keys findable by touch — they get the verbs you fire hundreds of times a day.

```
        ┌──────────────────────────────────────────────┐
        │  LCD 160x43  — status strip / radial / detail │
        └──[L1]──[L2]──[L3]──[L4]────────────[BD]───────┘

  row 1  G1   G2   G3  ◉G4   G5   G6   G7
         diff retry hand APRV plan test commit
  row 2  G8   G9  ◉G10 ◉G11 ◉G12  G13  G14
         model ctx REJECT SEND  STOP  log  note
  row 3  G15  G16  G17  G18  G19  G20  G21     <- AGENT KEYS (7)
         SIS  MAXX DAM  ATL  TOP  REM  HER
  row 4                 G22                    <- STOP ALL (broadcast)
         [M1] [M2] [M3] [MR]                   <- layer + record
              ( joystick )  [TOP] [LEFT] [DOWN]
```

**The home cluster (`◉`) — the four keys you never look at.**

| Key | Indent | Action | Why it's here |
|---|---|---|---|
| `G4` | W | **approve** | The single most-pressed key in a gated agent workflow |
| `G10` | A | **reject** | Its opposite, one finger away |
| `G11` | S | **send / confirm** | Resting-finger position, the default verb |
| `G12` | D | **interrupt** | Stop the focused agent, reachable in a panic |

**Row 3 — Agent keys.** One agent per key. Tap = focus that agent (its detail view fills the LCD). Double-tap = bring its window/session to the foreground. Hold = open that agent's action menu on the stick.
Default fleet binding: `G15` Sisyphus, `G16` Maxx-Truth, `G17` Damien, `G18` Atlas, `G19` TopG, `G20` Remi, `G21` Hermes.

**Rows 1–2 (non-indented) — Command and context keys.** Verbs and nouns applied to the focused agent: diff, retry, handoff, plan, test, commit, model select, context/worktree select, log, note.

**G22 — Stop all.** Broadcast interrupt to every adapter. Always bound in every layer. Cannot be overridden by a community profile.

**L1–L4 — Screen-contextual soft keys.** Their labels are drawn in the bottom strip of the LCD, changing with the current view. `L1` held is also the shift modifier.

**BD** — cycle LCD view: `fleet → focused agent → queue → notifications → clock/stats`.

**M1/M2/M3** — layer select, with the matching LED lit. **MR** — start/stop macro capture (records the last N actions into a new binding).

### 4.2 Agent status, at a glance

`AgentState` (from Codex Micro's vocabulary, extended):

| State | Color | LCD glyph |
|---|---|---|
| `idle` | white | `·` |
| `unread` | green | `▣` |
| `thinking` | blue | animated spinner |
| `question` | peach | `?` |
| `error` | red | `!` |
| `blocked` | amber | `⊘` |
| `offline` | dim | `–` |

**Global RGB = the most severe state across all agents**, ranked `error > question > blocked > thinking > unread > idle`. So the keypad glows red the instant anything in the fleet errors, from across the room, with no screen needed.

**Fleet view (default LCD screen):**

```
┌────────────────────────────────────────────────────────┐
│ SIS ▣2  MAXX ·   DAM ⟳   ATL ·   TOP !   REM ?   HER · │
│ ────────────────────────────────────────────────────── │
│ DAM  refactor jcm parser            04:12  gpt-5.5     │
│ TOP  ERR: exec gateway denied 'git push'               │
│ ────────────────────────────────────────────────────── │
│ [focus] [approve] [reject]  [queue 3]                  │
└────────────────────────────────────────────────────────┘
```

Top line: 7 agents, 3-char tag + state glyph + unread count. Middle: the focused agent's current task, elapsed time, model. Bottom: L1–L4 labels.

### 4.3 The radial menu (the Codex Micro headline feature, done better)

Hold any key bound to a `radial` action → the LCD switches to a radial overlay → tilt the stick to a sector → release the key to fire, or push `TOP` to fire and keep the menu open.

```
        ┌───────────────────────────────┐
        │          review PR            │
        │  run tests    ⊕    open diff  │
        │         ╲    ( )    ╱         │
        │  debug   ────┼────  refactor  │
        │         ╱    │    ╲           │
        │  commit    write test   plan  │
        └───────────────────────────────┘
```

- 8 sectors, deadzone at center = cancel.
- Sector labels come from the profile; each is an `Action`.
- Nested: a sector may open a sub-radial (breadcrumb in the top-left).
- Selection is highlighted inverted-video as you tilt.
- Fires on release with a 40 ms debounce and a confirm flash.

This is the primary "launch a workflow" gesture. It is the reason the joystick exists.

### 4.4 Approval gating

Kyle's fleet already enforces `write_approval: true` and gated mission phases. Bosun makes the approval physical:

- Any adapter can raise an `approval_request` with a title and a body.
- The LCD switches to the approval view, the backlight goes peach, and the M-LEDs strobe.
- `L2` = approve, `L3` = reject, stick-up/down scrolls the body, `L4` = "open in editor" for the full diff.
- Approvals queue; the count shows in the fleet view.

A hardware approve button for an agent fleet is the single most useful thing this device can do. It sits under your left hand all day.

### 4.5 Reasoning dial (encoder emulation)

Hold `L1`, push the stick up/down. Each 25%-of-travel crossing = one detent, with a 120 ms repeat when held past 80%. Value renders as a bar on the LCD:

```
  reasoning   low ▓▓▓▓▓▓░░░░ high      (medium → high)
```

Bound by default to the focused adapter's `set_effort` capability. Adapters that don't expose it hide the control.

### 4.6 Macro capture (`MR` repurposed)

`MR` used to record keystroke macros. In Bosun it records **the last N Bosun actions** into a named composite action, then prompts on the LCD for which key to bind it to. Turning "focus Damien → set model → send prompt X" into one key takes 4 seconds.

---

## 5. Architecture

### 5.1 Process model

```
                       ┌──────────────────────────────────────┐
   USB HID             │            bosund (Rust)             │
  ┌────────┐  reports  │  ┌────────────────────────────────┐  │
  │  G13   │──────────▶│  │ device: hidapi read loop       │  │
  │        │◀──────────│  │  → decode → KeyEvent/AxisEvent │  │
  └────────┘  LCD/RGB  │  └──────────────┬─────────────────┘  │
                       │                 ▼                    │
                       │  ┌────────────────────────────────┐  │
                       │  │ engine: layers, chords, holds, │  │
                       │  │  radial FSM, macro recorder    │  │
                       │  └──────────────┬─────────────────┘  │
                       │                 ▼                    │
                       │  ┌────────────────────────────────┐  │
                       │  │ dispatcher: Action → target    │  │
                       │  └──┬──────────────┬──────────┬───┘  │
                       │     ▼              ▼          ▼      │
                       │  ┌──────┐   ┌───────────┐ ┌───────┐  │
                       │  │ hid  │   │ adapter   │ │ input │  │
                       │  │ out  │   │  bus      │ │ inject│  │
                       │  │(LCD, │   │(JSON-RPC) │ │(enigo)│  │
                       │  │ RGB) │   └─────┬─────┘ └───────┘  │
                       │  └──────┘         │                  │
                       │  ┌────────────────┼────────────────┐ │
                       │  │ control plane: HTTP + WS :7113  │ │
                       │  └────────────────┼────────────────┘ │
                       └───────────────────┼──────────────────┘
                    ┌──────────────┬───────┴──────┬─────────────┐
                    ▼              ▼              ▼             ▼
              vscode-adapter  claude-code-   devin-adapter  clawdbot-
              (TS, WS to      adapter (py)   (py, REST)     adapter
               extension)                                   (py, fleet API)
                    │
                    ▼
              VS Code / Cursor / Windsurf extension
```

### 5.2 Cargo workspace

```
bosun/
├─ crates/
│  ├─ bosun-hid/        G13 transport. hidapi. Zero policy.
│  ├─ bosun-proto/      Wire types: events, actions, adapter RPC. serde + schemars.
│  ├─ bosun-engine/     Layers, hold/tap/double-tap FSM, radial FSM, macro recorder.
│  ├─ bosun-lcd/        embedded-graphics DrawTarget + view widgets + compositor.
│  ├─ bosun-adapters/   Adapter host: spawn, handshake, health, restart, sandbox.
│  ├─ bosun-inject/     Keyboard/mouse injection (enigo) + per-OS quirks.
│  ├─ bosun-config/     Profile load/validate/merge, LGS importer, registry client.
│  └─ bosun-core/       Wires the above. The daemon's brain, no I/O of its own.
├─ apps/
│  ├─ bosund/           Daemon binary. tokio. systemd/launchd/service integration.
│  ├─ bosunctl/         CLI: profile, adapter, device, debug, dump.
│  └─ bosun-gui/        Tauri v2 tray + config UI (talks to :7113 like anyone else).
├─ adapters/            First-party adapters (each independently versioned)
│  ├─ vscode/           TS extension + Node bridge (Cursor, Windsurf, VSCodium too)
│  ├─ claude-code/      Python, wraps `claude` CLI + hooks
│  ├─ codex-cli/        Python, wraps `codex` / `cursor-agent`
│  ├─ devin/            Python, Devin REST v1/v3
│  ├─ hermes/           Python, Hermes CLI + hermes proxy endpoints
│  ├─ grok/             Python, xAI API (OpenAI-compatible)
│  ├─ clawdbot/         Python, Kyle's fleet API / Mission Control
│  ├─ n8n/              Python, webhook trigger + execution status poll
│  └─ shell/            Generic, consent-gated command runner
├─ profiles/            First-party profiles (data only)
├─ schemas/             Generated JSON Schema (CI-enforced, published)
└─ docs/
```

**Dependency rule:** `bosun-hid`, `bosun-proto`, `bosun-lcd` have no knowledge of agents. `bosun-core` knows nothing about specific vendors. All vendor knowledge lives in `adapters/`. This is what makes the thing outlive Cursor, Devin, and Grok.

### 5.3 Event pipeline

```
HID report (8B, ~5 ms)
  → decode        : bitmask diff vs previous → KeyDown/KeyUp; axis → AxisMoved (deadzone, EMA smoothing)
  → engine        : resolve (layer, key, gesture) → Binding
                    gestures: tap | double_tap | hold(ms) | chord | radial | detent
  → dispatcher    : Binding.action → one of
                      • ui.*        handled locally (change view, open radial)
                      • adapter.*   JSON-RPC call to a named adapter
                      • inject.*    keystroke/text/mouse via enigo
                      • device.*    set backlight, LEDs, LCD
                      • composite   ordered list of the above, with `on_error`
  → feedback      : LCD repaint + RGB update, always within 16 ms of the action firing
```

Latency budget, key-press to LCD acknowledgement: **< 25 ms**. This is a felt property; treat a regression past 40 ms as a bug.

### 5.4 Adapter protocol

JSON-RPC 2.0. Subprocess adapters speak it over stdio (newline-delimited). Long-lived/remote adapters speak it over the daemon's WebSocket at `ws://127.0.0.1:7113/adapter`.

**Daemon → adapter (requests):**

| Method | Params | Returns |
|---|---|---|
| `initialize` | `{ bosun_version, capabilities }` | `AdapterManifest` |
| `list_targets` | `{}` | `Target[]` — the addressable agents/sessions |
| `invoke` | `{ target_id, action, args }` | `{ ok, message?, data? }` |
| `set_effort` | `{ target_id, level }` | `{ ok }` |
| `interrupt` | `{ target_id }` | `{ ok }` |
| `resolve_approval` | `{ approval_id, decision, note? }` | `{ ok }` |
| `shutdown` | `{}` | — |

**Adapter → daemon (notifications):**

| Method | Params |
|---|---|
| `state_changed` | `{ target_id, state, detail?, unread? }` |
| `approval_requested` | `{ approval_id, target_id, title, body, options[] }` |
| `notify` | `{ target_id, level, text }` |
| `targets_changed` | `{}` — daemon re-polls `list_targets` |

**Adapter manifest** (returned from `initialize`, also shipped as `adapter.toml`):

```toml
id            = "vscode"
name          = "VS Code family"
version       = "0.3.0"
transport     = "stdio"          # stdio | ws
exec          = ["node", "bridge.js"]
capabilities  = ["invoke", "interrupt", "set_effort", "approvals", "targets"]
effort_levels = ["low", "medium", "high"]

[[actions]]
id     = "send_prompt"
label  = "Send prompt"
args   = [{ name = "text", type = "string", required = true }]

[[actions]]
id     = "run_command"
label  = "Run VS Code command"
args   = [{ name = "command", type = "string", required = true },
          { name = "args",    type = "json",   required = false }]
```

**Adapter lifecycle:** spawned on daemon start or lazily on first use; health-pinged every 10 s; restarted with exponential backoff to a 60 s cap; three consecutive crashes → marked `offline`, surfaced on the LCD, no further restarts until the user resets it. An adapter that dies never takes the daemon with it.

---

## 6. Cross-platform matrix

| Concern | Windows | macOS | Linux |
|---|---|---|---|
| HID read/write | `hidapi` → `HidUsb` (**verified**) | `hidapi` → IOHIDManager | `hidapi` → hidraw |
| Driver install | **none** | **none** | **none** |
| Permission needed | none | **Input Monitoring (TCC)** | udev rule for `hidraw` |
| Permission prompt trigger | — | first `IOHIDDeviceOpen`; must be granted to the *bundle*, so ship `Bosun.app` and run the daemon inside it | — |
| Device produces stray input? | **no** (vendor usage page) | no | no |
| Keystroke injection | `SendInput` via `enigo` | `CGEventPost` — needs **Accessibility** | `uinput` (Wayland-safe) with `XTEST` fallback |
| Injection permission | none | Accessibility (TCC) | user in `input` group, or udev rule for `/dev/uinput` |
| Autostart | Scheduled Task at logon (not a Service — needs a desktop session for injection) | `launchd` LaunchAgent | systemd **user** unit |
| Packaging | MSI (WiX) + winget | signed+notarized `.app` in a `.dmg` + Homebrew cask | AppImage + `.deb` + `.rpm` + AUR + Flatpak |
| Coexists with LGS/G HUB? | **yes** (verified, shared handle) — but §7.6 | n/a | n/a |
| Signing | Authenticode (needed to avoid SmartScreen) | Developer ID + notarization (**mandatory**) | none |

**udev rules (`/etc/udev/rules.d/70-bosun.rules`):**

```
# G13 hidraw access for the logged-in user
KERNEL=="hidraw*", ATTRS{idVendor}=="046d", ATTRS{idProduct}=="c21c", MODE="0660", TAG+="uaccess"
SUBSYSTEM=="usb", ATTRS{idVendor}=="046d", ATTRS{idProduct}=="c21c", MODE="0660", TAG+="uaccess"
# uinput for keystroke injection
KERNEL=="uinput", MODE="0660", GROUP="input", OPTIONS+="static_node=uinput"
```

**macOS note:** Input Monitoring is granted to the *application bundle*, and it blanket-gates all HID access, not just keyboards. So `bosund` must live inside `Bosun.app/Contents/MacOS/` and the LaunchAgent must point at the binary *inside the bundle*. Running it from a bare terminal path grants the permission to Terminal instead, which is a support-ticket generator. Build it right the first time.

---

## 7. Data model

### 7.1 Profile (`profile.json`, JSON Schema published)

```jsonc
{
  "schema": "https://bosun.sh/schema/profile/1.json",
  "id": "raaptech/fleet",
  "name": "RaapTech Agent Fleet",
  "version": "1.2.0",
  "author": "RaapTech LLC",
  "device": "logitech-g13",
  "requires_adapters": ["clawdbot", "claude-code", "vscode"],
  "grants": { "shell": false, "network": false },

  "device_defaults": {
    "backlight": { "mode": "severity", "idle": [40, 40, 60] },
    "lcd_view": "fleet"
  },

  "targets": [
    { "key": "sisyphus", "adapter": "clawdbot", "target_id": "sisyphus", "tag": "SIS" },
    { "key": "damien",   "adapter": "clawdbot", "target_id": "damien",   "tag": "DAM" }
  ],

  "layers": [
    {
      "id": "M1",
      "name": "Fleet",
      "bindings": [
        { "key": "G15", "gesture": "tap",      "action": { "type": "ui.focus", "target": "sisyphus" } },
        { "key": "G15", "gesture": "hold:350", "action": { "type": "ui.radial", "menu": "agent_actions" } },
        { "key": "G4",  "gesture": "tap",      "action": { "type": "ui.approve" } },
        { "key": "G10", "gesture": "tap",      "action": { "type": "ui.reject" } },
        { "key": "G11", "gesture": "tap",      "action": { "type": "adapter.invoke",
                                                           "adapter": "clawdbot", "action": "send_prompt",
                                                           "args": { "text": "@focused status" } } },
        { "key": "G12", "gesture": "tap",      "action": { "type": "adapter.interrupt" } },
        { "key": "G22", "gesture": "tap",      "action": { "type": "ui.stop_all" } }
      ]
    }
  ],

  "menus": [
    {
      "id": "agent_actions",
      "sectors": [
        { "at": "N",  "label": "review PR",  "action": { "type": "adapter.invoke", "action": "review_pr" } },
        { "at": "NE", "label": "open diff",  "action": { "type": "adapter.invoke", "action": "open_diff" } },
        { "at": "E",  "label": "refactor",   "action": { "type": "ui.submenu", "menu": "refactor_kinds" } }
      ]
    }
  ],

  "screens": [
    { "id": "fleet", "widgets": [
        { "type": "agent_strip", "rect": [0, 0, 160, 9] },
        { "type": "focus_detail", "rect": [0, 11, 160, 22] },
        { "type": "softkeys", "rect": [0, 35, 160, 8] } ] }
  ]
}
```

### 7.2 Action types

| Type | Purpose | Consent |
|---|---|---|
| `ui.focus` / `ui.view` / `ui.radial` / `ui.submenu` / `ui.approve` / `ui.reject` / `ui.stop_all` | Local UI | none |
| `device.backlight` / `device.leds` / `device.lcd_text` | Device feedback | none |
| `adapter.invoke` / `adapter.interrupt` / `adapter.set_effort` | Agent control | adapter must be installed |
| `inject.keys` / `inject.text` | Keyboard emulation | none (but never auto-bound from a community profile without preview) |
| `composite` | Ordered list with `on_error: stop \| continue` | union of children |
| `shell.run` | Arbitrary command | **explicit per-profile grant, command shown in a diff at install time** |

`shell.run` is deliberately awkward. Community profiles that need it will be rare and should be scrutinized. This is the security boundary.

### 7.3 Storage layout

```
Linux/macOS: ~/.config/bosun/          Windows: %APPDATA%\Bosun\
├─ config.toml            device selection, log level, port, autostart
├─ profiles/
│  ├─ active -> raaptech-fleet/        symlink or pointer file
│  └─ raaptech-fleet/
│     ├─ profile.json
│     ├─ lock.json                     resolved adapter versions + grant record
│     └─ assets/                       1bpp images for the LCD
├─ adapters/<id>/                      installed adapters + their config
└─ state/                              macro recordings, last view, calibration
```

`~/.config/bosun/profiles/` is a plain directory of JSON — **git-friendly by construction**, which is how a community shares them and how Kyle version-controls his.

---

## 8. Adapter catalog

Priority order. Each is independently shippable; the daemon works with zero adapters installed (it just does keyboard emulation).

### Tier 1 — build in v1

**`vscode`** — covers **VS Code, Cursor, Windsurf, VSCodium, and every fork**, which is the highest-leverage single adapter.
Mechanism: a Bosun VS Code extension registers a URI handler and opens a local WebSocket back to the daemon. The extension can then execute *any* VS Code command via `vscode.commands.executeCommand`, which reaches Cursor's and Windsurf's agent commands too since they are registered commands. Fallback for forks that block extensions: `code --open-url "vscode://raaptech.bosun/run?cmd=..."`.
Exposed actions: `run_command`, `send_prompt` (types into the agent panel), `open_diff`, `next_change`, `accept_change`, `reject_change`, `open_file`, `switch_workspace`.
State: derives `thinking`/`idle` from the agent panel's state where the fork exposes it; otherwise `idle` + explicit notifications.

**`claude-code`** — wraps the `claude` CLI in non-interactive mode plus hooks for state. Hooks are how you get real `thinking`/`question`/`error` transitions instead of polling.
Actions: `send_prompt`, `resume`, `interrupt`, `set_model`, `run_slash_command` (reaches Kyle's `/audit`, `/next`, `/compile-idea`, Deep Council).

**`codex-cli` / `cursor-agent`** — `cursor-agent` supports print mode, `--model`, `--output-format`, `--mode=plan|ask`, `--resume`/`--continue`, and session listing via `agent ls`. That is enough to drive turns and track sessions from outside. Same adapter shape covers the Codex CLI.

**`clawdbot`** — Kyle's fleet. Targets = Sisyphus, Maxx-Truth, Damien, Atlas, TopG, Remi, Axel, Hermes, Prometheus. This adapter is where charter-check, write_approval, and mission-phase gating surface as physical approvals on the keypad. **This is the one that makes the device indispensable to Kyle specifically** — build it in v1 even though it is not community-relevant.

**`shell`** — the escape hatch. Consent-gated. Makes the device useful before any other adapter exists.

### Tier 2 — v1.1

**`devin`** — REST. `POST /v1/sessions` to create, `POST /v1/sessions/{id}/message` to send, `GET /v1/sessions/{id}` to poll status; Bearer auth with a service API key. Polling at 3 s is fine; Devin sessions are long. Maps cleanly onto `Target` + `state_changed`.

**`hermes`** — Nous Hermes v0.15.2 on WSL2, plus `hermes proxy`'s OpenAI-compatible endpoints. Actions: start mission, send message, escalate to Deep Council tier N, read council verdict. `set_effort` maps to council tier (1–5), with the cost gate before tiers 4–5 surfacing as a **physical approval** — exactly the right place for it.

**`grok`** — xAI's API is OpenAI-compatible (`/v1/chat/completions`). Straight adapter; also the template for "any OpenAI-compatible endpoint", which covers OpenRouter, Ollama, and Kyle's `hermes proxy` shims in one implementation. Ship it as `openai-compatible` with `grok` as a preset.

**`n8n`** — fire a webhook, poll execution status, surface failures as `error` state. Turns the G13 into a physical trigger board for Kyle's automation.

### Tier 3 — community

JetBrains (via its own plugin + the same WS bridge), Aider, Continue, Zed, tmux/session control, GitHub Actions status, Jira/Linear.

**Adapter authoring is a documented, first-class path.** A working Python adapter should be ~120 lines. Ship a `bosun-adapter-template` repo and a `bosunctl adapter scaffold <id>` command.

---

## 9. LCD rendering

**Pipeline:** `Screen` (from profile) → widgets → `embedded-graphics` draws into a `Framebuffer<160, 48, 1bpp>` → pack to the 960-byte G13 layout → 992-byte report 3 → `hid_write`.

```rust
// bosun-lcd/src/frame.rs
pub struct G13Frame { buf: [u8; 960] }

impl G13Frame {
    #[inline]
    pub fn set(&mut self, x: u8, y: u8, on: bool) {
        if x >= 160 || y >= 48 { return; }
        let off = x as usize + (y as usize >> 3) * 160;
        let m = 1u8 << (y & 7);
        if on { self.buf[off] |= m } else { self.buf[off] &= !m }
    }

    pub fn to_report(&self) -> [u8; 992] {
        let mut r = [0u8; 992];
        r[0] = 0x03;
        r[32..].copy_from_slice(&self.buf);
        r
    }
}
// impl DrawTarget for G13Frame -> all of embedded-graphics for free
```

**Widgets to build:** `agent_strip`, `focus_detail`, `softkeys`, `radial`, `approval`, `bar` (reasoning/progress), `marquee` (scrolling long text), `clock`, `sparkline` (token burn / queue depth), `image` (1bpp asset).

**Refresh policy:** dirty-rect diffing, coalesced at **30 Hz max**. Never redraw on a timer when nothing changed — the G13 LCD is a USB interrupt endpoint and hammering it wastes bus bandwidth and pins a core.

**Fonts:** `embedded-graphics` `FONT_5X8` for the strip (32 chars/line), `FONT_6X10` for detail, `FONT_4X6` for softkey labels. 43 px tall = 5 lines at 8 px, or 4 lines with padding. Design to **4 usable lines**.

**Host-side simulator:** `embedded-graphics-simulator` renders every screen in a desktop window with no hardware attached. This is what makes LCD work testable in CI and developable on a laptop with no G13. Build it in M2, not later.

---

## 10. Community profile ecosystem

**Registry:** a GitHub repo, `bosun-profiles`, containing `index.json` + one directory per profile. No server to run, no account system, PRs are the moderation queue. Serve it from GitHub Pages / a Cloudflare Pages front-end for browsing.

**CLI:**

```
bosunctl profile search cursor
bosunctl profile install raaptech/fleet
bosunctl profile install ./my-profile          # local dir
bosunctl profile diff raaptech/fleet@1.3.0     # what changes on upgrade
bosunctl profile export --with-assets          # produces a shareable dir
bosunctl profile import-lgs "<path>.xml"       # LGS migration
```

**Install-time safety gate** — this is the whole reason profiles are data:

```
Installing raaptech/fleet 1.2.0
  requires adapters : clawdbot (installed), claude-code (installed), vscode (MISSING)
  grants requested  : shell = NO, network = NO
  binds G22         : ui.stop_all  (reserved, allowed)
  binds inject.text : 3 bindings   -> review before accepting
      G14 tap  -> types: "git push origin HEAD"
      ...
Accept? [y/N]
```

Any `shell.run` or `inject.*` action is printed verbatim before install. A profile that only uses `ui.*`, `device.*`, and `adapter.invoke` installs with a one-line summary.

**Versioning:** semver on profiles, `requires_adapters` with version ranges, `lock.json` records what was actually resolved plus the grant decision. Upgrades that add a new grant re-prompt.

**Curated first-party set:** `bosun/cursor`, `bosun/vscode`, `bosun/claude-code`, `bosun/devin`, `bosun/omarchy` (G-keys as Super-chords for DHH's Omarchy), `bosun/gaming-classic` (a WASD-and-macros profile so the existing G13 community has a reason to switch), `raaptech/fleet`.

---

## 11. Milestones

Effort is in **focused build sessions** (roughly a half-day of an agent-assisted developer). Assignments use Kyle's fleet conventions.

| M | Deliverable | Acceptance criteria | Sessions | Owner |
|---|---|---|---|---|
| **M0** | *Complete* — protocol proven on live hardware | §2 results reproduced | ✅ done | — |
| **M1** | `bosun-hid` + `bosunctl device` | `bosunctl device watch` prints decoded key/axis events on all 3 OSes; `device rgb 255 0 0`, `device lcd test` work; hot-plug recovers within 2 s; `Device` trait + TOML descriptor in place — the G13 ships as data, not a hardcode | 3 | Damien |
| **M2** | `bosun-lcd` + simulator | All widgets render in `embedded-graphics-simulator`; golden-image tests in CI; `bosunctl lcd render fleet` pushes to hardware | 3 | Damien |
| **M3** | `bosun-engine` + profile schema | Tap/hold/double-tap/chord/detent FSM unit-tested against recorded report streams; layers switch on M1–M3 with LEDs; profile JSON validates against published schema | 4 | Damien, Atlas reviews |
| **M4** | `bosund` + control plane + `shell` and `inject` actions | Daemon runs as a user service on all 3 OSes and survives sleep/unplug; HTTP+WS API documented; **the device is genuinely useful at this point** | 4 | Damien |
| **M5** | Adapter host + `clawdbot` + `claude-code` adapters | Agent status reaches the LCD; approvals resolve from L2/L3; adapter crash does not kill the daemon; restart backoff verified | 5 | Damien, TopG gates |
| **M6** | Radial menu + reasoning dial + macro capture | Radial selection accurate at all 8 sectors with a measured deadzone; end-to-end latency < 25 ms measured with a logic trace or timestamp diff | 3 | Damien |
| **M7** | `vscode` adapter (+ Cursor/Windsurf verification) | Arbitrary VS Code command executes from a G-key in all three forks; prompt send works in Cursor and Windsurf | 4 | Damien |
| **M8** | Packaging + signing + LGS importer | MSI, notarized `.app`, AppImage/deb/rpm all install and autostart clean on fresh VMs; Kyle's existing LGS XML imports to a working profile | 5 | Damien, TopG signs off |
| **M9** | Registry, docs site, adapter template, launch | `bosunctl profile install` works from the public index; 6 first-party profiles; adapter authoring guide; Show HN / r/G13 / r/MechanicalKeyboards post | 4 | Remi drafts, Kyle ships |

**Total: ~35 sessions.** M1→M4 is the critical path to something you use daily; that's 14 sessions. Everything after M4 is leverage.

**Sequencing rule:** M1 and M2 are parallel-safe (separate crates, no shared state). M3 blocks everything after it. M5 and M7 are parallel. Use git worktrees per Kyle's established parallelization pattern.

**No-ADR-no-merge applies.** ADRs needed at minimum for: HID-vs-libusb (write it up from §2, it is the project's founding decision), adapter transport choice, the profile security model, and the injection strategy per OS.

---

## 12. Testing

| Layer | Approach |
|---|---|
| `bosun-hid` decode | Golden test vectors: recorded 8-byte reports → expected event streams. Captured once from real hardware, replayed forever in CI. |
| LCD widgets | `embedded-graphics-simulator` → PNG → perceptual-hash comparison against golden images. Catches layout regressions with no hardware. |
| Engine FSM | Property tests (`proptest`) over synthetic event streams: no stuck keys, no lost releases, layer state always consistent, radial always terminates. |
| Profiles | Every first-party profile validated against the JSON Schema in CI. Fuzz the loader with malformed profiles. |
| Adapters | Mock adapter that speaks the JSON-RPC protocol and can be told to hang, crash, or emit garbage. Assert the daemon survives all three. |
| Hardware-in-the-loop | A tagged `#[ignore]` test suite that runs only with `BOSUN_HW=1`. Kyle's workstation runs it pre-release. |
| Latency | A dedicated bench that timestamps `report received → LCD write issued`. Assert p99 < 25 ms. Fail CI on regression. |
| Install | Fresh-VM install tests on Windows 11, macOS, and Ubuntu, driven from CI. Catches the macOS bundle-permission trap. |

**Mock device:** `bosun-hid` sits behind a `Transport` trait with `HidTransport` and `MockTransport` implementations. Everything above `bosun-hid` is testable with zero hardware. Do this on day one; retrofitting it is expensive.

---

## 13. Risks

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| **R1** | **Input reports may not reach a second reader while LGS holds the device.** Not yet verified — nobody was at the keypad during the 20 s test window. | Medium | Windows HIDCLASS gives each open file object its own report ring buffer, so this should work. **Verify in the first 5 minutes of M1** (`bosunctl device watch`, press a key). If it fails, the answer is "uninstall LGS", which you want anyway. Cost of being wrong: one paragraph in the README. |
| R2 | macOS Input Monitoring blanket-gates HID; a badly-packaged daemon gets the permission attached to Terminal | Medium | Ship the daemon *inside* the `.app` bundle from M8 onward. Document with screenshots. Karabiner-Elements has trained the audience to expect this. |
| R3 | LGS/G HUB fight over the device (both painting the LCD, both setting RGB) | Low | Detect `LCore.exe`/`lghub_agent.exe` at startup and warn. Ship a `bosunctl doctor` that tells the user to disable LGS's G13 profile or uninstall it. Coexistence is proven; it's just visually confusing. |
| R4 | Cursor/Windsurf change their internal command IDs and break the adapter | Medium | Adapters are separately versioned and hot-reloadable. Command IDs live in the adapter's config, not its code — users can patch a broken binding without a release. |
| R5 | Hardware is discontinued; supply is finite | Low (by design) | Abstract the device behind a `Device` trait in `bosun-hid` from M1. Adding the G15/G510/G19 (same LCD family, similar reports) or a QMK macropad later is then an afternoon, and the entire agent layer is reused. **This is what makes the project outlive the G13.** Integration classes and candidate matrix: §14. |
| R6 | GPL contamination from reading `khampf/g13` or `cavefish-dev/g13-driver` | Medium | §2 protocol facts were measured from the device, not copied. Do not paste from those repos. If a contributor submits GPL-derived code, reject it. Note this in `CONTRIBUTING.md`. |
| R7 | Scope creep into a general macro tool | Medium | The gaming profile exists to attract users, not to be a feature area. Every feature request gets asked: does this help a person command an agent fleet? |
| R8 | Windows SmartScreen / macOS Gatekeeper block the unsigned installer, killing adoption | Medium | Budget for an Authenticode cert (~$200–400/yr) and an Apple Developer account ($99/yr) in M8. Unsigned distribution of a *driver-like* tool is a non-starter. |
| R9 | Linux Wayland blocks `XTEST` injection | Low | Use `uinput` as the primary path — it works under Wayland. `XTEST` is the fallback, not the default. |

---

## 14. Hardware-agnostic device layer

Bosun starts on the G13 because one is on your desk, but nothing above `bosun-hid` is allowed to know that (D11). Every candidate device falls into one of four integration classes, and the class answers the question that matters up front: **does supporting this device mean writing a data file, or installing/replacing drivers?**

### The four integration classes

**Class A — vendor-page HID. Nothing to install, total control.** The device talks on a vendor-defined usage page; the OS ignores it entirely and we own it from userspace: buttons, screens, LEDs, everything. The G13 is Class A (verified in §2). So is **every Elgato Stream Deck** — Elgato publishes an official HID protocol spec for makers, per-key LCDs and (on the +) real rotary dials included. Class A is Bosun's home turf.

**Class B — keyboard-page pads. Nothing to install, keystrokes only.** The pad *is* a keyboard: keys emit ordinary scancodes, usually reprogrammable to F13–F24 or Super-chords with a one-time vendor tool. No screen, no status backchannel, and grabbing their input exclusively is OS-hostile — so Bosun meets them with **hotkey-bridge mode**: the daemon registers global hotkeys (F13–F24, Super+X) and treats them as Bosun triggers. Zero driver work, works with every $40 Amazon pad. You lose the screen and per-device identity; you keep the entire action/adapter layer. Razer Tartarus V2/Pro, Azeron, Redragon K585, Koolertron all land here. (Azeron's analog stick is the nice exception: it appears as a standard HID gamepad axis, readable from userspace without grabbing — so an Azeron gets stick-driven radial menus too.)

**Class C — open-firmware pads (QMK/VIA). Nothing to install, and the firmware is yours.** QMK's Raw HID feature is a private bidirectional channel on usage page `0xFF60` (32-byte frames) — the same kind of side-channel the G13's vendor page gives us, on $25–60 hardware you can buy new, often with real rotary encoders (the Codex Micro dial, for real) and small OLEDs. Class C is the recommended *new-buy* path and the long-term insurance against G13 supply drying up.

**Class D — driver-swap devices. Refused.** Anything that needs Zadig/WinUSB, a kext, or a kernel module to be reachable. This is the line D1 draws, and exactly the mistake the existing Windows G13 driver made. If a device is only reachable this way, Bosun doesn't support it.

### Candidate device matrix

| Device | ~Price | Class | Install burden | Screen | Analog | Dials |
|---|---|---|---|---|---|---|
| **Logitech G13** (used) | $20–40 | **A** — verified | none | 160×43 LCD | stick | – |
| Elgato Stream Deck MK.2 | ~$150 | A — official spec | none | 15 key-LCDs | – | – |
| Elgato Stream Deck + | ~$200 | A — official spec | none | 8 keys + touch strip | – | **4 real dials** |
| Razer Tartarus V2 | ~$80 | B | Synapse once, to remap | – | 8-way d-pad | wheel |
| Razer Tartarus Pro | ~$130 | B | Synapse once | – | analog keys (Synapse-locked) | wheel |
| Azeron Cyborg / Cyro | $170+ | B + gamepad stick | Azeron app (Win/Linux), onboard memory | – | **true analog stick** | – |
| Redragon K585 Diti | ~$40 | B | Windows tool once, then standalone | – | – | – |
| VIA/QMK pads (DOIO, TogKey, …) | $25–60 | **C** | none (VIA web app) | some OLED | – | 1–3 encoders |

Verdicts: **start on the G13**; best device #2 is a Stream Deck (Class A with an official spec; the + adds true dial parity with the Codex Micro); best cheap new-buy is a VIA pad with an encoder.

### Device descriptors — hardware as data

Class A/C devices are added as **data, not forks**:

```toml
# devices/logitech-g13.toml
id    = "logitech-g13"
class = "A"
match = { vid = 0x046D, pid = 0xC21C, usage_page = 0xFF00 }

[caps]
keys   = ["G1".."G22", "L1".."L4", "M1", "M2", "M3", "MR", "BD", "TOP", "LEFT", "DOWN"]
axes   = [{ id = "stick", x_byte = 1, y_byte = 2, bits = 8 }]
screen = { w = 160, h = 43, format = "1bpp-bands", report = 3, pad = 31 }
rgb    = { zones = 1, report = 7 }
leds   = { names = ["M1", "M2", "M3", "MR"], report = 5 }
```

Declarative byte/bit maps cover the normal cases; anything weirder implements the `Device` trait directly. Profiles stay per-device in v1; the v2 path is **role slots** — a profile declares semantic slots (`approve`, `reject`, `send`, `interrupt`, `radial`, `dial`) and each descriptor maps slots to physical controls, so one profile follows you across hardware.

### The Super-key bridge (Omarchy and friends)

Super-key traffic flows both directions:

- **Out:** the first-party `bosun/omarchy` profile makes G-keys emit Super-chords through `uinput` — Wayland-native, so it works on Hyprland, where XTEST doesn't. Omarchy's entire UI is Super-key-driven by design, so the G13 becomes a physical Omarchy command strip with zero OS-side configuration.
- **In:** hotkey-bridge mode (Class B above) means any dumb pad that can emit F13–F24 or Super-chords drives *Bosun's* actions with no driver work at all.

### Virtual devices

The control plane already makes a phone or browser page just another device: a PWA speaking the daemon's WebSocket is a soft-Bosun — the "Codex Micro as an iPhone app" trick falls out of the architecture for free. Not scheduled for v1; noted so nobody builds it as a special case.

---

## 15. Open decisions for Kyle

Name's locked — **Bosun**. Three things left before M1:

1. **Public or RaapTech-internal?** The plan assumes public + community registry, which is where the leverage is (and it's good marketing for RaapTech). If you'd rather it stay internal, drop M9 and the registry work — that's 4 sessions saved and a much smaller security surface.

2. **`clawdbot` adapter priority.** I put it in v1 because it's the thing that makes the device indispensable *to you* on day one. But it's the least community-relevant piece. Keep it in v1, or defer to v1.1 and ship the generic adapters first?

3. **Device #2.** Buy one early — it keeps the `Device` trait honest by M3. My pick from the §14 matrix: a used Stream Deck MK.2 (Class A, officially documented, per-key screens) or a ~$30 VIA pad with an encoder (Class C — the real dial). Either costs less than being wrong about the abstraction.

Everything else in this document is decided.

---

## Appendix A — Bootstrap commands

```bash
# Workspace
cargo new --lib bosun && cd bosun
cargo add hidapi tokio --features tokio/full
cargo add serde serde_json schemars thiserror anyhow tracing tracing-subscriber
cargo add embedded-graphics
cargo add --dev embedded-graphics-simulator proptest

# Verify the device from Rust in five lines
cargo run --example probe
```

```rust
// examples/probe.rs — the entire Phase 0 result, in Rust
use hidapi::HidApi;

fn main() -> anyhow::Result<()> {
    let api = HidApi::new()?;
    let dev = api.open(0x046d, 0xc21c)?;

    // RGB backlight: feature report 7 = [id, R, G, B, 0]
    dev.send_feature_report(&[0x07, 0x00, 0x40, 0xff, 0x00])?;

    // M-key LEDs: feature report 5 = [id, mask, 0, 0, 0]
    dev.send_feature_report(&[0x05, 0b0001, 0, 0, 0])?;

    // LCD: output report 3 = [0x03] + 31 pad + 960 framebuffer
    let mut frame = [0u8; 992];
    frame[0] = 0x03;
    for col in 0..160 { frame[32 + col] |= 0x01; }        // top scanline
    dev.write(&frame)?;

    // Input: 8 bytes, [id, X, Y, 5 key bytes]
    let mut buf = [0u8; 8];
    loop {
        let n = dev.read_timeout(&mut buf, 1000)?;
        if n == 8 {
            let keys = u64::from_le_bytes(
                [buf[3], buf[4], buf[5], buf[6], buf[7], 0, 0, 0]);
            println!("x={:3} y={:3} keys={:040b}", buf[1], buf[2], keys);
        }
    }
}
```

```powershell
# Windows: confirm the device and its report lengths (what §2 ran)
Get-PnpDevice | ? InstanceId -match 'PID_C21C' | fl Status,Class,FriendlyName,Service
```

## Appendix B — Key bit table

| Bit | Name | Bit | Name | Bit | Name | Bit | Name |
|---|---|---|---|---|---|---|---|
| 0 | G1 | 10 | G11 | 20 | G21 | 30 | M2 |
| 1 | G2 | 11 | G12 | 21 | G22 | 31 | M3 |
| 2 | G3 | 12 | G13 | 22 | *(unused)* | 32 | MR |
| 3 | G4 | 13 | G14 | 23 | *LIGHT_STATE* | 33 | LEFT |
| 4 | G5 | 14 | G15 | 24 | BD | 34 | DOWN |
| 5 | G6 | 15 | G16 | 25 | L1 | 35 | TOP |
| 6 | G7 | 16 | G17 | 26 | L2 | 36 | *(unused)* |
| 7 | G8 | 17 | G18 | 27 | L3 | 37 | *LIGHT* |
| 8 | G9 | 18 | G19 | 28 | L4 | 38 | *LIGHT2* |
| 9 | G10 | 19 | G20 | 29 | M1 | 39 | *MISC_TOGGLE* |

*Italic* entries are device state flags, not buttons. Mask `0x0080_0080_0000` before diffing.

## Appendix C — LGS profile import mapping

Source: `%LOCALAPPDATA%\Logitech\Logitech Gaming Software\profiles\{GUID}.xml`
(Kyle's is `{09D92D75-3C8C-4723-B06C-4090BCB899C0}.xml`, 47 KB, last written 2026-08-30.)

```xml
<profiles xmlns="http://www.logitech.com/Cassandra/2010.7/Profile">
  <profile name="Default Profile" guid="{...}">
    <macros>
      <macro name="A" guid="{63BE7C7A-...}">
        <keystroke><key value="A"/></keystroke>
      </macro>
    </macros>
    <assignments devicecategory="Logitech.Gaming.Function.Keyboard">
      <assignment shiftstate="1" contextid="G1" macroguid="{D5AC487F-...}"/>
      <assignment shiftstate="4" contextid="G1" macroguid="{D5AC487F-...}"/>
    </assignments>
  </profile>
</profiles>
```

| LGS concept | Bosun equivalent |
|---|---|
| `<profile>` | one profile directory |
| `shiftstate="1" \| "2" \| "4"` | layer `M1` \| `M2` \| `M3` |
| `contextid="G1"` | `key: "G1"` |
| `<macro><keystroke>` | `inject.keys` |
| `<macro><multikey>` | `inject.keys` with a sequence |
| `<macro><textblock>` | `inject.text` |
| `<macro><mousefunction>` | `inject.mouse` |
| `<macro><shortcut>` (launch app) | `shell.run` — **flag for consent at import** |
| `devicecategory` (multiple blocks) | filter to the G13's blocks; ignore mouse/keyboard blocks |
| macro `color` attribute | ignored (no per-key RGB on the G13) |

Importer is a standalone `bosunctl profile import-lgs` subcommand: parse XML → resolve `macroguid` → emit `profile.json` → report anything it couldn't map. Target: Kyle's existing profile imports with zero manual fixes.

---

## Appendix D — Source notes

Protocol facts in §2 were measured directly from the attached device on 2026-08-30 via `SetupAPI`/`hid.dll`/`kernel32` from Python 3.13, and cross-checked for consistency against the public behavior of long-standing open-source G13 drivers. No code from those projects is used, and none should be — see R6.

Reference reading (context and prior art, **not** code sources):

- [khampf/g13](https://github.com/khampf/g13) — maintained C++/libusb Linux driver, the most complete prior art
- [RunicLuke/logitech-g13](https://github.com/RunicLuke/logitech-g13) — Python/MIT userspace driver with LCD, web GUI, on-device menu
- [cavefish-dev/g13-driver](https://github.com/cavefish-dev/g13-driver) — Rust/GPL Windows driver; **requires a Zadig WinUSB swap and has no LCD or RGB**, which §2 shows is unnecessary
- [Lordbooker/linux-g13-driver](https://github.com/Lordbooker/linux-g13-driver), [AreteDriver/G13_Linux](https://github.com/AreteDriver/G13_Linux), [g13-linux on PyPI](https://pypi.org/project/g13-linux/) — additional forks
- [OpenAI Codex Micro — Tom's Hardware](https://www.tomshardware.com/peripherals/keyboards/openais-first-hardware-device-is-an-rgb-macropod-codex-micro-features-13-low-profile-keys-and-a-joystick-for-controlling-ai-coding-agents) and [TechCrunch](https://techcrunch.com/2026/07/15/amid-hardware-legal-battle-openai-releases-a-230-keyboard-for-codex/) — the interaction model being cloned
- [Cursor Agent CLI](https://cursor.com/blog/cli) and [CLI docs](https://cursor.com/docs/cli/overview) — headless agent control surface
- [Devin API — send a message](https://docs.devin.ai/api-reference/v1/sessions/send-a-message-to-an-existing-devin-session) — REST adapter shape
- [VS Code commands API](https://code.visualstudio.com/api/extension-guides/command) and [URI handler sample](https://github.com/microsoft/vscode-extension-samples/blob/main/uri-handler-sample/README.md) — the VS Code-family adapter mechanism
- [macOS IOHIDManager permission](https://nachtimwald.com/2020/11/08/macos-iohidmanager-permission-issue/) — the Input Monitoring trap in R2
- [Elgato Stream Deck HID API](https://docs.elgato.com/streamdeck/hid/intro/) — official vendor-HID spec; the Class A case for device #2
- [QMK Raw HID](https://docs.qmk.fm/features/rawhid) — usage page `0xFF60`, the Class C channel
- [The Omarchy Manual](https://learn.omacom.io/2/the-omarchy-manual) — DHH's Super-key-driven Hyprland distro behind the `bosun/omarchy` profile
- [Azeron Software](https://azeron.com/pages/software) — Win/Linux configurator with onboard profiles; the Class B reference
