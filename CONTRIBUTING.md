# Contributing

Read `AGENTS.md` and the accepted ADRs before changing code.

## Required gates

Run before each commit:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

New behavior follows RED-GREEN-REFACTOR. Hardware-only tests must be ignored by default and gated by `BOSUN_HW=1`.

## Licensing and clean-room protocol

Contributions must be compatible with MIT OR Apache-2.0. Do not read, copy, translate, or adapt code from GPL G13 projects, including `khampf/g13` and `cavefish-dev/g13-driver`. Protocol facts in this repository were measured from hardware and are the only implementation source.

No ADR, no merge for architectural changes. Material changes to the locked decisions require owner approval before implementation.
