# Build and launch GTA VI with diagnostics, the D3D12 debug layer and the
# launcher's settings. Every variable below can be overridden by setting it
# in the environment before running this script.
$ErrorActionPreference = 'Continue'

$gameDir = 'C:\GTA VI\GTA6.Xbox.v1.0.2.1-CYBERLEEK'
$gameExe = Join-Path $gameDir 'game_scarlett_master.exe'
$logPath = Join-Path $env:USERPROFILE 'run-gta-diag.log'

if (-not (Test-Path -LiteralPath $gameExe -PathType Leaf)) {
    throw "Game executable not found: $gameExe"
}

# Defaults: diagnostics, launcher configuration and the wave64 -> wave32 opt-ins.
# GPU-based validation (VICTORY_D3D12_GBV) is left off: it slows compute enough
# to trip the title's watchdog.
$defaults = [ordered]@{
    VICTORY_D3D12_DEBUG          = '1'
    VICTORY_D3D12_DRED           = '1'
    VICTORY_GPU_TIMING           = '1'
    VICTORY_TEXTURE_NO_WAIT      = '1'
    VICTORY_PIPELINE_WORKERS     = '10'
    VICTORY_WINDOW_MAX           = '1920x1080'
    VICTORY_WIDE_GROUPS          = '1'
    VICTORY_WAVE_LDS_WIDE        = '1'
    VICTORY_LOG                  = 'info'
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
    Write-Host "Writing diagnostic output to $logPath"
    & cargo run -j 10 --features diagnostics --profile diag -p victory -- run $gameExe --execute --console series-s 2>&1 |
        Tee-Object -FilePath $logPath
    if ($LASTEXITCODE -ne 0) {
        throw "Diagnostic cargo run failed with exit code $LASTEXITCODE"
    }
}
finally {
    foreach ($name in $set) {
        Remove-Item "Env:$name" -ErrorAction SilentlyContinue
    }
    Pop-Location
}
