"""The engineer's handbook in the game (spec H2, H5, H6): its pages, made from the guide.

The guide's data (docs/showcase/js/data.js, the window mock-ups in ui.js) are plain scripts that
fill the page's `RC`; Node runs them here as the page does, with no browser, and hands their
entries over as JSON. From them this script writes

- the page table `mod/rustcraft/rules/src/handbook/pages.rs` (rustcraft_rules::handbook::pages):
  the sections in the guide's order and each page's section, tier, item, and how many lines of
  each kind its lang has. The table is Rust, as export_mc.py's `rust_table` writes the moving
  parts, so the window needs no file of its own at run time;
- the pages' texts into the content mod's lang files as `rustcraft.handbook.*` keys: Russian from
  the guide, English from handbook_en.py (shorter: a page's name and its summary). Every key
  under `rustcraft.handbook.` belongs to this script: it takes them out of both files and writes
  them again after the others, so a run gives the same files.

What a page holds, in the order the window shows it: the name and the summary, the crafting
recipe (the window reads it from the game's recipes by the page's item, so only the item is
here), the paragraphs, the stats as key and value, the diagnoses with what to do about each
(`RC.DIAG_FIX`), and for a block with a window its numbered parts with a line each (H6, ui.js
`RC.UI`). The glossary's page holds the glossary as its stats, the tiers' page the tier table.
The guide's links and markup become plain text. The animated scenes (H3) come from the guide's
scenes.js through scenes_mc.py: their table `rules/src/handbook/scenes.rs` and their captions
`rustcraft.handbook.scene.<id>.<n>`, written by the same run.

From the repository's root, with Node on the PATH:

    py -3 assets/models/handbook_mc.py
"""
import html
import json
import re
import shutil
import subprocess
import sys
from pathlib import Path

sys.dont_write_bytecode = True
import handbook_en  # noqa: E402
import scenes_mc  # noqa: E402
import windows_mc  # noqa: E402

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
GUIDE = ROOT / 'docs' / 'showcase' / 'js'
CONTENT = ROOT / 'mod' / 'content' / 'assets' / 'rustcraft'
LANG = CONTENT / 'lang'
TABLE = ROOT / 'mod' / 'rustcraft' / 'rules' / 'src' / 'handbook' / 'pages.rs'
PREFIX = 'rustcraft.handbook.'
HANDBOOK = 'handbook'

# The guide's page id -> the content mod's item whose icon and crafting recipe the page shows. A
# page with none (an overview, a fluid, an entity) shows the handbook's own icon and no recipe.
ITEM = {
    'housing': 'housing', 'node': 'node', 'ch_item': 'item_channel', 'ch_fluid': 'fluid_channel',
    'ch_energy': 'energy_channel', 'facade': 'facade', 'wrench': 'wrench',
    'crystal': 'crystal', 'structure': 'crystal', 'solar': 'solar', 'turbine': 'turbine',
    'gen_fuel': 'gen_fuel', 'windmill': 'windmill', 'gen_lava': 'gen_lava', 'battery': 'battery',
    'alloy': 'alloy', 'furnace': 'furnace', 'crusher': 'crusher', 'tank': 'tank', 'pump': 'pump',
    'crafter': 'crafter', 'placer': 'placer', 'trash': 'trash', 'disintegrator': 'disintegrator',
    'breaker': 'breaker', 'mixer': 'mixer', 'charger': 'charger', 'charge_plate': 'charge_plate',
    'repair': 'repair', 'kitchen': 'kitchen', 'fishery': 'fishery', 'feeder': 'feeder',
    'butcher': 'butcher', 'shearer': 'shearer', 'apiary': 'apiary', 'brewer': 'brewer',
    'xp_tank': 'xp_tank', 'enchanter': 'enchanter', 'archeo': 'archeo', 'projector': 'projector',
    'builder': 'builder',
    'mod_speed': 'module_speed', 'mod_eco': 'module_economy', 'mod_cap': 'module_capacity',
    'mod_store_cap': 'module_store_cap', 'mod_output': 'module_output', 'mod_scout': 'module_scout',
    'mod_area': 'module_area', 'mod_storage': 'module_storage', 'mod_rare': 'module_rare',
    'mod_pick': 'module_pick', 'mod_stratum': 'module_stratum', 'mod_tunnel': 'module_tunnel',
    'mod_height': 'module_height', 'mod_lava': 'module_lava', 'mod_build': 'module_build',
    'mod_radius': 'module_radius', 'mod_bleed': 'module_bleed', 'mod_poison': 'module_poison',
    'mod_slow': 'module_slow', 'mod_heal': 'module_heal', 'mod_guard': 'module_guard',
    'mod_power': 'module_power', 'mod_haste': 'module_haste', 'mod_recipes': 'module_recipes',
    'cell': 'cell', 'terminal': 'terminal', 'tablet': 'tablet',
    'monorail': 'monorail', 'lift': 'monorail', 'station': 'station',
    'chassis': 'chassis', 'dock_unload': 'dock_unload', 'dock_load': 'dock_load',
    'dock_charge': 'dock_charge', 'tool_saw': 'tool_saw', 'tool_reaper': 'tool_reaper',
    'tool_hold': 'tool_hold', 'tool_grab': 'tool_grab', 'tool_bore': 'tool_bore',
    'tool_gun': 'tool_shooter', 'tool_scoop': 'tool_scoop', 'tool_map': 'tool_map',
    'upgrades': 'upgrade_speed', 'portal_dock': 'portal_dock',
    'miner': 'miner', 'vein': 'vein', 'anchor': 'anchor', 'turret': 'turret',
    'armor': 'tech_chestplate_2', 'pmodules': 'pm_dash', 'jetpack': 'jetpack', 'blade': 'blade_2',
    'eshield': 'eshield_2', 'rifle': 'rifle', 'scout_compass': 'scout_compass',
    'backpack': 'backpack', 'harpoon': 'harpoon', 'zipline': 'zipline', 'guide': HANDBOOK,
    'enchants': 'book_capacity', 'alloys': 'coppersteel', 'dusts': 'dust_iron',
}

# What a page's recipes make beyond its item, where the guide's entry shows the recipes of several
# items (game ids without the namespace; every tier of each counts, as `_<n>` names it). A page
# whose entry has no recipes shows none; one with recipes shows those of its item and these.
MAKES = {
    'alloys': ['coppersteel', 'sparksteel', 'crystallite', 'starsteel'],
    'dusts': ['dust_iron', 'dust_copper', 'dust_gold'],
    'fluids': ['#fluid'],
    'armor': ['tech_helmet_2', 'tech_chestplate_2', 'tech_leggings_2', 'tech_boots_2'],
    'pmodules': ['pm_dash', 'pm_run', 'pm_jump', 'pm_absorber', 'pm_capacity', 'pm_night',
                 'pm_magnet', 'pm_damper'],
    'upgrades': ['upgrade_battery', 'upgrade_speed'],
    'enchants': ['book_capacity', 'book_thrift'],
}
ANY_FLUID = '#fluid'

# The window's own words.
UI = {
    'title': ('Справочник инженера', "Engineer's Handbook"),
    'contents': ('Содержание', 'Contents'),
    'sections': ('Разделы', 'Sections'),
    'recipe': ('Рецепт', 'Recipe'),
    'recipes': ('Рецепты', 'Recipes'),
    'see': ('См. также', 'See also'),
    'diagnoses': ('Диагнозы', 'Diagnoses'),
    'window': ('Окно блока', "The block's window"),
    'sheet': ('%s / %s', '%s / %s'),
    'scene': ('Сцена', 'Scene'),
    'step': ('Шаг %s из %s', 'Step %s of %s'),
    'given': ('Справочник инженера у тебя: правый клик открывает его',
              "You have the Engineer's Handbook: a right click opens it"),
}
TIERS_EN = ['', 'Coppersteel', 'Sparksteel', 'Crystallite', 'Starsteel']

# The guide's scripts as its page loads them, and its entries as JSON.
NODE = r"""
const vm = require('vm'), fs = require('fs'), path = require('path');
const dir = process.argv[1];
const RC = { TIER_COLOR: ['', '#c57f52', '#e2bd55', '#69d2d8', '#a184f2'], shade: (c) => c, CH_COLOR: {} };
const ctx = vm.createContext({ RC });
for (const f of ['data.js', 'ui.js']) vm.runInContext(fs.readFileSync(path.join(dir, f), 'utf8'), ctx, { filename: f });
const label = (h) => { const m = /<label>([\s\S]*?)<\/label>/.exec(h || ''); return m ? m[1] : ''; };
const ui = {};
for (const [id, w] of Object.entries(RC.UI)) ui[id] = { title: w.title, parts: w.parts.map((p) => [label(p.html), p.note || '']), html: w.parts.map((p) => p.html) };
process.stdout.write(JSON.stringify({
  cats: RC.CATS, tiers: RC.TIERS, tierTable: RC.TIER_TABLE, fix: RC.DIAG_FIX, glossary: RC.GLOSSARY, ui,
  entries: RC.ENTRIES.map((e) => ({ id: e.id, cat: e.cat, tier: e.tier || 0, name: e.name, sum: e.sum || '',
    text: e.text || [], stats: e.stats || [], diag: e.diag || [], page: e.page || '',
    recipes: (e.recipes || []).length, see: e.see || [],
    tables: (e.tables || []).map((t) => ({ title: t.title, note: t.note || '', head: t.head, rows: t.rows })) })),
}));
"""


def guide():
    node = shutil.which('node')
    if not node:
        sys.exit('handbook_mc.py: Node is not on the PATH (https://nodejs.org); it runs the guide\'s data.js')
    done = subprocess.run([node, '-e', NODE, str(GUIDE)], capture_output=True, check=False)
    if done.returncode != 0:
        sys.exit('handbook_mc.py: the guide did not load:\n' + done.stderr.decode('utf-8', 'replace'))
    return json.loads(done.stdout.decode('utf-8'))


def plain(s):
    """The guide's text as the game draws it: no links or markup, one line."""
    s = re.sub(r'<[^>]+>', '', str(s))
    return re.sub(r'\s+', ' ', html.unescape(s)).strip()


def stats_of(e, g):
    if e['page'] == 'glossary':
        return g['glossary']
    rows = list(e['stats'])
    if e['page'] == 'tiers':
        for t, tier in enumerate(g['tiers']):
            if tier:
                rows.append(['Ступень %d · %s' % (t, tier['name']), tier['when']])
        rows += [[r[0], ' / '.join(str(v) for v in r[1:])] for r in g['tierTable']]
    return rows


def page_lines(e, g):
    """The Russian lang lines of one page and its row of the table."""
    p = PREFIX + 'page.' + e['id']
    ru = {p + '.name': plain(e['name']), p + '.sum': plain(e['sum'])}
    text = [plain(t) for t in e['text'] if plain(t)]
    for i, t in enumerate(text):
        ru['%s.text.%d' % (p, i)] = t
    stats = [(plain(k), plain(v)) for k, v in stats_of(e, g)]
    for i, (k, v) in enumerate(stats):
        ru['%s.stat.%d.key' % (p, i)] = k
        ru['%s.stat.%d.value' % (p, i)] = v
    for i, d in enumerate(e['diag']):
        ru['%s.diag.%d' % (p, i)] = plain(d)
        ru['%s.diag.%d.fix' % (p, i)] = plain(g['fix'].get(d, ''))
    window = g['ui'].get(e['id'])
    parts = window['parts'] if window else []
    for i, (lab, note) in enumerate(parts):
        if plain(lab):
            ru['%s.part.%d.label' % (p, i)] = plain(lab)
        ru['%s.part.%d' % (p, i)] = plain(note)
    for t, tab in enumerate(e['tables']):
        ru['%s.table.%d.title' % (p, t)] = plain(tab['title'])
        if plain(tab['note']):
            ru['%s.table.%d.note' % (p, t)] = plain(tab['note'])
        for c, h in enumerate(tab['head']):
            ru['%s.table.%d.head.%d' % (p, t, c)] = plain(h)
        for r, row in enumerate(tab['rows']):
            for c, v in enumerate(row):
                ru['%s.table.%d.row.%d.%d' % (p, t, r, c)] = plain(v)
    counts = (len(text), len(stats), len(e['diag']), len(parts))
    return ru, counts


def rust_str(s):
    return json.dumps(s, ensure_ascii=False)


def table(rows, sections):
    lines = []
    for r in rows:
        item = 'Some(%s)' % rust_str('rustcraft:' + r['item']) if r['item'] else 'None'
        makes = ', '.join(rust_str(m if m == ANY_FLUID else 'rustcraft:' + m) for m in r['makes'])
        tables = ', '.join('[%d, %d]' % t for t in r['tables'])
        see = ', '.join(rust_str(x) for x in r['see'])
        lines.append('    Page { id: %s, section: %d, tier: %d, item: %s, text: %d, stats: %d, diagnoses: %d, parts: %d, makes: &[%s], tables: &[%s], see: &[%s] },'
                     % ((rust_str(r['id']), r['section'], r['tier'], item) + r['counts'] + (makes, tables, see)))
    return ('//! The handbook\'s pages, written by assets/models/handbook_mc.py from the guide\n'
            '//! (docs/showcase/js/data.js, ui.js): do not edit by hand, run the script.\n\n'
            'use super::Page;\n\n'
            '/// The guide\'s sections, in its order: `rustcraft.handbook.section.<id>`.\n'
            '#[rustfmt::skip]\npub const SECTIONS: &[&str] = &[%s];\n\n'
            '#[rustfmt::skip]\npub const PAGES: &[Page] = &[\n%s\n];\n'
            % (', '.join(rust_str(s) for s in sections), '\n'.join(lines)))


def write_lang(name, ours):
    path = LANG / name
    raw = path.read_text(encoding='utf-8')
    keys = json.loads(raw)
    kept = {k: v for k, v in keys.items() if not k.startswith(PREFIX)}
    kept.update(ours)
    text = json.dumps(kept, ensure_ascii=False, indent=2) + '\n'
    if text != raw:
        path.write_text(text, encoding='utf-8', newline='\n')
        return True
    return False


def main():
    g = guide()
    sections = [c[0] for c in g['cats']]
    missing = [c for c in sections if c not in handbook_en.SECTIONS]
    missing += [e['id'] for e in g['entries'] if e['id'] not in handbook_en.PAGES]
    if missing:
        sys.exit('handbook_mc.py: handbook_en.py has no English for ' + ', '.join(missing))
    ru, en, rows = {}, {}, []
    ids = {e['id'] for e in g['entries']}
    for key, (r, e) in UI.items():
        ru[PREFIX + key], en[PREFIX + key] = r, e
    for t, tier in enumerate(g['tiers']):
        if tier:
            ru['%stier.%d' % (PREFIX, t)] = 'Ступень %d · %s' % (t, tier['name'])
            en['%stier.%d' % (PREFIX, t)] = 'Tier %d · %s' % (t, TIERS_EN[t])
    for cid, name in g['cats']:
        ru[PREFIX + 'section.' + cid] = plain(name)
        en[PREFIX + 'section.' + cid] = handbook_en.SECTIONS[cid]
    for e in g['entries']:
        lines, counts = page_lines(e, g)
        ru.update(lines)
        name, summary = handbook_en.PAGES[e['id']]
        en[PREFIX + 'page.%s.name' % e['id']] = name
        en[PREFIX + 'page.%s.sum' % e['id']] = summary
        item = ITEM.get(e['id'])
        if item and item != HANDBOOK and not (CONTENT / 'items' / (item + '.json')).exists():
            sys.exit('handbook_mc.py: the page %s names %s, which the content mod has no item for' % (e['id'], item))
        makes = []
        if e['recipes']:
            makes = MAKES.get(e['id'], []) + ([item] if item else [])
        for m in makes:
            if m != ANY_FLUID and m != HANDBOOK and not (CONTENT / 'items' / (m + '.json')).exists():
                sys.exit('handbook_mc.py: the page %s makes %s, which the content mod has no item for' % (e['id'], m))
        rows.append({'id': e['id'], 'section': sections.index(e['cat']), 'tier': e['tier'],
                     'item': item, 'counts': counts, 'makes': list(dict.fromkeys(makes)),
                     'tables': [(max(len(t['head']), max((len(x) for x in t['rows']), default=0)), len(t['rows']))
                                for t in e['tables']],
                     'see': [x for x in e['see'] if x in ids]})
    TABLE.parent.mkdir(parents=True, exist_ok=True)
    text = table(rows, sections).encode('utf-8')
    if not TABLE.exists() or TABLE.read_bytes() != text:
        TABLE.write_bytes(text)
    have = lambda name: (CONTENT / 'items' / (name + '.json')).exists()  # noqa: E731
    scene_table, scene_ru, scene_en, done = scenes_mc.build({e['id'] for e in g['entries']}, have)
    ru.update(scene_ru)
    en.update(scene_en)
    scenes_mc.write(scene_table)
    ids = {e['id'] for e in g['entries']}
    shown = {pid: {'title': w['title'], 'parts': [{'html': h} for h in w['html']]}
             for pid, w in g['ui'].items() if pid in ids}
    window_table, window_ru = windows_mc.build(shown)
    ru.update(window_ru)
    windows_mc.write(window_table)
    changed = [n for n, d in (('ru_ru.json', ru), ('en_us.json', en)) if write_lang(n, d)]
    print('%d pages in %d sections; %d Russian and %d English lines%s' % (
        len(rows), len(sections), len(ru), len(en), ('; rewrote ' + ', '.join(changed)) if changed else ''))
    dropped = ', '.join('%s x%d' % kv for kv in sorted(done['dropped'].items())) or 'nothing'
    print('%d scenes, %d steps, %d looks, %d block lists (at most %d blocks a step); dropped: %s; '
          '%d labels (badges and name tags); %d captions have digits (H4)' % (
              done['scenes'], done['steps'], done['looks'], done['lists'], done['most'], dropped,
              done['badges'], done['numbers']))


if __name__ == '__main__':
    main()
