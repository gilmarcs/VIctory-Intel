# Build and launch GTA VI with the release profile and the launcher's
# settings. Every variable below can be overridden by setting it in the
# environment before running this script.
$ErrorActionPreference = 'Continue'

$gameDir = 'C:\GTA VI\GTA6.Xbox.v1.0.2.1-CYBERLEEK'
$gameExe = Join-Path $gameDir 'game_scarlett_master.exe'
$logPath = Join-Path $env:USERPROFILE 'run-gta.log'

if (-not (Test-Path -LiteralPath $gameExe -PathType Leaf)) {
    throw "Não encontrei o executável do jogo: $gameExe"
}

$defaults = [ordered]@{
    VICTORY_TEXTURE_NO_WAIT      = '1'
    VICTORY_PIPELINE_WORKERS     = '10'
    VICTORY_WINDOW_MAX           = '1920x1080'
    VICTORY_WIDE_GROUPS          = '1'
    VICTORY_WAVE_LDS_WIDE        = '1'
    # Presses Enter (button A) every 2 s from second 10 to 180, so the start
    # screen needs no one. Set VICTORY_KEYS to something else to change it.
    VICTORY_KEYS                 = '10-180/2:Enter'
}

$set = @()
foreach ($name in $defaults.Keys) {
    if (-not (Test-Path "Env:$name")) {
        Set-Item "Env:$name" $defaults[$name]
        $set += $name
    }
}

$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
    Write-Host "Writing output to $logPath"
    & cargo run -j 10 --release -p victory -- run $gameExe --execute --console series-s 2>&1 |
        Tee-Object -FilePath $logPath
    if ($LASTEXITCODE -ne 0) {
        throw "cargo run falhou com código $LASTEXITCODE"
    }
}
finally {
    foreach ($name in $set) {
        Remove-Item "Env:$name" -ErrorAction SilentlyContinue
    }
    Pop-Location
}
