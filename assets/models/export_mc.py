"""Exports the RustCraft block models to Minecraft block models for the Horizon content mod.

Run with plain Python 3 from anywhere; no Blender is needed:

    py -I assets/models/export_mc.py

It reads the same box definitions build.py turns into Blender objects (transport.py, machines.py,
energy.py, storage.py, branches.py and kit.py) and rewrites, under
mod/content/assets/rustcraft/:

- models/block/*.json and textures/block/*.png: every block model in the vanilla format, with one
  palette texture per model; the files the housing and the node use, and the rest staged for
  later blocks;
- blockstates/housing.json, node.json, crystal.json and the machines' (furnace, crusher, gen_fuel,
  alloy, crafter, pump, terminal, tank, station), and monorail.json: the rail's parts by tier,
  turned onto its mount (slice 6);
- the capsule's cabin and brackets as standalone models the client draws round its seat;
- the zipline's rope, its anchor (a block with its blockstate) and the harpoon's tip;
- textures/item/*.png: the guide's icons of the channels, the wrench, the dusts, the alloys and the
  capacity module (icons_mc.py).

Every box becomes one element. Blender is Z up and Minecraft Y up: a point (x, y, z) of the model
files becomes (x, z, y) in Minecraft, so x+ of a model is east, y+ south and z+ up. That swap is
a mirror, the same one the guide's left-handed scenes make, so a block in the game looks as the
guide shows it. Colours are flat: each material is a 2 x 2 cell of a 16 x 16 palette texture, and
every face samples the middle of its material's cell. glow_* materials get light_emission 15.
Tinted materials take the tier-1 colour, which the palette already gives them; the node has one
model per tier. The guide's parts that a block shows only in some states, or moves, are listed at
the end of the run; the ones a block does not show at rest are left out of its model.

Gear (gear.py) is worn by the player and is not a block, so it is not exported.
"""
import fnmatch
import json
import math
import struct
import sys
import zlib
from pathlib import Path

HERE = Path(__file__).resolve().parent
ASSETS = HERE.parent.parent / 'mod' / 'content' / 'assets' / 'rustcraft'
sys.dont_write_bytecode = True
sys.path.insert(0, str(HERE))

import anim_mc  # noqa: E402
import branches  # noqa: E402
import energy  # noqa: E402
import gear  # noqa: E402
import gear_mc  # noqa: E402
import icons_mc  # noqa: E402
import kit  # noqa: E402
import machines  # noqa: E402
import storage  # noqa: E402
import transport  # noqa: E402

BLOCK_SECTIONS = (transport, machines, energy, storage, branches)
TIER = icons_mc.TIER
# Element corners must stay in -16..32 (horizon-assets model.rs, Element::decode).
LOW, HIGH = -16, 32
PAGE, CELL = 16, 2
PER_PAGE = (PAGE // CELL) ** 2
GLOW = 15
EPS = 1e-9
# Roof plates and lenses stand up to half a pixel out of the block; the report names parts that
# stand out further.
OVERHANG = 0.5


def palette():
    """The colours of build.py: kit's and every section's; a name means one colour everywhere."""
    colors = dict(kit.COLORS)
    for section in BLOCK_SECTIONS + (gear,):
        for name, color in section.COLORS.items():
            assert colors.setdefault(name, color) == color, 'colour %s differs between sections' % name
    return colors


COLORS = palette()

# ---------------------------------------------------------------- reading the models


class Recorder:
    """Stands in for build.Model: keeps every box, after its tilt and quarter turns, in pixels.
    A box is (part, material, lo, hi, rot), Z up; rot is None or (axis vector, degrees, origin)."""

    def __init__(self):
        self.boxes = []
        self.pivots = {}

    def pivot(self, part, x, y, z):
        """The point the guide turns a part about; used here only for REST_TURN."""
        self.pivots[part] = (x, y, z)

    def box(self, mat, x, y, z, w, d, h, part='body', tilt=None, pivot=None, ring=False, quarter=0):
        lo, hi = (x, y, z), (x + w, y + d, z + h)
        for turn in range(quarter, quarter + (4 if ring else 1)):
            q = turn % 4
            a, b = turned(lo, q), turned(hi, q)
            box_lo = tuple(min(a[i], b[i]) for i in range(3))
            box_hi = tuple(max(a[i], b[i]) for i in range(3))
            rot = None
            if tilt:
                axis = {'X': (1, 0, 0), 'Y': (0, 1, 0), 'Z': (0, 0, 1)}[tilt[0]]
                at = pivot or (x + w / 2, y + d / 2, z)
                rot = (turned(axis, q, about=0), tilt[1], turned(at, q))
            self.boxes.append((part, mat, box_lo, box_hi, rot))


def turned(p, q, about=8):
    """p after q quarter turns about the vertical through (about, about): (x, y) -> (-y, x)."""
    x, y, z = p
    for _ in range(q):
        x, y = 2 * about - y, x
    return (x, y, z)


# Models scaled about the middle of their floor to fit the game's limit of two blocks above the
# block (LOW..HIGH). Slice 18: none; the windmill keeps the guide's three blocks (LIFTED below).
SCALED = {}
# Slice 18: models taller than the game's limit (an element reaches at most 32 pixels up). The
# boxes that reach above `top` pixels become part 'top' (unless they belong to a moving part), and
# the parts named here are lowered by `by` pixels in their own models; anim_mc moves them back up
# (still_move), so the windmill stands three blocks tall as in the guide.
LIFTED = {'windmill': {'top': 32, 'by': 16, 'parts': ('top', 'rotor')}}


def scaled(p, k):
    return (8 + (p[0] - 8) * k, 8 + (p[1] - 8) * k, p[2] * k)


def read(draw, name=None, rest_turn=True):
    """The boxes of a model; with its name, parts the guide holds turned at rest are turned so
    (unless rest_turn is off: a moving part's own model is drawn unturned)."""
    return read_full(draw, name, rest_turn)[0]


def read_full(draw, name=None, rest_turn=True):
    """(boxes, pivots) of a model, scaled as read() scales them."""
    rec = Recorder()
    draw(rec)
    turns = REST_TURN.get(name, {}) if rest_turn else {}
    k = SCALED.get(name)
    boxes = []
    lift = LIFTED.get(name)
    for part, mat, lo, hi, rot in rec.boxes:
        if lift and part == 'body' and hi[2] > lift['top']:
            part = 'top'
        if lift and part in lift['parts']:
            lo, hi = (lo[0], lo[1], lo[2] - lift['by']), (hi[0], hi[1], hi[2] - lift['by'])
            if rot:
                rot = (rot[0], rot[1], (rot[2][0], rot[2][1], rot[2][2] - lift['by']))
        if k:
            lo, hi = scaled(lo, k), scaled(hi, k)
            if rot:
                rot = (rot[0], rot[1], scaled(rot[2], k))
        if part in turns:
            assert rot is None, 'a tilted box in a part turned at rest: %s %s' % (name, part)
            axis, degrees = turns[part]
            rot = ({'X': (1, 0, 0), 'Y': (0, 1, 0), 'Z': (0, 0, 1)}[axis], degrees, rec.pivots[part])
        boxes.append((part, mat, lo, hi, rot))
    pivots = {part: (scaled(p, k) if k else p) for part, p in rec.pivots.items()}
    if lift:
        pivots = {part: ((p[0], p[1], p[2] - lift['by']) if part in lift['parts'] else p) for part, p in pivots.items()}
    return boxes, pivots


# ---------------------------------------------------------------- to Minecraft


def swap(p):
    return (p[0], p[2], p[1])


def quarter_mc(lo, hi, axis, n, origin):
    """The box lo..hi after n quarter turns about `axis` through `origin`, right-handed as
    Horizon's ElementMatrix turns an element."""
    i, j = {0: (1, 2), 1: (2, 0), 2: (0, 1)}[axis]
    c, s = [(1, 0), (0, 1), (-1, 0), (0, -1)][n % 4]
    corners = []
    for k in range(8):
        p = [hi[a] if k >> a & 1 else lo[a] for a in range(3)]
        u, v = p[i] - origin[i], p[j] - origin[j]
        p[i], p[j] = origin[i] + c * u - s * v, origin[j] + s * u + c * v
        corners.append(p)
    return (tuple(min(p[a] for p in corners) for a in range(3)), tuple(max(p[a] for p in corners) for a in range(3)))


def to_mc(box):
    """(from, to, rotation) of a box in Minecraft numbers. The swap of y and z is a mirror, so a
    turn by a degrees about an axis becomes a turn by -a about the swapped axis. Whole quarter
    turns go into the box itself, so the angle left is within -45..45, as vanilla wants it."""
    part, mat, lo, hi, rot = box
    lo, hi = swap(lo), swap(hi)
    if not rot:
        return lo, hi, None
    vec, angle, origin = rot
    vec, origin = swap(vec), swap(origin)
    axis = next(a for a in range(3) if vec[a])
    angle = -angle * vec[axis]
    k = math.floor(angle / 90 + 0.5)
    rest = angle - 90 * k
    if k % 4:
        lo, hi = quarter_mc(lo, hi, axis, k, origin)
    if abs(rest) < EPS:
        return lo, hi, None
    return lo, hi, ('xyz'[axis], rest, origin)


# Minecraft face: (axis, high side?)
FACES = {'down': (1, False), 'up': (1, True), 'north': (2, False), 'south': (2, True), 'west': (0, False), 'east': (0, True)}


def hidden(face, index, lo, hi, solids):
    """True when another unturned element hides the face: the face lies inside it, or on a face of
    it turned the same way that covers it. Of two such faces in one plane only the larger, or of
    two equal ones the later, is kept, as the guide's depth test keeps the later one; the game would
    otherwise flicker between them (the collars of the housing's arms over the beam ends)."""
    axis, high = FACES[face]
    plane = hi[axis] if high else lo[axis]
    others = [a for a in range(3) if a != axis]
    area = (hi[others[0]] - lo[others[0]]) * (hi[others[1]] - lo[others[1]])
    for other, s_lo, s_hi in solids:
        if other == index or not all(s_lo[a] <= lo[a] + EPS and hi[a] <= s_hi[a] + EPS for a in others):
            continue
        if s_lo[axis] < plane - EPS and plane + EPS < s_hi[axis]:
            return True
        if abs((s_hi[axis] if high else s_lo[axis]) - plane) < EPS:
            s_area = (s_hi[others[0]] - s_lo[others[0]]) * (s_hi[others[1]] - s_lo[others[1]])
            if s_area > area + EPS or (abs(s_area - area) <= EPS and other > index):
                return True
    return False


def cull(face, lo, hi):
    """The side of the block a face lies on, if it lies on one and within it."""
    axis, high = FACES[face]
    plane = hi[axis] if high else lo[axis]
    if plane != (16 if high else 0):
        return None
    if all(0 <= lo[a] and hi[a] <= 16 for a in range(3) if a != axis):
        return face
    return None


def num(v):
    r = round(v, 4)
    return int(r) if r == int(r) else r


class Skin:
    """The palette of one model: a colour per material, in cells of 16 x 16 pages."""

    def __init__(self, texture, tints=None, alpha=255):
        self.texture = texture
        self.tints = tints or {}
        self.alpha = alpha
        self.colors = []

    def color(self, mat):
        return self.tints[mat] if mat in self.tints else COLORS[mat]

    def slot(self, mat):
        """(page key, uv) of the material's cell."""
        color = self.color(mat)
        if color not in self.colors:
            self.colors.append(color)
        i = self.colors.index(color)
        page, cell = divmod(i, PER_PAGE)
        x, y = cell % (PAGE // CELL) * CELL, cell // (PAGE // CELL) * CELL
        half = CELL / 2
        return 'p%d' % page, [x + half - 0.5, y + half - 0.5, x + half + 0.5, y + half + 0.5]

    def page_names(self):
        count = max(1, -(-len(self.colors) // PER_PAGE))
        return [self.texture if p == 0 else '%s_p%d' % (self.texture, p) for p in range(count)]

    def textures(self):
        names = self.page_names()
        out = {'particle': 'rustcraft:block/' + names[0]}
        out.update(('p%d' % p, 'rustcraft:block/' + n) for p, n in enumerate(names))
        return out

    def images(self):
        """{texture name: rows of pixels}; unused cells repeat the first colour, so pages stay opaque."""
        out = {}
        for p, name in enumerate(self.page_names()):
            colors = self.colors[p * PER_PAGE:(p + 1) * PER_PAGE] or ['#ff00ff']
            rows = [[rgba(colors[0], self.alpha)] * PAGE for _ in range(PAGE)]
            for i, color in enumerate(colors):
                x0, y0 = i % (PAGE // CELL) * CELL, i // (PAGE // CELL) * CELL
                for y in range(y0, y0 + CELL):
                    for x in range(x0, x0 + CELL):
                        rows[y][x] = rgba(color, self.alpha)
            out[name] = rows
        return out


def rgba(color, alpha=255):
    return tuple(int(color[i:i + 2], 16) for i in (1, 3, 5)) + (alpha,)


def elements(boxes, skin, stats, tint_index=False):
    """The elements of the boxes. stats collects what the caller reports; tint_index gives every
    face tint index 0, for a part the client colours (the tank's fluid)."""
    placed = [(box, *to_mc(box)) for box in boxes]
    solids = [(i, lo, hi) for i, (_, lo, hi, rot) in enumerate(placed) if not rot]
    out = []
    for index, (box, lo, hi, rot) in enumerate(placed):
        mat = box[1]
        if any(not (LOW <= v <= HIGH) for v in lo + hi):
            stats['outside_limit'] = True
        # Bounds of from/to (a turned element may reach a little further).
        b_lo, b_hi = stats.setdefault('bounds', ([0.0] * 3, [16.0] * 3))
        for a in range(3):
            b_lo[a], b_hi[a] = min(b_lo[a], lo[a]), max(b_hi[a], hi[a])
            if lo[a] < -OVERHANG or hi[a] > 16 + OVERHANG:
                stats.setdefault('overhang_parts', set()).add(box[0])
        faces = {}
        for face in FACES:
            if lo[FACES[face][0]] == hi[FACES[face][0]]:
                continue
            if not rot and hidden(face, index, lo, hi, solids):
                continue
            key, uv = skin.slot(mat)
            f = {'uv': [num(v) for v in uv], 'texture': '#' + key}
            if tint_index:
                f['tintindex'] = 0
            side = None if rot else cull(face, lo, hi)
            if side:
                f['cullface'] = side
            faces[face] = f
        if not faces:
            continue
        el = {'from': [num(v) for v in lo], 'to': [num(v) for v in hi]}
        if rot:
            axis, angle, origin = rot
            el['rotation'] = {'angle': num(angle), 'axis': axis, 'origin': [num(v) for v in origin]}
            if abs(angle / 22.5 - round(angle / 22.5)) > EPS:
                stats.setdefault('free_angles', set()).add(num(angle))
        if mat.startswith('glow'):
            el['light_emission'] = GLOW
        el['faces'] = faces
        out.append(el)
    return out


# ---------------------------------------------------------------- which parts show at rest

# The parts a block shows only in some states, after RC.POSES in docs/showcase/js/poses.js, left
# out of its model at rest (standing, tier 1, not linked). Patterns as fnmatch takes them.
ALWAYS_HIDDEN = ['tier', 'link_*']
REST_HIDDEN = {
    'rail': ['y-', 'y+', 'arc_*', 'slope'],
    'tank': ['level_fluid'],
    'furnace': ['fire', 'flame'], 'alloy': ['fire', 'flame'], 'gen_fuel': ['fire', 'flame'], 'trash': ['fire', 'flame'],
    'crafter': ['lit*'],
    'disintegrator': ['beam'],
    'dock_unload': ['lit1', 'lit2'], 'dock_load': ['lit1', 'lit2'],
    'seed': ['stage1', 'stage2'],
    'repair': ['sparks'], 'kitchen': ['steam'], 'apiary': ['bees'], 'brewer': ['bubbles'],
    'enchanter': ['ring'], 'projector': ['cone', 'holo'],
}
# Parts the guide holds turned at rest: (axis, degrees) about the part's pivot, right-handed in
# the model's numbers. The enchanter's book stands half closed while it is off (1.3 radians).
REST_TURN = {'enchanter': {'page_l': ('X', -math.degrees(1.3)), 'page_r': ('X', math.degrees(1.3))}}
# Colours of tinted materials where a block's default is not tier 1: the junction node is of the
# last tier (P.node_hub, its light shade 0.35 towards white).
DEFAULT_TINTS = {'node_hub': {'tint': TIER[4], 'tint_light': kit.shade(TIER[4], 0.35)}}
# Carts keep the bracket for a floor rail and no tool.
REST_ONLY = {'capsule': ['body', 'below'], 'worker': ['body', 'below']}
# Parts the guide moves (turns, slides, stretches or swaps) while the block works: in the game
# they need a block-entity renderer; here they stand still in their rest place.
MOVING = {
    'node_hub': 'rotor_x rotor_y rotor_z', 'pdock': 'vortex', 'tank': 'level_fluid',
    'capsule': 'whole model turns to its heading', 'worker': 'whole model turns; tools aim, slide, spin',
    'furnace': 'flame', 'alloy': 'flame', 'gen_fuel': 'flame', 'trash': 'flame', 'crusher': 'roll_a roll_b',
    'crafter': 'lit0..lit8 (lamps in turn)', 'mixer': 'stirrer', 'placer': 'piston', 'breaker': 'drill',
    'pump': 'plunger', 'miner': 'scanner drill', 'vein': 'core', 'disintegrator': 'cutter half_a half_b',
    'windmill': 'rotor', 'battery': 'level_charge', 'dock_unload': 'lit0..lit2 (lamps in turn)',
    'dock_load': 'lit0..lit2 (lamps in turn)', 'seed': 'stage0..stage2 (by state)', 'zipanchor': 'roller',
    'turret': 'aim_yaw aim_pitch', 'repair': 'wheel clamp sparks', 'kitchen': 'lid steam', 'fishery': 'winch line',
    'feeder': 'auger level_feed', 'shearer': 'arm blade_a blade_b level_milk', 'apiary': 'bees level_honey',
    'brewer': 'bubbles level_brew', 'xp_tank': 'orb level_xp', 'enchanter': 'book page_l page_r ring',
    'archeo': 'brush', 'projector': 'holo', 'builder': 'bridge head', 'butcher': 'arm',
}


def at_rest(name, part):
    if name in REST_ONLY:
        return part in REST_ONLY[name]
    patterns = ALWAYS_HIDDEN + REST_HIDDEN.get(name, [])
    return not any(fnmatch.fnmatchcase(part, p) for p in patterns)


# ---------------------------------------------------------------- files


def png(pixels):
    """PNG bytes of RGBA rows, written with zlib and struct alone."""
    height, width = len(pixels), len(pixels[0])
    raw = b''.join(b'\x00' + b''.join(bytes(p) for p in row) for row in pixels)

    def chunk(kind, data):
        body = kind + data
        return struct.pack('>I', len(data)) + body + struct.pack('>I', zlib.crc32(body))

    header = struct.pack('>IIBBBBB', width, height, 8, 6, 0, 0, 0)
    return b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', header) + chunk(b'IDAT', zlib.compress(raw, 9)) + chunk(b'IEND', b'')


def model_json(textures, els, parent=None, render_type=None):
    """A model with one element per line, so a diff shows which boxes changed."""
    head = {}
    if parent:
        head['parent'] = parent
    if render_type:
        head['render_type'] = render_type
    head['textures'] = textures
    lines = ['{']
    lines += ['  %s: %s,' % (json.dumps(k), json.dumps(v)) for k, v in head.items()]
    lines.append('  "elements": [')
    lines += ['    %s%s' % (json.dumps(e, separators=(', ', ': ')), ',' if i < len(els) - 1 else '') for i, e in enumerate(els)]
    lines += ['  ]', '}', '']
    text = '\n'.join(lines)
    json.loads(text)
    return text


# Block models and textures other tools write, which flush() leaves alone: the gear seen on players
# (tools/gen_worn_models.py) and the tier stripes (tools/tiers.py). Round 31: the housing's
# bottleneck mark is made here (housing_weak) and its channel strips are gone (built into its model).
FOREIGN = ('worn_*', 'tier_stripe_*')
# Files gear_mc would write that other tools write over: the energy blade's and shield's items go
# dark without charge (tools/gen_gear_dark.py), the scout compass turns its needle
# (tools/gen_scout_compass.py).
FOREIGN_FILES = ('items/blade_?.json', 'items/eshield_?.json', 'items/scout_compass.json')


class Writer:
    def __init__(self):
        self.files = {}

    def text(self, rel, text):
        self.files[rel] = text.encode('utf-8')

    def image(self, rel, pixels):
        self.files[rel] = png(pixels)

    def skin(self, skin):
        for name, rows in skin.images().items():
            self.image('textures/block/%s.png' % name, rows)

    def flush(self):
        # Everything in models/block and textures/block is made here but what other tools make
        # (FOREIGN); drop what is no longer made.
        for folder, ext in (('models/block', '.json'), ('textures/block', '.png')):
            for old in (ASSETS / folder).glob('*' + ext):
                foreign = any(fnmatch.fnmatchcase(old.stem, p) for p in FOREIGN)
                if '%s/%s' % (folder, old.name) not in self.files and not foreign:
                    old.unlink()
        for rel, data in sorted(self.files.items()):
            if any(fnmatch.fnmatchcase(rel, p) for p in FOREIGN_FILES):
                continue
            path = ASSETS / rel
            path.parent.mkdir(parents=True, exist_ok=True)
            if not path.exists() or path.read_bytes() != data:
                path.write_bytes(data)
        return len(self.files)


# ---------------------------------------------------------------- the blocks of the slice

# The housing's arms by side of the model, and the Minecraft side each one points to.
ARMS = {'x+': 'east', 'x-': 'west', 'y+': 'south', 'y-': 'north', 'z+': 'up', 'z-': 'down'}
BLOCK = 'minecraft:block/block'
# The bottleneck's frame the client draws round a housing while the wrench is in hand
# (rustcraft_pipe18): twelve edges 0.75 pixel thick round the cage, tinted red by the client.
WEAK_LO, WEAK_HI, WEAK_T = 4.25, 11.75, 0.75


def housing(out, report):
    """Round 31: the housing's blockstate and models. Its three channels are built in and run in its
    arms, from the middle of the block to each joined side, so they follow the line whichever way
    it turns (a housing joined to nothing shows them along x); its tier, 1 to 4, colours its corner
    caps and the collars of its arms, as the node's tier colours its pips. The client draws no strip
    of its own: a housing shows its channels as soon as its block is there."""
    boxes = read(transport.pipe)
    tints = {'tint_%d' % t: TIER[t] for t in range(1, 5)}
    skin = Skin('housing', tints)
    stats = {}
    by_part = {}
    for b in boxes:
        by_part.setdefault(b[0], []).append(b)

    def tiered(part_boxes, tier):
        return [(p, 'tint_%d' % tier if mat == 'tint' else mat, lo, hi, rot) for p, mat, lo, hi, rot in part_boxes]

    # Fix the palette over all parts and tiers first, so every model reads the same texture.
    for b in boxes:
        if b[1] != 'tint':
            skin.slot(b[1])
    for t in range(1, 5):
        skin.slot('tint_%d' % t)
    models = {}
    for t in range(1, 5):
        models['housing_core_%d' % t] = tiered(by_part['body'], t)
        # An arm with its collar in one model, so the collar hides the ends of the arm's beams.
        for arm, side in ARMS.items():
            models['housing_arm_%s_%d' % (side, t)] = tiered(by_part[arm] + by_part['collar_' + arm], t)
    models['housing_channels'] = by_part['ch_x']
    for name, part_boxes in models.items():
        out.text('models/block/%s.json' % name, model_json(skin.textures(), elements(part_boxes, skin, stats)))
    # In the inventory: a straight piece of the item's tier, its channels along it.
    for t in range(1, 5):
        piece = by_part['body'] + by_part['x-'] + by_part['collar_x-'] + by_part['x+'] + by_part['collar_x+']
        name = 'housing_inventory' if t == 1 else 'housing_inventory_%d' % t
        out.text('models/block/%s.json' % name, model_json(skin.textures(), elements(tiered(piece, t), skin, stats), BLOCK))
    out.skin(skin)
    out.text('models/block/housing_weak.json', weak_frame())
    sides = ('north', 'east', 'south', 'west', 'up', 'down')
    parts = []
    for t in range(1, 5):
        parts.append({'when': {'tier': str(t)}, 'apply': {'model': 'rustcraft:block/housing_core_%d' % t}})
    for side in sides:
        for t in range(1, 5):
            parts.append({'when': {side: 'true', 'tier': str(t)},
                          'apply': {'model': 'rustcraft:block/housing_arm_%s_%d' % (side, t)}})
    parts.append({'when': {s: 'false' for s in sides}, 'apply': {'model': 'rustcraft:block/housing_channels'}})
    out.text('blockstates/housing.json', '{\n  "multipart": [\n%s\n  ]\n}\n' % ',\n'.join('    ' + json.dumps(p) for p in parts))
    for t in range(2, 5):
        out.text('items/housing_%d.json' % t,
                 '{\n  "model": { "type": "minecraft:model", "model": "rustcraft:block/housing_inventory_%d" }\n}\n' % t)
    report.append(('pipe', 'housing_core_<tier> x4, housing_arm_<side>_<tier> x24, housing_channels, '
                   'housing_inventory x4, housing_weak', stats))


def weak_frame():
    """The bottleneck frame: the twelve edges of a box round the cage, white for the client's tint."""
    lo, hi, t = WEAK_LO, WEAK_HI, WEAK_T
    edges = []
    for axis in range(3):
        for a in (lo, hi - t):
            for b in (lo, hi - t):
                start, end = [0.0] * 3, [0.0] * 3
                others = [i for i in range(3) if i != axis]
                start[axis], end[axis] = lo, hi
                start[others[0]], end[others[0]] = a, a + t
                start[others[1]], end[others[1]] = b, b + t
                edges.append((start, end))
    face = {'uv': [0, 0, 2, 2], 'texture': '#s', 'tintindex': 0}
    elements = [{'from': [num(v) for v in s], 'to': [num(v) for v in e], 'shade': False,
                 'faces': {f: dict(face) for f in ('down', 'up', 'north', 'south', 'west', 'east')}}
                for s, e in edges]
    textures = {'particle': 'minecraft:block/white_concrete', 's': 'minecraft:block/white_concrete'}
    return json.dumps({'textures': textures, 'elements': elements}, indent=1) + '\n'


def node(out, report):
    boxes = read(transport.node)
    variants = {}
    for tier in range(1, 5):
        name = 'node_tier%d' % tier
        skin = Skin(name, {'tint': TIER[tier]})
        stats = {}
        out.text('models/block/%s.json' % name, model_json(skin.textures(), elements(boxes, skin, stats), BLOCK))
        out.skin(skin)
        variants['tier=%d' % tier] = {'model': 'rustcraft:block/' + name}
    out.text('blockstates/node.json', '{\n  "variants": {\n%s\n  }\n}\n' % ',\n'.join(
        '    %s: %s' % (json.dumps(k), json.dumps(v)) for k, v in variants.items()))
    report.append(('node', 'node_tier1..4', stats))


# The machines copy vanilla's furnace in the game: their states are `facing` times
# `lit`.
FACING_Y = {'north': 0, 'east': 90, 'south': 180, 'west': 270}


def machine_states(out):
    """Blockstates of the crystal, the cell and the machines, whose models come from staged() below.
    Round 31: a block with connectors is a multipart, its body by `facing` and the connector of
    each side whose property is true, unturned (rustcraft_rules::connectors)."""
    out.text('blockstates/crystal.json', '{\n  "variants": {\n    "": {"model": "rustcraft:block/crystal"}\n  }\n}\n')
    out.text('blockstates/cell.json', multipart([{'apply': {'model': 'rustcraft:block/cell'}}] + link_parts('cell')))
    # The pump, the terminal, the tank and the station copy the furnace too, for a menu on use;
    # since slice 9 so do the slice-7 and slice-8 blocks but the solar panel (SLICE7_BLOCKS).
    furnaces = ('furnace', 'crusher', 'gen_fuel', 'alloy', 'crafter', 'pump', 'terminal', 'tank', 'station')
    for machine in furnaces + tuple(b for b in SLICE7_PANELS if b not in SLICE7_BLOCKS) + SLICE12_BLOCKS:
        model_name = BLOCK_MODEL.get(machine, machine)
        parts = []
        for facing, y in FACING_Y.items():
            model = {'model': 'rustcraft:block/' + model_name}
            if y:
                model['y'] = y
            parts.append({'when': {'facing': facing}, 'apply': model})
        out.text('blockstates/%s.json' % machine, multipart(parts + link_parts(model_name)))


# Round 31: the sides a machine shows a connector on, as its state properties name them, and the
# model side of each (rustcraft_rules::connectors::FACES; no connector below).
LINK_PROPS = (('north', 'y-'), ('east', 'x+'), ('south', 'y+'), ('west', 'x-'), ('up', 'z+'))
# Models whose connectors the block entity draws instead (rustcraft_power, slice 18).
LINKED = ('crystal', 'windmill', 'solar')


def has_links(model_name):
    """Whether a model takes the magistral's connectors through its blockstate."""
    if model_name in LINKED:
        return False
    for section in BLOCK_SECTIONS:
        if model_name in section.MODELS:
            return any(b[0] == 'link_z+' for b in read(section.MODELS[model_name], model_name))
    return False


def link_parts(model_name):
    """The blockstate parts of a model's connectors, none for a model without them."""
    if not has_links(model_name):
        return []
    return [{'when': {prop: 'true'}, 'apply': {'model': 'rustcraft:block/%s__link_%s' % (model_name, prop)}}
            for prop, _ in LINK_PROPS]


def multipart(parts):
    """A multipart blockstate, one part to a line."""
    return '{\n  "multipart": [\n%s\n  ]\n}\n' % ',\n'.join('    ' + json.dumps(p) for p in parts)


# Slice 12: the last blocks copy the furnace as the machines do; the portal dock's model is `pdock`.
SLICE12_BLOCKS = ('butcher', 'trash', 'disintegrator', 'charger', 'charge_plate', 'repair', 'kitchen', 'fishery',
                  'feeder', 'shearer', 'apiary', 'brewer', 'xp_tank', 'enchanter', 'archeo', 'projector',
                  'builder', 'portal_dock', 'vein')
BLOCK_MODEL = {'portal_dock': 'pdock'}


# Models drawn see-through, with their palette's alpha: the solar panel lets the light through
# (E19, no shadow under it), so the guide draws it at 40% (docs/showcase/js/poses.js `P.solar`). A
# texture with alpha 1..254 goes to the engine's translucent pass, which blends it; the model also
# names that pass, as NeoForge's `render_type` does.
SEE_THROUGH = {'solar': 102}
TRANSLUCENT = 'minecraft:translucent'


def staged(out, report, skipped):
    """Every other block model, at rest, for blocks that come later."""
    for section in BLOCK_SECTIONS:
        for name, draw in section.MODELS.items():
            if name in ('pipe', 'node'):
                continue
            every = read(draw, name)
            moving = anim_mc.parts_of(name)
            boxes = [b for b in every if at_rest(name, b[0]) and b[0] not in moving]
            alpha = SEE_THROUGH.get(name, 255)
            skin = Skin(name, DEFAULT_TINTS.get(name), alpha)
            # One palette for the block and its moving parts, so their models share a texture.
            for b in every:
                if b[0] not in ALWAYS_HIDDEN:
                    skin.slot(b[1])
            stats = {}
            els = elements(boxes, skin, stats)
            if moving:
                anim_parts(out, name, draw, skin, stats)
            energy_parts(out, name, every, skin, stats)
            if stats.get('outside_limit'):
                skipped.append(name)
                continue
            render = TRANSLUCENT if alpha < 255 else None
            out.text('models/block/%s.json' % name, model_json(skin.textures(), els, BLOCK, render))
            out.skin(skin)
            dropped = sorted({b[0] for b in every} - {b[0] for b in boxes})
            stats['dropped'] = dropped
            stats['pages'] = len(skin.page_names())
            report.append((name, '%d elements' % len(els), stats))


# ---------------------------------------------------------------- moving parts (slice 12)

# Blocks whose id is not their model's name.
MODEL_BLOCK = {'pdock': 'portal_dock'}
ANIM_TABLE = []
AXES = {'x': 0, 'y': 2, 'z': 1}


def anim_parts(out, name, draw, skin, stats):
    """The moving parts of model `name` (anim_mc.ANIM), unturned, each a standalone model drawn by
    the client's block-entity renderer, and their rows of the Rust table."""
    boxes, pivots = read_full(draw, name, rest_turn=False)
    spec = anim_mc.parts_of(name)
    order = list(spec)
    for part in order:
        p = spec[part]
        part_boxes = [b for b in boxes if b[0] == part]
        assert part_boxes, 'no boxes in moving part %s of %s' % (part, name)
        model = '%s__%s' % (name, part)
        if p.get('tinted'):
            white = Skin(model, dict((b[1], '#ffffff') for b in part_boxes))
            els = elements(part_boxes, white, stats, tint_index=True)
            out.text('models/block/%s.json' % model, model_json(white.textures(), els))
            out.skin(white)
        else:
            out.text('models/block/%s.json' % model, model_json(skin.textures(), elements(part_boxes, skin, stats)))
        pivot = [v / 16 for v in swap(pivots.get(part, (8, 8, 8)))]
        bottom = min(swap(b[2])[1] for b in part_boxes) / 16
        ANIM_TABLE.append({
            'block': 'rustcraft:' + MODEL_BLOCK.get(name, name), 'model': 'rustcraft:block/' + model,
            'pivot': pivot, 'bottom': bottom, 'on': [motion_mc(m) for m in p['on']],
            'off': p['off'] if isinstance(p['off'], str) else [motion_mc(m) for m in p['off']],
            'show': p.get('show'), 'carry': (order.index(p['carry']) - order.index(part)) if p.get('carry') else 0,
            'level': bool(p.get('level')), 'tinted': bool(p.get('tinted')),
        })


def motion_mc(m):
    """A motion in Minecraft numbers: the y/z swap is a mirror, so turns change sign; vectors are
    swapped and go from pixels to blocks."""
    kind, what, base, amp, shape, freq, phase = m
    if kind == 'rot':
        return ('Rot', AXES[what], -base, -amp, shape, freq, phase)
    vec = [v / 16 for v in swap(what)]
    return ('Move', vec, base, amp, shape, freq, phase)


def rust_f(v):
    s = repr(float(round(v, 6)))
    return s if ('.' in s or 'e' in s) else s + '.0'


def rust_motion(m):
    kind, what, base, amp, shape, freq, phase = m
    target = ('Target::Rot(%d)' % what) if kind == 'Rot' else ('Target::Move([%s])' % ', '.join(rust_f(v) for v in what))
    return 'Motion { target: %s, base: %s, amp: %s, shape: Shape::%s, freq: %s, phase: %s }' % (
        target, rust_f(base), rust_f(amp), shape.capitalize(), rust_f(freq), rust_f(phase))


def rust_show(show):
    if not show:
        return 'Show::Always'
    if show[0] == 'below':
        return 'Show::Below { rate: %s, modulo: %d, index: %d }' % (rust_f(show[1]), show[2], show[3])
    if show[0] == 'eq':
        return 'Show::Eq { rate: %s, modulo: %d, index: %d }' % (rust_f(show[1]), show[2], show[3])
    return 'Show::Sin { freq: %s, threshold: %s }' % (rust_f(show[1]), rust_f(show[2]))


def rust_table(out):
    rows = []
    for r in ANIM_TABLE:
        off = r['off']
        if off == 'rest':
            off_s = 'Off::Rest'
        elif off == 'hide':
            off_s = 'Off::Hide'
        elif off == 'always':
            off_s = 'Off::Always'
        else:
            off_s = 'Off::Moves(&[%s])' % ', '.join(rust_motion(m) for m in off)
        rows.append('''    Part {
        block: %s,
        model: %s,
        pivot: [%s],
        bottom: %s,
        on: &[%s],
        off: %s,
        show: %s,
        carry: %d,
        level: %s,
        tinted: %s,
    },''' % (json.dumps(r['block']), json.dumps(r['model']), ', '.join(rust_f(v) for v in r['pivot']), rust_f(r['bottom']),
             ', '.join(rust_motion(m) for m in r['on']), off_s, rust_show(r['show']), r['carry'],
             'true' if r['level'] else 'false', 'true' if r['tinted'] else 'false'))
    text = ('//! The moving parts of the block models, written by assets/models/export_mc.py from\n'
            '//! assets/models/anim_mc.py: do not edit by hand, run the exporter.\n\n'
            'use super::{Motion, Off, Part, Shape, Show, Target};\n\n'
            '#[rustfmt::skip]\npub const PARTS: &[Part] = &[\n%s\n];\n' % '\n'.join(rows))
    path = HERE.parent.parent / 'mod' / 'rustcraft' / 'rules' / 'src' / 'anim' / 'parts.rs'
    path.parent.mkdir(parents=True, exist_ok=True)
    data = text.encode('utf-8')
    if not path.exists() or path.read_bytes() != data:
        path.write_bytes(data)


# The monorail (spec 4.6, rustcraft_rules::monorail::shape): the guide's floor rail cut into its
# parts, one model each per tier, and a multipart blockstate that shows the parts the rail's
# `shape` names, turned onto its `mount` as vanilla's end rod turns its model by `facing`.
# Local side k of the floor piece is the model's x+, y+, x-, y- (east, south, west, north).
RAIL_HALVES = ('x+', 'y+', 'x-', 'y-')
RAIL_ARCS = ('arc_x+y+', 'arc_y+x-', 'arc_x-y-', 'arc_y-x+')
# (x, y) turns of the blockstate by mount, the side of the cell the rail hangs on.
RAIL_MOUNTS = {'down': (0, 0), 'up': (180, 0), 'south': (90, 0), 'north': (90, 180), 'west': (90, 90), 'east': (90, 270)}
# Two halves at a right angle show as the arc between them: arc k joins sides k and k + 1.
RAIL_CORNERS = {(1 << k) | (1 << ((k + 1) % 4)): k for k in range(4)}


def turned_boxes(boxes, q):
    """The boxes after q quarter turns about the vertical through the block's middle, as
    Recorder.box turns them."""
    out = []
    for part, mat, lo, hi, rot in boxes:
        a, b = turned(lo, q), turned(hi, q)
        box_lo = tuple(min(a[i], b[i]) for i in range(3))
        box_hi = tuple(max(a[i], b[i]) for i in range(3))
        if rot:
            vec, angle, origin = rot
            rot = (turned(vec, q, about=0), angle, turned(origin, q))
        out.append((part, mat, box_lo, box_hi, rot))
    return out


def rail_halves_shown(mask):
    """The local sides whose half a rail of `mask` shows: its own, a lone half's opposite too
    (a straight end), x for none; nothing for a corner, which shows its arc."""
    if mask in RAIL_CORNERS:
        return []
    sides = [k for k in range(4) if mask >> k & 1]
    if len(sides) == 1:
        sides.append((sides[0] + 2) % 4)
    if not sides:
        sides = [0, 2]
    return sorted(sides)


def rail_rise(m):
    """The piece of a floor rail up the wall at its x+ side (rustcraft_rules::monorail::shape
    `Rise`): the rail's section stood on end against that wall, from the floor to the top of the
    cell, where the wall rail's lower half meets it, with a tie halfway up. Round 31: it stands on
    the floor rail instead of reaching through it, its foot runs round the tie and its light edge
    is as narrow as the rails' light tops, so no two of their faces share a plane (z-fighting)."""
    m.box('steel_dark', 15.5, 6.5, 2, 0.5, 3, 5.25, part='rise')
    m.box('steel_dark', 15.5, 6.5, 8.75, 0.5, 3, 7.25, part='rise')
    m.box('tint', 14.25, 7, 2, 1.25, 2, 14, part='rise')
    m.box('tint_light', 14, 7.05, 2, 0.25, 1.9, 14, part='rise')
    m.box('steel_mid', 15.25, 5, 7.25, 0.75, 6, 1.5, part='rise')


def monorail(out, report):
    boxes = read(transport.rail)
    rise = read(rail_rise)
    by_part = {}
    for b in boxes:
        by_part.setdefault(b[0], []).append(b)
    parts = {'hub': by_part['body']}
    for k in range(4):
        parts['half%d' % k] = by_part[RAIL_HALVES[k]]
        parts['arc%d' % k] = by_part[RAIL_ARCS[k]]
        parts['climb%d' % k] = turned_boxes(by_part['slope'], k)
        parts['rise%d' % k] = turned_boxes(rise, k)
    stats = {}
    for tier in range(1, 5):
        skin = Skin('monorail_t%d' % tier, {'tint': TIER[tier], 'tint_light': kit.shade(TIER[tier], 0.3)})
        for b in boxes:
            skin.slot(b[1])
        for name, part_boxes in parts.items():
            out.text('models/block/monorail_t%d_%s.json' % (tier, name), model_json(skin.textures(), elements(part_boxes, skin, stats)))
        straight = parts['hub'] + parts['half0'] + parts['half2']
        out.text('models/block/monorail_t%d_inventory.json' % tier, model_json(skin.textures(), elements(straight, skin, stats), BLOCK))
        out.skin(skin)
    masks = range(16)
    entries = []
    for mount, (x, y) in RAIL_MOUNTS.items():
        turn = {}
        if x:
            turn['x'] = x
        if y:
            turn['y'] = y
        for tier in range(1, 5):
            when = {'mount': mount, 'tier': str(tier)}

            def part(name, shapes):
                if shapes:
                    entries.append({'when': dict(when, shape='|'.join(shapes)),
                                    'apply': dict({'model': 'rustcraft:block/monorail_t%d_%s' % (tier, name)}, **turn)})
            # The states of rustcraft_rules::monorail::shape::Shape::values: s<mask>, c<k>,
            # c<k>f<fork> (a climb with a fork on its lower level) and r<k>s<mask> (a floor rail
            # under a wall rail, mask holding k).
            forks = [(k, f) for k in range(4) for f in (1, 2, 3)]
            rises = [(k, m) for k in range(4) for m in masks if m >> k & 1]

            def fork_halves(k, f):
                return [(k + 2) % 4] + [(k + 1) % 4] * (f & 1) + [(k + 3) % 4] * (f >> 1 & 1)
            part('hub', ['s%d' % m for m in masks if m not in RAIL_CORNERS]
                 + ['c%df%d' % kf for kf in forks]
                 + ['r%ds%d' % km for km in rises if km[1] not in RAIL_CORNERS])
            for j in range(4):
                part('half%d' % j, ['s%d' % m for m in masks if j in rail_halves_shown(m)]
                     + ['c%df%d' % (k, f) for k, f in forks if j in fork_halves(k, f)]
                     + ['r%ds%d' % (k, m) for k, m in rises if j in rail_halves_shown(m)])
            for mask, j in sorted(RAIL_CORNERS.items(), key=lambda e: e[1]):
                part('arc%d' % j, ['s%d' % mask] + ['r%ds%d' % (k, m) for k, m in rises if m == mask])
            for k in range(4):
                part('climb%d' % k, ['c%d' % k] + ['c%df%d' % (k, f) for f in (1, 2, 3)])
            for k in range(4):
                part('rise%d' % k, ['r%ds%d' % (k, m) for m in masks if m >> k & 1])
    out.text('blockstates/monorail.json', '{\n  "multipart": [\n%s\n  ]\n}\n' % ',\n'.join('    ' + json.dumps(e) for e in entries))
    report.append(('rail', 'monorail_t1..4_{hub,half0..3,arc0..3,climb0..3,rise0..3,inventory}', stats))


# The capsule (rustcraft_rules::monorail::capsule): no block, a model the client draws round a
# seat mob, its cabin and each bracket apart, so the bracket can follow the rail.
CAPSULE_PARTS = ('body', 'below', 'above', 'side')


def capsule(out, report):
    boxes = read(transport.capsule)
    skin = Skin('capsule_parts')
    for b in boxes:
        skin.slot(b[1])
    stats = {}
    for part in CAPSULE_PARTS:
        part_boxes = [b for b in boxes if b[0] == part]
        out.text('models/block/capsule_%s.json' % part, model_json(skin.textures(), elements(part_boxes, skin, stats)))
    out.skin(skin)
    report.append(('capsule', 'capsule_body, capsule_below, capsule_above, capsule_side', stats))




# The worker (rustcraft_rules::worker; slice 7): no block, a model the client draws round its seat
# mob: its chassis, each bracket (floor, ceiling, wall) and each part of each tool apart, so the
# bracket follows the rail and the tool's parts move as the guide's poses move them (slice 18,
# docs/showcase/js/poses.js `P.worker`). The pivots of the moving parts go to the rules crate
# (rustcraft_rules::worker::pose::PIVOTS) in block units, Minecraft axes.
WORKER_PARTS = [
    'body', 'below', 'above', 'side',
    'saw', 'aim_yaw_saw', 'reach_saw', 'saw_disc',
    'reaper', 'reaper_reel',
    'hold',
    'grab', 'grab_left', 'grab_right',
    'bore', 'bore_bit',
    'gun', 'aim_yaw_gun', 'aim_pitch_gun',
    'scoop', 'aim_yaw_scoop', 'reach_scoop', 'scoop_bucket',
    'map', 'map_drum',
]


def worker_parts(out, report):
    boxes, pivots = read_full(transport.worker)
    skin = Skin('worker_parts')
    for b in boxes:
        skin.slot(b[1])
    stats = {}
    for name in WORKER_PARTS:
        part_boxes = [b for b in boxes if b[0] == name and not (name == 'body' and b[1] == 'chassis')]
        assert part_boxes, 'no boxes in worker part %s' % name
        out.text('models/block/worker_%s.json' % name, model_json(skin.textures(), elements(part_boxes, skin, stats)))
    out.skin(skin)
    # The chassis's shell, white and tinted: the client paints it in its tool's colour
    # (models.js `TOOL`, rustcraft_rules::worker::pose::colour).
    paint = [b for b in boxes if b[0] == 'body' and b[1] == 'chassis']
    white = Skin('worker_paint', dict((b[1], '#ffffff') for b in paint))
    out.text('models/block/worker_paint.json', model_json(white.textures(), elements(paint, white, stats, tint_index=True)))
    out.skin(white)
    rows = []
    for name in WORKER_PARTS:
        if name in pivots:
            pv = [v / 16 for v in swap(pivots[name])]
            rows.append('    (%s, [%s]),' % (json.dumps(name), ', '.join(rust_f(v) for v in pv)))
    text = ('//! The pivots of the worker parts that move, written by assets/models/export_mc.py from\n'
            '//! assets/models/transport.py: do not edit by hand, run the exporter.\n\n'
            '/// (part, pivot in blocks, Minecraft axes, in the model frame).\n'
            '#[rustfmt::skip]\npub const PIVOTS: &[(&str, [f64; 3])] = &[\n%s\n];\n' % '\n'.join(rows))
    path = HERE.parent.parent / 'mod' / 'rustcraft' / 'rules' / 'src' / 'worker' / 'pivots.rs'
    data = text.encode('utf-8')
    if not path.exists() or path.read_bytes() != data:
        path.write_bytes(data)
    report.append(('worker', 'worker_' + ', worker_'.join(WORKER_PARTS), stats))


# The slice-7 and slice-8 blocks: since slice 9 they copy the furnace for their own windows
# (machine_states); the solar panel alone still copies glass with a `facing` of its own
# (rustcraft_content), and its blockstate turns the staged model as the machines' does.
SLICE7_PANELS = ('dock_unload', 'dock_charge', 'turret', 'anchor',
                 'solar', 'windmill', 'gen_lava', 'battery', 'miner', 'placer', 'breaker',
                 'mixer', 'dock_load', 'turbine')
SLICE7_BLOCKS = ('solar',)


def slice7_states(out):
    for block in SLICE7_BLOCKS:
        variants = []
        for facing, y in FACING_Y.items():
            model = {'model': 'rustcraft:block/' + block}
            if y:
                model['y'] = y
            variants.append('    %s: %s' % (json.dumps('facing=%s' % facing), json.dumps(model)))
        out.text('blockstates/%s.json' % block, '{\n  "variants": {\n%s\n  }\n}\n' % ',\n'.join(variants))


# Slice 14: the zipline's rope (station.rs draw_rope): a bar one texel thick, one block along +z,
# stretched and turned by the client; the rail's texture, its dark corner.
ROPE_FACE = {'uv': [0, 0, 1, 1], 'texture': '#rope'}


def rope(out):
    model = {'textures': {'particle': 'rustcraft:block/rail', 'rope': 'rustcraft:block/rail'},
             'elements': [{'from': [7.5, 7.5, 0], 'to': [8.5, 8.5, 16],
                           'faces': {f: dict(ROPE_FACE) for f in ('down', 'up', 'north', 'south', 'west', 'east')}}]}
    out.text('models/block/zipline_rope.json', json.dumps(model, indent=2) + '\n')


# The zipline's anchor (rustcraft_zipline, rules::traversal): a plate, a stake and the ring its
# rope's cord goes up from (12 pixels, traversal::ANCHOR_TOP), in the rope's texture; and the
# harpoon's tip at the hook's end of its rope (station.rs draw_tip), pointing along +z from z 0.
ANCHOR_BOXES = (([5, 0, 5], [11, 1, 11]), ([7, 1, 7], [9, 10, 9]), ([6, 10, 6], [10, 12, 10]))
TIP_BOXES = (([7, 7, 0], [9, 9, 1]), ([6.5, 6.5, 1], [9.5, 9.5, 3]), ([7.25, 7.25, 3], [8.75, 8.75, 5]),
             ([7.75, 7.75, 5], [8.25, 8.25, 6]))


def rope_boxes(boxes):
    return [{'from': lo, 'to': hi, 'faces': {f: dict(ROPE_FACE) for f in ('down', 'up', 'north', 'south', 'west', 'east')}}
            for lo, hi in boxes]


def zip_parts(out):
    textures = {'particle': 'rustcraft:block/rail', 'rope': 'rustcraft:block/rail'}
    anchor = {'parent': BLOCK, 'ambientocclusion': False, 'textures': textures, 'elements': rope_boxes(ANCHOR_BOXES)}
    out.text('models/block/zip_anchor.json', json.dumps(anchor, indent=2) + '\n')
    out.text('blockstates/zip_anchor.json',
             '{\n  "variants": {\n    "": {"model": "rustcraft:block/zip_anchor"}\n  }\n}\n')
    tip = {'textures': textures, 'elements': rope_boxes(TIP_BOXES)}
    out.text('models/block/harpoon_tip.json', json.dumps(tip, indent=2) + '\n')


def items(out):
    for name in icons_mc.ICONS:
        out.image('textures/item/%s.png' % name, icons_mc.icon(name))


# ---------------------------------------------------------------- slice 18, energy stream

# The model's sides (energy.py's link_<side>, z up, y south) as the game's faces.
LINK_FACES = {'x-': 'west', 'x+': 'east', 'y-': 'north', 'y+': 'south', 'z-': 'down', 'z+': 'up'}
# Every face tinted, so the client's tint sets how bright it glows: the crystal's pulse
# (P.crystal), a structure lit with solution and dim without (P.crystalblock).
LIT = ('crystal', 'crystalblock')
# The seed's stages, one model each (P.seed shows stage<n> and hides the others).
STAGED = {'seed': 3}


def energy_parts(out, name, every, skin, stats):
    """Slice 18: the housing connectors link_<side> of the crystal, the windmill and the solar
    panel, each a standalone model `<name>__link_<face>` the client draws on the sides a housing
    touches (round 31: and of every machine, which its blockstate shows); and the seed's stages
    `seed_<n>`, its blockstate's models by `stage`."""
    if any(b[0].startswith('link_') for b in every):
        for side, face in LINK_FACES.items():
            boxes = [b for b in every if b[0] == 'link_' + side]
            if boxes:
                model = '%s__link_%s' % (name, face)
                out.text('models/block/%s.json' % model, model_json(skin.textures(), elements(boxes, skin, stats)))
    if name in LIT:
        boxes = [b for b in every if not any(fnmatch.fnmatchcase(b[0], p) for p in ALWAYS_HIDDEN)]
        out.text('models/block/%s__lit.json' % name,
                 model_json(skin.textures(), elements(boxes, skin, stats, tint_index=True)))
    if name in STAGED:
        for n in range(STAGED[name]):
            hidden = ['stage%d' % i for i in range(STAGED[name]) if i != n]
            boxes = [b for b in every if b[0] not in hidden and not any(fnmatch.fnmatchcase(b[0], p) for p in ALWAYS_HIDDEN)]
            out.text('models/block/%s_%d.json' % (name, n), model_json(skin.textures(), elements(boxes, skin, stats), BLOCK))


def main():
    out, report, skipped = Writer(), [], []
    housing(out, report)
    node(out, report)
    machine_states(out)
    monorail(out, report)
    capsule(out, report)
    worker_parts(out, report)
    slice7_states(out)
    rope(out)
    zip_parts(out)
    staged(out, report, skipped)
    rust_table(out)
    items(out)
    gear_mc.write(out)
    count = out.flush()
    print('wrote %d files under %s' % (count, ASSETS))
    for name, what, stats in report:
        notes = []
        if stats.get('overhang_parts'):
            lo, hi = stats['bounds']
            notes.append('reaches %s..%s; parts out of the block: %s' % (
                [num(v) for v in lo], [num(v) for v in hi], ' '.join(sorted(stats['overhang_parts']))))
        if stats.get('free_angles'):
            notes.append('angles off the 22.5 grid: ' + ' '.join(str(a) for a in sorted(stats['free_angles'])))
        if stats.get('pages', 1) > 1:
            notes.append('%d palette pages' % stats['pages'])
        if stats.get('dropped'):
            notes.append('left out at rest: ' + ' '.join(stats['dropped']))
        if name in MOVING:
            notes.append('moves: ' + MOVING[name])
        print('  %-14s %s%s' % (name, what, ('  | ' + '; '.join(notes)) if notes else ''))
    for name in skipped:
        print('  %-14s skipped: elements beyond %d..%d' % (name, LOW, HIGH))


if __name__ == '__main__':
    main()
