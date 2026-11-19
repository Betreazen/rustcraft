"""Slice 18: the content of the machines' higher tiers (spec 12.1 U1, guide `swap`).

For every tiered block in mod/rustcraft/rules/src/tiers.rs (FAMILIES) and every tier above its
first, this writes into the content mod:

- data/rustcraft/recipe/<name>_<tier>.json: the first tier's recipe with the tier's alloy in place
  of the first tier's (U1: «тот же рецепт, сплав этой ступени»);
- assets/rustcraft/items/<name>_<tier>.json: the item looks like the block's first tier;
- data/rustcraft/loot_table/blocks/<name>.json: the broken block drops its own tier's item;
- the lang lines «<name> · ступень N» / «<name> · tier N» for each new item;
- the stripe of a higher tier on the block (the handbook's `machine` models: «a machine above the
  first tier wears a stripe in the colour of its alloy»): a model and texture a tier, and each
  family's blockstate as a multipart with the stripe on its front for tiers 2-4. Run it again
  after the model exporter rewrites a blockstate.

Run from anywhere: `py tools/tiers.py`. It writes the same files each time.
"""

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RULES = ROOT / "mod/rustcraft/rules/src/tiers.rs"
CONTENT = ROOT / "mod/content"
ALLOYS = ["rustcraft:coppersteel", "rustcraft:sparksteel", "rustcraft:crystallite", "rustcraft:starsteel"]
TIER_WORD = {"ru_ru": "ступень", "en_us": "tier"}


def families():
    text = RULES.read_text(encoding="utf-8")
    body = text.split("pub const FAMILIES", 1)[1].split("];", 1)[0]
    return [(b, int(t)) for b, t in re.findall(r'Family::new\("([^"]+)",\s*(\d)\)', body)]


def write_json(path: Path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")


def item(block, first, tier):
    return block if tier == first else f"{block}_{tier}"


def recipes(block, first):
    name = block.split(":", 1)[1]
    source = CONTENT / f"data/rustcraft/recipe/{name}.json"
    if not source.exists():
        print(f"no recipe for {block}: skipped")
        return
    base = json.loads(source.read_text(encoding="utf-8"))
    for tier in range(first + 1, 5):
        r = json.loads(json.dumps(base))
        r["key"] = {k: (ALLOYS[tier - 1] if v == ALLOYS[first - 1] else v) for k, v in r["key"].items()}
        r["result"]["id"] = item(block, first, tier)
        r["result"]["count"] = 1
        write_json(CONTENT / f"data/rustcraft/recipe/{name}_{tier}.json", r)


def item_models(block, first):
    name = block.split(":", 1)[1]
    source = CONTENT / f"assets/rustcraft/items/{name}.json"
    model = json.loads(source.read_text(encoding="utf-8"))
    for tier in range(first + 1, 5):
        write_json(CONTENT / f"assets/rustcraft/items/{name}_{tier}.json", model)


def loot(block, first):
    name = block.split(":", 1)[1]
    children = [
        {
            "type": "minecraft:item",
            "name": item(block, first, tier),
            # 26.3's form (SRC crates/horizon-port-pack/src/carry.rs:1270-1276): one `condition`
            # object; the old `conditions` list with `block_state_property` never holds there.
            "condition": {"type": "minecraft:match_block", "blocks": block, "state": {"tier": str(tier)}},
        }
        for tier in range(first + 1, 5)
    ]
    # Anything else (its first tier, a tier an old world saved below it) drops the first tier.
    children.append({"type": "minecraft:item", "name": block})
    table = {
        "type": "minecraft:block",
        "pools": [
            {
                "condition": {"type": "minecraft:survives_explosion"},
                "entries": [{"type": "minecraft:alternatives", "children": children}],
                "rolls": 1,
            }
        ],
        "random_sequence": f"rustcraft:blocks/{name}",
    }
    write_json(CONTENT / f"data/rustcraft/loot_table/blocks/{name}.json", table)


def lang(fams):
    for code, word in TIER_WORD.items():
        path = CONTENT / f"assets/rustcraft/lang/{code}.json"
        table = json.loads(path.read_text(encoding="utf-8"))
        for block, first in fams:
            name = table.get("block." + block.replace(":", "."), block)
            for tier in range(first + 1, 5):
                key = "item." + item(block, first, tier).replace(":", ".")
                table[key] = f"{name} · {word} {tier}"
        path.write_text(json.dumps(table, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")


# The alloys' colours (docs/showcase/js/models.js TIER).
TIER_COLOUR = {2: (0xE2, 0xBD, 0x55), 3: (0x69, 0xD2, 0xD8), 4: (0xA1, 0x84, 0xF2)}


def png(path: Path, rgb):
    import struct
    import zlib

    w = h = 16
    raw = b"".join(b"\x00" + bytes(rgb) * w for _ in range(h))

    def chunk(kind, data):
        c = struct.pack(">I", len(data)) + kind + data
        return c + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)

    data = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
    data += chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)


def stripes():
    for tier, rgb in TIER_COLOUR.items():
        png(CONTENT / f"assets/rustcraft/textures/block/tier_stripe_{tier}.png", rgb)
        tex = f"rustcraft:block/tier_stripe_{tier}"

        def face(uv):
            return {"uv": uv, "texture": "#stripe"}

        write_json(
            CONTENT / f"assets/rustcraft/models/block/tier_stripe_{tier}.json",
            {
                "textures": {"stripe": tex, "particle": tex},
                # A quarter pixel in from the sides and the floor of the block in front, which
                # the stripe reaches into: a block there hides those faces instead of sharing
                # their planes (round 31, z-fighting).
                "elements": [
                    {
                        "from": [0.25, 0.25, -0.5],
                        "to": [15.75, 2.5, 0],
                        "faces": {
                            "north": face([0, 0, 16, 2.5]),
                            "east": face([0, 0, 0.5, 2.5]),
                            "west": face([0, 0, 0.5, 2.5]),
                            "up": face([0, 0, 16, 0.5]),
                            "down": face([0, 0, 16, 0.5]),
                        },
                    }
                ],
            },
        )


def blockstate(block, first):
    """The family's blockstate as a multipart: its own model by facing, and the stripe of tiers
    2-4 on its front."""
    name = block.split(":", 1)[1]
    path = CONTENT / f"assets/rustcraft/blockstates/{name}.json"
    state = json.loads(path.read_text(encoding="utf-8"))
    if "variants" in state:
        parts = []
        for key, variant in state["variants"].items():
            when = dict(kv.split("=", 1) for kv in key.split(",") if "=" in kv)
            parts.append({"when": when, "apply": variant} if when else {"apply": variant})
    else:
        parts = [p for p in state["multipart"] if "tier_stripe" not in json.dumps(p)]
    turns = {"north": 0, "east": 90, "south": 180, "west": 270}
    for tier in range(max(first, 2), 5):
        for facing, y in turns.items():
            apply = {"model": f"rustcraft:block/tier_stripe_{tier}"}
            if y:
                apply["y"] = y
            parts.append({"when": {"tier": str(tier), "facing": facing}, "apply": apply})
    write_json(path, {"multipart": parts})


def main():
    fams = families()
    stripes()
    for block, first in fams:
        recipes(block, first)
        item_models(block, first)
        loot(block, first)
        blockstate(block, first)
    lang(fams)
    print(f"{len(fams)} families written")


if __name__ == "__main__":
    main()
