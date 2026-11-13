"""The guide's item icons for the items the content mod has, as 16 x 16 RGBA pixel rows.

A port of the drawing in docs/showcase/js/icons.js (the shapes `channel`, `wrench`, `dust`, `ingot`
and `module`, the module sign `cap`, the palette and the icons ch_item, ch_fluid, ch_energy, wrench,
dust, dust_cu, dust_au, the alloys a1 to a4 and mod_cap). export_mc.py writes them as the item
textures; keep them in step with icons.js when an icon there changes.
"""
import math

TIER = ['', '#c57f52', '#e2bd55', '#69d2d8', '#a184f2']
FIXED = {
    'x': '#15171c', 'w': '#ffffff', 'm': '#9aa3b2', 'n': '#5d6470', 'o': '#d8dce4',
    'y': '#e2bd55', 'z': '#9a7a2a', 'r': '#e0564a', 's': '#ff9a3c', 't': '#ffe14a',
    'h': '#a8793f', 'i': '#6b4a2b', 'u': '#56dbe2', 'v': '#b8f6fa', 'j': '#2f8f96', 'q': '#59d38c',
}
CHANNEL = [
    '', '', '', '',
    '..kkkkkkkkkkkk..',
    '.knoooooooooonk.',
    '.knggnggggnggnk.',
    '.kneeneeeeneenk.',
    '.kneeneeeeneenk.',
    '.knffnffffnffnk.',
    '.knmmmmmmmmmmnk.',
    '..kkkkkkkkkkkk..',
]
WRENCH = [
    '..kk..kk........',
    '.kmk..kmk.......',
    '.kmk..kmk.......',
    '.kmmkkmmk.......',
    '..kmmmmok.......',
    '...kmmmmok......',
    '....kkmmmok.....',
    '......kcaak.....',
    '.......kcaak....',
    '........kcaak...',
    '.........kcaak..',
    '..........kabk..',
    '...........kk...',
]
DUST = [
    '', '', '', '', '', '',
    '.......kk.......',
    '.....kkcck......',
    '....kccaaakk....',
    '...kcaaaaabak...',
    '..kcaaabaaaabk..',
    '.kaaabaaaabaabk.',
    '.kbbbbbbbbbbbbk.',
    '..kkkkkkkkkkkk..',
]


INGOT = [
    '', '', '', '',
    '......kkkkkkkkk.',
    '.....klccccccck.',
    '....kccccccccbk.',
    '...kccccccccbbk.',
    '..kaaaaaaaaabbk.',
    '..kalaaaaaaabbk.',
    '..kaaaaaaaaabk..',
    '..kdddddddddk...',
    '...kkkkkkkkk....',
]
MODULE = [
    '',
    '.kkkkkkkkkkkkkk.',
    '.kcccccccccccbk.',
    '.kcaaaaaaaaaabk.',
    '.kcaaaaaaaaaabk.',
    '.kcaaaaaaaaaabk.',
    '.kcaaaaaaaaaabk.',
    '.kcaaaaaaaaaabk.',
    '.kcaaaaaaaaaabk.',
    '.kcaaaaaaaaaabk.',
    '.kcaaaaaaaaaabk.',
    '.kcaaaaaaaaaabk.',
    '.kbbbbbbbbbbbbk.',
    '.kxyxyxyxyxyxyk.',
    '.kkkkkkkkkkkkkk.',
]
# icons.js SIGN.cap: the capacity module's sign, drawn at (4, 3).
CAP = ['eeeeeee.', 'e.....e.', 'eeeeeee.', 'e.....e.', 'eeeeeee.', 'e.....e.', 'eeeeeee.', '........']


def dots(points, ch):
    """icons.js `dots`: one-pixel overlays."""
    return [(x, y, [ch]) for x, y in points]


# id: (rows, main colour a, accent e, overlays [(x, y, rows)])
ICONS = {
    'item_channel': (CHANNEL, '#5d6470', '#ffa53c', [(6, 6, ['xxxx', 'xwwx', 'xwwx', 'xxxx'])]),
    'fluid_channel': (CHANNEL, '#5d6470', '#4c9bf5', [(6, 6, ['..w.', '.ww.', 'wwww', '.ww.'])]),
    'energy_channel': (CHANNEL, '#5d6470', '#ffe14a', [(6, 6, ['..xx', '.xx.', 'xxxx', '.xx.'])]),
    'wrench': (WRENCH, TIER[1], TIER[1], []),
    # The guide's dust (ore dust), dust_cu and dust_au.
    'dust_iron': (DUST, '#9a8a7a', '#d8a47a', dots([(6, 11), (10, 10), (8, 12)], 's') + dots([(5, 12), (11, 12), (9, 9)], 't')),
    'dust_copper': (DUST, '#e0824a', '#ffd0a8', dots([(8, 8), (12, 11), (4, 12)], 'l')),
    'dust_gold': (DUST, '#f5d54a', '#ffffff', dots([(8, 8), (12, 11), (4, 12), (13, 6)], 'w')),
    # The guide's alloys a1 to a4: one ingot, the tier colour and a mark of its own.
    'coppersteel': (INGOT, TIER[1], '#d8dce4', [(3, 9, ['.ooooooooo'])]),
    'sparksteel': (INGOT, TIER[2], '#ffe14a', [(11, 0, ['..t..', '.twt.', 'ttwtt', '.twt.', '..t..'])]),
    'crystallite': (INGOT, TIER[3], '#b8f6fa', dots([(7, 5), (10, 7), (5, 9)], 'w') + dots([(8, 6), (6, 7), (8, 10)], 'v')),
    'starsteel': (INGOT, TIER[4], '#ffffff', [(10, 0, ['...w...', '...w...', '..www..', 'wwwgwww', '..www..', '...w...', '...w...'])]),
    # The guide's mod_cap: a module board with the capacity sign.
    'module_capacity': (MODULE, '#3b4a5a', '#e6e8ee', [(4, 3, CAP)]),
}




# ---- Slice 7 (icons.js SIGN, the turret boards and the tools): module cards, upgrade cards, the
# saw and the reaper.
SIGN = {
    'speed': ['g...g...', 'ee..ee..', '.ee..ee.', '..ee..ee', '.ee..ee.', 'ee..ee..', 'f...f...', '........'],
    'eco': ['....gge.', '..ggeee.', '.geeefe.', '.eeefee.', '.eefeee.', '..feee..', '.f......', 'f.......'],
    'storage': ['.ffffff.', 'fggggggf', 'feeeeeef', 'fffwwfff', 'feewweef', 'feeeeeef', 'ffffffff', '........'],
    'radius': ['..eeee..', '.e....e.', 'e..gg..e', 'e.g..g.e', 'e.g..g.e', 'e..gg..e', '.e....e.', '..eeee..'],
    'up': ['...ee...', '..eeee..', '.eeeeee.', 'eeeeeeee', '...ee...', '...ee...', '...ee...', '...ff...'],
    'bleed': ['...ee...', '...ee...', '..eeee..', '.geeeee.', 'egeeeeef', 'egeeeeef', '.eeeeff.', '..ffff..'],
    'poison': ['.eeeeee.', 'eeeeeeee', 'exxeexxe', 'exxeexxe', 'eeeffeee', '.eeeeee.', '.efefef.', '........'],
    'slow': ['e..e..e.', '.e.e.e..', '..eee...', 'eeegeee.', '..eee...', '.e.e.e..', 'e..e..e.', '........'],
    'heal': ['.ee..ee.', 'egeeeeee', 'eeeeeeee', 'eeeeeeef', '.eeeeef.', '..eeef..', '...ef...', '........'],
    'guard': ['eeeeeeee', 'eggeeeef', 'egeeeeef', 'egeeeeef', '.eeeeef.', '.eeeeef.', '..eeff..', '...ff...'],
    'power': ['......ee', '.....ege', '....ege.', 'f..ege..', '.ffge...', '..ff....', '.f......', 'f.......'],
    'store': ['...ff...', '.eeeeee.', '.e....e.', '.egggge.', '.e....e.', '.egggge.', '.e....e.', '.eeeeee.'],
    'output': ['....ee..', '...ee...', '..ee....', '.eeeeee.', '....ee..', '...ee...', '..ee....', '........'],
    'scout': ['.eeee...', 'eg...e..', 'e....e..', 'e....e..', '.eeee...', '....ff..', '.....ff.', '......ff'],
    'area': ['ee....ee', 'e......e', '..gggg..', '..g..g..', '..g..g..', '..gggg..', 'e......e', 'ee....ee'],
    'haste': ['....gee.', '...eee..', '..eee...', '.eeeeee.', '...eee..', '..eee...', '.ee.....', 'e.......'],
}
EFFECT_BOARD, SUPPORT_BOARD, UPGRADE_BOARD, MACHINE_BOARD = '#4a2a30', '#24453a', '#6a4a2a', '#3b4a5a'
for _name, _sign, _accent, _board in [
    ('module_speed', 'speed', '#ff6a3c', MACHINE_BOARD), ('module_economy', 'eco', '#8fe36a', MACHINE_BOARD),
    ('module_storage', 'storage', '#c9a45a', MACHINE_BOARD), ('module_radius', 'radius', '#ffe14a', MACHINE_BOARD),
    ('module_bleed', 'bleed', '#e0303a', EFFECT_BOARD), ('module_poison', 'poison', '#7cd04a', EFFECT_BOARD),
    ('module_slow', 'slow', '#8fb8ff', EFFECT_BOARD), ('module_heal', 'heal', '#ff6a8a', SUPPORT_BOARD),
    ('module_guard', 'guard', '#c9d2e0', SUPPORT_BOARD), ('module_power', 'power', '#ff8a3c', SUPPORT_BOARD),
    ('module_haste', 'haste', '#ffe14a', SUPPORT_BOARD),
    # The guide draws the upgrades as one card (`upgrades`, sign `up`); the speed upgrade takes the
    # speed sign on the same board.
    ('upgrade_battery', 'up', '#ffd24a', UPGRADE_BOARD), ('upgrade_speed', 'speed', '#ffd24a', UPGRADE_BOARD),
    # Slice 8 (icons.js module('mod_store_cap', 'store', …) and the miner's).
    ('module_store_cap', 'store', '#ffe14a', MACHINE_BOARD), ('module_output', 'output', '#ffb23c', MACHINE_BOARD),
    ('module_scout', 'scout', '#e6e8ee', MACHINE_BOARD), ('module_area', 'area', '#7af0ff', MACHINE_BOARD),
]:
    ICONS[_name] = (MODULE, _board, _accent, [(4, 3, SIGN[_sign])])
REAPER = [
    '', '',
    '...kkkkkkkkkk...',
    '..kcacacacacak..',
    '..kabababababk..',
    '..kcacacacacak..',
    '...kkkkkkkkkk...',
    '....kn....nk....',
    '.kkkknkkkknkkkk.',
    '.kmmmmmmmmmmmmk.',
    '.knnnnnnnnnnnnk.',
    '.kkkkkkkkkkkkkk.',
    '..kok.kok.kok...',
    '...k...k...k....',
]
ICONS['tool_reaper'] = (REAPER, '#8fd35a', '#8fd35a', [])


def saw_rows():
    """icons.js `saw`: twelve teeth round a disc with a hub."""
    rows = []
    for y in range(16):
        row = ''
        for x in range(16):
            dx, dy = x - 7.5, y - 7.5
            d = math.hypot(dx, dy)
            tooth = math.floor(((math.atan2(dy, dx) + math.pi) / (2 * math.pi)) * 12) % 2 == 0
            if d < 1.3:
                row += 'x'
            elif d < 2.6:
                row += 'y'
            elif d < 5.4:
                row += 'o' if x + y < 14 else 'm'
            elif d < 6.2:
                row += 'n'
            elif d < 7.6 and tooth:
                row += 'm' if d < 6.9 else 'k'
            else:
                row += '.'
        rows.append(row)
    return rows


ICONS['tool_saw'] = (saw_rows(), '#d8dce4', '#e2bd55', [])


def shade(color, f):
    """RC.shade of engine.js, with JavaScript's Math.round (half up)."""
    rgb = [int(color[i:i + 2], 16) for i in (1, 3, 5)]
    out = [c + (255 - c) * f if f >= 0 else c * (1 + f) for c in rgb]
    return '#' + ''.join('%02x' % max(0, min(255, math.floor(c + 0.5))) for c in out)


def palette(a, e):
    pal = dict(FIXED)
    pal.update(k=shade(a, -0.72), d=shade(a, -0.5), b=shade(a, -0.28), a=a, c=shade(a, 0.3), l=shade(a, 0.6),
               e=e, f=shade(e, -0.35), g=shade(e, 0.45))
    for t in range(1, 5):
        pal['A%d' % t] = TIER[t]
        pal['L%d' % t] = shade(TIER[t], 0.35)
    return pal


def rgba(color):
    return tuple(int(color[i:i + 2], 16) for i in (1, 3, 5)) + (255,)


def icon(name):
    """The icon as 16 rows of 16 (r, g, b, a); '.' is see-through, as on the guide's canvas."""
    rows, a, e, over = ICONS[name]
    pal = palette(a, e)
    pixels = [[(0, 0, 0, 0)] * 16 for _ in range(16)]

    def paint(x0, y0, art):
        for y, row in enumerate(art):
            for x, ch in enumerate(row):
                px, py = x0 + x, y0 + y
                if ch in '. ' or not (0 <= px < 16 and 0 <= py < 16):
                    continue
                pixels[py][px] = rgba(pal.get(ch, '#ff00ff'))

    paint(0, 0, rows)
    for x, y, art in over:
        paint(x, y, art)
    return pixels
