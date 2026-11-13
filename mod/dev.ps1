# Development loop for the RustCraft bundle (see README.md).
#   .\dev.ps1 test       the rules crate's tests (seconds, no game build)
#   .\dev.ps1 plan       static check of the bundle by horizon-mixins: targets, hooks, bindings,
#                        the crate (no compile, no native code run)
#   .\dev.ps1 init       once: the client instance and the dev server's folder (asks the EULA),
#                        then the content mod into both
#   .\dev.ps1 server     build the server engine, point the dev server at it, copy the content mod
#   .\dev.ps1 client     build the client into the instance, copy the content mod
#   .\dev.ps1 both       both builds
#   .\dev.ps1 content    only copy the content mod (mod\content) to the server and the instance
#   .\dev.ps1 config -Anticheat default|strict|off
#                        set the dev server's anticheat profile (server stopped; the old
#                        server.toml is kept as server.toml.bak), as the traversal prototype's
#   .\dev.ps1 serve      copy the content mod, run the dev server in this window (Ctrl-C stops it)
#   .\dev.ps1 play       copy the content mod, start the client on the dev server
#                        [-Account NAME for a second player]
#   .\dev.ps1 logs       RustCraft's lines from the server's and the client's latest.log
# server, client and both pass --trust-native-code: only the owner runs them.
# Add -BuildProfile performance or release to leave the default dev profile.
param(
    [ValidateSet('test', 'plan', 'init', 'server', 'client', 'both', 'content', 'config', 'serve', 'play', 'logs')]
    [string]$Part = 'test',
    [ValidateSet('dev', 'performance', 'release')]
    [string]$BuildProfile = 'dev',
    [ValidateSet('default', 'strict', 'off')]
    [string]$Anticheat = 'default',
    [string]$Account = ''
)
$ErrorActionPreference = 'Stop'

# The pair path_of_craft_tree and the traversal prototype build with; the bundle pins the same commit.
$HorizonVersion = 'nightly-2026-11-08-fdc87ae'
$ModId = 'rustcraft'
$Instance = 'rustcraft-dev'
$Launcher = "$env:LOCALAPPDATA\HorizonLauncher\horizon-launcher.exe"
$ServerTool = "$env:LOCALAPPDATA\Horizon\current\horizon-server.exe"
# Apart from path_of_craft_tree's dev server ($env:LOCALAPPDATA\Horizon\dev) and the prototype's.
$DevRoot = "$env:LOCALAPPDATA\Horizon\rustcraft-dev"
$Server = "$DevRoot\server"
$Engines = "$DevRoot\engines"
$ServerPort = 25568
$WorldName = 'rustcraft'
$InstanceDir = "$env:APPDATA\horizon\instances\$Instance"
$Bundle = Join-Path $PSScriptRoot 'rustcraft'
$Content = Join-Path $PSScriptRoot 'content'
$RepoRoot = Split-Path $PSScriptRoot
$Mixins = "$env:APPDATA\horizon-launcher\native\tools\5a64af938ec8\bin\horizon-mixins.exe"
$PinnedSource = "$env:APPDATA\horizon-launcher\native\source\7e4e421224743469035ba0c34fc21e569e64a17d"
$BuildLog = "$env:APPDATA\horizon-launcher\native\build.log"
$EnginesKept = 3
$CoreShare = 0.9
$LogLines = 40

function Invoke-Tests {
    # The repository's workspace holds only the rules crate (../Cargo.toml).
    Push-Location $RepoRoot
    try {
        cargo test --quiet
        if ($LASTEXITCODE -ne 0) { throw 'tests failed' }
    } finally {
        Pop-Location
    }
}

# horizon-mixins plan reads the pinned source and the manifest and writes a JSON plan: it neither
# compiles nor runs anything, so it needs no trust flag. It finds the bundle as
# <this folder>\rustcraft\horizon-mixin.toml; mod\content has no horizon-mixin.toml and is skipped.
function Invoke-Plan {
    foreach ($path in $Mixins, $PinnedSource) {
        if (-not (Test-Path $path)) { throw "$path is missing: build path_of_craft_tree once so the launcher fetches it" }
    }
    $out = Join-Path ([IO.Path]::GetTempPath()) 'rustcraft-plan.json'
    & $Mixins plan --workspace $PinnedSource --mods $PSScriptRoot --out $out
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
        throw "$Name build failed with exit code $($process.ExitCode); see $BuildLog"
    }
    Write-Host "$Name built in $([int]$timer.Elapsed.TotalSeconds) s"
}

function Build-Server {
    if (-not (Test-Path "$Server\server.toml")) { throw "No dev server at ${Server}: run .\dev.ps1 init first" }
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
    # A client build leaves out a mod whose helpers don't compile and still succeeds
    # (docs/native-mods.md, "Pinned to a Nightly"): say so instead of starting a game without it.
    if ((Test-Path $BuildLog) -and (Select-String -Path $BuildLog -Pattern "$ModId was left out" -Quiet)) {
        throw "The client was built without $ModId; see $BuildLog"
    }
}

function Initialize-TestBench {
    & $Launcher instance create $Instance
    if ($LASTEXITCODE -ne 0) { Write-Host "Instance $Instance not created (it may exist already)" }
    if (Test-Path "$Server\server.toml") {
        Write-Host "The dev server exists already: $Server"
        return
    }
    New-Item -ItemType Directory -Force $Server | Out-Null
    # Asks for the EULA, the whitelist and the operator: those answers are the owner's.
    & $ServerTool --dir $Server init --world $WorldName --port $ServerPort
    if ($LASTEXITCODE -ne 0) { throw 'horizon-server init failed' }
}

# The game reads content mods from horizon-mods\ in its working folder, beside options.txt: the
# instance folder for the client, the server folder for the dedicated server, which the
# supervisor starts with that folder as its working directory (horizon-server engine.rs:255-270;
# the game's content_root, game\mods.rs:94-104). It loads those turned on in horizon-mods.txt
# there (mod_list.rs:1-14). A folder is read as a package as an .hmod is; it has no file hash, so
# the server and the client match it by id and version (game\mods.rs:205-218).
function Install-Content([string]$GameDir) {
    if (-not (Test-Path $GameDir)) {
        Write-Host "Skipped ${GameDir}: it does not exist yet (run .\dev.ps1 init)"
        return
    }
    $target = Join-Path $GameDir "horizon-mods\$ModId"
    if (Test-Path $target) { Remove-Item -LiteralPath $target -Recurse -Force }
    New-Item -ItemType Directory -Force $target | Out-Null
    # Only what the game reads: the manifest, assets\ and data\ (make_textures.py stays here).
    Copy-Item -LiteralPath (Join-Path $Content 'horizon-mod.toml') -Destination $target
    foreach ($tree in 'assets', 'data') {
        Copy-Item -LiteralPath (Join-Path $Content $tree) -Destination $target -Recurse
    }
    Enable-Mod $GameDir
    Write-Host "Content mod copied to $target"
}

# horizon-mods.txt with `mod.<id>:true` and every other line kept. The same line keeps the
# native bundle on: it has the same id.
function Enable-Mod([string]$GameDir) {
    $file = Join-Path $GameDir 'horizon-mods.txt'
    $utf8 = [Text.UTF8Encoding]::new($false)
    $lines = @()
    if (Test-Path $file) { $lines = [IO.File]::ReadAllLines($file, $utf8) }
    $kept = @($lines | Where-Object { $_.Trim() -notmatch "^mod\.$ModId\s*:" })
    $text = (($kept + "mod.${ModId}:true") -join "`n") + "`n"
    [IO.File]::WriteAllText($file, $text, $utf8)
}

function Install-ContentEverywhere {
    Install-Content $Server
    Install-Content $InstanceDir
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

# The capsule's checks run under both profiles (spec 16.2): the engine's default, where
# prediction only writes logs\anticheat.log, and strict, where it sends players back.
$AnticheatProfiles = @{
    'default' = @()
    'strict'  = @('', '[anticheat.prediction]', 'setback = 5', 'alert = 15', 'kick = 60')
    'off'     = @('', '[anticheat]', 'enabled = false')
}

function Set-ServerConfig {
    $file = "$Server\server.toml"
    if (-not (Test-Path $file)) { throw "No dev server at ${Server}: run .\dev.ps1 init first" }
    $utf8 = [Text.UTF8Encoding]::new($false)
    $lines = Remove-AnticheatTables ([IO.File]::ReadAllLines($file, $utf8))
    $lines = $lines + $AnticheatProfiles[$Anticheat]
    Copy-Item $file "$file.bak" -Force
    [IO.File]::WriteAllLines($file, $lines, $utf8)
    & $ServerTool --dir $Server config | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Copy-Item "$file.bak" $file -Force
        throw 'horizon-server refused the new server.toml; the old one is back'
    }
    Write-Host "Anticheat profile '$Anticheat' set in $file (old file: server.toml.bak). Restart the server."
}

function Show-Logs {
    foreach ($log in "$Server\logs\latest.log", "$InstanceDir\logs\latest.log") {
        Write-Host "== $log"
        if (Test-Path $log) {
            # Our own lines, and the game's about content mods and native declarations.
            Select-String -Path $log -Pattern "$ModId|content mods|content not loaded|blocks and items" |
                Select-Object -Last $LogLines | ForEach-Object { Write-Host "  $($_.Line)" }
        } else {
            Write-Host '  (none yet)'
        }
    }
}

switch ($Part) {
    'test' { Invoke-Tests; return }
    'plan' { Invoke-Plan; return }
    'init' { Initialize-TestBench; Install-ContentEverywhere; return }
    'content' { Install-ContentEverywhere; return }
    'config' { Set-ServerConfig; return }
    'serve' { Install-Content $Server; & $ServerTool --dir $Server run; return }
    'play' {
        Install-Content $InstanceDir
        $arguments = @('play', '--instance', $Instance, '--server', "127.0.0.1:$ServerPort")
        if ($Account -ne '') { $arguments += @('--account', $Account) }
        & $Launcher @arguments
        return
    }
    'logs' { Show-Logs; return }
}
Invoke-Tests
Invoke-Plan
if ($Part -in 'server', 'both') { Build-Server; Install-Content $Server }
if ($Part -in 'client', 'both') { Build-Client; Install-Content $InstanceDir }
