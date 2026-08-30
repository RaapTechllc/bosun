# Agent-controller market watch

This is the standing intake process for user complaints, fixes, and interaction ideas from Codex Micro, ChatGPT/Codex desktop, Work Louder Creator Micro 2, and adjacent agent controllers.

The evidence baseline is [`CODEX-MICRO-COMPLAINTS.md`](CODEX-MICRO-COMPLAINTS.md). New signals are advisory until Kyle explicitly promotes them. A review never expands the current milestone silently; M1 remains G13 transport and hardware verification.

## Cadence

- Weekly until M4 is accepted.
- Monthly after M4 unless launch activity or a material competitor release justifies a temporary weekly watch.
- Report only material deltas. A no-change sweep stays silent.
- Re-check open defects for fixes as well as collecting new complaints.

## Sources

### Official product and software

- OpenAI Codex Micro documentation and product page.
- ChatGPT/Codex changelog.
- Work Louder Codex Micro setup guidance.
- Work Louder Creator Micro 2 firmware releases.

### Defects

- `openai/codex` issues mentioning Codex Micro, Work Louder, HID discovery, device freezes, or disconnects.
- Track the baseline issue family around `#33606`, `#33912`, `#34327`, and `#34099`, including linked siblings and closure/fix notes.

### OEM base hardware

- Work Louder feedback concerning device detection, charging, firmware updates, encoder behavior, Bluetooth/USB transitions, and Codex RPC compatibility.
- Label these **OEM** findings. Creator Micro 2 behavior is not automatically a Codex Micro defect and never becomes G13 M1 work.

### Market and first-hand use

- Follow-up hands-on reviews from PCMag, Kingy AI, and other identifiable owners.
- Hacker News, Reddit, YouTube, and product forums when a post contains first-hand use or links to reproducible evidence.
- Treat comments, search snippets, and unattributed summaries as discovery leads rather than proof.

## Intake record

Use `.github/ISSUE_TEMPLATE/market-signal.md` for anything worth retaining. Every record includes:

- discovery date and source URL;
- product surface: hardware, app/integration, OEM firmware, or general interaction idea;
- complaint, fix, praised behavior, or requested idea;
- evidence level: `new`, `repeated`, `OEM repeated`, `anecdote`, or `design fact`;
- current status: open, fixed, regressed, unclear, or not applicable;
- proposed Bosun milestone and existing requirement, if any;
- disposition: `watch`, `issue`, or `plan-change`.

Dedupe against the baseline brief and prior market-signal issues. A changed status on an existing defect is a material delta.

## Promotion rules

- `watch`: retain evidence; no implementation work.
- `issue`: create a scoped backlog item only when the behavior maps to an accepted milestone and has a checkable acceptance condition.
- `plan-change`: show Kyle the exact old/new scope, evidence, cost, and rollback, then stop for approval.
- A single anecdote can motivate a test hypothesis, but not a product claim.
- Locked decisions in `docs/BOSUN-PLAN.md` are not reopened by popularity alone.

## Copy strengths as well as fixing defects

Continue watching for improvements to:

- glanceable agent state;
- single-tap selection versus double-tap foregrounding;
- physical approve/decline and interruption;
- reasoning-effort controls;
- joystick workflow discovery;
- remappable, inspectable layers.

Record praised behavior with the same evidence discipline as complaints.

## Bosun guardrails reinforced by the baseline

- One canonical profile model serves CLI, GUI, importer, and registry. ChatGPT settings versus Work Louder Input is a competitor split-brain defect, not a pattern to copy.
- HID discovery and I/O must never block a GUI or control-plane thread.
- RGB state must have a text or glyph equivalent.
- Adapter version or capability mismatch must fail once with an actionable state, not reconnect forever.
- Device, adapter, and UI paths must degrade and recover independently.

## Hard exclusions

Do not promote these without new, direct evidence:

- key chatter;
- capacitive-sensor false triggers;
- a Codex Micro joystick deadzone defect;
- accidental radial-sector firing;
- the `$1,850` resale figure;
- claims that Codex Micro has an on-device radial menu. Official behavior is four thresholded stick directions.

Do not copy source code or documentation from third-party controller projects. Protocol observations may inform clean-room work only when independently measured and permitted by the repository's licensing rules.

## Sweep output

A material weekly report should contain only:

1. **New or changed signal** — one sentence plus source.
2. **Evidence strength** — including whether it is first-hand and repeated.
3. **Bosun relevance** — existing milestone/requirement or genuinely new idea.
4. **Disposition** — `watch`, `issue`, or `plan-change`.
5. **What changed since the prior sweep** — no replay of the whole baseline.
