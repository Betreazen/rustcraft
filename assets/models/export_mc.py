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

import branches  # noqa: E402
import energy  # noqa: E402
import gear  # noqa: E402
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
# block (LOW..HIGH): the windmill, three blocks tall in the guide, stands two thirds as big.
SCALED = {'windmill': 0.68}


def scaled(p, k):
    return (8 + (p[0] - 8) * k, 8 + (p[1] - 8) * k, p[2] * k)


def read(draw, name=None):
    """The boxes of a model; with its name, parts the guide holds turned at rest are turned so."""
    rec = Recorder()
    draw(rec)
    turns = REST_TURN.get(name, {})
    k = SCALED.get(name)
    boxes = []
    for part, mat, lo, hi, rot in rec.boxes:
        if k:
            lo, hi = scaled(lo, k), scaled(hi, k)
            if rot:
                rot = (rot[0], rot[1], scaled(rot[2], k))
        if part in turns:
            assert rot is None, 'a tilted box in a part turned at rest: %s %s' % (name, part)
            axis, degrees = turns[part]
            rot = ({'X': (1, 0, 0), 'Y': (0, 1, 0), 'Z': (0, 0, 1)}[axis], degrees, rec.pivots[part])
        boxes.append((part, mat, lo, hi, rot))
    return boxes


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

    def __init__(self, texture, tints=None):
        self.texture = texture
        self.tints = tints or {}
        self.colors = []

    def color(self, mat):
        return self.tints.get(mat, COLORS[mat])

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
            rows = [[rgba(colors[0])] * PAGE for _ in range(PAGE)]
            for i, color in enumerate(colors):
                x0, y0 = i % (PAGE // CELL) * CELL, i // (PAGE // CELL) * CELL
                for y in range(y0, y0 + CELL):
                    for x in range(x0, x0 + CELL):
                        rows[y][x] = rgba(color)
            out[name] = rows
        return out


def rgba(color):
    return tuple(int(color[i:i + 2], 16) for i in (1, 3, 5)) + (255,)


def elements(boxes, skin, stats):
    """The elements of the boxes. stats collects what the caller reports."""
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


def model_json(textures, els, parent=None):
    """A model with one element per line, so a diff shows which boxes changed."""
    head = {}
    if parent:
        head['parent'] = parent
    head['textures'] = textures
    lines = ['{']
    lines += ['  %s: %s,' % (json.dumps(k), json.dumps(v)) for k, v in head.items()]
    lines.append('  "elements": [')
    lines += ['    %s%s' % (json.dumps(e, separators=(', ', ': ')), ',' if i < len(els) - 1 else '') for i, e in enumerate(els)]
    lines += ['  ]', '}', '']
    text = '\n'.join(lines)
    json.loads(text)
    return text


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
        # Everything in models/block and textures/block is made here; drop what is no longer made.
        for folder, ext in (('models/block', '.json'), ('textures/block', '.png')):
            for old in (ASSETS / folder).glob('*' + ext):
                if '%s/%s' % (folder, old.name) not in self.files:
                    old.unlink()
        for rel, data in sorted(self.files.items()):
            path = ASSETS / rel
            path.parent.mkdir(parents=True, exist_ok=True)
            if not path.exists() or path.read_bytes() != data:
                path.write_bytes(data)
        return len(self.files)


# ---------------------------------------------------------------- the blocks of the slice

# The housing's arms by side of the model, and the Minecraft side each one points to.
ARMS = {'x+': 'east', 'x-': 'west', 'y+': 'south', 'y-': 'north', 'z+': 'up', 'z-': 'down'}
BLOCK = 'minecraft:block/block'


def housing(out, report):
    boxes = read(transport.pipe)
    skin = Skin('housing')
    stats = {}
    by_part = {}
    for b in boxes:
        by_part.setdefault(b[0], []).append(b)
    # Fix the palette over all parts first, so every part's model reads the same texture.
    for b in boxes:
        skin.slot(b[1])
    models = {'housing_core': by_part['body']}
    models.update(('housing_arm_' + side, by_part[arm]) for arm, side in ARMS.items())
    for name, part_boxes in models.items():
        out.text('models/block/%s.json' % name, model_json(skin.textures(), elements(part_boxes, skin, stats)))
    # In the inventory: a straight piece, the core with its east and west arms.
    inventory = by_part['body'] + by_part['x-'] + by_part['x+']
    out.text('models/block/housing_inventory.json', model_json(skin.textures(), elements(inventory, skin, stats), BLOCK))
    out.skin(skin)
    parts = [{'apply': {'model': 'rustcraft:block/housing_core'}}]
    for side in ('north', 'east', 'south', 'west', 'up', 'down'):
        parts.append({'when': {side: 'true'}, 'apply': {'model': 'rustcraft:block/housing_arm_' + side}})
    out.text('blockstates/housing.json', '{\n  "multipart": [\n%s\n  ]\n}\n' % ',\n'.join('    ' + json.dumps(p) for p in parts))
    report.append(('pipe', 'housing_core, housing_arm_<side> x6, housing_inventory', stats))


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
    """Blockstates of the crystal, the cell and the machines, whose models come from staged() below."""
    for single in ('crystal', 'cell'):
        out.text('blockstates/%s.json' % single,
                 '{\n  "variants": {\n    "": {"model": "rustcraft:block/%s"}\n  }\n}\n' % single)
    # The pump, the terminal, the tank and the station copy the furnace too, for a menu on use.
    for machine in ('furnace', 'crusher', 'gen_fuel', 'alloy', 'crafter', 'pump', 'terminal', 'tank', 'station'):
        variants = []
        for facing, y in FACING_Y.items():
            for lit in ('false', 'true'):
                model = {'model': 'rustcraft:block/' + machine}
                if y:
                    model['y'] = y
                variants.append('    %s: %s' % (json.dumps('facing=%s,lit=%s' % (facing, lit)), json.dumps(model)))
        out.text('blockstates/%s.json' % machine, '{\n  "variants": {\n%s\n  }\n}\n' % ',\n'.join(variants))


def staged(out, report, skipped):
    """Every other block model, at rest, for blocks that come later."""
    for section in BLOCK_SECTIONS:
        for name, draw in section.MODELS.items():
            if name in ('pipe', 'node'):
                continue
            every = read(draw, name)
            boxes = [b for b in every if at_rest(name, b[0])]
            skin = Skin(name, DEFAULT_TINTS.get(name))
            stats = {}
            els = elements(boxes, skin, stats)
            if stats.get('outside_limit'):
                skipped.append(name)
                continue
            out.text('models/block/%s.json' % name, model_json(skin.textures(), els, BLOCK))
            out.skin(skin)
            dropped = sorted({b[0] for b in every} - {b[0] for b in boxes})
            stats['dropped'] = dropped
            stats['pages'] = len(skin.page_names())
            report.append((name, '%d elements' % len(els), stats))


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


def monorail(out, report):
    boxes = read(transport.rail)
    by_part = {}
    for b in boxes:
        by_part.setdefault(b[0], []).append(b)
    parts = {'hub': by_part['body']}
    for k in range(4):
        parts['half%d' % k] = by_part[RAIL_HALVES[k]]
        parts['arc%d' % k] = by_part[RAIL_ARCS[k]]
        parts['climb%d' % k] = turned_boxes(by_part['slope'], k)
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
            part('hub', ['s%d' % m for m in masks if m not in RAIL_CORNERS])
            for k in range(4):
                part('half%d' % k, ['s%d' % m for m in masks if k in rail_halves_shown(m)])
            for mask, k in sorted(RAIL_CORNERS.items(), key=lambda e: e[1]):
                part('arc%d' % k, ['s%d' % mask])
            for k in range(4):
                part('climb%d' % k, ['c%d' % k])
    out.text('blockstates/monorail.json', '{\n  "multipart": [\n%s\n  ]\n}\n' % ',\n'.join('    ' + json.dumps(e) for e in entries))
    report.append(('rail', 'monorail_t1..4_{hub,half0..3,arc0..3,climb0..3,inventory}', stats))


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
# mob, its chassis, its floor bracket and each tool apart, so the tool can follow what it carries.
WORKER_PARTS = {
    'body': ['body'],
    'below': ['below'],
    'saw': ['saw', 'aim_yaw_saw', 'reach_saw', 'saw_disc'],
    'reaper': ['reaper', 'reaper_reel'],
}


def worker_parts(out, report):
    boxes = read(transport.worker)
    skin = Skin('worker_parts')
    for b in boxes:
        skin.slot(b[1])
    stats = {}
    for name, parts in WORKER_PARTS.items():
        part_boxes = [b for b in boxes if b[0] in parts]
        out.text('models/block/worker_%s.json' % name, model_json(skin.textures(), elements(part_boxes, skin, stats)))
    out.skin(skin)
    report.append(('worker', 'worker_body, worker_below, worker_saw, worker_reaper', stats))


# The slice-7 blocks copy the iron block with a `facing` of their own (rustcraft_content): the
# blockstate turns the staged model as the machines' does.
SLICE7_BLOCKS = ('dock_unload', 'dock_charge', 'turret', 'anchor',
                 # Slice 8: the energy sources, the battery, the miner, the placer and the breaker.
                 'solar', 'windmill', 'gen_lava', 'battery', 'miner', 'placer', 'breaker')


def slice7_states(out):
    for block in SLICE7_BLOCKS:
        variants = []
        for facing, y in FACING_Y.items():
            model = {'model': 'rustcraft:block/' + block}
            if y:
                model['y'] = y
            variants.append('    %s: %s' % (json.dumps('facing=%s' % facing), json.dumps(model)))
        out.text('blockstates/%s.json' % block, '{\n  "variants": {\n%s\n  }\n}\n' % ',\n'.join(variants))


def items(out):
    for name in icons_mc.ICONS:
        out.image('textures/item/%s.png' % name, icons_mc.icon(name))


def main():
    out, report, skipped = Writer(), [], []
    housing(out, report)
    node(out, report)
    machine_states(out)
    monorail(out, report)
    capsule(out, report)
    worker_parts(out, report)
    slice7_states(out)
    staged(out, report, skipped)
    items(out)
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
