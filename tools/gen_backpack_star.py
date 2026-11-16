"""Starsteel's backpack (slice 18): its icon is crystallite's backpack recoloured to starsteel's
hue, with its model, item definition and recipe.

    py tools/gen_backpack_star.py
"""
import colorsys
import json
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent / "mod" / "content"
TEX = ROOT / "assets" / "rustcraft" / "textures" / "item"


def mean_hue(im: Image.Image) -> float:
    hs = [colorsys.rgb_to_hsv(r / 255, g / 255, b / 255) for r, g, b, a in im.getdata() if a and max(r, g, b) > 40]
    return sum(h for h, s, v in hs) / max(1, len(hs))


def write(path: Path, data) -> None:
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write(json.dumps(data, indent=2) + "\n")


def main():
    base = Image.open(TEX / "backpack_2.png").convert("RGBA")
    hue = mean_hue(Image.open(TEX / "starsteel.png").convert("RGBA"))
    out = base.copy()
    px = out.load()
    for y in range(out.height):
        for x in range(out.width):
            r, g, b, a = px[x, y]
            if a == 0:
                continue
            h, s, v = colorsys.rgb_to_hsv(r / 255, g / 255, b / 255)
            nr, ng, nb = colorsys.hsv_to_rgb(hue, min(1.0, s * 0.8), v * 0.85)
            px[x, y] = (round(nr * 255), round(ng * 255), round(nb * 255), a)
    out.save(TEX / "backpack_3.png")
    assets = ROOT / "assets" / "rustcraft"
    write(assets / "models" / "item" / "backpack_3.json", {"parent": "minecraft:item/generated", "textures": {"layer0": "rustcraft:item/backpack_3"}})
    write(assets / "items" / "backpack_3.json", {"model": {"type": "minecraft:model", "model": "rustcraft:item/backpack_3"}})
    recipe = {
        "type": "minecraft:crafting_shaped",
        "category": "misc",
        "pattern": ["lbl", "lcl", "lll"],
        "key": {"l": "minecraft:leather", "b": "rustcraft:starsteel", "c": "minecraft:chest"},
        "result": {"id": "rustcraft:backpack_3", "count": 1},
    }
    write(ROOT / "data" / "rustcraft" / "recipe" / "backpack_3.json", recipe)


if __name__ == "__main__":
    main()
