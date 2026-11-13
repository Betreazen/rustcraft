"""Copies the vanilla textures the guide needs out of the owner's own game client.

Reads only the listed PNG files from the client jar, decodes them with the standard library and
writes docs/showcase/local/mc-assets.js: a classic script that sets window.RC_MC to
{ 'block/stone': [width, height, base64 RGBA], ... }. Raw pixels instead of PNG data URIs let the
page build canvases at once, without waiting for images to load.

The output holds Mojang assets and must never be committed: docs/showcase/local/ is in .gitignore.

Run from anywhere:  py -I tools/local-mc-assets/extract.py [--jar PATH] [--out PATH]
"""

import argparse
import base64
import json
import os
import struct
import sys
import zipfile
import zlib

DEFAULT_JAR = os.path.join(
    os.environ.get("APPDATA", ""), "horizon-launcher", "minecraft", "versions", "26.3", "26.3.jar"
)
HERE = os.path.dirname(os.path.abspath(__file__))
DEFAULT_OUT = os.path.normpath(os.path.join(HERE, "..", "..", "docs", "showcase", "local", "mc-assets.js"))
PREFIX = "assets/minecraft/textures/"

# Biome colours of plains, as the game paints grey textures.
GRASS = (0x91, 0xBD, 0x59)
FOLIAGE = (0x77, 0xAB, 0x2F)
WATER = (0x3F, 0x76, 0xE4)

# name in RC_MC -> options. src: texture path when it differs from the name; frame: keep only the
# first square frame of an animation strip; tint: multiply by a colour; over: [path, tint] drawn on
# top; fill: colour put under see-through pixels, because the scene shader ignores alpha.
BLOCKS = {
    "block/stone": {},
    "block/deepslate": {},
    "block/dirt": {},
    "block/bedrock": {},
    "block/grass_block_top": {"tint": GRASS},
    "block/grass_block_side": {"over": ["block/grass_block_side_overlay", GRASS]},
    "block/farmland": {},
    "block/farmland_moist": {},
    "block/oak_planks": {},
    "block/oak_log": {},
    "block/oak_log_top": {},
    "block/oak_leaves": {"tint": FOLIAGE, "fill": (0x2C, 0x45, 0x14)},
    "block/water_still": {"frame": True, "tint": WATER},
    "block/lava_still": {"frame": True},
    "block/glass": {"fill": (0xC4, 0xDE, 0xE8)},
    "block/bricks": {},
    "block/obsidian": {},
    "block/netherrack": {},
    "block/white_wool": {},
    "block/crafting_table_top": {},
    "block/crafting_table_front": {},
    "block/crafting_table_side": {},
    "block/furnace_front": {},
    "block/furnace_front_on": {},
    "block/furnace_side": {},
    "block/furnace_top": {},
    "block/dropper_front": {},
    "block/coal_ore": {},
    "block/iron_ore": {},
    "block/copper_ore": {},
    "block/gold_ore": {},
    "block/redstone_ore": {},
    "block/lapis_ore": {},
    "block/diamond_ore": {},
    "block/emerald_ore": {},
    "block/nether_quartz_ore": {},
    "block/ancient_debris_side": {},
}
# Crop stages keep their see-through pixels: the models read the plant's shape from them.
for _s in range(8):
    BLOCKS["block/wheat_stage%d" % _s] = {}

# Plants of the base game and the blocks they grow on. Cut-out plant textures keep their
# see-through pixels too; full blocks get no fill because their textures are opaque. Stems are
# tinted by the page, because their colour depends on their age.
PLANTS = {
    "block/sugar_cane": {"tint": GRASS},
    "block/cactus_side": {},
    "block/cactus_top": {},
    "block/cactus_bottom": {},
    "block/bamboo_stalk": {},
    "block/bamboo_stage0": {},
    "block/bamboo_small_leaves": {},
    "block/bamboo_large_leaves": {},
    "block/kelp": {"frame": True},
    "block/kelp_plant": {"frame": True},
    "block/pumpkin_side": {},
    "block/pumpkin_top": {},
    "block/melon_side": {},
    "block/melon_top": {},
    "block/pumpkin_stem": {},
    "block/melon_stem": {},
    "block/attached_pumpkin_stem": {},
    "block/attached_melon_stem": {},
    "block/nether_wart_stage0": {},
    "block/nether_wart_stage1": {},
    "block/nether_wart_stage2": {},
    "block/cave_vines": {},
    "block/cave_vines_lit": {},
    "block/cave_vines_plant": {},
    "block/cave_vines_plant_lit": {},
    "block/cocoa_stage0": {},
    "block/cocoa_stage1": {},
    "block/cocoa_stage2": {},
    "block/jungle_log": {},
    "block/jungle_log_top": {},
    "block/soul_sand": {},
    "block/torchflower": {},
    "block/torchflower_crop_stage0": {},
    "block/torchflower_crop_stage1": {},
}
for _s in range(4):
    for _crop in ("carrots", "potatoes", "beetroots", "sweet_berry_bush"):
        PLANTS["block/%s_stage%d" % (_crop, _s)] = {}
BLOCKS.update(PLANTS)

# Blocks round the machines of the Path-of-craft-tree branches (spec section 22).
for _name in ("sand", "gravel", "clay", "suspicious_sand_0", "suspicious_gravel_0", "sandstone", "sandstone_top",
              "beehive_end", "beehive_side", "beehive_front", "beehive_front_honey", "bookshelf"):
    BLOCKS["block/" + _name] = {}

ITEMS = [
    "iron_ingot", "copper_ingot", "gold_ingot", "netherite_ingot", "redstone", "diamond", "quartz",
    "lapis_lazuli", "ender_pearl", "coal", "brick", "stick", "hopper", "bucket", "water_bucket",
    "blaze_rod", "glowstone_dust", "sugar", "bone_meal", "honey_bottle", "nether_wart", "string",
    "leather", "book", "compass_00", "iron_pickaxe", "shears", "bow", "slime_ball", "spider_eye",
    "glistering_melon_slice",
]
ENTITIES = [
    "entity/chest/normal", "entity/player/wide/steve", "entity/zombie/zombie",
    "entity/cow/cow_temperate", "entity/sheep/sheep", "entity/sheep/sheep_wool", "entity/bee/bee",
]


# ---------------------------------------------------------------- PNG decoding
def _paeth(a, b, c):
    p = a + b - c
    pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
    if pa <= pb and pa <= pc:
        return a
    return b if pb <= pc else c


def decode_png(data):
    """Returns (width, height, bytearray RGBA). Handles every non-interlaced PNG."""
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError("not a PNG file")
    pos, idat, palette, trns = 8, [], None, None
    width = height = depth = ctype = interlace = None
    while pos < len(data):
        (length,) = struct.unpack(">I", data[pos:pos + 4])
        kind = data[pos + 4:pos + 8]
        body = data[pos + 8:pos + 8 + length]
        pos += 12 + length
        if kind == b"IHDR":
            width, height, depth, ctype, _, _, interlace = struct.unpack(">IIBBBBB", body)
        elif kind == b"PLTE":
            palette = body
        elif kind == b"tRNS":
            trns = body
        elif kind == b"IDAT":
            idat.append(body)
        elif kind == b"IEND":
            break
    if width is None:
        raise ValueError("PNG without IHDR")
    if interlace:
        raise ValueError("interlaced PNG is not supported")
    channels = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}[ctype]
    bits = channels * depth
    stride = (width * bits + 7) // 8
    step = max(1, bits // 8)
    raw = zlib.decompress(b"".join(idat))
    rows, prev, at = [], bytearray(stride), 0
    for _ in range(height):
        kind = raw[at]
        line = bytearray(raw[at + 1:at + 1 + stride])
        at += 1 + stride
        for i in range(stride):
            a = line[i - step] if i >= step else 0
            b = prev[i]
            c = prev[i - step] if i >= step else 0
            if kind == 1:
                line[i] = (line[i] + a) & 255
            elif kind == 2:
                line[i] = (line[i] + b) & 255
            elif kind == 3:
                line[i] = (line[i] + ((a + b) >> 1)) & 255
            elif kind == 4:
                line[i] = (line[i] + _paeth(a, b, c)) & 255
        rows.append(line)
        prev = line

    def samples(line):
        if depth == 8:
            return list(line)
        if depth == 16:
            return [line[i] for i in range(0, len(line), 2)]
        out, mask = [], (1 << depth) - 1
        for byte in line:
            for shift in range(8 - depth, -1, -depth):
                out.append((byte >> shift) & mask)
        return out

    scale = 255 // ((1 << depth) - 1) if depth < 8 else 1
    tkey = struct.unpack(">H", trns[:2])[0] if trns and ctype == 0 else None
    rgba = bytearray()
    for line in rows:
        s = samples(line)
        for x in range(width):
            px = s[x * channels:(x + 1) * channels]
            if ctype == 3:
                i = px[0]
                r, g, b = palette[3 * i:3 * i + 3]
                a = trns[i] if trns and i < len(trns) else 255
            elif ctype == 0:
                r = g = b = px[0] * scale
                a = 0 if tkey is not None and px[0] == tkey else 255
            elif ctype == 4:
                r = g = b = px[0]
                a = px[1]
            elif ctype == 2:
                r, g, b = px
                a = 255
            else:
                r, g, b, a = px
            rgba += bytes((r, g, b, a))
    return width, height, rgba


# ---------------------------------------------------------------- processing
def tint(rgba, color):
    out = bytearray(rgba)
    for i in range(0, len(out), 4):
        for k in range(3):
            out[i + k] = out[i + k] * color[k] // 255
    return out


def over(base, top):
    out = bytearray(base)
    for i in range(0, len(out), 4):
        a = top[i + 3]
        if a:
            for k in range(3):
                out[i + k] = (top[i + k] * a + out[i + k] * (255 - a)) // 255
            out[i + 3] = max(out[i + 3], a)
    return out


def fill(rgba, color):
    out = bytearray(rgba)
    for i in range(0, len(out), 4):
        a = out[i + 3]
        for k in range(3):
            out[i + k] = (out[i + k] * a + color[k] * (255 - a)) // 255
        out[i + 3] = 255
    return out


def read(jar, path):
    return decode_png(jar.read(PREFIX + path + ".png"))


def build(jar):
    found, missing = {}, []

    def keep(name, w, h, rgba):
        found[name] = [w, h, base64.b64encode(bytes(rgba)).decode("ascii")]

    for name, opt in BLOCKS.items():
        try:
            w, h, px = read(jar, opt.get("src", name))
            if opt.get("frame") and h > w:
                px, h = px[:w * w * 4], w
            if "tint" in opt:
                px = tint(px, opt["tint"])
            if "over" in opt:
                ow, oh, top = read(jar, opt["over"][0])
                if (ow, oh) == (w, h):
                    px = over(px, tint(top, opt["over"][1]))
            if "fill" in opt:
                px = fill(px, opt["fill"])
            keep(name, w, h, px)
        except KeyError:
            missing.append(name)
    for name in ["item/" + n for n in ITEMS] + ENTITIES:
        try:
            keep(name, *read(jar, name))
        except KeyError:
            missing.append(name)
    return found, missing


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--jar", default=DEFAULT_JAR, help="client jar of the game")
    ap.add_argument("--out", default=DEFAULT_OUT, help="script to write")
    args = ap.parse_args()
    if not os.path.isfile(args.jar):
        sys.exit("No client jar at " + args.jar)
    with zipfile.ZipFile(args.jar) as jar:
        found, missing = build(jar)
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    text = (
        "// Generated by tools/local-mc-assets/extract.py from the owner's game client.\n"
        "// Mojang textures: local use only, never commit (docs/showcase/local/ is in .gitignore).\n"
        "window.RC_MC = " + json.dumps(found, separators=(",", ":"), sort_keys=True) + ";\n"
    )
    with open(args.out, "w", encoding="utf-8", newline="\n") as f:
        f.write(text)
    print("Wrote %d textures to %s (%d KB)" % (len(found), args.out, len(text) // 1024))
    if missing:
        print("Not in this client: " + ", ".join(missing))


if __name__ == "__main__":
    main()
