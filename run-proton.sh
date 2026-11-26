#!/usr/bin/env bash
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
victory=${VICTORY_EXE:-"$root/target/x86_64-pc-windows-msvc/diag/victory.exe"}
game=${1:-"/run/media/gilmar/8CEE31B4EE319802/GTA VI/GTA6.Xbox.v1.0.2.1-CYBERLEEK/game_scarlett_master.exe"}
if [[ $game == --help || $game == -h ]]; then
    echo "Uso: $0 [/caminho/para/game_scarlett_master.exe]"
    echo "Sem argumento, usa o executável padrão configurado no script."
    echo "Opcional: PROTON_PATH=/caminho/para/Proton STEAM_DIR=/caminho/para/Steam"
    echo "DirectX 12: dá preferência ao DXGI do DXVK e ao D3D12 do vkd3d-proton."
    echo "MangoHud ativado por padrão (instale com: sudo pacman -S mangohud)."
    echo "Use MANGOHUD=0 para desativar ou MANGOHUD_CONFIG para personalizar."
    exit 0
fi
if (( $# > 1 )); then
    echo "Informe no máximo um executável do jogo." >&2
    exit 2
fi
[[ -f $victory ]] || { echo "VIctory não encontrado: $victory" >&2; exit 1; }
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
# Overlay Vulkan; a instalação do MangoHud é feita no Linux, fora do prefixo.
export MANGOHUD=${MANGOHUD:-1}
# Em GPUs Intel, proc_vram mostra o uso do processo; vram do sistema não é suportado.
export MANGOHUD_CONFIG=${MANGOHUD_CONFIG:-position=top-left,fps,frametime,gpu_stats,cpu_stats,ram,proc_vram}
# O prefixo já deve ter sido preparado pelo Proton, com DXVK e vkd3d-proton.
export WINEDLLOVERRIDES="${WINEDLLOVERRIDES:+${WINEDLLOVERRIDES};}dxgi,d3d12,d3d12core=n,b"
# Build de diagnóstico: camada de depuração D3D12 e logs do vkd3d-proton.
export VICTORY_D3D12_DEBUG=${VICTORY_D3D12_DEBUG:-1}
export VKD3D_DEBUG=${VKD3D_DEBUG:-info}
# Cada lista de uma submissão roda e é cronometrada sozinha; as lentas saem no
# log com seus comandos mais caros. Desligue com VICTORY_GPU_TIMING=0.
export VICTORY_GPU_TIMING=${VICTORY_GPU_TIMING:-1}
# O timing só anota os comandos (e seus pipelines) com o DRED ligado.
export VICTORY_D3D12_DRED=${VICTORY_D3D12_DRED:-1}
# A janela do título (3840x2160) é reduzida a 1080p; os alvos de renderização
# do jogo continuam nos tamanhos dele. Sobrescreva com VICTORY_WINDOW_MAX=LxA.
export VICTORY_WINDOW_MAX=${VICTORY_WINDOW_MAX:-1920x1080}
# Limita as threads que traduzem shaders e criam pipelines.
export VICTORY_PIPELINE_WORKERS=${VICTORY_PIPELINE_WORKERS:-10}
# Atualiza texturas sem esperar a memória do título se estabilizar (o padrão do
# código é desligado). Desligue com VICTORY_TEXTURE_NO_WAIT=0.
export VICTORY_TEXTURE_NO_WAIT=${VICTORY_TEXTURE_NO_WAIT:-1}
mkdir -p -- "$STEAM_COMPAT_DATA_PATH"
windows_game="Z:${game//\//\\}"
echo "Proton: $proton"
echo "Prefixo: $STEAM_COMPAT_DATA_PATH"

# Cada execução grava o que sai no terminal em um arquivo novo; latest.log
# aponta para o último. PROTON_LOG=1 grava também o log do Proton/Wine.
log_dir=${VICTORY_LOG_DIR:-"${XDG_STATE_HOME:-$HOME/.local/state}/cool_fox/logs/run"}
mkdir -p -- "$log_dir"
log="$log_dir/run-$(date +%Y%m%d-%H%M%S).log"
ln -sfn -- "$log" "$log_dir/latest.log"
export PROTON_LOG=${PROTON_LOG:-1}
export PROTON_LOG_DIR=${PROTON_LOG_DIR:-$log_dir}
echo "Log: $log"
status=0
"$proton/proton" runinprefix "$victory" run "$windows_game" --execute 2>&1 | tee -- "$log" || status=${PIPESTATUS[0]}
exit "$status"
