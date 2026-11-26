#!/usr/bin/env bash
# Apaga os caches derivados do VIctory (shaders traduzidos, imagens e pipelines
# do vkd3d-proton) para que tudo seja recriado do zero na próxima execução.
#
# Não toca em: saves do jogo (AppData\Local\victory\data), logs, o prefixo do
# Proton, nem os arquivos do pacote do jogo.
#
# Uso: ./clear_shaders.sh          (pede confirmação)
#      ./clear_shaders.sh --yes    (sem confirmação)
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
prefix=${STEAM_COMPAT_DATA_PATH:-"${XDG_DATA_HOME:-$HOME/.local/share}/cool_fox/proton"}
cache="$prefix/pfx/drive_c/users/steamuser/AppData/Local/victory/cache"

targets=(
    "$cache/shaders"
    "$cache/images"
    "$root/vkd3d-proton.cache"
    "$root/vkd3d-proton.cache.write"
)

if pgrep -f 'victory\.exe' >/dev/null 2>&1; then
    echo "O VIctory está em execução. Feche o jogo antes de limpar os caches." >&2
    exit 1
fi

existing=()
for t in "${targets[@]}"; do
    [[ -e $t ]] && existing+=("$t")
done
if (( ${#existing[@]} == 0 )); then
    echo "Nada a apagar."
    exit 0
fi

echo "Serão apagados:"
for t in "${existing[@]}"; do
    echo "  $t ($(du -sh -- "$t" | cut -f1))"
done

if [[ ${1:-} != --yes ]]; then
    read -r -p "Continuar? [s/N] " answer
    [[ $answer == [sS] ]] || { echo "Cancelado."; exit 1; }
fi

for t in "${existing[@]}"; do
    rm -rf -- "$t"
done
echo "Caches removidos. Serão recriados na próxima execução."
