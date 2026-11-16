# Back-to-back A/B of dev-server engines (the PC drifts by the hour: compare only within one
# run). For each engine in turn: start it, empty 20 s, 16 rows 30 s, 64 rows 3 x 10 s + 30 s,
# stop. Holds heavy.lock throughout; puts back the engine the server had.
param([Parameter(Mandatory)][string[]]$Engines)
$ErrorActionPreference = 'Stop'
$Tool = "$env:LOCALAPPDATA\Horizon\current\horizon-server.exe"
$Server = "$env:LOCALAPPDATA\Horizon\rustcraft-dev\server"
$Lock = 'C:\ai-bridge\heavy.lock'
$here = $PSScriptRoot
$toml = Join-Path $Server 'server.toml'
$was = ((Select-String -Path $toml -Pattern "^path = '([^']+)'").Matches[0].Groups[1].Value)
function Mspt([int]$s) { & $Tool --dir $Server report $s 2>&1 | Select-String 'MSPT|level_overworld' | ForEach-Object { '  ' + $_.Line.Trim() } }
$fs = [System.IO.File]::Open($Lock, 'CreateNew', 'Write')
$w = New-Object System.IO.StreamWriter($fs)
$w.Write("rustcraft session: A/B engines on rustcraft-dev, $(Get-Date -Format s)"); $w.Close()
try {
    foreach ($e in $Engines) {
        $exe = Join-Path $env:LOCALAPPDATA "Horizon\rustcraft-dev\engines\$e\horizon-engine.exe"
        & $Tool --dir $Server config set "engine.path=$exe" | Out-Null
        Start-Process -FilePath $Tool -ArgumentList @('--dir', $Server, 'start') -WindowStyle Hidden
        for ($i = 0; $i -lt 90; $i++) {
            Start-Sleep 2
            if (((& $Tool --dir $Server status 2>&1) -join ' ') -match 'tps') { break }
        }
        Write-Host "######## engine $e  $(Get-Date -Format HH:mm:ss)"
        & $Tool --dir $Server cmd 'forceload add 596 596 638 734' | Out-Null
        & $here\localscene.ps1 -Rows 64 -Clear -NoReport | Out-Null
        Start-Sleep 20
        Write-Host '== empty'; Mspt 20
        & $here\localscene.ps1 -Rows 16 -NoReport | Out-Null
        Start-Sleep 15
        Write-Host '== 256 machines'; Mspt 30
        & $here\localscene.ps1 -Rows 16 -Clear -NoReport | Out-Null
        & $here\localscene.ps1 -Rows 64 -NoReport | Out-Null
        Start-Sleep 15
        Write-Host '== 1024 machines, 3 x 10 s'; 1..3 | ForEach-Object { Mspt 10 }
        Write-Host '== 1024 machines, 30 s'; Mspt 30
        & $here\localscene.ps1 -Rows 64 -Clear -NoReport | Out-Null
        & $Tool --dir $Server cmd 'forceload remove all' | Out-Null
        & $Tool --dir $Server stop | Out-Null
        Start-Sleep 5
    }
} finally {
    & $Tool --dir $Server config set "engine.path=$was" | Out-Null
    Remove-Item $Lock -ErrorAction SilentlyContinue
}
