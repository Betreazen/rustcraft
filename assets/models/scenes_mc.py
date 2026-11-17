"""The handbook's animated scenes in the game (spec H3, H4, H5), made from the guide's scenes.

The guide's scenes (docs/showcase/js/scenes.js, RC.SCENES) are lists of steps whose `run` changes
a scene state (engine.js `Scene`: objects put, moved and removed, flows of items, energy and
fluid along paths of cells, beams). Node runs every scene here as the guide's player does
(player.js `go`: the state is cumulative, `now` is the time the step starts at) and takes a
snapshot of each step at its start:

- the blocks present: each object's look in the game (`look_of`: a RustCraft type is the content
  mod's item, an environment type a vanilla item, water, lava and the projector's ghost a tinted
  cube), its cell, and when it vanishes during the step (`gone`). A block whose three faces
  towards the viewer (+x, +y, +z) are covered by full cubes is left out: nothing of it shows;
- the moving objects: their look and their path, speed, way (once, loop, ping) and how far into
  their move the step starts;
- the flows (their kind, colour, dots, speed and path of cells) and the beams;
- the night's veil (`tint`) and how long the step stays (player.js `stepTime`);
- the caption, as the lang key `rustcraft.handbook.scene.<id>.<n>`: Russian from the guide; the
  guide has no English, so en_us has no captions and the English window shows the scene without
  one.

handbook_mc.py calls `build` and writes its table and lines with the pages' own, so one run owns
every `rustcraft.handbook.` key. The table is `mod/rustcraft/rules/src/handbook/scenes.rs`
(rustcraft_rules::handbook::scenes); its types and what the window does with them are in
`rules/src/handbook/scene.rs`. The step's badges and name tags (`s.badges`, `s.tags`) become
labels: a point or a moving object to stand on, a tone, and their text as the lang key
`rustcraft.handbook.scene.<id>.<n>.label.<k>`. A dropped item (`drop`) is a little cube of its
colour. What the game cannot show is left out: the props that only change a model's pose (a
facing, a machine running, a tank's level; see the plan's engine limits) and types with no item
(`DROPPED`).
"""
import json
import shutil
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
GUIDE = ROOT / 'docs' / 'showcase' / 'js'
ITEMS = ROOT / 'mod' / 'content' / 'assets' / 'rustcraft' / 'items'
TABLE = ROOT / 'mod' / 'rustcraft' / 'rules' / 'src' / 'handbook' / 'scenes.rs'
sys.path.insert(0, str(HERE))
import poses_mc  # noqa: E402
POSES = poses_mc.Poses()
PREFIX = 'rustcraft.handbook.scene.'

# The guide's scripts as its page loads them (index.html), with what models.js gives the others,
# and every scene's steps as snapshots.
NODE = r"""
const vm = require('vm'), fs = require('fs'), path = require('path');
const dir = process.argv[1];
const ctx = vm.createContext({});
ctx.window = ctx;
const run = (f) => vm.runInContext(fs.readFileSync(path.join(dir, f), 'utf8'), ctx, { filename: f });
run('engine.js');
const RC = ctx.RC;
RC.TIER_COLOR = ['', '#c57f52', '#e2bd55', '#69d2d8', '#a184f2'];
RC.CH_COLOR = { i: '#ffa53c', f: '#4c9bf5', e: '#ffe14a', '-': '#15171c' };
for (const f of ['data.js', 'info.js', 'scenes.js']) run(f);
const stepTime = (st) => Math.max(st.dur || 5, 3 + st.text.length / 14);
const KEEP = ['tier', 'tool', 'color', 'on'];
const out = [];
for (const [id, def] of Object.entries(RC.SCENES)) {
  const s = new RC.Scene();
  let time = 0;
  const steps = def.steps.map((st) => {
    s.now = time;
    st.run(s);
    const objs = [];
    for (const o of s.objs.values()) {
      if (o.gone != null && o.gone <= time) continue;
      const r = { id: o.id, type: o.type, at: [o.x, o.y, o.z] };
      for (const k of KEEP) if (o[k] != null) r[k] = o[k];
      if (o.gone != null) r.gone = o.gone - time;
      if (o.path) r.move = { pts: o.path.map((p) => p.slice(0, 3)), speed: o.speed, mode: o.mode, since: time - o.t0 };
      objs.push(r);
    }
    const snap = {
      text: st.text, dur: stepTime(st), objs,
      flows: s.flows.map((f) => ({ pts: f.pts.map((p) => p.slice(0, 3)), color: f.color, n: f.n, speed: f.speed })),
      beams: s.beams.map((b) => ({ a: b.a, b: b.b, color: b.color })),
      badges: s.badges.map((b) => ({ at: [b.x + 0.5, b.y + 0.5, b.z + 2.15], text: b.text, kind: b.kind || 'bad' })),
      tags: s.tags.map((tg) => ({ obj: tg.obj, text: tg.text, lift: tg.lift || 1.45 })),
      tint: s.tint || null,
    };
    time += stepTime(st);
    return snap;
  });
  out.push({ id, steps });
}
process.stdout.write(JSON.stringify(out));
"""

# The guide's type -> the content mod's item, where the names differ.
RENAME = {
    'pipe': 'housing', 'node_hub': 'node', 'crystalblock': 'crystal', 'zipanchor': 'zip_anchor',
    'zip_handle': 'zipline', 'pdock': 'portal_dock',
}
# The guide's environment and props -> the base game's items (block items where there is one;
# entities as their spawn eggs; the guide's druse and seed as the amethyst they stand in for, as
# the mod's recipes do; the capsule, which has no item, as a minecart).
VANILLA = {
    'stone': 'stone', 'deepslate': 'deepslate', 'dirt': 'dirt', 'grass': 'grass_block',
    'planks': 'oak_planks', 'log': 'oak_log', 'leaves': 'oak_leaves', 'bedrock': 'bedrock',
    'glass': 'glass', 'bricks': 'bricks', 'wool': 'white_wool', 'netherrack': 'netherrack',
    'soul_sand': 'soul_sand', 'sand': 'sand', 'gravel': 'gravel', 'clay': 'clay',
    'sandstone': 'sandstone', 'sus_sand': 'suspicious_sand', 'sus_gravel': 'suspicious_gravel',
    'farmland': 'farmland', 'chest': 'chest', 'table': 'crafting_table', 'tree': 'oak_log',
    'jungle_log': 'jungle_log', 'beehive': 'beehive', 'portal': 'obsidian',
    'ore_iron': 'iron_ore', 'ore_coal': 'coal_ore', 'ore_copper': 'copper_ore',
    'ore_diamond': 'diamond_ore', 'ore_redstone': 'redstone_ore', 'ore_gold': 'gold_ore',
    'geode': 'budding_amethyst', 'seed': 'amethyst_cluster', 'capsule': 'minecart',
    'crop': 'wheat', 'carrots': 'carrot', 'potatoes': 'potato', 'beetroots': 'beetroot',
    'nether_wart': 'nether_wart', 'torchflower': 'torchflower', 'sugar_cane': 'sugar_cane',
    'cactus': 'cactus', 'bamboo': 'bamboo', 'kelp': 'kelp', 'pumpkin': 'pumpkin',
    'melon': 'melon', 'pumpkin_stem': 'pumpkin_seeds', 'melon_stem': 'melon_seeds',
    'sweet_berry_bush': 'sweet_berries', 'cave_vines': 'glow_berries', 'cocoa': 'cocoa_beans',
    'mob': 'zombie_head', 'skeleton': 'skeleton_skull', 'cow': 'cow_spawn_egg',
    'sheep': 'sheep_spawn_egg', 'bee': 'bee_spawn_egg', 'player': 'player_head',
    'axe': 'iron_axe', 'anvil': 'anvil',
}
# Cells no item shows: a tinted cube (ARGB), as the guide draws water, lava and the ghost.
TINT = {'water': 0xB03F76E4, 'lava': 0xF0E8601C, 'lavapool': 0xF0E8601C, 'ghost': 0x7066E8F0}
# Types the game has nothing for (none now: a dropped item is a little cube of its colour, `drop`).
DROPPED = set()
# A dropped item's cube stands this far down in its cell (models.js `drop`: 0.24 high at 0.05).
DROP_DOWN = 0.33
# Types whose model is a full opaque cube: what covers the faces of the block behind it.
FULL = {'stone', 'deepslate', 'dirt', 'grass', 'planks', 'log', 'bedrock', 'bricks', 'wool',
        'netherrack', 'soul_sand', 'sand', 'gravel', 'clay', 'sandstone', 'sus_sand',
        'sus_gravel', 'jungle_log', 'ore_iron', 'ore_coal', 'ore_copper', 'ore_diamond',
        'ore_redstone', 'ore_gold'}
# Flow colours of the guide (scenes.js IT, EN, FL, SOL, LAVA): energy and fluids; others are items.
ENERGY = {'#ffe14a'}
FLUID = {'#4c9bf5', '#59d38c', '#f2731c'}
TOOL = {'gun': 'tool_shooter'}
ARMOR = {'armor_helmet': 'tech_helmet', 'armor_chest': 'tech_chestplate',
         'armor_legs': 'tech_leggings', 'armor_boots': 'tech_boots'}
ALLOY = ['', 'coppersteel', 'sparksteel', 'crystallite', 'starsteel']


def tiered(base, tier, low):
    """The content mod's item of `base` at `tier`: `base` for 1, else `base_<tier>`, from `low`."""
    t = max(low, min(4, int(tier or 1)))
    return base if t == 1 else '%s_%d' % (base, t)


def look_of(o, have):
    """('item', id) or ('tint', argb) for a scene object, None when the game has nothing for it.
    `have(name)` says whether the content mod has the item rustcraft:<name>."""
    t = o['type']
    if t in TINT:
        return ('tint', TINT[t])
    if t == 'drop':
        return ('drop', 0xFF000000 | rgb(o.get('color') or '#d8a47a'))
    if t in DROPPED:
        return None
    name = None
    if t == 'rail':
        name = tiered('monorail', o.get('tier'), 1)
    elif t == 'alloyblock':
        name = ALLOY[max(1, min(4, int(o.get('tier') or 1)))]
    elif t == 'worker':
        tool = o.get('tool')
        name = 'tool_' + TOOL.get(tool, tool).removeprefix('tool_') if tool else 'chassis'
    elif t in ARMOR:
        name = tiered(ARMOR[t], o.get('tier'), 2)
    elif t in ('blade', 'eshield'):
        name = tiered(t, o.get('tier'), 2)
    elif t in VANILLA:
        return ('item', 'minecraft:' + VANILLA[t])
    else:
        name = RENAME.get(t, t)
    if have(name):
        return POSES.look(name, o.get('on')) or ('item', 'rustcraft:' + name)
    return None


def guide():
    node = shutil.which('node')
    if not node:
        sys.exit('scenes_mc.py: Node is not on the PATH (https://nodejs.org); it runs the guide\'s scenes.js')
    done = subprocess.run([node, '-e', NODE, str(GUIDE)], capture_output=True, check=False)
    if done.returncode != 0:
        sys.exit('scenes_mc.py: the guide\'s scenes did not run:\n' + done.stderr.decode('utf-8', 'replace'))
    return json.loads(done.stdout.decode('utf-8'))


def cell(at):
    return tuple(round(v) for v in at) if all(float(v).is_integer() for v in at) else None


def hidden(objs):
    """Ids of static blocks whose +x, +y and +z neighbours are static full cubes for the whole
    step: their faces towards the viewer are covered."""
    full = {cell(o['at']) for o in objs if o['type'] in FULL and 'move' not in o and 'gone' not in o}
    full.discard(None)
    out = set()
    for o in objs:
        c = cell(o['at'])
        if c is None or 'move' in o:
            continue
        x, y, z = c
        if (x + 1, y, z) in full and (x, y + 1, z) in full and (x, y, z + 1) in full:
            out.add(o['id'])
    return out


def num(v):
    """A float for Rust, short: whole numbers as `3.`, others to four places."""
    v = round(float(v), 4)
    if v.is_integer():
        return '%d.' % v
    return repr(v)


def point(p):
    return '[%s]' % ','.join(num(v) for v in p)


def rgb(colour):
    return int(colour.lstrip('#')[:6], 16)


def tint_argb(css):
    """`rgba(r,g,b,a)` -> ARGB."""
    if not css:
        return 0
    r, g, b, a = (float(v) for v in css[css.index('(') + 1:css.index(')')].split(','))
    return (round(a * 255) << 24) | (int(r) << 16) | (int(g) << 8) | int(b)


class Table:
    """The Rust table, its looks, block lists and paths each written once."""

    def __init__(self):
        self.looks, self.look_index = [], {}
        self.lists, self.list_index = [], {}
        self.paths, self.path_index = [], {}

    def look(self, look):
        if look not in self.look_index:
            self.look_index[look] = len(self.looks)
            self.looks.append(look)
        return self.look_index[look]

    def blocks(self, rows):
        key = tuple(rows)
        if key not in self.list_index:
            self.list_index[key] = len(self.lists)
            self.lists.append(rows)
        return 'B%d' % self.list_index[key]

    def path(self, pts):
        key = '&[%s]' % ','.join(point(p) for p in pts)
        if key not in self.path_index:
            self.path_index[key] = len(self.paths)
            self.paths.append(key)
        return 'P%d' % self.path_index[key]


MODE = {'once': 'Once', 'loop': 'Loop', 'ping': 'Ping'}
TONE = {'bad': 'Bad', 'warn': 'Warn', 'ok': 'Ok', 'info': 'Info'}


def step_rust(t, st, have, dropped):
    """One step as a Rust `Step`, and its looks noted in `t`."""
    objs = st['objs']
    covered = hidden(objs)
    blocks, movers, mover_of = [], [], {}
    for o in objs:
        look = look_of(o, have)
        if look is None:
            dropped[o['type']] = dropped.get(o['type'], 0) + 1
            continue
        i = t.look(look)
        if 'move' in o:
            m = o['move']
            mover_of[o['id']] = len(movers)
            movers.append('Mover { look: %d, path: %s, speed: %s, mode: Mode::%s, since: %s }' % (
                i, t.path(m['pts']), num(m['speed']), MODE[m['mode']], num(m['since'])))
        elif o['id'] not in covered:
            at = o['at'] if look[0] != 'drop' else [o['at'][0], o['at'][1], o['at'][2] - DROP_DOWN]
            if 'gone' in o:
                blocks.append('bg(%d,%s,%s)' % (i, point(at), num(o['gone'])))
            else:
                blocks.append('b(%d,%s)' % (i, point(at)))
    flows = []
    for f in st['flows']:
        kind = 'Energy' if f['color'] in ENERGY else 'Fluid' if f['color'] in FLUID else 'Item'
        flows.append('Flow { kind: Kind::%s, rgb: 0x%06x, dots: %d, speed: %s, path: %s }' % (
            kind, rgb(f['color']), int(f['n']), num(f['speed']), t.path(f['pts'])))
    beams = []
    by_id = {o['id']: o for o in objs}
    for bm in st['beams']:
        to = bm['b']
        if isinstance(to, str):
            if to in mover_of:
                end = 'End::Mover(%d)' % mover_of[to]
            elif to in by_id:
                o = by_id[to]
                end = 'End::Point(%s)' % point([o['at'][0] + 0.5, o['at'][1] + 0.5, o['at'][2] + 1.1])
            else:
                continue
        else:
            end = 'End::Point(%s)' % point(to)
        beams.append('Beam { from: %s, to: %s, rgb: 0x%06x }' % (point(bm['a']), end, rgb(bm['color'])))
    labels, texts = [], []
    for tg in st['tags']:
        o = by_id.get(tg['obj'])
        if o is None:
            continue
        if tg['obj'] in mover_of:
            at, lift = 'End::Mover(%d)' % mover_of[tg['obj']], tg['lift']
        else:
            at, lift = 'End::Point(%s)' % point([o['at'][0] + 0.5, o['at'][1] + 0.5, o['at'][2] + tg['lift']]), 0
        labels.append('Label { at: %s, lift: %s, tone: Tone::Tag }' % (at, num(lift)))
        texts.append(tg['text'])
    for bd in st['badges']:
        labels.append('Label { at: End::Point(%s), lift: 0., tone: Tone::%s }' % (point(bd['at']), TONE[bd['kind']]))
        texts.append(bd['text'])
    return ('Step { dur: %s, blocks: %s, movers: &[%s], flows: &[%s], beams: &[%s], tint: 0x%08x, labels: &[%s] }' % (
        num(st['dur']), t.blocks(blocks), ', '.join(movers), ', '.join(flows), ', '.join(beams),
        tint_argb(st['tint']), ', '.join(labels)), len(blocks) + len(movers), texts)


def rust_str(s):
    return json.dumps(s, ensure_ascii=False)


def build(page_ids, have):
    """The scenes of the pages `page_ids`: (the Rust table, the Russian lines, the English lines,
    a summary). `have(name)` says whether the content mod has rustcraft:<name>."""
    t = Table()
    scenes, ru, dropped = [], {}, {}
    steps_n, badges, numbers, most = 0, 0, 0, 0
    for sc in guide():
        if sc['id'] not in page_ids:
            sys.exit('scenes_mc.py: the guide\'s scene %s has no page' % sc['id'])
        steps = []
        for n, st in enumerate(sc['steps']):
            rust, shown, texts = step_rust(t, st, have, dropped)
            steps.append(rust)
            most = max(most, shown)
            ru['%s%s.%d' % (PREFIX, sc['id'], n)] = st['text']
            for k, text in enumerate(texts):
                ru['%s%s.%d.label.%d' % (PREFIX, sc['id'], n, k)] = text
            badges += len(texts)
            numbers += any(c.isdigit() for c in st['text'])
        steps_n += len(steps)
        scenes.append('    Scene { id: %s, steps: &[\n        %s,\n    ] },' % (
            rust_str(sc['id']), ',\n        '.join(steps)))
    looks = []
    for look in t.looks:
        kind, value = look[0], look[1]
        if kind == 'item':
            looks.append('Look::Item(%s)' % rust_str(value))
        elif kind == 'model':
            looks.append('Look::Model(%s, %s)' % (rust_str(value), rust_str(look[2])))
        else:
            looks.append('Look::%s(0x%08x)' % ('Drop' if kind == 'drop' else 'Tint', value))
    lists = ['const B%d: &[Block] = &[%s];' % (i, ','.join(rows)) for i, rows in enumerate(t.lists)]
    paths = ['const P%d: &[[f32; 3]] = %s;' % (i, p) for i, p in enumerate(t.paths)]
    text = ('//! The handbook\'s scenes, written by assets/models/scenes_mc.py (through handbook_mc.py)\n'
            '//! from the guide\'s docs/showcase/js/scenes.js: do not edit by hand, run the script.\n\n'
            'use super::scene::{Beam, Block, End, Flow, Kind, Label, Look, Mode, Mover, Scene, Step, Tone, b, bg};\n\n'
            '/// What the scenes\' blocks look like, by `Block::look` and `Mover::look`.\n'
            '#[rustfmt::skip]\npub const LOOKS: &[Look] = &[\n    %s,\n];\n\n'
            '#[rustfmt::skip]\npub const SCENES: &[Scene] = &[\n%s\n];\n\n'
            '// The steps\' block lists and the paths of movers and flows, each once.\n'
            '%s\n%s\n'
            % (',\n    '.join(looks), '\n'.join(scenes),
               '\n'.join('#[rustfmt::skip]\n' + line for line in lists),
               '\n'.join('#[rustfmt::skip]\n' + line for line in paths)))
    stills = POSES.write()
    summary = {'stills': stills, 'scenes': len(scenes), 'steps': steps_n, 'looks': len(t.looks), 'lists': len(t.lists),
               'dropped': dropped, 'badges': badges, 'numbers': numbers, 'most': most}
    return text, ru, {}, summary


def write(text):
    data = text.encode('utf-8')
    TABLE.parent.mkdir(parents=True, exist_ok=True)
    if not TABLE.exists() or TABLE.read_bytes() != data:
        TABLE.write_bytes(data)
        return True
    return False


if __name__ == '__main__':
    sys.exit('scenes_mc.py: run assets/models/handbook_mc.py, which writes the scenes with the pages')
