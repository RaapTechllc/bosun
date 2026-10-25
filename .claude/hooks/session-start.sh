#!/bin/bash
# SessionStart hook for Claude Code cloud sessions: installs what the M1
# gates in CONTRIBUTING.md need (mirrors the Linux job in .github/workflows/ci.yml).
set -euo pipefail

if [ "${CLAUDE_CODE_REMOTE:-}" != "true" ]; then
  exit 0
fi

cd "${CLAUDE_PROJECT_DIR:-$(dirname "$0")/../..}"

# hidapi's linux-static-hidraw backend links libudev.
if ! pkg-config --exists libudev 2>/dev/null; then
  SUDO=""
  if [ "$(id -u)" -ne 0 ]; then SUDO="sudo"; fi
  $SUDO apt-get update -qq || true
  DEBIAN_FRONTEND=noninteractive $SUDO apt-get install -y -qq libudev-dev pkg-config
fi

# rust-toolchain.toml pins stable with clippy and rustfmt.
rustup toolchain install

# Warm the crate cache so the snapshotted container can build offline.
if ! cargo fetch --locked; then
  echo "session-start: cargo fetch failed; if the error names static.crates.io," \
    "add it to the environment's allowed network domains." >&2
fi
