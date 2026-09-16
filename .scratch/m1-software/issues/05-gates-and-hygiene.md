# 05: CI gates, license hygiene, Windows MSVC

**What to build:** Format, clippy with warnings denied, workspace tests, and cargo-deny are green without `BOSUN_HW=1`. Hardware tests stay ignored and skip unless that gate is set. Windows CI asserts the MSVC host triple. No GPL identifiers.

**Blocked by:** 01, 02, 03, 04

**Status:** ready-for-agent

- [ ] `cargo fmt --all -- --check` (AC-11)
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` (AC-11)
- [ ] `cargo test --workspace --all-features` (AC-11)
- [ ] `cargo deny check` (AC-12)
- [ ] Ignored hardware tests skip with a clear message unless `BOSUN_HW=1`
- [ ] Windows MSVC expectation documented; `windows-latest` asserts `x86_64-pc-windows-msvc` (AC-13)
- [ ] `cargo tree -p bosun-hid` has no Tokio (AC-3)
