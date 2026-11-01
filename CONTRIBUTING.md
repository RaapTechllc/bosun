# Contributing

Read `AGENTS.md` and the accepted ADRs before changing code.

## Required gates

Run before each commit:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

New behavior follows RED-GREEN-REFACTOR. Hardware-only tests must be ignored by default and gated by `BOSUN_HW=1`. Running `cargo test -- --ignored` without that variable must skip with a clear message and pass.

## Windows MSVC toolchain (AC-13)

Hardware runs and `windows-latest` CI use the MSVC rustup toolchain. Before any live G13 session, check:

```text
rustc -vV
```

The host line must be `x86_64-pc-windows-msvc`. On Kyle's workstation, `C:\Users\Kyle\.cargo\bin` must precede Chocolatey's Rust shims. If a GNU `cargo` wins PATH, use `rustup run stable cargo ...` instead. CI's `windows-latest` job asserts this host triple.

## Licensing and clean-room protocol

Contributions must be compatible with MIT OR Apache-2.0. Do not read, copy, translate, or adapt code from GPL G13 projects, including `khampf/g13` and `cavefish-dev/g13-driver`. Protocol facts in this repository were measured from hardware and are the only implementation source.

No ADR, no merge for architectural changes.

## Merging

Agents decide and proceed, noting assumptions in the PR. Agents may merge once tests and CI pass and reviews are done: merge commits only, no rebase, squash, or force-push. Kyle's only manual step is production deploy. Escalate to Kyle: sends in his name, spending money, live trading beyond caps, deletions, production deploy, credentials/secrets, client data.
