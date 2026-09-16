# Domain Docs

How the engineering skills should consume this repo's domain documentation.

## Before exploring, read these

- **`CONTEXT.md`** at the repo root
- **`AGENTS.md`** — locked M1 contract; wins on conflict
- **`docs/PRD.md`** §5 — acceptance criteria
- **`docs/BOSUN-PLAN.md`** §2 — measured protocol facts
- **`docs/adr/`** — accepted ADRs in the area you are touching

If a listed file does not exist, proceed silently.

## File structure

Single-context repo: one `CONTEXT.md` plus `docs/adr/` at the repo root.

## Use the glossary's vocabulary

When output names a domain concept, use the term as defined in `CONTEXT.md`. Do not drift to synonyms the glossary avoids.

## Flag ADR conflicts

If output contradicts an existing ADR, surface it explicitly rather than silently overriding.
