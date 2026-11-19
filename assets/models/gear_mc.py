"""Slice 13: the gear's files for the content mod, written by export_mc.py.

- The technical armour's look on the player: an equipment asset a tier
  (`assets/rustcraft/equipment/tech_t<n>.json`) with its two layers, `humanoid` (helmet,
  chestplate, boots) and `humanoid_leggings`. The game draws them on the vanilla armour boxes, so
  they are textures in the vanilla layout, painted here at twice its 64 × 32 (the boxes keep their
  UVs; a pixel of the figure is 2 × 2 here). The look follows the guide's armour (gear.py): steel
  plates with bevels and seams, trim in the tier's colour, a cyan visor, a yellow energy core on
  the chest, conduits, a buckle and heel vents. Round 32: the glowing pixels are repeated in an
  emissive overlay `<name>_e.png` beside each texture, which the game draws at full brightness.
- Every gear item's item model, the icon from icons_mc.py; the blades are held as swords.
"""
import json

import icons_mc

TIERS = (2, 3, 4)
PIECES = ('helmet', 'chestplate', 'leggings', 'boots')
GLOW, GLOW_HOT = '#ffe14a', '#fff6b8'
VISOR, VISOR_HOT, VISOR_DARK = '#56dbe2', '#b8f6fa', '#2f8f96'
# Steel from the brightest edge to black: the guide's steel, case and plate colours (kit.py).
STEEL = ('#a7adb7', '#8a919d', '#6b7380', '#4a5060', '#2b3038', '#15171c')
# Texture pixels per pixel of the vanilla layout.
S = 2
CLEAR = (0, 0, 0, 0)


def box_faces(u, v, w, h, d):
    """Faces of a box w × h × d at (u, v) in the vanilla layout, in its pixels: top and bottom over
    (u + d, v) and the four sides below, as ModelPart's cube UVs unfold them."""
    return {
        'top': (u + d, v, w, d), 'bottom': (u + d + w, v, w, d),
        'right': (u, v + d, d, h), 'front': (u + d, v + d, w, h),
        'left': (u + d + w, v + d, d, h), 'back': (u + d + w + d, v + d, w, h),
    }


# The armour boxes of HumanoidModel: head (8 × 8 × 8), body (8 × 12 × 4), arm and leg (4 × 12 × 4).
HEAD, BODY = box_faces(0, 0, 8, 8, 8), box_faces(16, 16, 8, 12, 4)
ARM, LEG = box_faces(40, 16, 4, 12, 4), box_faces(0, 16, 4, 12, 4)


def rgba(color, alpha=255):
    return tuple(int(color[i:i + 2], 16) for i in (1, 3, 5)) + (alpha,)


def palette(tier):
    """Plate tones (light edge, face, dark edge) and flat colours of a tier."""
    a = icons_mc.TIER[tier]
    a_light, a_dark = icons_mc.shade(a, 0.32), icons_mc.shade(a, -0.42)
    hi, steel_light, steel, steel_dark, seam, black = STEEL
    return {
        'tone': {
            'bright': (hi, steel_light, steel), 'steel': (steel_light, steel, steel_dark),
            'dark': (steel, steel_dark, seam), 'tier': (a_light, a, a_dark),
        },
        'steel_light': steel_light, 'steel_dark': steel_dark, 'seam': seam, 'black': black,
        'tier_light': a_light, 'tier_dark': a_dark,
    }


class Sheet:
    """A layer's texture and its emissive overlay."""

    def __init__(self):
        self.base = [[CLEAR] * (64 * S) for _ in range(32 * S)]
        self.glow = [[CLEAR] * (64 * S) for _ in range(32 * S)]

    def face(self, region, flip=False):
        return Face(self, region, flip)


class Face:
    """One face of a box, drawn in texture pixels from its top left corner. flip=True mirrors it,
    so a side face is drawn with its front edge on the right whichever side it is."""

    def __init__(self, sheet, region, flip):
        u, v, w, h = region
        self.sheet, self.u, self.v, self.w, self.h, self.flip = sheet, u * S, v * S, w * S, h * S, flip

    def fill(self, x, y, w, h, color, glow=False):
        c = rgba(color)
        for yy in range(max(0, y), min(self.h, y + h)):
            for xx in range(max(0, x), min(self.w, x + w)):
                px = self.u + (self.w - 1 - xx if self.flip else xx)
                self.sheet.base[self.v + yy][px] = c
                self.sheet.glow[self.v + yy][px] = c if glow else CLEAR

    def glow(self, x, y, w, h, color):
        self.fill(x, y, w, h, color, glow=True)

    def plate(self, x, y, w, h, tone):
        """A plate with a bevel: its top and left edge lighter, its bottom and right edge darker."""
        light, mid, dark = tone
        self.fill(x, y, w, h, mid)
        self.fill(x, y, w, 1, light)
        self.fill(x, y, 1, h, light)
        self.fill(x, y + h - 1, w, 1, dark)
        self.fill(x + w - 1, y + 1, 1, h - 1, dark)


def faces(sheet, box, names=('front', 'right', 'left', 'back')):
    """The named faces of a box; the left face is mirrored, so the front edge of both sides is
    their right edge. On a limb `right` is the outer side, as the left limbs are mirrored."""
    return {n: sheet.face(box[n], n == 'left') for n in names}


def visor(f, x, w):
    """The visor band across rows 5..10 of a helmet face: dark frame, cyan glass."""
    f.fill(x, 5, w, 1, STEEL[-1])
    f.fill(x, 10, w, 1, STEEL[-1])
    f.glow(x, 6, w, 1, VISOR_HOT)
    f.glow(x, 7, w, 2, VISOR)
    f.glow(x, 9, w, 1, VISOR_DARK)


def helmet(sheet, p):
    # A closed helmet: a brow band and a crest in the tier's colour, a visor that wraps round to
    # the sides, a vented faceguard, an energy sensor over each ear and a vent at the back.
    t = p['tone']
    f = faces(sheet, HEAD, ('front', 'right', 'left', 'back', 'top'))
    for name in ('front', 'right', 'left', 'back'):
        f[name].plate(0, 0, 16, 16, t['steel'])
        f[name].plate(0, 0, 16, 5, t['tier'])
    front = f['front']
    front.fill(7, 0, 2, 4, p['tier_light'])
    visor(front, 0, 16)
    front.glow(3, 7, 2, 1, VISOR_HOT)
    front.plate(3, 11, 10, 5, t['steel'])
    for x in (5, 7, 8, 10):
        front.fill(x, 12, 1, 3, p['seam'])
    for name in ('right', 'left'):
        side = f[name]
        visor(side, 10, 6)
        side.fill(10, 6, 1, 4, p['black'])
        side.plate(2, 6, 6, 6, t['tier'])
        side.fill(3, 7, 4, 4, p['black'])
        side.glow(4, 8, 2, 2, GLOW)
        side.fill(1, 13, 8, 1, p['seam'])
        side.fill(10, 11, 1, 5, p['steel_dark'])
    back = f['back']
    back.fill(7, 0, 2, 4, p['tier_light'])
    back.fill(1, 7, 14, 1, p['seam'])
    back.fill(4, 9, 8, 6, p['black'])
    for y in (10, 12):
        back.fill(5, y, 6, 1, p['steel_dark'])
    top = f['top']
    top.plate(0, 0, 16, 16, t['steel'])
    top.plate(6, 0, 4, 16, t['tier'])
    top.fill(7, 1, 1, 14, p['tier_light'])
    for x in (2, 13):
        top.fill(x, 1, 1, 14, p['steel_dark'])


def chestplate(sheet, p):
    # A breastplate with two chest plates round a yellow energy core in a frame of the tier's
    # colour, conduits from the core to the sides, a segmented belly and a trim of the tier's
    # colour at the waist; a power pack with two glowing vents on the back; pauldrons of the
    # tier's colour, ribbed elbows and bracers with a glowing strip on the arms.
    t = p['tone']
    f = faces(sheet, BODY, ('front', 'right', 'left', 'back', 'top'))
    for name in ('front', 'right', 'left', 'back'):
        w = 16 if name in ('front', 'back') else 8
        f[name].plate(0, 0, w, 24, t['steel'])
        f[name].fill(0, 0, w, 2, p['steel_dark'])
        f[name].plate(0, 21, w, 3, t['tier'])
    front = f['front']
    front.fill(5, 0, 6, 1, p['black'])
    front.plate(1, 2, 7, 8, t['bright'])
    front.plate(8, 2, 7, 8, t['bright'])
    front.plate(0, 12, 3, 9, t['dark'])
    front.plate(13, 12, 3, 9, t['dark'])
    for y in (13, 16):
        front.plate(3, y, 10, 3, t['bright'])
    front.plate(3, 19, 10, 2, t['steel'])
    front.glow(0, 10, 5, 1, GLOW)
    front.glow(11, 10, 5, 1, GLOW)
    front.fill(4, 5, 8, 8, p['black'])
    front.plate(5, 6, 6, 6, t['tier'])
    front.glow(6, 7, 4, 4, GLOW)
    front.glow(7, 8, 2, 2, GLOW_HOT)
    for name in ('right', 'left'):
        side = f[name]
        side.plate(1, 3, 5, 17, t['dark'])
        for y in (8, 14):
            side.fill(1, y, 5, 1, p['seam'])
        side.glow(4, 10, 4, 1, GLOW)
    back = f['back']
    back.plate(7, 2, 2, 19, t['dark'])
    back.plate(2, 4, 12, 11, t['dark'])
    for x in (4, 9):
        back.fill(x, 6, 3, 7, p['black'])
        back.glow(x + 1, 7, 1, 5, GLOW)
        back.glow(x + 1, 9, 1, 1, GLOW_HOT)
    top = f['top']
    top.plate(0, 0, 16, 8, t['steel'])
    top.fill(4, 2, 8, 4, p['steel_dark'])
    arm = faces(sheet, ARM, ('front', 'right', 'left', 'back', 'top'))
    for name in ('front', 'right', 'left', 'back'):
        a = arm[name]
        a.plate(0, 0, 8, 10, t['tier'])
        a.fill(0, 10, 8, 1, p['black'])
        a.fill(0, 11, 8, 1, p['steel_dark'])
        a.fill(0, 12, 8, 1, p['black'])
        a.fill(0, 13, 8, 1, p['steel_dark'])
        a.plate(0, 14, 8, 6, t['steel'])
        a.plate(0, 20, 8, 2, t['tier'])
    outer = arm['right']
    outer.fill(1, 4, 6, 1, p['tier_dark'])
    outer.glow(2, 6, 4, 1, GLOW)
    outer.plate(1, 15, 6, 4, t['dark'])
    outer.glow(2, 16, 4, 2, GLOW)
    arm['front'].fill(4, 15, 1, 4, p['seam'])
    arm['top'].plate(0, 0, 8, 8, t['tier'])


def leggings(sheet, p):
    # A belt with a buckle of the tier's colour and a glowing pin, hip plates, thigh plates with a
    # stripe of the tier's colour on the outer side, knee caps of the tier's colour, ribbed backs
    # of the knees and shin plates.
    t = p['tone']
    body = faces(sheet, BODY)
    for name, f in body.items():
        w = 16 if name in ('front', 'back') else 8
        f.fill(0, 16, w, 1, p['seam'])
        f.plate(0, 17, w, 4, t['dark'])
        f.plate(0, 21, w, 3, t['steel'])
    body['front'].plate(5, 16, 6, 6, t['tier'])
    body['front'].fill(6, 17, 4, 4, p['black'])
    body['front'].glow(7, 18, 2, 2, GLOW)
    for x in (3, 12):
        body['back'].fill(x, 18, 1, 1, p['steel_light'])
    leg = faces(sheet, LEG)
    for f in leg.values():
        f.plate(0, 0, 8, 8, t['steel'])
        f.fill(0, 8, 8, 6, p['steel_dark'])
        f.plate(0, 14, 8, 10, t['steel'])
    front = leg['front']
    front.plate(1, 1, 6, 6, t['bright'])
    front.plate(1, 8, 6, 6, t['tier'])
    front.fill(3, 10, 2, 2, p['tier_light'])
    front.plate(1, 15, 6, 7, t['bright'])
    leg['right'].plate(3, 1, 2, 6, t['tier'])
    leg['right'].fill(1, 10, 6, 1, p['seam'])
    for y in (8, 10, 12):
        leg['back'].fill(0, y, 8, 1, p['black'])


def boots(sheet, p):
    # Ankle boots, five pixels of the figure high, below the knee caps of the leggings: a cuff of the tier's colour, a toe cap of the tier's
    # colour, a strap with a glowing strip on the outer side, a glowing heel vent and thick black
    # soles with a tread.
    t = p['tone']
    leg = faces(sheet, LEG)
    for f in leg.values():
        f.plate(0, 14, 8, 2, t['tier'])
        f.plate(0, 16, 8, 5, t['steel'])
        f.fill(0, 21, 8, 1, p['steel_dark'])
        f.fill(0, 22, 8, 2, p['black'])
    leg['front'].plate(0, 18, 8, 3, t['tier'])
    leg['front'].fill(2, 18, 4, 1, p['tier_light'])
    leg['right'].plate(1, 16, 6, 3, t['dark'])
    leg['right'].glow(2, 17, 4, 1, GLOW)
    leg['back'].fill(2, 16, 4, 4, p['black'])
    leg['back'].glow(3, 17, 2, 2, GLOW)
    sole = sheet.face(LEG['bottom'])
    sole.fill(0, 0, 8, 8, p['black'])
    for y in (1, 4, 6):
        sole.fill(1, y, 6, 1, p['seam'])


def paint(tier, leggings_layer):
    """The two images (texture, emissive overlay) of a layer of a tier."""
    sheet, p = Sheet(), palette(tier)
    if leggings_layer:
        leggings(sheet, p)
    else:
        helmet(sheet, p)
        chestplate(sheet, p)
        boots(sheet, p)
    return sheet.base, sheet.glow


def write(out):
    for t in TIERS:
        name = 'tech_t%d' % t
        for kind, legs in (('humanoid', False), ('humanoid_leggings', True)):
            base, glow = paint(t, legs)
            out.image('textures/entity/equipment/%s/%s.png' % (kind, name), base)
            out.image('textures/entity/equipment/%s/%s_e.png' % (kind, name), glow)
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
