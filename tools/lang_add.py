"""Adds lang lines to the content mod's ru_ru and en_us, after the others (stream blocks).

Usage: py tools/lang_add.py key "русский" "English" [key "ru" "en" ...]
"""
import json
import sys
from pathlib import Path

CONTENT = Path(__file__).resolve().parent.parent / "mod/content/assets/rustcraft/lang"
args = sys.argv[1:]
assert len(args) % 3 == 0, "key ru en triples"
for code, idx in (("ru_ru", 1), ("en_us", 2)):
    path = CONTENT / f"{code}.json"
    table = json.loads(path.read_text(encoding="utf-8"))
    for i in range(0, len(args), 3):
        table[args[i]] = args[i + idx]
    path.write_text(json.dumps(table, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
