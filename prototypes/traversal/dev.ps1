# Development loop for the RustCraft traversal prototype (see README.md).
#   .\dev.ps1 test       game-free tests only (seconds, no game build)
#   .\dev.ps1 plan       static check of the bundle by horizon-mixins: targets, hooks, bindings
#                        (no compile, no native code run)
#   .\dev.ps1 init       once: the client instance and the test server's folder (asks the EULA)
#   .\dev.ps1 server     build the server engine and point the test server at it
#   .\dev.ps1 client     build the client into the prototype's instance
#   .\dev.ps1 both       both builds
#   .\dev.ps1 config -Anticheat default|strict|off [-AllowFlight true|false]
#                        set the test server's anticheat profile (server stopped)
#   .\dev.ps1 serve      run the test server in this window (Ctrl-C stops it)
#   .\dev.ps1 play       start the client on the test server [-Account NAME for a second player]
#   .\dev.ps1 logs       list the newest journals of the server and the client
# server, client and both pass --trust-native-code: only the owner runs them.
# Add -BuildProfile performance or release to leave the default dev profile.
param(
    [ValidateSet('test', 'plan', 'init', 'server', 'client', 'both', 'config', 'serve', 'play', 'logs')]
    [string]$Part = 'test',
    [ValidateSet('dev', 'performance', 'release')]
    [string]$BuildProfile = 'dev',
    [ValidateSet('default', 'strict', 'off')]
    [string]$Anticheat = 'default',
    [ValidateSet('', 'true', 'false')]
    [string]$AllowFlight = '',
    [string]$Account = ''
)
$ErrorActionPreference = 'Stop'

# The pair path_of_craft_tree builds with; the bundle pins the same commit.
$HorizonVersion = 'nightly-2026-11-08-fdc87ae'
$Instance = 'traversal-dev'
$Launcher = "$env:LOCALAPPDATA\HorizonLauncher\horizon-launcher.exe"
$ServerTool = "$env:LOCALAPPDATA\Horizon\current\horizon-server.exe"
# Apart from path_of_craft_tree's dev server ($env:LOCALAPPDATA\Horizon\dev) and instance.
$DevRoot = "$env:LOCALAPPDATA\Horizon\traversal-dev"
$Server = "$DevRoot\server"
$Engines = "$DevRoot\engines"
$ServerPort = 25567
$WorldName = 'traversal'
$InstanceDir = "$env:APPDATA\horizon\instances\$Instance"
$Bundle = Join-Path $PSScriptRoot 'mixins\traversal'
$Mixins = "$env:APPDATA\horizon-launcher\native\tools\5a64af938ec8\bin\horizon-mixins.exe"
$PinnedSource = "$env:APPDATA\horizon-launcher\native\source\7e4e421224743469035ba0c34fc21e569e64a17d"
$EnginesKept = 3
$CoreShare = 0.9

function Invoke-Tests {
    Push-Location $PSScriptRoot
    try {
        cargo test --quiet
        if ($LASTEXITCODE -ne 0) { throw 'tests failed' }
    } finally {
        Pop-Location
    }
}

# horizon-mixins plan reads the pinned source and the manifest and writes a JSON plan: it neither
# compiles nor runs anything, so it needs no trust flag.
function Invoke-Plan {
    foreach ($path in $Mixins, $PinnedSource) {
        if (-not (Test-Path $path)) { throw "$path is missing: build path_of_craft_tree once so the launcher fetches it" }
    }
    $out = Join-Path ([IO.Path]::GetTempPath()) 'traversal-plan.json'
    & $Mixins plan --workspace $PinnedSource --mods (Split-Path $Bundle) --out $out
    if ($LASTEXITCODE -ne 0) { throw 'horizon-mixins refused the bundle (the line above says why)' }
    Write-Host "Plan written to $out"
}

function Invoke-Build([string[]]$Arguments, [string]$Name) {
    # The launcher downloads faster without the editor's proxy variables.
    $env:HTTP_PROXY = $null
    $env:HTTPS_PROXY = $null
    $timer = [Diagnostics.Stopwatch]::StartNew()
    $process = Start-Process $Launcher -ArgumentList $Arguments -PassThru -NoNewWindow
    $null = $process.Handle
    $threads = [int][Math]::Max(1, [Math]::Floor([Environment]::ProcessorCount * $CoreShare))
    $process.ProcessorAffinity = [IntPtr]((1L -shl $threads) - 1)
    $process.WaitForExit()
    if ($process.ExitCode -ne 0) {
        throw "$Name build failed with exit code $($process.ExitCode); see $env:APPDATA\horizon-launcher\native\build.log"
    }
    Write-Host "$Name built in $([int]$timer.Elapsed.TotalSeconds) s"
}

function Build-Server {
    if (-not (Test-Path "$Server\server.toml")) { throw "No test server at ${Server}: run .\dev.ps1 init first" }
    $output = Join-Path $Engines (Get-Date -Format 'yyyyMMdd-HHmmss')
    New-Item -ItemType Directory -Force $Engines | Out-Null
    Invoke-Build @('native-build', "`"$Bundle`"", '--server-output', "`"$output`"", '--version', $HorizonVersion,
        '--profile', $BuildProfile, '--trust-native-code') 'Server'
    & $ServerTool --dir $Server config set "engine.path=$output\horizon-engine.exe"
    if ($LASTEXITCODE -ne 0) { throw 'engine.path not set' }
    Get-ChildItem $Engines -Directory | Sort-Object Name -Descending | Select-Object -Skip $EnginesKept |
        ForEach-Object {
            try { Remove-Item -LiteralPath $_.FullName -Recurse -Force -ErrorAction Stop } catch { Write-Host "Kept $($_.Name): in use" }
        }
}

function Build-Client {
    Invoke-Build @('native-build', "`"$Bundle`"", '--instance', $Instance, '--version', $HorizonVersion,
        '--profile', $BuildProfile, '--trust-native-code') 'Client'
}

function Initialize-TestBench {
    & $Launcher instance create $Instance
    if ($LASTEXITCODE -ne 0) { Write-Host "Instance $Instance not created (it may exist already)" }
    if (Test-Path "$Server\server.toml") {
        Write-Host "The test server exists already: $Server"
        return
    }
    New-Item -ItemType Directory -Force $Server | Out-Null
    # Asks for the EULA, the whitelist and the operator: those answers are the owner's.
    & $ServerTool --dir $Server init --world $WorldName --port $ServerPort
    if ($LASTEXITCODE -ne 0) { throw 'horizon-server init failed' }
}

# server.toml without any [anticheat] or [anticheat.*] table, as lines.
function Remove-AnticheatTables([string[]]$Lines) {
    $kept = [Collections.Generic.List[string]]::new()
    $inside = $false
    foreach ($line in $Lines) {
        $trimmed = $line.Trim()
        if ($trimmed.StartsWith('[')) { $inside = $trimmed -match '^\[anticheat(\]|\.)' }
        if (-not $inside) { $kept.Add($line) }
    }
    return , $kept.ToArray()
}

# The lines with `allow-flight` set (or removed for '') under [properties].
function Set-AllowFlightLine([string[]]$Lines, [string]$Value) {
    $kept = [Collections.Generic.List[string]]::new()
    $section = ''
    foreach ($line in $Lines) {
        $trimmed = $line.Trim()
        if ($trimmed.StartsWith('[')) { $section = $trimmed }
        if ($section -eq '[properties]' -and $trimmed -match '^allow-flight\s*=') { continue }
        $kept.Add($line)
        if ($trimmed -eq '[properties]' -and $Value -ne '') { $kept.Add("allow-flight = $Value") }
    }
    return , $kept.ToArray()
}

$AnticheatProfiles = @{
    # The engine's default: prediction only writes logs\anticheat.log.
    'default' = @()
    # Prediction acting as it will once trusted (docs\anticheat.md): it sends players back.
    'strict'  = @('', '[anticheat.prediction]', 'setback = 5', 'alert = 15', 'kick = 60')
    # Only vanilla's checks: moved too quickly, moved wrongly, floating.
    'off'     = @('', '[anticheat]', 'enabled = false')
}

function Set-ServerConfig {
    $file = "$Server\server.toml"
    if (-not (Test-Path $file)) { throw "No test server at ${Server}: run .\dev.ps1 init first" }
    $utf8 = [Text.UTF8Encoding]::new($false)
    $lines = Remove-AnticheatTables ([IO.File]::ReadAllLines($file, $utf8))
    if ($AllowFlight -ne '') { $lines = Set-AllowFlightLine $lines $AllowFlight }
    $lines = $lines + $AnticheatProfiles[$Anticheat]
    Copy-Item $file "$file.bak" -Force
    [IO.File]::WriteAllLines($file, $lines, $utf8)
    # The supervisor checks the file as it starts the server; `config` reads it the same way now.
    & $ServerTool --dir $Server config | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Copy-Item "$file.bak" $file -Force
        throw 'horizon-server refused the new server.toml; the old one is back'
    }
    Write-Host "Anticheat profile '$Anticheat' set in $file (old file: server.toml.bak). Restart the server."
}

function Show-Logs {
    foreach ($dir in "$Server\config\traversal", "$InstanceDir\config\traversal") {
        Write-Host "== $dir"
        if (Test-Path $dir) {
            Get-ChildItem $dir | Sort-Object LastWriteTime -Descending | Select-Object -First 8 |
                ForEach-Object { Write-Host "  $($_.LastWriteTime.ToString('HH:mm:ss'))  $($_.Name)" }
        } else {
            Write-Host '  (none yet)'
        }
    }
}

switch ($Part) {
    'test' { Invoke-Tests; return }
    'plan' { Invoke-Plan; return }
    'init' { Initialize-TestBench; return }
    'config' { Set-ServerConfig; return }
    'serve' { & $ServerTool --dir $Server run; return }
    'play' {
        $arguments = @('play', '--instance', $Instance, '--server', "127.0.0.1:$ServerPort")
        if ($Account -ne '') { $arguments += @('--account', $Account) }
        & $Launcher @arguments
        return
    }
    'logs' { Show-Logs; return }
}
Invoke-Tests
Invoke-Plan
if ($Part -in 'server', 'both') { Build-Server }
if ($Part -in 'client', 'both') { Build-Client }
