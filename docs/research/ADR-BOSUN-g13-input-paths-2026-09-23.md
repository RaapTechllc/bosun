# ADR-BOSUN-2026-09-23 — G13 input paths: proprietary HID vs LGS keystrokes vs Omarchy profile

**Status:** Research note (not a build plan; no code)  
**Date:** 2026-09-23 (America/Chicago)  
**Audience:** Chief of Staff (copy); Kyle reviewing Bosun after AC-R1  
**Repo:** `RaapTechllc/bosun` (public; dual MIT OR Apache-2.0)  
**Context:** AC-R1 measured **fail both** on 2026-09-20 (`docs/hardware-notes.md`): 0 input reports on the vendor usage-page path `0xFF00`, with LGS/LCore running and with LCore stopped. Kyle now reports that pressing G13 buttons produces keystrokes that behave like a number pad. Goal is **not** remapping G-buttons into WASD for games; keep proprietary G-controls as Bosun→agent commands, plus an optional profile that drives **Omarchy** Super-key bindings.

---

## Short answer

1. **Layout:** Official Logitech count is **22 G-keys (G1–G22)**, not G1–G29; plus **3 M-keys (M1–M3)**, **MR**, **4 LCD soft keys (L1–L4)**, backlight/BD controls, a **mini-joystick** with adjacent thumb buttons, and a **160×43 monochrome LCD**. The large lower surface is a **palm / wrist rest**, not a capacitive mouse pad.  
2. **HID:** The G13’s measured report descriptor is **one vendor-defined collection on Usage Page `0xFF00`**. Stock OS HID does **not** turn G-button presses into Keyboard/Keypad (`0x07`) or Consumer (`0x0C`) events. Numpad-like keystrokes are almost certainly **LGS (or another injector) synthesizing mapped keys**, not a second hardware keyboard interface carrying distinct G1…Gn IDs.  
3. **Distinguish G1 from keypad “1”:** **Not on the standard keyboard stream.** If the OS only sees Keyboard/Keypad Page usages (e.g. Usage ID `0x59` = Keypad 1), G1 and a real keypad 1 are the same key. Distinction requires reading the **vendor `0xFF00` input report** (Bosun’s planned path) — which AC-R1 failed to observe.  
4. **Community:** Prefer **MIT** userspace projects (and Bosun’s own measured protocol). Avoid **GPL** G13 drivers and kernel `hid-lg-*` as implementation sources (already banned in-repo). Many Linux tools still use **libusb**, which conflicts with Bosun ADR-0001 (stock HID / hidapi only).  
5. **Omarchy:** **Real** (DHH / Omacom; Arch + Hyprland “agentic” distro). A Bosun **key-emitter profile** that types Super+chords is a **sensible secondary mode**; keep it separate from the proprietary→agent profile.

---

## 1) Physical layout (sourced)

### Official Logitech specifications (VERIFIED 2026-09-23)

Source: Logitech Support, “G13 Technical Specifications”  
https://support.logi.com/hc/en-sg/articles/360023467053-G13-Technical-Specifications  

| Spec | Value |
|------|--------|
| USB VID_PID | `VID_046D&PID_C21C` |
| G-Keys | **22** |
| M-Keys | **3** |
| LCD | Monochrome, non-adjustable tilt |
| Mini-Joystick | Yes |
| Connection | Corded Full Speed USB 2.0 |
| Gameboard size | 171 × 243 × 38 mm; ~600 g |

### Official user guide cover photo (VERIFIED 2026-09-23)

Logitech/Logicool PDF: https://download01.logitech.com/web/ftp/pub/pdf/G13_ug_web.pdf  
(Fetched; Illustrator image pages; cover shows top-down device with LCD at top, G-key grid, thumbstick cluster on the right, large palm rest at bottom. Text extraction empty; layout confirmed visually from rendered page-1.png.)

### Community / ArchWiki naming diagram (VERIFIED 2026-09-23; GFDL wiki, used for labeling only)

https://wiki.archlinux.org/title/Logitech_G13  

Approximate layout (ArchWiki ASCII; note a typo “G10” in the fourth row in the wiki — physical fourth row is G20–G22):

```
     |  160x43 LCD  |
     BD L1 L2 L3 L4 LIGHT_STATE
        M1 M2 M3 MR
G1  G2  G3  G4  G5  G6  G7
G8  G9  G10 G11 G12 G13 G14
   G15  G16 G17 G18  G19
       G20  G21  G22
Joystick + TOP / LEFT / DOWN (+ stick click)
```

### Bosun in-repo capability table (measured protocol; authoritative for this seat)

`docs/BOSUN-PLAN.md` §2 (repo tip checked 2026-09-23) and `devices/logitech-g13.toml`:

| Control class | Names / count |
|---------------|----------------|
| Programmable macro keys | `G1`–`G22` (22) |
| LCD soft keys | `L1`–`L4` (4) |
| Mode / record | `M1`, `M2`, `M3`, `MR` |
| Thumb cluster | `TOP`, `LEFT`, `DOWN` (physical map of names still pending per `hardware-notes.md`) |
| Other bits named in descriptor | `BD`, backlight-related |
| Axes | `stick-x`, `stick-y` (bytes 1–2 of input report) |
| Screen | 160×43 visible, 1bpp bands, output report id 3 |

**Marketing “25 programmable keys”** = 22 G-keys + 3 thumb buttons (plan §2).  
**Kyle’s “G1–G29”:** **not** the official G-key labeling. Total named discrete controls in the Bosun descriptor is on the order of **~34**, including LCD soft keys and mode keys — still not a G29 row silk-screen.

**“Mouse-pad-like surface”:** Official materials and the user-guide photo describe a contoured **palm / wrist rest**, not a trackpad. Pointing is the **mini-joystick** (+ adjacent buttons), not a flat capacitive pad.

---

## 2) HID interfaces and which path carries G-buttons

### USB HID Usage Tables (cite)

USB-IF, *HID Usage Tables* Version 1.5, PDF `hut1_5.pdf` (fetched 2026-09-23 from https://www.usb.org/sites/default/files/hut1_5.pdf ):

| Usage Page ID | Name | Relevance |
|---------------|------|-----------|
| `0x01` | Generic Desktop | Mouse / joystick / gamepad collections |
| `0x07` | Keyboard/Keypad | Includes **Keyboard 1** (Usage `0x1E`) and **Keypad 1 and End** (Usage `0x59`) |
| `0x0C` | Consumer | Media / consumer-control |
| `0xFF00`–`0xFFFF` | **Vendor-defined** | Device-specific reports; OS does not invent standard keyboard semantics |

### Measured G13 descriptor (Bosun Phase 0 — VERIFIED in-repo)

From `docs/BOSUN-PLAN.md` §2.2 (Windows live read) and ADR-0001 (`docs/adr/0001-hid-stack-only.md`, Accepted 2026-08-30):

- Device bound to Microsoft stock **`HidUsb`** (not a Logitech filter driver).  
- Report descriptor: **Usage page `0xFF00` (vendor-defined)**, usage `0x0000`, one link collection.  
- **INPUT** report id **1**: 8 bytes total (report ID + 7 data) — button bitmap + stick axes.  
- **OUTPUT** report id **3**: LCD framebuffer (992 bytes).  
- **FEATURE** reports 4–7: LEDs / RGB / other.  

**Plan quote (critical):** because the usage page is vendor-defined (`0xFF00`), **“the OS generates no keyboard or mouse events from this device. When LGS is removed, an unconfigured G13 produces zero system input.”**

Historical corroboration (do **not** treat as Bosun implementation source; GPL/kernel context):

- LWN “hid Logitech G13 Driver 0.0.5” (2010): device identifies as HID but **“does not support standard HID input messages”**; a custom input device was synthesized in-driver. https://lwn.net/Articles/376851/  
- Modern mainline work extending `hid-lg-g15` for G13 (LKML / lore, 2025–2026): looks for application collection `0xff000000`, connects **hidraw**, exposes synthetic “Gaming Keypad” + “Thumbstick” input devices — i.e. **kernel translation of vendor reports**, not a stock Keyboard Page hardware interface. (Kernel sources are GPL; Bosun must not copy them — PRD R6 / AC-12.)

### Implication for Kyle’s numpad observation

| Hypothesis | Fit to sources |
|------------|----------------|
| **A. Hardware second interface** that emits Keyboard/Keypad usages for G-keys | **Poor fit.** Measured descriptor is vendor-only; unconfigured G13 → zero OS keys. |
| **B. LGS (or similar) maps G-keys → keystrokes and injects them** | **Best fit.** Explains numpad-like behavior **while AC-R1 saw 0 vendor reports** on the `0xFF00` path Bosun opened. |
| **C. G15-style separate F-key keyboard device** | **G15 family**, not G13. Kernel commentary on G15 disables F1–F12 keyboard emulation separately; G13 is described as macropad + LCD + joystick. |

**Verdict:** G-button **identity** lives in the **vendor input report bitmap**. Standard keycodes (including Keypad 1–9) are a **downstream mapping layer**, typically LGS profiles — not the native wire format for distinguishing G1 from G2.

---

## 3) Can stock-OS-HID-only distinguish G1 from keypad “1”?

Bosun constraint (ADR-0001): **stock HID stack only** (`hidapi` / Windows HID / IOHIDManager / Linux hidraw). No libusb, WinUSB, Zadig, kext, or kernel module.

| Listening path | Can distinguish G1 vs keypad “1”? | Notes |
|----------------|-----------------------------------|--------|
| OS keyboard / Raw Input on **Keyboard/Keypad Page `0x07`** | **No** | Same Usage ID (e.g. Keypad 1 = `0x59`) whether produced by a real keypad or by LGS injection. |
| Consumer Control `0x0C` | **No** (wrong semantic) | Media keys, not G-key identity. |
| Vendor collection **`0xFF00` input report id 1** | **Yes**, in principle | Bits map to named G1…G22 / L* / M* / thumb controls per Bosun codec. |
| AC-R1 result (2026-09-20) | **Did not observe reports** | Fail both with LGS up and LCore stopped on the enumerated `usage_page=ff00` path. So **today** Bosun cannot rely on distinction until the empty-read failure is understood. |

**Practical reading for Kyle’s goal (“proprietary buttons → agent commands”):**

- Treating the **numpad keystream as input** cannot meet the goal: collisions with real typing, no stable G-identity, fights LGS mappings, and violates the “not WASD remap” product intent.  
- The intended path remains **vendor HID reports**. AC-R1 fail-both is still the blocker; the new observation does **not** prove a second hardware keyboard for G-keys — it suggests **investigate LGS injection + why ReadFile on `0xFF00` returned silence** (handle/sharing, report-ID filtering, wrong sibling path, device sleep, or measurement window). Research only here — no redesign hop claimed.

Windows note from Phase 0: shared open with LGS worked for RGB/LCD feature/output writes; exclusive open failed sharing violation. Input multi-reader behavior was the open risk **R1** and is what AC-R1 measured as fail-both.

---

## 4) Community approaches without Logitech software (license-aware)

Bosun policy: dual **MIT OR Apache-2.0**; **no GPL** G13 source (`CONTRIBUTING` / PRD R6 / AC-12). Prefer documentation of **licenses and access model**, not recipes.

| Project / class | License (as stated publicly) | Access model | Notes for Bosun |
|-----------------|------------------------------|--------------|-----------------|
| **Bosun measured protocol** (`BOSUN-PLAN.md` §2) | MIT OR Apache-2.0 | Stock HID / hidapi | **Authoritative** for this seat. |
| `jonas-werner/g13-configurator` | MIT (project/docs 2026) | Userspace; pyusb claim/detach often | Useful as existence proof; **libusb-oriented** — conflicts with ADR-0001 if copied as transport. |
| `g13-linux` / Arete G13_Linux (PyPI) | MIT | hidapi **or** libusb backends; udev | Permissive; still not a protocol source to paste; prefer measure. |
| `RunicLuke/logitech-g13`, `golgote/G13` | MIT (GitHub metadata cited in prior Bosun research notes) | Userspace | Prior art; clean-room rule still applies. |
| `cavefish-dev/g13-driver` | GPL-3 | Often WinUSB/Zadig Class D | **Hard avoid.** |
| `khampf/g13` and forks / AUR `g13-git` | Mixed / treat as contaminated | libusb daemon `g13d` → uinput | **Do not consult source** (in-repo ban). |
| Mainline `hid-lg-g15` + G13 patches | GPL-2 kernel | Kernel translates vendor → EV_KEY | Interesting for Linux *users* on 6.19+, **forbidden as Bosun code source.** |

**Windows without Zadig:** Phase 0 already showed stock `HidUsb` + userspace HID APIs reach LCD/RGB/LEDs. Input remains the open question after AC-R1. Class D driver swaps are explicitly refused in Bosun planning.

---

## 5) Omarchy profile angle

### Is Omarchy real?

**Yes (VERIFIED 2026-09-23).**

- Site: https://omarchy.org/ — “Beautiful, fun & agentic Linux by DHH”; Arch-based opinionated distribution with Hyprland.  
- Manual: https://omarchy.org/manual/ ; hotkeys https://omarchy.org/manual/hotkeys/ ; navigation https://omarchy.org/manual/navigation/  
- Announcement: DHH, “Omarchy is out,” 2025-06-26, https://world.hey.com/dhh/omarchy-is-out-4666dd31  
- GitHub: `omacom/omarchy` (and historical `basecamp/omarchy` references); MIT. Companion research: `/workspace/rails-research/bosun-src-omarchy.md`.

Navigation doctrine: “Everything in Omarchy happens via the keyboard — EVERYTHING!” Super is the chord root (`Super + Space` menu, `Super + Return` terminal, `Super + K` binding cheat-sheet, etc.).

### Is Bosun-as-key-emitter a sensible profile?

**Yes, as a secondary profile mode** — with clear separation from agent-command mode.

| Mode | Role | Sensible? |
|------|------|-----------|
| **Agent profile** | G13 vendor events → Bosun/agent commands (approve, switch agent, status on LCD) | Primary product thesis; needs working `0xFF00` input. |
| **Omarchy profile** | G13 vendor events → emit Super+chords Omarchy already defines | Sensible: reuses 200+ bindings; matches keyboard-first OS; community precedent **controller-control** plugin maps gamepad → uinput Super chords (https://github.com/perminder-klair/omarchy-controller-control, 2026-08). |

Caveats (sourced):

- User-edited `~/.config/hypr/bindings.lua` can desync emitted chords.  
- Wayland injection needs seat permissions (`uinput` / input group) — same class of problem as other emitters; **out of M1** per current Bosun scope (no injection in M1).  
- Key-emitter mode is **not** a substitute for fixing AC-R1; it still needs proprietary G-identity upstream of the emitter.

---

## Ranked recommendation (research only)

1. **Do not** treat LGS numpad/keyboard injection as the Bosun G-button API. It cannot distinguish G1 from keypad 1 and fights the proprietary→agent goal.  
2. **Keep** ADR-0001 stock-HID vendor-page strategy; treat Kyle’s new observation as evidence that **LGS is alive on the keyboard plane**, not that the hardware grew a Keyboard Page for G-keys.  
3. **Before any redesign:** re-instrument AC-R1-class measurement with explicit checks — report ID 1 reads, path enumeration of all HID collections for `046D:C21C`, LGS process state, and whether injected keys appear on a **different** device node than the `0xFF00` path. (Measurement plan only; no implementation in this note.)  
4. **Correct vocabulary** in product copy: G1–G22 (+ L/M/thumb), not G1–G29; palm rest ≠ mouse pad.  
5. **Omarchy:** approve as a **named profile** later (post-M1), key-emitter style; keep agent profile as default thesis.  
6. **Licenses:** continue MIT/Apache-only; do not pull GPL G13 or kernel `hid-lg-*` into the tree.

---

## Sources (access 2026-09-23 unless noted)

1. Logitech — G13 Technical Specifications: https://support.logi.com/hc/en-sg/articles/360023467053-G13-Technical-Specifications  
2. Logitech/Logicool — G13 User Guide PDF: https://download01.logitech.com/web/ftp/pub/pdf/G13_ug_web.pdf  
3. USB-IF — HID Usage Tables 1.5: https://www.usb.org/sites/default/files/hut1_5.pdf (Usage Page summary incl. `0x07`, `0x0C`, `0xFF00–FFFF`; Keyboard/Keypad usages incl. Keypad 1 = `0x59`)  
4. `RaapTechllc/bosun` — `docs/BOSUN-PLAN.md` §2 measured protocol; `docs/adr/0001-hid-stack-only.md`; `docs/hardware-notes.md` AC-R1; `devices/logitech-g13.toml`; `docs/PRD.md` AC-R1 / R1 / R6  
5. ArchWiki — Logitech G13: https://wiki.archlinux.org/title/Logitech_G13  
6. LWN — hid Logitech G13 Driver 0.0.5: https://lwn.net/Articles/376851/  
7. Omarchy — https://omarchy.org/ ; hotkeys / navigation manuals; DHH Hey post 2025-06-26  
8. Companion notes — `/workspace/rails-research/bosun-src-omarchy.md`; prior seat notes `/workspace/bosun-research-notes.md`

## Non-goals

No code, no HID redesign hop, no libusb/Zadig recommendation, no GPL source consultation, no spend, no keys.
