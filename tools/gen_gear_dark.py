"""The energy blades' and shields' look without charge (slice 18, guide `blade`: «без заряда
гаснет», `eshield`: «поле светится, пока есть заряд»): each texture desaturated and darkened as
`<name>_off.png`, its model, and the item's `minecraft:condition` on the custom model data flag
the server sets while it holds charge (rustcraft_gear).

    py tools/gen_gear_dark.py
"""
import colorsys
import json
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent / "mod" / "content" / "assets" / "rustcraft"
ITEMS = [f"blade_{t}" for t in (2, 3, 4)] + [f"eshield_{t}" for t in (2, 3, 4)]


def dark(im: Image.Image) -> Image.Image:
    out = im.copy()
    px = out.load()
    for y in range(out.height):
        for x in range(out.width):
            r, g, b, a = px[x, y]
            if a == 0:
                continue
            h, l, s = colorsys.rgb_to_hls(r / 255, g / 255, b / 255)
            # The bright field loses its colour and glow; the dark hilt stays as it was.
            l = min(l, 0.18 + l * 0.45)
            s *= 0.15
            nr, ng, nb = colorsys.hls_to_rgb(h, l, s)
            px[x, y] = (round(nr * 255), round(ng * 255), round(nb * 255), a)
    return out


def main():
    for name in ITEMS:
        tex = ROOT / "textures" / "item"
        dark(Image.open(tex / f"{name}.png").convert("RGBA")).save(tex / f"{name}_off.png")
        lit = json.loads((ROOT / "models" / "item" / f"{name}.json").read_text(encoding="utf-8"))
        off = {"parent": lit["parent"], "textures": {"layer0": f"rustcraft:item/{name}_off"}}
        (ROOT / "models" / "item" / f"{name}_off.json").write_text(json.dumps(off, indent=2) + "\n", encoding="utf-8")
        item = {
            "model": {
                "type": "minecraft:condition",
                "property": "minecraft:custom_model_data",
                "index": 0,
                "on_true": {"type": "minecraft:model", "model": f"rustcraft:item/{name}"},
                "on_false": {"type": "minecraft:model", "model": f"rustcraft:item/{name}_off"},
            }
        }
        with open(ROOT / "items" / f"{name}.json", "w", encoding="utf-8", newline="\n") as f:
            f.write(json.dumps(item, indent=2) + "\n")


if __name__ == "__main__":
    main()
