#!/usr/bin/env bash
# Build de release otimizada para desempenho (alvo Windows x64, via cargo-xwin).
#
# Diferenças em relação ao build.sh (perfil diag):
#   - sem a feature `diagnostics` (sem camada de debug D3D12, DRED, timing de GPU
#     nem logs de depuração no código);
#   - LTO "fat" e uma única unidade de código (codegen-units=1), para melhor
#     inlining e otimização entre crates. Leva mais tempo para compilar.
#
# Variáveis opcionais:
#   VICTORY_RELEASE_LTO=thin|fat|off   (padrão: fat)
#   VICTORY_RELEASE_CGU=N              (padrão: 1; use 16 para compilar mais rápido)
#   CARGO_JOBS=N                       (padrão: 10, máximo recomendado nesta máquina)
# Os argumentos extras são repassados ao cargo-xwin.
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

lto=${VICTORY_RELEASE_LTO:-fat}
case $lto in
    off) lto=false ;;
    fat|thin) ;;
    *) echo "VICTORY_RELEASE_LTO inválido: $lto (use thin, fat ou off)" >&2; exit 2 ;;
esac
cgu=${VICTORY_RELEASE_CGU:-1}
jobs=${CARGO_JOBS:-10}

export CARGO_PROFILE_RELEASE_LTO=$lto
export CARGO_PROFILE_RELEASE_CODEGEN_UNITS=$cgu
# O release já usa opt-level 3 por padrão; símbolos de depuração só ocupam espaço.
export CARGO_PROFILE_RELEASE_DEBUG=${CARGO_PROFILE_RELEASE_DEBUG:-line-tables-only}

echo "Build release: LTO=$lto, codegen-units=$cgu, jobs=$jobs"
"$cargo_xwin" xwin build --locked --release \
    -j "$jobs" -p victory --target x86_64-pc-windows-msvc "$@"

exe="$root/target/x86_64-pc-windows-msvc/release/victory.exe"
[[ -f $exe ]] && echo "Pronto: $exe"
