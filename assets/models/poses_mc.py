"""The poses of the handbook's scenes in the game: a block model with its moving parts, as a still.

In the world the moving parts of a block (assets/models/anim_mc.py: a furnace's fire, a crusher's
rolls, a windmill's rotor, a tank's fluid) are drawn by the client's block-entity renderer, so
the block's item icon shows the block without them. The handbook's scenes draw blocks as icons,
so scenes_mc.py asks this script for a still of each block in the state the guide's scene shows
(`on`: working or standing, poses.js): one model `rustcraft:item/hb/<block>_<state>` with the
block's elements and those of the parts shown in that state, level parts at the guide's level,
and an item definition `rustcraft:hb/<block>_<state>` (assets/rustcraft/items/hb/) that the
window draws with `native_item_model` (API 111). A tinted part (the tank's fluid) gets the
guide's colour as a constant tint.

Every file under models/item/hb/ and items/hb/ belongs to this script: a run writes them again.
"""
import json
import re
from pathlib import Path

import anim_mc

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
CONTENT = ROOT / 'mod' / 'content' / 'assets' / 'rustcraft'
BLOCK_MODELS = CONTENT / 'models' / 'block'
OUT_MODELS = CONTENT / 'models' / 'item' / 'hb'
OUT_ITEMS = CONTENT / 'items' / 'hb'
# Blocks whose model is not named as their item (export_mc.py MODEL_BLOCK).
MODEL_OF = {'portal_dock': 'pdock'}
# The guide's levels when a scene gives none (poses.js `level`).
LEVEL = {'tank': 0.75, 'battery': 0.6, 'feeder': 0.7, 'shearer': 0.6, 'apiary': 0.6,
         'brewer': 0.7, 'xp_tank': 0.65}
# A tinted part's colour (the guide's water).
TINT = {'tank': 0x3F76E4}


class Poses:
    def __init__(self):
        self.made = {}

    def look(self, name, on):
        """('model', item, item model) for the block `name` (rustcraft:<name>) working or not, or
        None when it has no moving parts."""
        model = MODEL_OF.get(name, name)
        spec = anim_mc.parts_of(model)
        if not spec or not (BLOCK_MODELS / (model + '.json')).exists():
            return None
        on = bool(on) or any(p['off'] == 'always' for p in spec.values())
        parts = []
        for part, p in spec.items():
            show = p.get('show')
            if show and show[0] == 'eq' and show[3] != 0:
                continue
            if not on and p['off'] == 'hide':
                continue
            parts.append(part)
        key = '%s_%s' % (name, 'on' if on else 'still')
        self.made[key] = (model, parts)
        return ('model', 'rustcraft:' + name, 'rustcraft:hb/' + key)

    def write(self):
        """Writes every still asked for, and removes the ones no scene asks for any more."""
        OUT_MODELS.mkdir(parents=True, exist_ok=True)
        OUT_ITEMS.mkdir(parents=True, exist_ok=True)
        keep = set()
        for key, (model, parts) in sorted(self.made.items()):
            merged, tinted = merge(model, parts)
            keep.add(key + '.json')
            write_json(OUT_MODELS / (key + '.json'), merged)
            item = {'type': 'minecraft:model', 'model': 'rustcraft:item/hb/' + key}
            if tinted:
                item['tints'] = [{'type': 'minecraft:constant', 'value': TINT.get(model, 0xFFFFFF)}]
            write_json(OUT_ITEMS / (key + '.json'), {'model': item})
        for folder in (OUT_MODELS, OUT_ITEMS):
            for f in folder.glob('*.json'):
                if f.name not in keep:
                    f.unlink()
        return len(self.made)


def write_json(path, value):
    text = json.dumps(value, ensure_ascii=False, separators=(',', ':')) + '\n'
    data = text.encode('utf-8')
    if not path.exists() or path.read_bytes() != data:
        path.write_bytes(data)


def merge(model, parts):
    """The block's model with its parts' elements: their textures renamed where they clash,
    level parts squeezed to the guide's level. Returns (model JSON, whether a part is tinted)."""
    base = json.loads((BLOCK_MODELS / (model + '.json')).read_text(encoding='utf-8'))
    textures = dict(base.get('textures', {}))
    elements = list(base.get('elements', []))
    tinted = False
    spec = anim_mc.parts_of(model)
    for part in parts:
        pj = json.loads((BLOCK_MODELS / ('%s__%s.json' % (model, part))).read_text(encoding='utf-8'))
        rename = {}
        for k, v in pj.get('textures', {}).items():
            if k in textures and textures[k] != v:
                rename[k] = '%s_%s' % (part, k)
                textures[rename[k]] = v
            elif k not in textures:
                textures[k] = v
        els = json.loads(re.sub(r'"#(\w+)"', lambda m: '"#%s"' % rename.get(m.group(1), m.group(1)),
                                json.dumps(pj.get('elements', []))))
        if spec[part].get('level'):
            els = squeeze(els, LEVEL.get(model, 1.0))
        tinted |= bool(spec[part].get('tinted'))
        elements += els
    out = {'parent': base.get('parent', 'minecraft:block/block'), 'textures': textures, 'elements': elements}
    if 'display' in base:
        out['display'] = base['display']
    return out, tinted


def squeeze(els, level):
    """Elements stretched upright from their bottom to `level` of their height."""
    bottom = min(e['from'][1] for e in els)
    out = []
    for e in els:
        e = dict(e)
        e['from'] = [e['from'][0], bottom + (e['from'][1] - bottom) * level, e['from'][2]]
        e['to'] = [e['to'][0], bottom + (e['to'][1] - bottom) * level, e['to'][2]]
        out.append(e)
    return out
