# A screenshot of the zipline's rope (slice 14) on the joint bench, for the README.
# Run it as shot.ps1 is run (joint.ps1 round -WithClient, serve, open; then this; then stop, close).
#
# The scene: a stone floor at y 119 and a pillar 30 blocks east of it. The rider rc_b (survival)
# hooks the pillar's top with a zipline and rides there at 8 blocks a second; the camera rc_a
# (spectator) looks at the ride from the side. Both bots play muted (master volume 0).
param(
    [Parameter(Mandatory)] [string]$Out,
    [string]$Camera = 'rc_a',
    [string]$Rider = 'rc_b'
)
$ErrorActionPreference = 'Stop'
$Ctl = "$env:LOCALAPPDATA\Horizon\current\horizon-ctl.exe"
$ServerTool = "$env:LOCALAPPDATA\Horizon\current\horizon-server.exe"
$Server = "$env:LOCALAPPDATA\Horizon\joint-dev\server"
$Instance = "$env:APPDATA\horizon\instances\joint-dev"
$Port = 25570
$Inv = [Globalization.CultureInfo]::InvariantCulture
New-Item -ItemType Directory -Force $Out | Out-Null

function Console([string]$Line) {
    $o = & $ServerTool --dir $Server cmd $Line 2>&1
    if ($LASTEXITCODE -ne 0) { Write-Warning "console: $Line -> $o" } else { Write-Host "console: $Line -> $(($o -join ' ').Trim())" }
}

function Client {
    $instances = Get-Content -Raw "$env:APPDATA\horizon-launcher\instances.json" | ConvertFrom-Json
    $version = $instances.instances.'joint-dev'.version
    "$env:APPDATA\horizon-launcher\versions\$version\horizon-client.exe"
}

function Bot([string]$Socket, [string]$Line, [int]$Timeout = 60) {
    $o = & $Ctl --socket $Socket --timeout $Timeout @($Line -split ' ') 2>&1
    if ($LASTEXITCODE -ne 0) { Write-Warning "bot: $Line -> $o" }
    $o
}

function Launch([string]$Name) {
    $dir = Join-Path $env:TEMP "rustcraft-bench\bots\$Name"
    New-Item -ItemType Directory -Force $dir | Out-Null
    $socket = Join-Path $dir 'ctl.sock'
    $options = Join-Path $dir 'options.txt'
    $lines = if (Test-Path $options) { @(Get-Content $options | Where-Object { $_ -notmatch '^soundCategory_master:' -and $_ -notmatch '^(guiScale|hideGui|fov):' }) } else { @() }
    [IO.File]::WriteAllLines($options, [string[]](@($lines) + 'soundCategory_master:0.0' + 'fov:0.0'), [Text.UTF8Encoding]::new($false))
    $reply = & $Ctl --socket $socket launch --game (Client) --offscreen --home $Instance --game-dir $Instance `
        --player-dir $dir --name $Name --server "127.0.0.1:$Port" 2>&1
    if ($LASTEXITCODE -ne 0) { throw "launch of $Name failed: $reply" }
    Bot $socket 'wait-ready 180' 200 | Out-Null
    $socket
}

# ---- The scene ----
$BX, $BY, $BZ = 300, 120, 100
$Far = 30
Console 'time set 6000'
Console 'weather clear'
Console 'gamerule advance_time false'
Console 'gamerule advance_weather false'
Console "forceload add $($BX - 8) $($BZ - 16) $($BX + $Far + 8) $($BZ + 16)"
Console "fill $($BX - 4) $($BY - 1) $($BZ - 10) $($BX + $Far + 4) $($BY - 1) $($BZ + 10) minecraft:smooth_stone"
Console "fill $($BX - 4) $BY $($BZ - 10) $($BX + $Far + 4) $($BY + 14) $($BZ + 10) minecraft:air"
# The pillar the rope hooks: its top block at y BY+5, room over it.
Console "fill $($BX + $Far) $BY $BZ $($BX + $Far) $($BY + 5) $BZ minecraft:polished_deepslate"
# A start block for the rider, so the rope rises a little.
Console "setblock $BX $BY $BZ minecraft:polished_deepslate"

# ---- The bots ----
$riderSock = Launch $Rider
Console "gamemode survival $Rider"
Console ([string]::Format($Inv, 'tp {0} {1} {2} {3}', $Rider, $BX + 0.5, $BY + 1, $BZ + 0.5))
Console "give $Rider rustcraft:zipline 1"
Start-Sleep -Seconds 2
Bot $riderSock 'select rustcraft:zipline' | Out-Null
$camSock = Launch $Camera
Console "gamemode spectator $Camera"
Bot $camSock 'key f1 tap' | Out-Null
Console ([string]::Format($Inv, 'tp {0} {1} {2} {3}', $Camera, $BX + $Far / 2, $BY + 4, $BZ - 11))
Start-Sleep -Seconds 2
Bot $camSock ([string]::Format($Inv, 'look-at {0} {1} {2}', $BX + $Far / 2, $BY + 3, $BZ + 0.5)) | Out-Null
Start-Sleep -Seconds 5

# ---- The ride ----
Bot $riderSock ([string]::Format($Inv, 'look-at {0} {1} {2}', $BX + $Far + 0.5, $BY + 5.9, $BZ + 0.5)) | Out-Null
Start-Sleep -Milliseconds 500
Bot $riderSock 'use-item' | Out-Null
# The seat spawns and mounts in a few ticks; the ride takes about 4 s.
Start-Sleep -Milliseconds 1200
for ($i = 1; $i -le 3; $i++) {
    $file = Join-Path $Out "zipline-$i.png"
    Bot $camSock "screenshot $file" | Out-Null
    Write-Host "shot $file"
    Start-Sleep -Milliseconds 700
}
Bot $riderSock 'messages' | Select-Object -Last 6

# The rope stays tied between its anchors after the ride (Z2).
Start-Sleep -Seconds 3
$file = Join-Path $Out 'zipline-rope.png'
Bot $camSock "screenshot $file" | Out-Null
Write-Host "shot $file"
Console "data get entity $Rider Pos"

# The engineer's handbook, opened by the rider (H2, H5).
Console "give $Rider rustcraft:handbook 1"
Start-Sleep -Seconds 2
Bot $riderSock 'select rustcraft:handbook' | Out-Null
Bot $riderSock 'key f1 tap' | Out-Null
Bot $riderSock 'use-item' | Out-Null
Start-Sleep -Seconds 2
$file = Join-Path $Out 'handbook-contents.png'
Bot $riderSock "screenshot $file" | Out-Null
Write-Host "shot $file"
Bot $riderSock 'screen' | Select-Object -First 4
# A page: the section «Machines», then its first entry (GUI pixels; the window is 320 x 210).
Bot $riderSock 'click-at 86 77' | Out-Null
Start-Sleep -Milliseconds 500
Bot $riderSock 'click-at 233 43' | Out-Null
Start-Sleep -Seconds 1
$file = Join-Path $Out 'handbook-page.png'
Bot $riderSock "screenshot $file" | Out-Null
Write-Host "shot $file"
# The page's last sheet: the guide's animated scene (H3), at two moments of its steps.
Bot $riderSock 'key right tap' | Out-Null
Start-Sleep -Seconds 2
foreach ($n in 1, 2) {
    $file = Join-Path $Out "handbook-scene-$n.png"
    Bot $riderSock "screenshot $file" | Out-Null
    Write-Host "shot $file"
    Start-Sleep -Seconds 5
}
Bot $riderSock 'key esc tap' | Out-Null

foreach ($s in $camSock, $riderSock) {
    Bot $s 'release' | Out-Null
    & $Ctl --socket $s quit 2>&1 | Out-Null
}
Console "forceload remove $($BX - 8) $($BZ - 16) $($BX + $Far + 8) $($BZ + 16)"
