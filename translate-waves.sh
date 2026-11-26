#!/usr/bin/env bash
# Traduz os shaders do jogo com o victory.exe de diagnóstico, pelo Proton, e resume o que
# aconteceu com cada programa de wave64 num host de 32 lanes: convertido para wave32, recusado
# ou outra falha. Não precisa do jogo aberto.
#
# Uso: ./translate-waves.sh [--no-build] [--rpf /caminho/shaders.rpf] [--out /pasta]
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
cd -- "$root"

build=1
validate=0
pipelines=0
rpf="/run/media/gilmar/8CEE31B4EE319802/GTA VI/GTA6.Xbox.v1.0.2.1-CYBERLEEK/shaders.rpf"
out_dir=""
while (( $# > 0 )); do
    case $1 in
        --no-build) build=0 ;;
        --validate) validate=1 ;;
        --pipelines) pipelines=1 ;;
        --rpf) rpf=$2; shift ;;
        --out) out_dir=$2; shift ;;
        -h|--help)
            sed -n '2,6p' "$0"; exit 0 ;;
        *) echo "Opção desconhecida: $1" >&2; exit 2 ;;
    esac
    shift
done

victory=${VICTORY_EXE:-"$root/target/x86_64-pc-windows-msvc/diag/victory.exe"}
[[ -f $rpf ]] || { echo "shaders.rpf não encontrado: $rpf" >&2; exit 1; }

if (( build )); then
    ./build.sh
fi
[[ -f $victory ]] || { echo "VIctory não encontrado: $victory" >&2; exit 1; }

# Proton e Steam: a mesma busca de run-proton.sh.
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
[[ -n $proton && -f $proton/proton ]] || { echo 'Proton não encontrado. Defina PROTON_PATH.' >&2; exit 1; }

export STEAM_COMPAT_CLIENT_INSTALL_PATH=$steam_dir
export STEAM_COMPAT_DATA_PATH=${STEAM_COMPAT_DATA_PATH:-"${XDG_DATA_HOME:-$HOME/.local/share}/cool_fox/proton"}
# O tradutor lê a largura do host por aqui (AGENTS.md); 32 é a Arc.
export VICTORY_HOST_MAX_WAVE=${VICTORY_HOST_MAX_WAVE:-32}

# Saída: uma pasta nova por execução, latest aponta para a última.
out_dir=${out_dir:-"${XDG_STATE_HOME:-$HOME/.local/state}/cool_fox/logs/translate"}
run_dir="$out_dir/run-$(date +%Y%m%d-%H%M%S)"
mkdir -p -- "$run_dir"
ln -sfn -- "$run_dir" "$out_dir/latest"
list="$run_dir/list.txt"
census="$run_dir/census.txt"
export VICTORY_SHADER_CENSUS=$census

windows_rpf="Z:${rpf//\//\\}"
echo "Proton:  $proton"
echo "Saída:   $run_dir"
set +e
validate_args=()
if (( validate )); then
    # dxil.dll do DirectX Shader Compiler (v1.9.2609, x64), em caminho Windows, como pede o AGENTS.md
    dxil=${VICTORY_DXIL:-"$HOME/.local/share/cool_fox/dxc/dxil.dll"}
    [[ -f $dxil ]] || { echo "dxil.dll não encontrado: $dxil (veja AGENTS.md)" >&2; exit 1; }
    export VICTORY_DXIL="Z:${dxil//\//\\}"
    validate_args=(--validate)
fi
pipeline_args=()
if (( pipelines )); then
    # cria as pipelines de host (D3D12 do vkd3d-proton), como run-proton.sh configura
    export WINEDLLOVERRIDES="${WINEDLLOVERRIDES:+${WINEDLLOVERRIDES};}dxgi,d3d12,d3d12core=n,b"
    pipeline_args=(--pipelines)
fi
"$proton/proton" runinprefix "$victory" translate "$windows_rpf" --list "${validate_args[@]}" "${pipeline_args[@]}" --jobs "${JOBS:-10}" \
    > "$list" 2> "$run_dir/err.txt"
status=$?
set -e
echo "translate saiu com $status (stderr em $run_dir/err.txt)"

# Resumo: o census traz o flag wave64 de cada programa; a lista traz o resultado. Cruzamos pelo
# rótulo (o texto antes de dois espaços).
if [[ ! -s $census ]]; then
    echo "Sem census: a tradução não chegou a emitir nenhum programa. Veja $run_dir/err.txt" >&2
    exit 1
fi
# Só compute: a recusa de wave64 num host de 32 vale para o estágio Cs (o flag do census
# também vale para VS/PS/MS, onde não há essa conversão).
awk -F'\t' '
    FILENAME == ARGV[1] { if ($1 ~ / Cs$/) wave[$1] = ($2 == "wave64=1"); next }
    FILENAME == ARGV[2] {
        i = index($0, "  ")
        if (i == 0) next
        label = substr($0, 1, i - 1); rest = substr($0, i + 2)
        # as falhas trazem " at 0x<offset>" depois do rótulo; o census não
        sub(/ at 0x[0-9a-fA-F]+$/, "", label)
        if (rest ~ /^emitted /) ok[label] = 1
        else if (rest ~ /no wave32 lowering/) refused[label] = rest
        else if (rest ~ /^pipeline|^validated/) next
        else other[label] = rest
    }
    END {
        # Sucesso: o census (só programas emitidos). Recusa e falha: a lista, que traz também
        # os que não emitiram; por isso o total de wave64 vem da união dos dois.
        for (l in wave) if (wave[l]) seen[l] = 1
        for (l in refused) if (l ~ / Cs$/) seen[l] = 1
        for (l in other) if (l ~ / Cs$/ && !(l in ok)) seen[l] = 1
        conv = 0; ref = 0; oth = 0
        for (l in seen) {
            if (l in ok) { conv++; continue }
            if (l in refused) { ref++; rsn[refused[l]]++; continue }
            if (l in other) { oth++; rsn[other[l]]++ }
        }
        total = conv + ref + oth
        printf "programas de compute com wave64 (host de 32): %d\n", total
        printf "  convertidos para wave32:          %d\n", conv
        printf "  recusados (no wave32 lowering):   %d\n", ref
        printf "  outra falha:                      %d\n", oth
        if (ref + oth > 0) {
            print "motivos:"
            for (r in rsn) printf "  %4d  %s\n", rsn[r], substr(r, 1, 160)
        }
    }' "$census" "$list" | tee "$run_dir/summary.txt"
if (( pipelines )); then
    # pipelines criadas e recusadas, só para compute convertido (census) e para o resto
    created=$(grep -c ' Cs.*  pipeline created' "$list" || true)
    failed=$(grep -c ' Cs.*  pipeline: ' "$list" || true)
    echo "pipelines de compute criadas: $created, recusadas: $failed (motivos em $list)" | tee -a "$run_dir/summary.txt"
fi
echo "Detalhes: $list e $census"
