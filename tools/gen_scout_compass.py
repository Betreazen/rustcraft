"""The scout compass's 32 needle frames (slice 18, guide `scout_compass`: the needle points at the
target). Each frame is the item's texture with its dial cleared and the needle drawn turned
clockwise by i/32 of a turn from straight up, as the item model's `minecraft:compass` value reads
(0 = the target straight ahead). Also writes the frame models and the item's range_dispatch.

    py tools/gen_scout_compass.py
"""
import json
import math
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent / "mod" / "content" / "assets" / "rustcraft"
FRAMES = 32
CENTRE = (8.0, 9.0)  # the dial's middle, in pixel edges
DIAL = 3.2
DARK = (21, 23, 28, 255)
TIP = (224, 86, 74, 255)
TAIL = (255, 255, 255, 255)
HUB = (154, 163, 178, 255)
NEEDLE = {TIP[:3], TAIL[:3], HUB[:3]}


def segment_distance(p, a, b):
    ax, ay = a
    bx, by = b
    px, py = p
    dx, dy = bx - ax, by - ay
    t = max(0.0, min(1.0, ((px - ax) * dx + (py - ay) * dy) / (dx * dx + dy * dy)))
    return math.hypot(px - (ax + t * dx), py - (ay + t * dy))


def frame(base: Image.Image, i: int) -> Image.Image:
    im = base.copy()
    a = 2 * math.pi * i / FRAMES
    way = (math.sin(a), -math.cos(a))
    cx, cy = CENTRE
    tip = (cx + way[0] * 2.9, cy + way[1] * 2.9)
    tail = (cx - way[0] * 2.4, cy - way[1] * 2.4)
    for y in range(16):
        for x in range(16):
            p = (x + 0.5, y + 0.5)
            r = math.hypot(p[0] - cx, p[1] - cy)
            if r <= DIAL + 1.3 and im.getpixel((x, y))[:3] in NEEDLE:
                im.putpixel((x, y), DARK)
            if r > DIAL:
                continue
            if segment_distance(p, (cx, cy), tip) < 0.62:
                im.putpixel((x, y), TIP)
            elif segment_distance(p, (cx, cy), tail) < 0.62:
                im.putpixel((x, y), TAIL)
    hub = (int(cx - 0.5), int(cy - 0.5))
    im.putpixel(hub, HUB)
    return im


def main():
    base = Image.open(ROOT / "textures" / "item" / "scout_compass.png").convert("RGBA")
    entries = []
    for i in range(FRAMES):
        name = f"scout_compass_{i:02d}"
        frame(base, i).save(ROOT / "textures" / "item" / f"{name}.png")
        model = {"parent": "minecraft:item/generated", "textures": {"layer0": f"rustcraft:item/{name}"}}
        (ROOT / "models" / "item" / f"{name}.json").write_text(json.dumps(model, indent=2) + "\n", encoding="utf-8")
        threshold = 0.0 if i == 0 else i - 0.5
        entries.append({"threshold": threshold, "model": {"type": "minecraft:model", "model": f"rustcraft:item/{name}"}})
    entries.append({"threshold": FRAMES - 0.5, "model": {"type": "minecraft:model", "model": "rustcraft:item/scout_compass_00"}})
    item = {
        "model": {
            "type": "minecraft:range_dispatch",
            "property": "minecraft:compass",
            "target": "lodestone",
            "scale": FRAMES,
            "entries": entries,
            "fallback": {"type": "minecraft:model", "model": "rustcraft:item/scout_compass"},
        }
    }
    (ROOT / "items" / "scout_compass.json").write_text(json.dumps(item, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
