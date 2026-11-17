"""Models of the gear seen on a player (slice 18): the jetpack on the back, its flame, the
starsteel backpack and the zipline's handle with its rollers. They are standalone block models
(API 151) the client draws after the player's model (rustcraft_gear::draw_looks); coordinates are
block sixteenths with the player's axis at x 8, z 8, feet at y 0, +z the way they face (the
handle is drawn a block higher: models stay within -16..32).

    py tools/gen_worn_models.py
"""
import json
from pathlib import Path

OUT = Path(__file__).resolve().parent.parent / "mod" / "content" / "assets" / "rustcraft" / "models" / "block"


def box(frm, to, tex):
    faces = {}
    for face in ("north", "south", "east", "west", "up", "down"):
        faces[face] = {"uv": [0, 0, 16, 16], "texture": f"#{tex}"}
    return {"from": frm, "to": to, "faces": faces}


MODELS = {
    "worn_jetpack": (
        {"metal": "minecraft:block/iron_block", "trim": "minecraft:block/orange_terracotta"},
        [
            box([5.0, 13.0, 3.0], [7.5, 22.5, 5.5], "metal"),
            box([8.5, 13.0, 3.0], [11.0, 22.5, 5.5], "metal"),
            box([7.5, 15.0, 5.0], [8.5, 21.0, 6.0], "trim"),
            box([5.25, 12.0, 3.25], [7.25, 13.0, 5.25], "trim"),
            box([8.75, 12.0, 3.25], [10.75, 13.0, 5.25], "trim"),
        ],
    ),
    "worn_flame": (
        {"flame": "minecraft:block/glowstone"},
        [box([5.5, 8.0, 3.5], [7.0, 12.0, 5.0], "flame"), box([9.0, 8.0, 3.5], [10.5, 12.0, 5.0], "flame")],
    ),
    "worn_backpack": (
        {"pack": "minecraft:block/brown_wool", "metal": "minecraft:block/netherite_block"},
        [
            box([4.0, 12.0, 2.0], [12.0, 23.0, 5.75], "pack"),
            box([3.5, 16.0, 1.5], [12.5, 17.0, 6.0], "metal"),
            box([5.0, 13.0, 1.5], [11.0, 15.0, 2.0], "metal"),
        ],
    ),
    "worn_handle": (
        {"metal": "minecraft:block/iron_block", "roller": "minecraft:block/black_concrete"},
        [
            box([2.0, 16.0, 7.5], [14.0, 17.0, 8.5], "metal"),
            box([11.5, 9.0, 7.5], [12.5, 16.0, 8.5], "metal"),
        ],
    ),
    # Wave 2: one roller, drawn twice over the bar and turned as it rolls along the rope
    # (rustcraft_gear::draw_looks turns it about its middle, x 4.5, y 18.5, z 8).
    "worn_handle_roller": (
        {"roller": "minecraft:block/black_concrete"},
        [
            box([3.0, 17.0, 6.5], [6.0, 20.0, 9.5], "roller"),
        ],
    ),
}


def main():
    for name, (textures, elements) in MODELS.items():
        model = {"textures": {"particle": next(iter(textures.values())), **textures}, "elements": elements}
        with open(OUT / f"{name}.json", "w", encoding="utf-8", newline="\n") as f:
            f.write(json.dumps(model, indent=2) + "\n")


if __name__ == "__main__":
    main()
