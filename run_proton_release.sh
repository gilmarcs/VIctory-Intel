#!/usr/bin/env bash
# Executa a build de release (gerada por build_release.sh) com o Proton, sem a
# sobrecarga dos diagnósticos. Para depurar, use run-proton.sh (build diag).
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
victory=${VICTORY_EXE:-"$root/target/x86_64-pc-windows-msvc/release/victory.exe"}
game=${1:-"/run/media/gilmar/8CEE31B4EE319802/GTA VI/GTA6.Xbox.v1.0.2.1-CYBERLEEK/game_scarlett_master.exe"}
if [[ $game == --help || $game == -h ]]; then
    echo "Uso: $0 [/caminho/para/game_scarlett_master.exe]"
    echo "Sem argumento, usa o executável padrão configurado no script."
    echo "Opcional: VICTORY_EXE=/caminho/victory.exe PROTON_PATH=/caminho/Proton STEAM_DIR=/caminho/Steam"
    echo "Os diagnósticos ficam desligados; as variáveis VICTORY_* e VKD3D_* do ambiente têm prioridade."
    exit 0
fi
if (( $# > 1 )); then
    echo "Informe no máximo um executável do jogo." >&2
    exit 2
fi
[[ -f $victory ]] || { echo "VIctory (release) não encontrado: $victory. Compile com ./build_release.sh" >&2; exit 1; }
[[ -f $game ]] || { echo "Executável do jogo não encontrado: $game" >&2; exit 1; }
game=$(realpath -- "$game")

steam_dir=${STEAM_DIR:-}
if [[ -z $steam_dir ]]; then
    for dir in "$HOME/.local/share/Steam" "$HOME/.steam/steam" "$HOME/.var/app/com.valvesoftware.Steam/data/Steam"; do
        if [[ -d $dir/steamapps ]]; then steam_dir=$dir; break; fi
    done
fi
[[ -n $steam_dir && -d $steam_dir ]] || { echo 'Steam não encontrada. Defina STEAM_DIR.' >&2; exit 1; }

proton=${PROTON_PATH:-}
if [[ -z $proton ]]; then
    for dir in "$steam_dir/steamapps/common/Proton - Experimental" "$steam_dir/steamapps/common"/Proton\ * "$steam_dir/compatibilitytools.d"/*; do
        if [[ -f $dir/proton ]]; then proton=$dir; break; fi
    done
fi
[[ -n $proton && -f $proton/proton ]] || { echo 'Proton não encontrado. Instale-o pela Steam ou defina PROTON_PATH para a pasta dele.' >&2; exit 1; }

export STEAM_COMPAT_CLIENT_INSTALL_PATH=$steam_dir
export STEAM_COMPAT_DATA_PATH=${STEAM_COMPAT_DATA_PATH:-"${XDG_DATA_HOME:-$HOME/.local/share}/cool_fox/proton"}
# DXVK (DXGI) e vkd3d-proton (D3D12), como no run-proton.sh.
export WINEDLLOVERRIDES="${WINEDLLOVERRIDES:+${WINEDLLOVERRIDES};}dxgi,d3d12,d3d12core=n,b"

# Desempenho: sem camada de debug, sem DRED, sem timing de GPU e sem log do vkd3d.
export VICTORY_D3D12_DEBUG=${VICTORY_D3D12_DEBUG:-0}
export VICTORY_D3D12_DRED=${VICTORY_D3D12_DRED:-0}
export VICTORY_GPU_TIMING=${VICTORY_GPU_TIMING:-0}
export VKD3D_DEBUG=${VKD3D_DEBUG:-none}
# Janela de 1080p e alvos de renderização do jogo, como no run-proton.sh.
export VICTORY_WINDOW_MAX=${VICTORY_WINDOW_MAX:-1920x1080}
# Pipelines e shaders em paralelo (limite de 10 threads por comando).
export VICTORY_PIPELINE_WORKERS=${VICTORY_PIPELINE_WORKERS:-10}
# Atualiza texturas sem esperar a memória do título se estabilizar.
export VICTORY_TEXTURE_NO_WAIT=${VICTORY_TEXTURE_NO_WAIT:-1}
mkdir -p -- "$STEAM_COMPAT_DATA_PATH"
windows_game="Z:${game//\//\\}"
echo "Proton: $proton"
echo "Prefixo: $STEAM_COMPAT_DATA_PATH"
echo "VIctory: $victory"

# O log do terminal é gravado; o log do Proton/Wine fica desligado (I/O custa).
# PROTON_LOG=1 o liga para diagnóstico.
log_dir=${VICTORY_LOG_DIR:-"${XDG_STATE_HOME:-$HOME/.local/state}/cool_fox/logs/run-release"}
mkdir -p -- "$log_dir"
log="$log_dir/run-$(date +%Y%m%d-%H%M%S).log"
ln -sfn -- "$log" "$log_dir/latest.log"
export PROTON_LOG=${PROTON_LOG:-0}
export PROTON_LOG_DIR=${PROTON_LOG_DIR:-$log_dir}
echo "Log: $log"
status=0
"$proton/proton" runinprefix "$victory" run "$windows_game" --execute 2>&1 | tee -- "$log" || status=${PIPESTATUS[0]}
exit "$status"
