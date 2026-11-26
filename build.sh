#!/usr/bin/env bash
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
cd -- "$root"

cargo_xwin="${CARGO_HOME:-$HOME/.cargo}/bin/cargo-xwin"
if [[ ! -x $cargo_xwin ]]; then
    if ! cargo_xwin=$(command -v cargo-xwin); then
        echo 'cargo-xwin não encontrado. Instale-o com: cargo install --locked cargo-xwin' >&2
        exit 1
    fi
fi

exec "$cargo_xwin" xwin build --locked --profile diag \
    -j 10 -p victory --features diagnostics --target x86_64-pc-windows-msvc "$@"
