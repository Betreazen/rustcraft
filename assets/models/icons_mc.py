"""The guide's item icons for the items the content mod has, as 16 x 16 RGBA pixel rows.

A port of the drawing in docs/showcase/js/icons.js (the shapes `channel`, `wrench`, `dust`, `ingot`
and `module`, the module sign `cap`, the palette and the icons wrench, dust, dust_cu, dust_au, the
alloys a1 to a4 and mod_cap). export_mc.py writes them as the item textures; keep them in step with
icons.js when an icon there changes. The old channel items keep the icons the guide drew for its
channel pages until 0.21 (spec 45): the items stay registered, hidden, for stacks in plain chests.
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
    'pick': ['nmmmmmmn', 'mmeemmmm', 'mmegmmem', 'mmmmmmee', 'memmmmmm', 'meemmgmm', 'mmmmmeem', 'nmmmmmmn'],
    'tunnel': ['eeeeeeee', 'effffffe', 'efxxxxfe', 'efxxxxfe', 'efxxxxfe', 'efxxxxfe', 'effffffe', 'eeeeeeee'],
    'height': ['gggggggg', '...ee...', '..eeee..', '.eeeeee.', '...ee...', '...ee...', '...ee...', 'ffffffff'],
    'lava': ['...ee..x', '..eeeex.', '.eegexe.', '.egexee.', '.eexeee.', '..xeee..', '.x......', 'x.......'],
    'build': ['eeefeeef', 'eeefeeef', 'ffffffff', 'efeeefee', 'efeeefee', 'ffffffff', 'eeefeeef', 'eeefeeef'],
    'rare': ['..eeee..', '.gwgeef.', 'eggeeeef', '.eeeeef.', '..eeef..', '...ef...', '........', '........'],
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
    # Slice 13 (icons.js module('mod_rare', 'rare', …)).
    ('module_rare', 'rare', '#5fe6dd', MACHINE_BOARD),
    ('module_pick', 'pick', '#f5d54a', MACHINE_BOARD), ('module_stratum', 'rare', '#d9c38a', MACHINE_BOARD),
    # Slice 14 (icons.js module('mod_tunnel', …) and the bore's others).
    ('module_tunnel', 'tunnel', '#9aa3b2', MACHINE_BOARD), ('module_height', 'height', '#7af0ff', MACHINE_BOARD),
    ('module_lava', 'lava', '#ff7a2a', MACHINE_BOARD), ('module_build', 'build', '#d08060', MACHINE_BOARD),
]:
    ICONS[_name] = (MODULE, _board, _accent, [(4, 3, SIGN[_sign])])
# Slice 18 (icons.js module('mod_stack' / 'mod_yield' / 'mod_line', …)): the stack module of each
# alloy takes that alloy's tier colour.
SIGN['stack'] = ['..gggg..', '..eeee..', '.gggggg.', '.eeeeee.', 'gggggggg', 'eeeeeeee', 'ffffffff', '........']
SIGN['yield'] = ['...ee...', '...ee...', '.eeeeee.', '.eeeeee.', '...ee...', '...ee...', '......g.', '.....g.g']
SIGN['line'] = ['....e...', '....ee..', 'e.e.eee.', 'e.e.eeee', 'e.e.eee.', '....ee..', '....e...', '........']
for _name, _sign, _accent in [
    ('module_stack', 'stack', TIER[2]), ('module_stack_2', 'stack', TIER[3]), ('module_stack_3', 'stack', TIER[4]),
    ('module_yield', 'yield', '#7af0ff'), ('module_line', 'line', '#c58aff'),
]:
    ICONS[_name] = (MODULE, MACHINE_BOARD, _accent, [(4, 3, SIGN[_sign])])
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


# ---- Slice 10 (icons.js `crate`, `claw`, `scoop`, `backpack`, the channels): the tier-2 tools,
# the backpacks and the channels of tiers 2 and 3, whose tube carries a band of their alloy's colour.
CRATE = [
    '', '',
    '.kkkkkkkkkkkkkk.',
    '.kmhhhhhhhhhhmk.',
    '.khhhhhhhhhhhhk.',
    '.kiiiiiiiiiiiik.',
    '.khhhhhhhhhhhhk.',
    '.khhhhhhhhhhhhk.',
    '.kiiiiiiiiiiiik.',
    '.khhhhhhhhhhhhk.',
    '.khhhhhhhhhhhhk.',
    '.kiiiiiiiiiiiik.',
    '.khhhhhhhhhhhhk.',
    '.kmhhhhhhhhhhmk.',
    '.kkkkkkkkkkkkkk.',
]
CLAW = [
    '',
    '......kkkk......',
    '......knnk......',
    '......kmmk......',
    '....kkkmmkkk....',
    '...kaaaaaaaak...',
    '...kbbbbbbbbk...',
    '..kmk......kmk..',
    '.kmk........kmk.',
    '.kmk........kmk.',
    '.kmk........kmk.',
    '..kmk......kmk..',
    '...kak....kak...',
    '....k......k....',
]
BACKPACK = [
    '',
    '.....kkkkkk.....',
    '....kk....kk....',
    '...kkkkkkkkkk...',
    '..kcccccccccak..',
    '..kaaaaaaaaaak..',
    '..kbbbbbbbbbbk..',
    '..kaaaayyaaaak..',
    '..kaaaazzaaaak..',
    '..kaaaaaaaaabk..',
    '.kkaaaaaaaaabkk.',
    '.kakaaaaaaaabkak',
    '.kbkbbbbbbbbbkbk',
    '.kkkkkkkkkkkkkkk',
]


def scoop_rows():
    """icons.js `scoop`, outlined as `outlined` does."""
    px = {}

    def dot(x, y, ch):
        px[(x, y)] = ch
    for i in range(5):
        dot(1 + i, 1 + i, 'm')
        dot(2 + i, 1 + i, 'o')
        dot(1 + i, 2 + i, 'n')
    for x in range(7, 11):
        dot(x, 4, 'h')
    for x in range(5, 13):
        dot(x, 5, 'i' if x % 3 else 'h')
    for x in range(4, 14):
        dot(x, 6, 'c')
    for y in range(7, 11):
        for x in range(4, 14):
            dot(x, y, 'c' if x == 4 else 'b' if x == 13 else 'a')
    for x in range(5, 14):
        dot(x, 11, 'b' if x == 13 else 'a')
    for x in range(6, 14):
        dot(x, 12, 'b')
    for x in (7, 9, 11, 13):
        dot(x, 13, 'o')
    dot(5, 7, 'x')
    for x in range(6, 13):
        dot(x, 9, 'b')
    rows = []
    for y in range(16):
        row = ''
        for x in range(16):
            ch = px.get((x, y))
            if ch:
                row += ch
            elif any((x + dx, y + dy) in px for dx, dy in ((-1, 0), (1, 0), (0, -1), (0, 1))):
                row += 'k'
            else:
                row += '.'
        rows.append(row)
    return rows


ICONS['tool_hold'] = (CRATE, '#a8793f', '#a8793f', [])
ICONS['tool_grab'] = (CLAW, '#ffa53c', '#ffa53c', [])
ICONS['tool_scoop'] = (scoop_rows(), '#e08a3a', '#e08a3a', [])
ICONS['backpack'] = (BACKPACK, '#a8793f', TIER[2], [])
# The crystallite backpack: the same bag with a crystallite clasp.
ICONS['backpack_2'] = (BACKPACK, '#7a5a3a', TIER[3], [(7, 7, ['uu', 'jj'])])
for _base in ('item_channel', 'fluid_channel', 'energy_channel'):
    _rows, _a, _e, _over = ICONS[_base]
    ICONS[_base + '_2'] = (_rows, _a, _e, _over + [(3, 10, ['yyyyyyyyyy'])])
    ICONS[_base + '_3'] = (_rows, _a, _e, _over + [(3, 10, ['uuuuuuuuuu'])])


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


# ---------------------------------------------------------------- slice 13: the gear
# icons.js S.helmet, S.chestplate, S.leggings, S.boots (technical armour with a charge strip),
# S.jetpack, S.eshield, S.book with EMBLEM.star, and the drawings `sword` and `rifle` (outlined).
HELMET = ['', '', '', '...kkkkkkkkkk...', '..kcccccccccak..', '.kcaaaaaaaaaabk.', '.kaaaaaaaaaaabk.',
          '.kaakkkkkkkkabk.', '.kaak......kabk.', '.kbbk......kbbk.', '.kkk........kkk.']
CHESTPLATE = ['', '.kkkk......kkkk.', 'kcaak......kaabk', 'kaaakkkkkkkkaabk', 'kaaacaaaaaaaaabk',
              'kkkaaaaaaaaaakkk', '..kaaaaaaaaaak..', '..kaaaaaaaaaak..', '..kaaaaaaaaabk..', '..kaaaaaaaaabk..',
              '..kaaaaaaaaabk..', '..kbbbbbbbbbbk..', '..kkkkkkkkkkkk..']
LEGGINGS = ['', '', '..kkkkkkkkkkkk..', '..kcaaaaaaaaak..', '..kaaaaaaaaaak..', '..kaaaaaaaaaak..',
            '..kaaaakkaaaak..', '..kaaak..kaaak..', '..kaabk..kaabk..', '..kaabk..kaabk..', '..kaabk..kaabk..',
            '..kaabk..kaabk..', '..kaabk..kaabk..', '..kkkkk..kkkkk..']
BOOTS = ['', '', '', '', '', '', '..kkkk....kkkk..', '..kcak....kcak..', '..kaak....kaak..', '..kaak....kaak..',
         '.kaaak...kaaak..', 'kaaaak..kaaaak..', 'kbbbbk..kbbbbk..', 'kkkkkk..kkkkkk..']
JETPACK = ['', '....kk....kk....', '...kmmk..kmmk...', '..kaaaakkaaaak..', '..kcaaakkcaaak..', '..kcaaakkcaaak..',
           '..kaaaakkaaaak..', '...koyyyyyyok...', '..kkkkkkkkkkkk..', '..khhhhhhhhhhk..', '..kkkkkkkkkkkk..']
ESHIELD = ['', '.kkkkkkkkkkkkkk.', '.kcccccccccccbk.', '.kcggggggggggbk.', '.kcgeeeeeeeefbk.', '.kcgeeeweeeefbk.',
           '.kcgeewwweeefbk.', '.kcgeeeweeeefbk.', '.kcgeeeeeeeefbk.', '.kageeeeeeeefbk.', '..kageeeeeefbk..',
           '...kageeeefbk...', '....kabeefbk....', '.....kabbbk.....']
BOOK = ['', '..kkkkkkkkkkkk..'] + ['..kbaaaaaaaawk..'] * 11 + ['..kbbbbbbbbbwk..', '..kkkkkkkkkkkk..']
STAR = ['...g...', '...g...', '.ggegg.', '...g...', '...g...', '.......', '.......']
LEAF = ['..ee...', '.eeee..', 'eeeeee.', '.eeee..', '..ee...', '.......', '.......']


def outlined(paint):
    """icons.js `outlined`: the drawing, with k wherever a filled pixel meets an empty one."""
    px = {}
    paint(lambda x, y, ch: px.__setitem__((x, y), ch))
    rows = []
    for y in range(16):
        row = ''
        for x in range(16):
            ch = px.get((x, y))
            if ch:
                row += ch
            elif any((x + dx, y + dy) in px for dx, dy in ((-1, 0), (1, 0), (0, -1), (0, 1))):
                row += 'k'
            else:
                row += '.'
        rows.append(row)
    return rows


def _box(dot, x0, y0, w, h, ch):
    for y in range(y0, y0 + h):
        for x in range(x0, x0 + w):
            dot(x, y, ch)


def _sword(dot):
    for x in range(6, 15):
        dot(x, 15 - x, 'w')
    for x in range(6, 14):
        dot(x, 14 - x, 'g')
    for x in range(7, 15):
        dot(x, 16 - x, 'f')
    for x in range(2, 8):
        dot(x, x + 5, 'a')
    for x in range(3, 8):
        dot(x, x + 4, 'c')
    dot(5, 10, 't')
    for x in range(2, 5):
        dot(x, 15 - x, 'x')
    dot(3, 13, 'n')
    dot(1, 14, 'c')
    dot(2, 14, 'b')
    dot(1, 15, 'b')


def _rifle(dot):
    _box(dot, 1, 7, 3, 3, 'a')
    _box(dot, 1, 7, 3, 1, 'c')
    _box(dot, 1, 9, 3, 1, 'b')
    _box(dot, 4, 6, 7, 3, 'n')
    _box(dot, 4, 6, 7, 1, 'm')
    _box(dot, 5, 7, 5, 1, 'e')
    dot(9, 7, 'g')
    _box(dot, 11, 7, 4, 1, 'o')
    _box(dot, 11, 8, 4, 1, 'm')
    dot(12, 7, 'c')
    dot(12, 8, 'a')
    dot(15, 7, 'g')
    dot(15, 8, 'e')
    _box(dot, 6, 4, 4, 1, 'x')
    dot(10, 4, 'u')
    dot(7, 5, 'x')
    _box(dot, 6, 9, 2, 3, 'b')
    dot(8, 9, 'x')


ARMOR_STRIP = {'helmet': [(6, 5, ['tttt'])], 'chestplate': [(6, 6, ['tttt']), (7, 7, ['tt'])],
               'leggings': [(5, 4, ['tttttt'])], 'boots': [(4, 8, ['t']), (12, 8, ['t'])]}
for _t in (2, 3, 4):
    for _piece, _rows in (('helmet', HELMET), ('chestplate', CHESTPLATE), ('leggings', LEGGINGS), ('boots', BOOTS)):
        ICONS['tech_%s_%d' % (_piece, _t)] = (_rows, TIER[_t], '#ffe14a', ARMOR_STRIP[_piece])
    ICONS['blade_%d' % _t] = (outlined(_sword), TIER[_t], '#62e6f2', [])
    ICONS['eshield_%d' % _t] = (ESHIELD, TIER[_t], '#4fd2e6', [])
ICONS['rifle'] = (outlined(_rifle), TIER[4], '#7af0ff', [])
ICONS['jetpack'] = (JETPACK, TIER[4], '#ffe14a', [])
ICONS['zipline'] = (['', '', '....kkk..kkk....', 'nnnkmmmknkmmmknn', '....kkk..kkk....', '.....k....k.....', '....kaaaaaak....',
                     '....kbbbbbbk....', '......kaak......', '......kaak......', '...kkkkaakkkk...', '...khhhhhhhhk...', '...kkkkkkkkkk...'],
                    TIER[2], TIER[2], [])
ICONS['book_capacity'] = (BOOK, '#7a5ad0', '#ffffff', [(4, 4, STAR)] + dots([(3, 2), (11, 12), (10, 3), (5, 12)], 'l'))
ICONS['book_thrift'] = (BOOK, '#3558d6', '#b8f6fa', [(4, 4, LEAF)] + dots([(3, 2), (11, 12), (10, 3), (5, 12)], 'l'))
# Slice 15: the storage terminal's recipe book (icons.js book('mod_recipes', '#3558d6', 'grid', '#e6e8ee')).
GRID = ['xxxxxxx', 'xexexex', 'xxxxxxx', 'xexexex', 'xxxxxxx', 'xexexex', 'xxxxxxx']
ICONS['module_recipes'] = (BOOK, '#3558d6', '#e6e8ee', [(4, 4, GRID)])
# The engineer's handbook (icons.js book('guide', T[1], 'gear'): the emblem `gear` in the default
# brass).
GEAR = ['e.ee.e.', '.eeee..', 'ee..ee.', 'ee..ee.', '.eeee..', 'e.ee.e.', '.......']
ICONS['handbook'] = (BOOK, TIER[1], '#e2bd55', [(4, 4, GEAR)])
# Player modules: the module plate of the third tier with a sign of its own (drawn at 4, 3).
PM_SIGNS = {
    'pm_dash': ['..e....', '..ee...', 'eeeee..', '..ee...', '..e....', '.......'],
    'pm_run': ['.e..e..', 'ee.ee..', 'eeeee..', 'ee.ee..', '.e..e..', '.......'],
    'pm_jump': ['...e...', '..eee..', '.e.e.e.', '...e...', '...e...', '.eeeee.'],
    'pm_absorber': ['eeeeeee', '.e.e.e.', '.e.e.e.', '.e.e.e.', 'eeeeeee', '.......'],
    'pm_capacity': ['.eeeee.', 'ee...ee', 'e.eee.e', 'e.eee.e', 'eeeeeee', '.......'],
    'pm_night': ['..eee..', '.e...e.', 'e.eee.e', '.e...e.', '..eee..', '.......'],
    'pm_magnet': ['ee...ee', 'ee...ee', 'ee...ee', '.e...e.', '..eee..', '.......'],
    'pm_damper': ['..eee..', '.eeeee.', '.eeeee.', '..eee..', '...e...', '.......'],
}
for _name, _sign in PM_SIGNS.items():
    ICONS[_name] = (MODULE, TIER[3], '#ffe14a', [(4, 3, _sign)])


# Slice 13: icons.js `mapDrum` (outlined) and `compass`.
def _map_drum(dot):
    dot(8, 1, 'e')
    for y in range(2, 5):
        dot(8, y, 'n')
    for x in range(6, 11):
        if x != 8:
            dot(x, 3, 'm')
    for x in range(4, 12):
        dot(x, 5, 'c')
    for x in range(3, 13):
        dot(x, 6, 'a' if x in (3, 12) else 'c')
    for y in range(7, 12):
        for x in range(3, 13):
            dot(x, y, 'a' if x == 3 else 'b' if x == 12 else 'o')
    for x, y in [(5, 8), (6, 8), (5, 9), (6, 9), (7, 9), (9, 7), (10, 7), (10, 8), (11, 8)]:
        dot(x, y, 'q')
    for x, y in [(4, 11), (5, 11), (9, 10), (10, 10), (10, 11), (11, 11)]:
        dot(x, y, 'u')
    for x, y in [(7, 7), (8, 8), (8, 10)]:
        dot(x, y, 'r')
    for x in range(4, 12):
        dot(x, 12, 'b')
    for x in range(6, 10):
        dot(x, 13, 'n')
    for x in range(5, 11):
        dot(x, 14, 'm')


def _compass_rows():
    px = {}
    put = lambda x, y, ch: px.__setitem__((x, y), ch)
    for x in range(6, 10):
        put(x, 1, 'k')
    put(6, 2, 'k'); put(7, 2, 'm'); put(8, 2, 'm'); put(9, 2, 'k')
    for y in range(16):
        for x in range(16):
            d = math.hypot(x - 7.5, y - 8.5)
            if d >= 6.5:
                continue
            put(x, y, 'k' if d >= 5.7 else ('c' if x + y < 15 else 'b') if d >= 4.6 else 'f' if d >= 3.9 else 'x')
    for x, y in [(7, 5), (8, 5), (7, 12), (8, 12), (4, 8), (4, 9), (11, 8), (11, 9)]:
        put(x, y, 'e')
    put(5, 6, 'n'); put(6, 6, 'n'); put(5, 7, 'n'); put(10, 6, 'r'); put(9, 7, 'r')
    put(8, 8, 'm'); put(7, 9, 'm'); put(6, 10, 'w'); put(5, 11, 'w')
    return [''.join(px.get((x, y), '.') for x in range(16)) for y in range(16)]


ICONS['tool_map'] = (outlined(_map_drum), '#5ab0e6', '#7af0ff', [])
# Slice 14: icons.js S.bore.
ICONS['tool_bore'] = (['', '....kkkkkkkk....', '....kaaaaaak....', '....kbbbbbbk....', '...kkkkkkkkkk...', '...koomnoomnk...', '....kmnoomnk....', '....knoomnok....', '.....koomnk.....', '.....komnok.....', '......kmnk......', '......knok......', '.......kk.......'], TIER[3], TIER[3], [])
ICONS['scout_compass'] = (_compass_rows(), '#8a919d', '#7af0ff', [])
# Slice 14: icons.js S.tablet and S.harpoon.
ICONS['tablet'] = (['', '..kkkkkkkkkkkk..', '.kaaaaaaaaaaaak.', '.kaxxxxxxxxxxak.', '.kaxegxegxegxak.', '.kaxffxffxffxak.',
                    '.kaxxxxxxxxxxak.', '.kaxegxegxegxak.', '.kaxffxffxffxak.', '.kaxxxxxxxxxxak.', '.kaxeeeeggxxxak.',
                    '.kaxxxxxxxxxxak.', '.kaaaaacaaaaaak.', '.kbbbbbbbbbbbbk.', '..kkkkkkkkkkkk..'], TIER[4], '#5ad1e6', [])
ICONS['harpoon'] = (['..........kkkkk.', '...........kook.', '..........kmook.', '.........kmk.kk.', '........kak.....',
                     '.......kak......', '......kak.......', '.....kak........', '....kak.........', '..kkak..........',
                     '.khhk...........', 'khhhhk..........', 'khhhhk..........', '.kkkk...........'], TIER[3], TIER[3], [])


# ---- Slice 18 (energy): the crystal shard, icons.js `shard` (S.shard, a '#56dbe2', e white).
SHARD = [
    '',
    '..........kk....',
    '.........kvuk...',
    '........kvuujk..',
    '.......kvuuujk..',
    '......kvuuujk...',
    '.....kvuuujk....',
    '....kvuuujk.....',
    '...kvuuujk......',
    '..kuuuujk.......',
    '..kjuujk........',
    '...kjjk.........',
    '....kk..........',
]
ICONS['crystal_shard'] = (SHARD, '#56dbe2', '#ffffff', dots([(3, 3), (13, 9), (6, 11)], 'w'))


# ---- 0.21.2: the buckets the storage terminal draws for the mod's fluids (rustcraft_rules::
# fluid_icons). No guide icon: the guide shows these fluids as flasks (icons.js f_sol, f_fast,
# f_eco, f_rich), the game's windows as buckets beside its water, lava and milk buckets. A steel
# pail full to the rim, the liquid in the colour its tanks show (rules anim::fluid_tint; a rules
# test checks the colours here against it) and the guide flask's sign on the pail.
BUCKET = [
    '',
    '.....nnnnnn.....',
    '....n......n....',
    '...n........n...',
    '.kkkkkkkkkkkkkk.',
    '.kgggggeeeeeefk.',
    '.keeeeeeeeefffk.',
    '.kccccccccccbbk.',
    '..kcaaaaaaaabk..',
    '..kcaaaaaaaabk..',
    '..kcaaaaaaaabk..',
    '...kcaaaaaabk...',
    '...kcaaaaaabk...',
    '...kcaaaaaabk...',
    '....kbbbbbbk....',
    '.....kkkkkk.....',
]
PAIL = '#8a919d'
ICONS['solution_bucket'] = (BUCKET, PAIL, '#6cc24a', dots([(5, 9), (8, 10), (10, 9), (6, 12), (9, 12)], 'e'))
ICONS['accelerator_bucket'] = (BUCKET, PAIL, '#f5d547', [(5, 9, ['e..e.', '.e..e', 'e..e.'])])
ICONS['economizer_bucket'] = (BUCKET, PAIL, '#e39a2e', [(6, 9, ['..ee', '.ee.', 'ee..', 'e...'])])
ICONS['enricher_bucket'] = (BUCKET, PAIL, '#b65fd8', [(6, 9, ['.e.', 'eee', '.e.'])] + dots([(9, 12)], 'e'))
