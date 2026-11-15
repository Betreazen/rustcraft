"""Slice 13: the gear's files for the content mod, written by export_mc.py.

- The technical armour's look on the player: an equipment asset a tier
  (`assets/rustcraft/equipment/tech_t<n>.json`) with its two layers, `humanoid` (helmet,
  chestplate, boots) and `humanoid_leggings`, painted here in the vanilla 64 × 32 layout in the
  colours of the guide's armour (gear.py, icons.js): the plates in the tier's colour, darker seams,
  the charge strip yellow on the chest and the visor cyan, as the guide shows them.
- Every gear item's item model, the icon from icons_mc.py; the blades are held as swords.
"""
import json

import icons_mc

TIERS = (2, 3, 4)
PIECES = ('helmet', 'chestplate', 'leggings', 'boots')
GLOW = '#ffe14a'
VISOR = '#56dbe2'

# Faces of a box in the vanilla texture layout: a box w × h × d at (u, v) unfolds into
# top/bottom over (u + d, v) and the four sides below, as ModelPart's cube UVs do.
def box_faces(u, v, w, h, d):
    return {
        'top': (u + d, v, w, d), 'bottom': (u + d + w, v, w, d),
        'right': (u, v + d, d, h), 'front': (u + d, v + d, w, h),
        'left': (u + d + w, v + d, d, h), 'back': (u + d + w + d, v + d, w, h),
    }


# The armour boxes of HumanoidModel: head (8 × 8 × 8), body (8 × 12 × 4), arm and leg (4 × 12 × 4).
LAYER1 = {'head': box_faces(0, 0, 8, 8, 8), 'body': box_faces(16, 16, 8, 12, 4),
          'arm': box_faces(40, 16, 4, 12, 4), 'leg': box_faces(0, 16, 4, 12, 4)}
LAYER2 = {'body': box_faces(16, 16, 8, 12, 4), 'leg': box_faces(0, 16, 4, 12, 4)}


def rgba(color, alpha=255):
    return tuple(int(color[i:i + 2], 16) for i in (1, 3, 5)) + (alpha,)


def paint(layer, tier, leggings):
    a = icons_mc.TIER[tier]
    light, dark = icons_mc.shade(a, 0.3), icons_mc.shade(a, -0.45)
    px = [[(0, 0, 0, 0)] * 64 for _ in range(32)]
    for part, faces in layer.items():
        for name, (u, v, w, h) in faces.items():
            for y in range(v, v + h):
                for x in range(u, u + w):
                    edge = x in (u, u + w - 1) or y in (v, v + h - 1)
                    c = dark if edge else (light if name in ('top', 'front') and y < v + 2 else a)
                    px[y][x] = rgba(c)
            if name != 'front':
                continue
            if part == 'head':
                for x in range(u + 1, u + w - 1):
                    px[v + 4][x] = rgba(VISOR)
                    px[v + 5][x] = rgba(icons_mc.shade(VISOR, -0.3))
            if part == 'body' and not leggings:
                for x in range(u + 2, u + w - 2):
                    px[v + 3][x] = rgba(GLOW)
                px[v + 4][u + 3] = px[v + 4][u + 4] = rgba(GLOW)
            if part == 'body' and leggings:
                for x in range(u + 1, u + w - 1):
                    px[v + h - 3][x] = rgba(GLOW)
            if part == 'leg':
                px[v + h - 3][u + 1] = px[v + h - 3][u + 2] = rgba(GLOW)
    return px


def write(out):
    for t in TIERS:
        name = 'tech_t%d' % t
        out.image('textures/entity/equipment/humanoid/%s.png' % name, paint(LAYER1, t, False))
        out.image('textures/entity/equipment/humanoid_leggings/%s.png' % name, paint(LAYER2, t, True))
        out.text('equipment/%s.json' % name, json.dumps({'layers': {
            'humanoid': [{'texture': 'rustcraft:' + name}],
            'humanoid_leggings': [{'texture': 'rustcraft:' + name}],
        }}, indent=2) + '\n')
    names = ['tech_%s_%d' % (p, t) for t in TIERS for p in PIECES]
    names += ['blade_%d' % t for t in TIERS] + ['eshield_%d' % t for t in TIERS]
    names += ['rifle', 'zipline', 'jetpack', 'book_capacity', 'book_thrift', 'module_rare', 'module_pick', 'module_stratum', 'module_tunnel', 'module_height', 'module_lava', 'module_build', 'module_recipes', 'tool_map', 'tool_bore', 'scout_compass', 'harpoon', 'tablet', 'handbook'] + list(icons_mc.PM_SIGNS)
    for n in names:
        parent = 'minecraft:item/handheld' if n.startswith(('blade', 'rifle')) else 'minecraft:item/generated'
        out.text('models/item/%s.json' % n, json.dumps({'parent': parent, 'textures': {'layer0': 'rustcraft:item/' + n}}, indent=2) + '\n')
        out.text('items/%s.json' % n, json.dumps({'model': {'type': 'minecraft:model', 'model': 'rustcraft:item/' + n}}, indent=2) + '\n')
    return names
