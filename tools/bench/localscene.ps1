# The load scene (stress_scene.py) on RustCraft's own dev server (release build, fresh world): build ROWS rows
# of 16 machines, wait, record a report of SECONDS; prints the report's main lines.
#   localscene.ps1 -Rows 8 [-Seconds 40] [-Clear]
param([int]$Rows = 8, [int]$Seconds = 40, [switch]$Clear, [switch]$NoReport)
$ErrorActionPreference = 'Stop'
$Tool = "$env:LOCALAPPDATA\Horizon\current\horizon-server.exe"
$Server = "$env:LOCALAPPDATA\Horizon\rustcraft-dev\server"
$mode = if ($Clear) { 'clear' } else { 'build' }
$lines = py -3 (Join-Path $PSScriptRoot 'stress_scene.py') $mode $Rows 16
$bad = 0
foreach ($c in $lines) {
    $o = (& $Tool --dir $Server cmd $c 2>&1) -join ' '
    if ($o -match 'rror|nknown|ncorrect') { $bad++; if ($bad -le 3) { Write-Host "ERR $c -> $o" } }
}
Write-Host "$mode $Rows rows: $($lines.Count) commands, $bad refused"
if ($NoReport) { return }
Start-Sleep -Seconds 15
& $Tool --dir $Server report $Seconds 2>&1 | Select-Object -Last 9 | Select-Object -First 3
