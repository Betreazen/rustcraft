# Screenshots of RustCraft in the game for the README, on the joint bench (horizon-joint-dev).
# RustCraft's bot is rc_a, its area x 0..1000, z 0..1000 (horizon-joint-dev/README.md).
#
#   cd "C:\Claude code projects\horizon-joint-dev"
#   .\joint.ps1 round -WithClient          # a server and a client with the staged mods
#   .\joint.ps1 serve -Owner rustcraft     # in another window, or in the background
#   .\joint.ps1 open                       # offline mode for the bot
#   & "C:\Claude code projects\rustcraft\tools\bench\shot.ps1" -Out <folder>
#   .\joint.ps1 stop -Owner rustcraft; .\joint.ps1 close
#
# The scene is built through the server's console (no operator rights for the bot): a stone
# platform at y 119 with a magistral, the working machines in front (furnace and crusher on
# crystals, so their parts move), and every slice-12 block in two rows behind. The bot plays
# muted (the owner's rule: master volume 0 in its options.txt).
param(
    [Parameter(Mandatory)] [string]$Out,
    [string]$Name = 'rc_a'
)
$ErrorActionPreference = 'Stop'
$Ctl = "$env:LOCALAPPDATA\Horizon\current\horizon-ctl.exe"
$ServerTool = "$env:LOCALAPPDATA\Horizon\current\horizon-server.exe"
$Server = "$env:LOCALAPPDATA\Horizon\joint-dev\server"
$Instance = "$env:APPDATA\horizon\instances\joint-dev"
$Port = 25570
New-Item -ItemType Directory -Force $Out | Out-Null

function Console([string]$Line) {
    $o = & $ServerTool --dir $Server cmd $Line 2>&1
    if ($LASTEXITCODE -ne 0) { Write-Warning "console: $Line -> $o" }
}

function Client {
    $instances = Get-Content -Raw "$env:APPDATA\horizon-launcher\instances.json" | ConvertFrom-Json
    $version = $instances.instances.'joint-dev'.version
    "$env:APPDATA\horizon-launcher\versions\$version\horizon-client.exe"
}

function Bot([string]$Line, [int]$Timeout = 60) {
    $o = & $Ctl --socket $script:Socket --timeout $Timeout @($Line -split ' ') 2>&1
    if ($LASTEXITCODE -ne 0) { Write-Warning "bot: $Line -> $o" }
    $o
}

# ---- The scene ----
$BX, $BY, $BZ = 100, 120, 100
Console 'time set 6000'
Console 'weather clear'
Console 'gamerule doDaylightCycle false'
Console 'gamerule doWeatherCycle false'
Console "forceload add $($BX - 8) $($BZ - 16) $($BX + 24) $($BZ + 16)"
Console "fill $($BX - 6) $($BY - 1) $($BZ - 12) $($BX + 22) $($BY - 1) $($BZ + 12) minecraft:smooth_stone"
Console "fill $($BX - 6) $BY $($BZ - 12) $($BX + 22) $($BY + 8) $($BZ + 12) minecraft:air"

# The magistral along the front: housings with a node at each end.
Console "setblock $($BX - 2) $BY $($BZ - 2) rustcraft:node"
Console "fill $($BX - 1) $BY $($BZ - 2) $($BX + 17) $BY $($BZ - 2) rustcraft:housing"
Console "setblock $($BX + 18) $BY $($BZ - 2) rustcraft:node"

# Working machines: a crystal powers the machine it touches (E10).
$working = @(
    @('rustcraft:furnace', 'minecraft:raw_iron'),
    @('rustcraft:crusher', 'minecraft:raw_copper'),
    @('rustcraft:alloy', $null),
    @('rustcraft:mixer', $null),
    @('rustcraft:windmill', $null),
    @('rustcraft:portal_dock', $null),
    @('rustcraft:dock_unload', $null),
    @('rustcraft:tank', $null),
    @('rustcraft:turret', $null)
)
for ($i = 0; $i -lt $working.Count; $i++) {
    $px = $BX + 2 * $i
    Console "setblock $px $BY $BZ $($working[$i][0])"
    if ($working[$i][1]) {
        Console "setblock $px $($BY + 1) $BZ rustcraft:crystal"
        Console "item replace block $px $BY $BZ container.0 with $($working[$i][1]) 64"
    }
}

# Every slice-12 block, two rows behind.
$rows = @(
    @('trash', 'disintegrator', 'charger', 'repair', 'kitchen', 'fishery', 'feeder', 'shearer', 'apiary'),
    @('brewer', 'xp_tank', 'enchanter', 'archeo', 'projector', 'builder', 'charge_plate', 'butcher', 'miner')
)
for ($r = 0; $r -lt $rows.Count; $r++) {
    for ($i = 0; $i -lt $rows[$r].Count; $i++) {
        Console "setblock $($BX + 2 * $i) $BY $($BZ + 3 + 3 * $r) rustcraft:$($rows[$r][$i])"
    }
}

# ---- The bot ----
$dir = Join-Path $env:TEMP "rustcraft-bench\bots\$Name"
New-Item -ItemType Directory -Force $dir | Out-Null
$script:Socket = Join-Path $dir 'ctl.sock'
$options = Join-Path $dir 'options.txt'
$lines = if (Test-Path $options) { @(Get-Content $options | Where-Object { $_ -notmatch '^soundCategory_master:' -and $_ -notmatch '^(guiScale|hideGui|fov):' }) } else { @() }
[IO.File]::WriteAllLines($options, [string[]](@($lines) + 'soundCategory_master:0.0' + 'fov:0.0'), [Text.UTF8Encoding]::new($false))
$reply = & $Ctl --socket $script:Socket launch --game (Client) --offscreen --home $Instance --game-dir $Instance `
    --player-dir $dir --name $Name --server "127.0.0.1:$Port" 2>&1
if ($LASTEXITCODE -ne 0) { throw "launch failed: $reply" }
Bot 'wait-ready 180' 200 | Out-Null
Console "gamemode spectator $Name"
# F1 hides the hand and the HUD for every shot.
Bot 'key f1 tap' | Out-Null

# Shots: where the bot stands (x, y, z) and the point it looks at.
$shots = @(
    [pscustomobject]@{ Name = 'overview'; Ax = $BX + 8; Ay = $BY + 7; Az = $BZ - 12; Lx = $BX + 8; Ly = $BY; Lz = $BZ + 3 },
    [pscustomobject]@{ Name = 'working'; Ax = $BX + 6; Ay = $BY + 3; Az = $BZ - 6; Lx = $BX + 6; Ly = $BY; Lz = $BZ },
    [pscustomobject]@{ Name = 'farm'; Ax = $BX + 10; Ay = $BY + 4; Az = $BZ - 1; Lx = $BX + 10; Ly = $BY; Lz = $BZ + 4 },
    [pscustomobject]@{ Name = 'magic'; Ax = $BX + 6; Ay = $BY + 4; Az = $BZ + 2; Lx = $BX + 6; Ly = $BY; Lz = $BZ + 7 }
)
foreach ($s in $shots) {
    Console "tp $Name $($s.Ax) $($s.Ay) $($s.Az)"
    Start-Sleep -Seconds 2
    $inv = [Globalization.CultureInfo]::InvariantCulture
    Bot ([string]::Format($inv, "look-at {0} {1} {2}", $s.Lx + 0.5, $s.Ly + 0.5, $s.Lz + 0.5)) | Out-Null
    # Let the chunks draw and the parts move.
    Start-Sleep -Seconds 6
    $file = Join-Path $Out "$($s.Name).png"
    Bot "screenshot $file" | Out-Null
    Write-Host "shot $file"
}
Bot 'release' | Out-Null
& $Ctl --socket $script:Socket quit 2>&1 | Out-Null
Console "forceload remove $($BX - 8) $($BZ - 16) $($BX + 24) $($BZ + 16)"
