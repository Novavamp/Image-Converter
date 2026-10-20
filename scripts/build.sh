#!/usr/bin/env sh
set -eu
export SYSTEM_DEPS_DAV1D_BUILD_INTERNAL=auto
cargo build --release --workspace
printf '%s\n' 'Desktop app: target/release/pixelforge-desktop' 'CLI:         target/release/pixelforge'

