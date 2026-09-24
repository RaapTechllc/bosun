# Bosun profile concepts

**Status:** brainstorm capture (not shipped; no product code)  
**Date:** 2026-09-23 (America/Chicago)  
**Audience:** Kyle; Chief of Staff; Spec Desk / Dev Bay after owner locks  
**Repo:** `RaapTechllc/bosun`

Kyle’s named profile ideas for how G13 G-keys should behave after M1. This doc records intent and open decisions only. It does not change M1 scope or HID design.

## Input paths (research lock, 2026-09-23)

Folded from `docs/research/ADR-BOSUN-g13-input-paths-2026-09-23.md` (and in-repo measured protocol in `docs/BOSUN-PLAN.md` / ADR-0001):

| Fact | Implication for profiles |
|------|--------------------------|
| G13 report descriptor is **vendor Usage Page `0xFF00` only** | Stock OS does **not** emit Keyboard/Keypad events from G-keys. Proprietary G-identity lives on **vendor input reports**. |
| Numpad-like keystrokes under LGS | Almost certainly **LGS (or another injector) synthesizing mapped keys**, not a second hardware keyboard carrying distinct G1…Gn IDs. |
| Distinguishing G1 from keypad “1” | **Only** via the vendor `0xFF00` report. Keyboard-page stream cannot tell them apart. |
| AC-R1 (2026-09-20) | **Fail both** (0 reports with LGS up and LCore stopped) on the measured `0xFF00` path — still the blocker for any proprietary→command profile. |
| **Omarchy** | **Real** (DHH / Omacom; Arch + Hyprland). Keyboard-first Super chords. |
| Key-emitter mode | Sensible **secondary** mode (emit Super+chords / snippets). Keep separate from proprietary→agent-command mode. Still needs working G-identity upstream. |
| Layout vocabulary | Official: **G1–G22** (+ M1–M3, MR, L1–L4, thumb cluster, stick). Not G1–G29. Palm rest ≠ mouse pad. |

**Do not** treat LGS numpad injection as the Bosun G-button API. Profiles below assume eventual vendor-report readout (post AC-R1 resolution) unless noted as key-emitter-only.

---

## Profiles

### 1. Windows-Omarchy profile

**Purpose:** Recreate the Linux-style Omarchy Super-key workflow on Windows using the G13 — a keyboard-first Windows dev setup.

**Mode:** Key-emitter (secondary). G-keys fire Super-chords (or Windows-equivalent chords) to open terminal, browser, tiling actions, etc.

**Example mappings (illustrative):**

| G-key | Action |
|-------|--------|
| G1 | Super+Return analogue → terminal |
| G2 | Super+Shift+Return analogue → browser |
| G3 | Focus / tile next |
| G4 | Launch app launcher / Start search |
| G5–G8 | Workspace / desktop switch |

**Inputs:** Proprietary G-identity from vendor `0xFF00` → Bosun emits OS key chords. Not “listen to LGS numpad.”

**Open questions:**

- Which Windows chord set maps closest to Omarchy (PowerToys FancyZones, Win+…, third-party tiling)?
- Is injection in-scope for M2 on Windows, or wait for a later milestone?
- Does this profile share chord tables with native Omarchy, or diverge per OS?

---

### 2. Omarchy profile

**Purpose:** Native Super-key bindings for Omarchy (Arch + Hyprland). Bosun as a **key-emitter**, separate from agent commands.

**Mode:** Key-emitter (secondary).

**Example mappings (illustrative; match Omarchy defaults where possible):**

| G-key | Action |
|-------|--------|
| G1 | Super+Return → terminal |
| G2 | Super+Shift+Return → browser |
| G3 | Super+Space → menu |
| G4 | Super+K → binding cheat-sheet |
| G5–Gn | Navigation / tiling chords from Omarchy hotkey docs |

**Inputs:** Vendor G-events → emit Super+chords Omarchy already defines. Do not conflate with agent-command profile.

**Open questions:**

- Pin to stock Omarchy bindings vs follow user-edited `~/.config/hypr/bindings.lua` (desync risk)?
- Wayland seat / `uinput` permissions — who owns setup docs?
- Profile switch UI: M-keys? LCD soft keys? Config file only?

---

### 3. Codex profile

**Purpose:** G-keys mapped to Codex CLI actions / keypad-style shortcuts (start session, run tests, commit, etc.).

**Mode:** Agent-command (primary thesis) and/or snippet/CLI trigger — TBD.

**Example mappings (illustrative):**

| G-key | Action |
|-------|--------|
| G1 | Start / resume Codex session |
| G2 | Run tests |
| G3 | Stage + commit (with confirm) |
| G4 | Ask Codex to explain selection |
| G5 | Abort / interrupt current turn |

**Inputs:** Prefer proprietary G-keys → Bosun commands that invoke Codex CLI. Key-emitter only if Codex is driven purely by typed shortcuts in a focused terminal.

**Open questions:**

- Exact Codex CLI surface (flags, TUI vs headless) as of M2?
- Confirm gates for destructive actions (commit, push)?
- Shared vs exclusive with Grokbot / agent profile?

---

### 4. Grokbot / agent profile

**Purpose:** G-keys mapped to common agent prompts or actions (e.g. “review this diff”, “explain this function”).

**Mode:** Agent-command (primary product thesis).

**Example mappings (illustrative):**

| G-key | Action |
|-------|--------|
| G1 | “Review this diff” |
| G2 | “Explain this function” |
| G3 | Approve / continue |
| G4 | Switch active agent / room |
| G5 | Status snapshot → LCD |

**Inputs:** Vendor `0xFF00` G-identity → Bosun/agent commands. LCD for status. This is the default thesis; key-emitter profiles must not replace it.

**Open questions:**

- Which agent runtime(s) first (Grok Bot, Cursor, Codex, multi)?
- Prompt pack storage format (TOML next to device descriptor)?
- M1 already blocked on AC-R1 — does M2 assume AC-R1 fixed?

---

### 5. Master-prompt-on-a-key

**Purpose:** A saved starter prompt (e.g. “this is the prompt I send when I start a new project”) bound to one G-key; press it, full prompt is inserted. Snippet-expander pattern.

**Mode:** Key-emitter / paste-injector (secondary). Kyle flagged it as possibly weird; capture for the record.

**Example mappings (illustrative):**

| G-key | Action |
|-------|--------|
| G22 (or user-chosen) | Insert / paste master project-start prompt |
| Hold+G22 | Edit / show prompt on LCD (stretch) |

**Inputs:** Proprietary G-key → emit or clipboard-paste a stored string into the focused field. Not an agent command unless wired that way later.

**Open questions:**

- Clipboard paste vs synthetic keystrokes vs agent inbox?
- Multi-prompt bank (M1/M2/M3 mode keys select bank)?
- Security: prompts may contain secrets — local-only storage?

---

## Decisions for Kyle

1. **Which profiles ship in M2?** Candidates: Grokbot/agent (core), Omarchy and/or Windows-Omarchy (key-emitter), Codex, master-prompt. Recommend: agent profile first if AC-R1 is unblocked; at most one key-emitter profile in M2.
2. **Do key-emitter and agent-command modes coexist?** Research recommendation: **yes, as named profiles**, not as one mixed stream. Default stay agent-command; Omarchy / Windows-Omarchy / master-prompt are secondary modes.
3. **Does master-prompt belong in M2 or later?** Default recommendation: **later** unless Kyle wants snippet-expander as an early demo of key-emitter without full Omarchy chord tables.

Until those three are locked, treat this file as brainstorm only — do not invent ACs or product code from it.

## Related

- `docs/PRD.md` — M1 ACs; AC-R1 / R1 stop
- `docs/BOSUN-PLAN.md` — measured HID protocol
- `docs/adr/0001-hid-stack-only.md` — stock HID only
- `docs/hardware-notes.md` — AC-R1 measurement
- `docs/research/ADR-BOSUN-g13-input-paths-2026-09-23.md` — G13 input-path research
