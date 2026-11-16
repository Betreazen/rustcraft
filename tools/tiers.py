"""Slice 18: the content of the machines' higher tiers (spec 12.1 U1, guide `swap`).

For every tiered block in mod/rustcraft/rules/src/tiers.rs (FAMILIES) and every tier above its
first, this writes into the content mod:

- data/rustcraft/recipe/<name>_<tier>.json: the first tier's recipe with the tier's alloy in place
  of the first tier's (U1: «тот же рецепт, сплав этой ступени»);
- assets/rustcraft/items/<name>_<tier>.json: the item looks like the block's first tier;
- data/rustcraft/loot_table/blocks/<name>.json: the broken block drops its own tier's item;
- the lang lines «<name> · ступень N» / «<name> · tier N» for each new item.

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
            "conditions": [
                {"condition": "minecraft:block_state_property", "block": block, "properties": {"tier": str(tier)}}
            ],
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


def main():
    fams = families()
    for block, first in fams:
        recipes(block, first)
        item_models(block, first)
        loot(block, first)
    lang(fams)
    print(f"{len(fams)} families written")


if __name__ == "__main__":
    main()
