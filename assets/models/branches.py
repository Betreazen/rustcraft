"""Blocks for the Path-of-craft-tree branches (spec 22): repair stand, kitchen machine, fishing station,
feeder, shearer-milker, apiary collector, brewing cube, experience condenser, enchanting machine,
archaeology table, holo projector, building machine; and the butcher (spec 23).

Every machine wears the tier stripe (part tier) somewhere on its top, as the other machines do; the
guide shows it above the first tier. Moving parts are named in the pose of each block in
docs/showcase/js/poses.js; fills are parts level_*, stretched upright from their bottom."""
from kit import family, frame, panel, shade, shell, tier

FEED, MILK, HONEY, BREW, XP, HOLO = '#d9b44a', '#f4f1ea', '#ffb32a', '#c05ad8', '#7cfc5a', '#5ae6ff'
COLORS = {
    # Repair stand: a grinding stone, sparks and a lamp strip.
    'grind': '#a89a86', 'grind_dark': shade('#a89a86', -0.3), 'glow_spark': '#ffd27a', 'glow_repair': '#ff9a3c',
    # Kitchen machine: the hob, the oven window, the pot and its steam.
    'glow_hob': '#ff5a2a', 'glow_oven': '#ffb24a', 'oven_dark': shade('#ffb24a', -0.75),
    **family('pot', '#b9c0cc'), 'steam': '#e8eef4', 'steam_light': '#ffffff',
    # Fishing station: orange floats, rope, a bobber, a lantern and a crate of fish.
    'float': '#e8762a', 'float_light': '#f4f0e6', 'float_dark': shade('#e8762a', -0.35),
    'line': '#e6e8ee', 'rope': '#cfc8b8', 'bobber': '#e0564a', 'glow_lantern': '#ffd27a',
    'crate': '#a8793f', 'crate_dark': shade('#a8793f', -0.3), 'fish': '#9fb8c8',
    # Feeder: the grain and a green lamp.
    'feed': FEED, 'feed_light': shade(FEED, 0.3), 'glow_feeder': '#8fd35a',
    # Shearer-milker: the milk, the shears, wool in the bin and an amber lamp.
    'milk': MILK, 'milk_light': '#ffffff', 'shears': '#d8dce4', 'shears_grip': '#e0564a',
    'wool': '#e8e4da', 'wool_dark': shade('#e8e4da', -0.2), 'glow_shearer': '#ffd24a',
    # Apiary collector: the hive, the honey, the bees.
    **family('hive', '#c9a45a'), 'glow_honey': HONEY, 'glow_honey_top': shade(HONEY, 0.35),
    'bee': '#f5d54a', 'bee_dark': '#2b2618', 'wing': '#e6f2f8',
    # Brewing cube: the brew, three potions, corks, glass and the blaze heater.
    'glow_brew': BREW, 'glow_brew_top': shade(BREW, 0.4),
    'glow_potion_r': '#e0564a', 'glow_potion_b': '#4c9bf5', 'glow_potion_g': '#59d38c',
    'cork': '#a8793f', 'glass': '#dcedf5', 'glow_blaze': '#ffb23c',
    # Experience condenser: a dark green frame round the experience fluid.
    **family('xp_case', '#4a5a4a'), 'glow_xp': XP, 'glow_xp_top': shade(XP, 0.45), 'glow_xp_orb': '#d8ff6a',
    # Enchanting machine: obsidian, lapis, the book and the glyphs.
    **family('ench', '#3b3550'), 'lapis': '#3558d6', 'book_cover': '#7a2a3a', 'paper': '#efe6d0',
    'glow_glyph': '#9a7aff', 'glow_ench': '#c58aff',
    # Archaeology table: sand, a relic, the brush and a scanning lamp.
    'sand': '#dccf9a', 'sand_dark': shade('#dccf9a', -0.15), 'relic': '#b0603a', 'relic_light': shade('#b0603a', 0.3),
    'wood': '#8a6a3a', 'bristle': '#e6d6a8', 'glow_scan': '#ffcf5a',
    # Holo projector: the cyan light.
    'glow_holo': HOLO, 'glow_holo_light': shade(HOLO, 0.55),
    # Building machine: a yellow gantry, a glowing nozzle and the bricks it lays.
    **family('gantry', '#d8a83a'), 'glow_nozzle': '#ff8a3a', 'brick': '#9a5a44', 'brick_light': shade('#9a5a44', 0.25),
    # Butcher: a red lamp, the sword on its arm, raw meat in the loot hopper.
    'glow_butcher': '#e0564a', 'sword': '#dfe3ea', 'sword_edge': '#ffffff', 'sword_grip': '#7a5530',
    'meat': '#c0504a', 'meat_light': shade('#c0504a', 0.3),
}


def stripe(m, x, y, z, w, d):
    """The tier stripe where a machine has no full top: the part tier, a quarter pixel thick."""
    m.box('tint', x, y, z, w, d, 0.25, part='tier')


def base(m, h, body='case'):
    """A low machine body h pixels high: a plinth, a body between corner posts and a top slab."""
    m.box(body + '_dark', 0, 0, 0, 16, 16, 1.5)
    m.box(body, 0.5, 0.5, 1.5, 15, 15, h - 2.5)
    m.box(body, 0, 0, 1.5, 1.5, 1.5, h - 2.5, ring=True)
    m.box(body, 0, 0, h - 1, 16, 16, 1)
    m.box(body + '_light', 0.5, 0.5, h, 15, 15, 0.25)


def octagon(m, mat, x, y, z, w, d, h, axis, pivot, part):
    """A box and its copy turned 45 degrees about `axis` through `pivot`: a round-looking drum."""
    for a in (0, 45):
        m.box(mat, x, y, z, w, d, h, part=part, tilt=(axis, a), pivot=pivot)


def repair(m):
    # A low cabinet with a lamp strip on its faces.
    base(m, 7.5)
    stripe(m, 0, 0, 7.75, 16, 2)
    panel(m, 'black', 3, 3, 10, 2.5)
    panel(m, 'glow_repair', 4, 3.5, 8, 1.5, depth=0.4)
    # An anvil on the x+ half: foot, waist, face and a horn that sticks out forward.
    m.box('gun', 7.5, 5, 7.75, 7, 6, 1.25)
    m.box('steel_dark', 9, 6, 9, 4, 4, 1.5)
    m.box('steel_mid', 7.5, 4, 10.5, 8, 8, 2.5)
    m.box('steel', 7.5, 4, 13, 8, 8, 0.25)
    m.box('steel_mid', 15.5, 5.5, 11, 1.5, 5, 2)
    m.box('steel_mid', 17, 6.5, 11.5, 1, 3, 1.25)
    # A vice on the anvil holds a blade; its jaw clamp closes towards y- while the stand works.
    m.box('steel_dark', 9, 5.5, 13.25, 4, 1.5, 2.25)
    m.box('steel', 8.5, 7, 13.25, 6, 2, 0.5)
    m.box('white', 8.5, 7, 13.75, 6, 0.4, 0.1)
    m.box('copper', 14.5, 7.25, 13.25, 2.5, 1.5, 0.75)
    m.box('steel_dark', 9, 9.75, 13.25, 4, 1.5, 2.25, part='clamp')
    m.box('steel', 10.5, 11.25, 14, 1, 2.5, 0.75, part='clamp')
    m.box('steel_mid', 9.5, 13.5, 13.75, 3, 0.75, 1.75, part='clamp')
    # A grinder on the x- half: a motor and a stone wheel that turns about y; sparks fly off it.
    m.box('steel_mid', 0.75, 9.5, 7.75, 5, 4.5, 3.75)
    for z in (8.75, 10):
        m.box('black', 0.5, 10, z, 0.25, 3.5, 0.5)
    m.box('steel', 2.75, 8, 12, 1, 1.5, 1)
    m.box('steel_dark', 2.25, 9.5, 11.5, 2, 1.5, 2)
    m.pivot('wheel', 3.25, 7.25, 12.5)
    for a in (0, 45, 90, 135):
        m.box('grind', 0.25, 6.5, 11, 6, 1.5, 3, part='wheel', tilt=('Y', a), pivot=(3.25, 7.25, 12.5))
    m.box('grind_dark', 1.25, 6.4, 11.5, 4, 1.7, 2, part='wheel', tilt=('Y', 45), pivot=(3.25, 7.25, 12.5))
    m.box('steel_dark', 2.5, 6.25, 11.75, 1.5, 2, 1.5, part='wheel')
    for x, y, z in ((6.5, 7, 10.25), (7.25, 6.6, 9.5), (7.75, 7.5, 10.75), (6.9, 7.8, 9.1)):
        m.box('glow_spark', x, y, z, 0.5, 0.5, 0.5, part='sparks')


def kitchen(m):
    # An oven below with a glowing window on every face.
    base(m, 11)
    stripe(m, 0, 15, 11.25, 16, 1)
    frame(m, 'black', 3, 2.5, 10, 6.5)
    panel(m, 'oven_dark', 4, 3.5, 8, 4.5)
    panel(m, 'glow_oven', 4.5, 4, 7, 1.5, depth=0.4)
    panel(m, 'glow_oven', 4.5, 6.5, 7, 0.5, depth=0.4)
    panel(m, 'steel', 5, 9.4, 6, 0.6, depth=0.9)
    # The hob on the top: a dark plate and a glowing coil round the pot.
    m.box('steel_dark', 1, 1, 11.25, 14, 14, 0.5)
    m.box('glow_hob', 12.25, 2.75, 11.75, 1, 10.5, 0.25, ring=True)
    # The pot with handles; its lid rattles while the machine cooks.
    m.box('pot', 4, 4, 11.75, 8, 8, 4.25)
    m.box('pot_dark', 3.9, 3.9, 12.25, 8.2, 8.2, 0.5)
    m.box('pot_light', 3.5, 3.5, 15.5, 9, 9, 0.5)
    for x in (2.5, 12.5):
        m.box('pot_dark', x, 7, 14, 1, 2, 1)
    m.box('pot_dark', 4, 4, 16, 8, 8, 0.75, part='lid')
    m.box('steel', 7, 7, 16.75, 2, 2, 0.75, part='lid')
    # A steam vent in the back corner; the steam rises from it.
    m.box('steel_mid', 1, 1, 11.75, 2.5, 2.5, 5)
    m.box('steel_dark', 0.75, 0.75, 16.75, 3, 3, 0.75)
    m.box('steam', 1.25, 1.25, 17.75, 2, 2, 1.5, part='steam')
    m.box('steam_light', 1.75, 1.5, 19.5, 1.5, 1.5, 1, part='steam')
    m.box('steam', 0.75, 2, 20.75, 1, 1, 0.75, part='steam')


def fishery(m):
    # Two floats along x carry the frame; their bottoms sit in the water.
    for y in (0.5, 12.5):
        m.box('float', 1, y, -3, 14, 3, 4)
        m.box('float_light', 0.75, y - 0.25, -0.75, 14.5, 3.5, 0.75)
        for x in (0.25, 15):
            m.box('float_dark', x, y + 0.5, -2.5, 0.75, 2, 3)
    for x in (1.5, 13):
        m.box('case_dark', x, 0.5, 1, 1.5, 15, 1.25)
    # Four posts and a top frame; the winch sits on it, the line runs down through the middle.
    m.box('case', 1.5, 1.5, 2.25, 1.5, 1.5, 10.25, ring=True)
    m.box('case', 3, 1.5, 11, 10, 1.5, 1.5, ring=True)
    m.box('case_light', 1.5, 1.5, 12.5, 1.5, 1.5, 0.25, ring=True)
    stripe(m, 3, 1.5, 12.5, 10, 1.5)
    for y in (1.5, 13):
        m.box('steel_dark', 6.5, y, 12.5, 3, 1.5, 5)
    m.box('steel', 7.5, 3, 14.5, 1, 10, 1)
    # The winch drum turns about y as it lets the line down and winds it up.
    m.pivot('winch', 8, 8, 15)
    octagon(m, 'rope', 6, 4.75, 13, 4, 6.5, 4, 'Y', (8, 8, 15), 'winch')
    for y in (4, 11.25):
        octagon(m, 'steel_mid', 5.5, y, 12.5, 5, 0.75, 5, 'Y', (8, 8, 15), 'winch')
    # The line with its bobber and hook; it reaches up into the drum, so it never ends short.
    m.box('line', 7.75, 7.75, -10, 0.5, 0.5, 26, part='line')
    m.box('bobber', 7.25, 7.25, -3, 1.5, 1.5, 1.5, part='line')
    m.box('float_light', 7.25, 7.25, -1.5, 1.5, 1.5, 0.75, part='line')
    m.box('steel', 7.5, 7.5, -10.5, 1, 1, 0.5, part='line')
    m.box('steel', 8.5, 7.75, -10.5, 0.5, 0.5, 1.25, part='line')
    # A lantern on one post and a crate of the catch on a float.
    m.box('steel_dark', 13, 13, 12.75, 1.5, 1.5, 0.5)
    m.box('glow_lantern', 13.25, 13.25, 13.25, 1, 1, 1.75)
    m.box('steel_dark', 13, 13, 15, 1.5, 1.5, 0.5)
    m.box('crate', 4, 13, 1, 3.5, 2.5, 2.5)
    m.box('crate_dark', 3.9, 12.9, 2.5, 3.7, 2.7, 0.4)
    m.box('fish', 4.5, 13.5, 3.5, 2.5, 1, 0.5)


def feeder(m):
    # A square trough with a hopper in the middle; the grain in the trough is level_feed.
    m.box('case_dark', 0, 0, 0, 16, 16, 2)
    m.box('case', 0, 0, 2, 16, 1.5, 5.5, ring=True)
    m.box('case_light', 0, 0, 7.5, 16, 1.5, 0.25, ring=True)
    stripe(m, 1.5, 0, 7.75, 13, 1.5)
    m.box('black', 3.5, -0.2, 3, 9, 0.2, 2, ring=True)
    m.box('glow_feeder', 4, -0.4, 3.5, 8, 0.25, 1, ring=True)
    m.box('feed', 1.5, 1.5, 2, 13, 13, 4.5, part='level_feed')
    m.box('feed_light', 1.5, 1.5, 6.5, 13, 13, 0.5, part='level_feed')
    # The hopper: a column, then a funnel widening up to an open top with grain and a stirrer.
    m.box('steel_mid', 5.5, 5.5, 2, 5, 5, 6)
    m.box('steel_dark', 10.5, 7, 6, 1.5, 2, 1.5, ring=True)
    m.box('case', 4.5, 4.5, 8, 7, 7, 2)
    m.box('case', 3.5, 3.5, 10, 9, 9, 2)
    m.box('case', 2.5, 2.5, 12, 11, 1, 4, ring=True)
    m.box('case_light', 2.5, 2.5, 16, 11, 1, 0.25, ring=True)
    m.box('feed', 3.5, 3.5, 12, 9, 9, 3)
    m.box('feed_light', 4, 4, 15, 8, 8, 0.25)
    m.pivot('auger', 8, 8, 15)
    for a in (0, 90):
        m.box('steel', 4.5, 7.5, 15.25, 7, 1, 0.5, part='auger', tilt=('Z', a), pivot=(8, 8, 15.25))
    m.box('steel_dark', 7, 7, 15, 2, 2, 1.5, part='auger')


def shearer(m):
    shell(m)
    tier(m)
    # A bin of wool behind a window on every face and a lamp over it.
    frame(m, 'black', 3, 3, 10, 7)
    panel(m, 'wool_dark', 4, 4, 8, 5)
    for y, z in ((4.5, 4.5), (7.5, 5), (5.5, 6.5), (9, 4.25)):
        panel(m, 'wool', y, z, 2.5, 2, depth=0.4)
    panel(m, 'black', 5.5, 11.5, 5, 2)
    panel(m, 'glow_shearer', 6, 12, 4, 1, depth=0.4)
    # A small milk tank on the roof; the milk is level_milk.
    m.box('steel_dark', 1.5, 9.5, 16.25, 5, 5, 0.5)
    for x in (1.5, 5.75):
        for y in (9.5, 13.75):
            m.box('steel_mid', x, y, 16.75, 0.75, 0.75, 4.25)
    m.box('steel_mid', 1.5, 9.5, 21, 5, 5, 0.75)
    m.box('milk', 2, 10, 16.75, 4, 4, 3.75, part='level_milk')
    m.box('milk_light', 2, 10, 20, 4, 4, 0.5, part='level_milk')
    m.box('glass', 6.25, 11, 18.5, 0.25, 0.75, 1.75)
    # The arm on a turntable in the middle swings about z; the shears at its end cut about x.
    m.box('steel_dark', 6, 6, 16.25, 4, 4, 1)
    m.pivot('arm', 8, 8, 17.25)
    m.box('steel_mid', 7, 7, 17.25, 2, 2, 1.75, part='arm')
    m.box('steel', 8, 7.25, 17.75, 9, 1.5, 1, part='arm')
    m.box('steel_mid', 16, 7.25, 15, 1.5, 1.5, 3.75, part='arm')
    m.box('steel_dark', 15.75, 7, 14.25, 2, 2, 1, part='arm')
    for part, y in (('blade_a', 7.4), ('blade_b', 8.1)):
        m.pivot(part, 16.75, 8, 14.5)
        m.box('shears', 16.25, y, 10.5, 1, 0.5, 4, part=part)
        m.box('shears_grip', 16.25, y, 14.5, 1, 0.5, 1, part=part)


def apiary(m):
    # The hive in the middle: light wood with darker bands and an entrance on every face.
    m.box('case_dark', 0, 0, 0, 16, 16, 1.5)
    m.box('hive', 2, 2, 1.5, 12, 12, 8)
    for z in (3.5, 6.5):
        m.box('hive_dark', 1.75, 1.75, z, 12.5, 12.5, 0.75)
    m.box('black', 13.9, 6.5, 2.25, 0.35, 3, 0.9, ring=True)
    # A frame hugs it: corner posts, bands round the top and the middle, pads pressing on it.
    m.box('case', 0, 0, 1.5, 2.5, 2.5, 9.5, ring=True)
    m.box('case', 2.5, 0.5, 9.5, 11, 1.5, 1.5, ring=True)
    m.box('case', 14.5, 2.5, 5, 1, 11, 1.5, ring=True)
    m.box('steel_mid', 14, 6, 4.25, 0.5, 4, 3, ring=True)
    m.box('case', 0.5, 0.5, 11, 15, 15, 1)
    stripe(m, 0.5, 0.5, 12, 15, 1.5)
    # The honey tank on the top; the honey is level_honey.
    m.box('case_light', 3, 3, 12, 1, 1, 4, ring=True)
    m.box('black', 4, 4, 12, 8, 8, 0.25)
    m.box('glow_honey', 4, 4, 12.25, 8, 8, 3, part='level_honey')
    m.box('glow_honey_top', 4, 4, 15, 8, 8, 0.5, part='level_honey')
    m.box('case', 3, 3, 16, 10, 10, 0.75)
    m.box('steel_dark', 6.5, 6.5, 16.75, 3, 3, 0.75)
    m.box('glass', 12.1, 5, 13, 0.25, 1, 2.5, ring=True)
    # Three bees fly round it while it works.
    m.pivot('bees', 8, 8, 8)
    for a, z in ((20, 7), (140, 10.5), (260, 8.5)):
        turn = dict(part='bees', tilt=('Z', a), pivot=(8, 8, z))
        m.box('bee', 16.5, 7.25, z, 2.5, 1.5, 1.5, **turn)
        m.box('bee_dark', 17.25, 7.15, z - 0.1, 0.6, 1.7, 1.7, **turn)
        m.box('bee_dark', 18.6, 7.15, z - 0.1, 0.5, 1.7, 1.7, **turn)
        m.box('wing', 17, 6.25, z + 1.5, 1.25, 3.5, 0.4, **turn)


def brewer(m):
    shell(m, top=False)
    tier(m)
    # A rack of three bottles on every face over the glowing blaze heater.
    panel(m, 'glow_blaze', 3, 2, 10, 1, depth=0.35)
    panel(m, 'black', 2, 3.5, 12, 7)
    m.box('steel_mid', 15.5, 2, 3.5, 1.5, 12, 0.75, ring=True)
    for y in (2, 13.25):
        m.box('steel_dark', 15.5, y, 4.25, 1.5, 0.75, 5.25, ring=True)
    m.box('steel_mid', 15.5, 2, 9.5, 1.5, 12, 0.75, ring=True)
    for y, potion in ((3.5, 'glow_potion_r'), (7, 'glow_potion_b'), (10.5, 'glow_potion_g')):
        m.box(potion, 15.6, y, 4.25, 1.4, 2, 2.25, ring=True)
        m.box('glass', 15.6, y, 6.5, 1.4, 2, 0.75, ring=True)
        m.box('glass', 15.9, y + 0.5, 7.25, 0.8, 1, 1, ring=True)
        m.box('cork', 15.9, y + 0.5, 8.25, 0.8, 1, 0.6, ring=True)
    # The vat in the open top; the brew is level_brew and bubbles while it brews.
    m.box('glow_brew', 1.75, 1.75, 11.25, 12.5, 12.5, 2.5, part='level_brew')
    m.box('glow_brew_top', 1.75, 1.75, 13.75, 12.5, 12.5, 0.5, part='level_brew')
    for x, y, s in ((4, 5, 1.25), (9, 10, 1.5), (11, 4, 1), (6, 11, 1), (8, 7, 0.75)):
        m.box('glow_brew_top', x, y, 14, s, s, 0.75, part='bubbles')


def xp_tank(m):
    # A tank in a dark green frame; the experience fluid is level_xp.
    m.box('xp_case_dark', 0, 0, 0, 16, 16, 1.5)
    m.box('black', 1.5, 1.5, 1.5, 13, 13, 0.25)
    m.box('xp_case', 0, 0, 1.5, 2, 2, 12, ring=True)
    m.box('xp_case', 0, 0, 13.5, 16, 16, 1.5)
    m.box('xp_case_light', 0.5, 0.5, 15, 15, 15, 0.25)
    m.box('xp_case_dark', 2, 0.5, 7, 12, 0.75, 1, ring=True)
    m.box('glass', 15.25, 3.5, 9, 0.25, 1, 3.5, ring=True)
    m.box('glass', 15.25, 5, 10.5, 0.25, 0.75, 2, ring=True)
    m.box('glow_xp', 2, 2, 1.75, 12, 12, 11.25, part='level_xp')
    m.box('glow_xp_top', 2, 2, 12.5, 12, 12, 0.5, part='level_xp')
    stripe(m, 0, 0, 15.25, 16, 2)
    # The condenser on the top: a glowing coil, a funnel and an orb of experience over it.
    m.box('steel_dark', 4, 4, 15.25, 8, 8, 0.75)
    m.box('glow_xp', 11, 4.5, 16, 0.75, 7, 0.75, ring=True)
    m.box('steel_mid', 5.5, 5.5, 16, 5, 5, 1.25)
    m.box('steel_dark', 6.5, 6.5, 17.25, 3, 3, 0.5)
    m.pivot('orb', 8, 8, 19.5)
    m.box('glow_xp_orb', 6.75, 6.75, 18.25, 2.5, 2.5, 2.5, part='orb', tilt=('Z', 45), pivot=(8, 8, 18.25))
    m.box('glow_xp_top', 7.25, 7.25, 20.75, 1.5, 1.5, 0.5, part='orb', tilt=('Z', 45), pivot=(8, 8, 20.75))


def enchanter(m):
    # An obsidian pedestal with lapis inlay and four glowing candles on its corners.
    m.box('ench_dark', 0, 0, 0, 16, 16, 2)
    m.box('ench', 1, 1, 2, 14, 14, 4)
    m.box('lapis', 15, 3, 3, 0.25, 10, 1.5, ring=True)
    m.box('glow_ench', 15, 7, 4.75, 0.3, 2, 0.75, ring=True)
    m.box('ench_light', 2.5, 2.5, 6, 11, 11, 1)
    stripe(m, 3, 1, 6, 10, 1.5)
    m.box('ench_dark', 1, 1, 6, 2, 2, 3, ring=True)
    m.box('glow_ench', 1.5, 1.5, 9, 1, 1, 1, ring=True)
    m.box('ench', 5, 5, 7, 6, 6, 3)
    m.box('glow_ench', 11, 6.5, 8, 0.25, 3, 1, ring=True)
    m.box('ench_dark', 3.5, 4, 10, 9, 8, 1)
    m.box('lapis', 4, 4.5, 11, 8, 7, 0.25)
    # The book floats over the pedestal: a spine (book) and two halves hinged on it about x.
    hinge = (8, 8, 12.5)
    m.pivot('book', *hinge)
    m.box('book_cover', 4.5, 7.5, 12, 7, 1, 1, part='book')
    for part, y0, sign in (('page_r', 8, 1), ('page_l', 3.5, -1)):
        m.pivot(part, *hinge)
        m.box('book_cover', 4.5, y0, 12, 7, 4.5, 0.5, part=part)
        m.box('paper', 5, y0 + (0 if sign > 0 else 0.5), 12.5, 6, 4, 0.5, part=part)
        for x in (6, 7.25, 8.5, 9.75):
            m.box('steel_mid', x, y0 + (0.75 if sign > 0 else 1.25), 13, 0.35, 2.5, 0.05, part=part)
    # A ring of glyphs floats round the book while the machine works.
    m.pivot('ring', 8, 8, 14)
    for k in range(8):
        tall = k % 2 == 0
        m.box('glow_glyph', 14.25, 7.25 if tall else 7.5, 13.5 if tall else 13.75, 0.6, 1.5 if tall else 1, 2 if tall else 1.5,
              part='ring', tilt=('Z', 45 * k + 22.5), pivot=(8, 8, 14))


def archeo(m):
    # A table on four legs with a shelf of finds.
    m.box('case_dark', 0.5, 0.5, 0, 2, 2, 9, ring=True)
    m.box('case_dark', 1, 1, 3, 14, 14, 0.75)
    m.box('relic', 4, 9, 3.75, 3, 3, 2)
    m.box('relic_light', 4.5, 9.5, 5.75, 2, 2, 0.5)
    m.box('relic', 9.5, 4.5, 3.75, 2, 2.5, 1.25)
    m.box('case', 0, 0, 9, 16, 16, 1.5)
    m.box('case_light', 0.5, 0.5, 10.5, 15, 15, 0.25)
    m.box('black', 16, 6, 9.25, 0.25, 4, 1, ring=True)
    m.box('glow_scan', 16, 8.75, 9.5, 0.4, 1, 0.5, ring=True)
    stripe(m, 2, 0.5, 10.75, 12, 1)
    # A tray of sand with a relic in it.
    m.box('steel_dark', 2, 2, 10.75, 12, 1, 1.75, ring=True)
    m.box('sand', 3, 3, 10.75, 10, 10, 1.25)
    for y in (4.5, 8, 11):
        m.box('sand_dark', 3, y, 12, 10, 0.75, 0.1)
    m.box('relic', 6, 6.5, 11.5, 3.5, 2.5, 1, tilt=('Z', 20))
    m.box('relic_light', 6.5, 7, 12.5, 2, 1, 0.25, tilt=('Z', 20))
    # A gantry over the tray; the brush hangs from its carriage and sweeps along x.
    m.box('steel_mid', 0.5, 0.5, 10.75, 1.25, 1.25, 4.5, ring=True)
    for y in (0.5, 14.25):
        m.box('steel_mid', 0.5, y, 15.25, 15, 1.25, 0.75)
    m.pivot('brush', 8, 8, 16)
    m.box('steel', 7, 0.25, 16, 2, 15.5, 0.75, part='brush')
    m.box('steel_dark', 7, 0, 15, 2, 1, 1.75, part='brush')
    m.box('steel_dark', 7, 15, 15, 2, 1, 1.75, part='brush')
    m.box('wood', 7.5, 7.5, 13, 1, 1, 3, part='brush')
    m.box('wood', 6.5, 6, 12.75, 3, 4, 0.75, part='brush')
    m.box('bristle', 6.75, 6.25, 12, 2.5, 3.5, 0.75, part='brush')
    m.box('glow_scan', 9.25, 7.25, 15.25, 0.75, 1.5, 0.75, part='brush')


def projector(m):
    # A low body with three light slits on every face and the emitter on the top.
    base(m, 7)
    stripe(m, 0, 0, 7.25, 16, 2)
    panel(m, 'black', 3, 2.5, 10, 3)
    for y in (4, 7.25, 10.5):
        panel(m, 'glow_holo', y, 3, 1.5, 2, depth=0.4)
    m.box('steel_dark', 4.5, 4.5, 7.25, 7, 7, 1)
    m.box('steel_mid', 5.5, 5.5, 8.25, 5, 5, 1.25)
    m.box('glow_holo', 6.5, 6.5, 9.5, 3, 3, 0.75)
    m.box('steel_mid', 11, 7.25, 7.25, 1.5, 1.5, 3, ring=True)
    m.box('glow_holo_light', 11.25, 7.5, 10.25, 1, 1, 0.5, ring=True)
    # The cone of light: four rays from the lens out to a square outline, and a ring between.
    m.box('glow_holo', 9.25, 7.75, 10.25, 0.5, 0.5, 12.8, ring=True, part='cone', tilt=('Y', 23), pivot=(9.5, 8, 10.25))
    m.box('glow_holo', 11.7, 4.1, 16, 0.4, 7.8, 0.4, ring=True, part='cone')
    m.box('glow_holo', 14.25, 1.5, 22, 0.5, 13, 0.5, ring=True, part='cone')
    # The hologram of a block, an outline that turns slowly in the cone.
    m.pivot('holo', 8, 8, 18)
    m.box('glow_holo_light', 5, 5, 15, 0.4, 0.4, 6, ring=True, part='holo')
    m.box('glow_holo_light', 5, 5, 15, 6, 0.4, 0.4, ring=True, part='holo')
    m.box('glow_holo_light', 5, 5, 20.6, 6, 0.4, 0.4, ring=True, part='holo')


def builder(m):
    # A build bed with a few bricks laid, in a yellow gantry.
    m.box('gantry_dark', 0, 0, 0, 16, 16, 1.5)
    m.box('steel_dark', 1, 1, 1.5, 14, 14, 0.25)
    for k in (4.5, 8, 11.5):
        m.box('plate', k, 1, 1.75, 0.25, 14, 0.05)
        m.box('plate', 1, k, 1.75, 14, 0.25, 0.05)
    for x, y, h in ((3, 3, 4), (7, 3, 4), (3, 7, 2), (7, 7, 2), (11, 3, 2)):
        m.box('brick', x, y, 1.75, 4, 4, h)
        m.box('brick_light', x, y, 1.75 + h, 4, 4, 0.25)
    m.box('gantry', 0, 0, 1.5, 1.5, 1.5, 14.5, ring=True)
    for y in (0, 14.5):
        m.box('gantry', 1.5, y, 14.5, 13, 1.5, 1.5)
    for x in (0, 14.5):
        m.box('gantry_dark', x, 1.5, 14.5, 1.5, 13, 1.5)
    stripe(m, 0, 1.5, 16, 1.5, 13)
    m.box('gantry_light', 0, 0, 16, 1.5, 1.5, 0.25, ring=True)
    # The bridge runs along x on the rails; the head runs along y on the bridge.
    m.box('steel_mid', 7, -0.25, 16, 2, 16.5, 1.25, part='bridge')
    for y in (-0.5, 14.75):
        m.box('steel_dark', 6.75, y, 15.5, 2.5, 1.75, 2, part='bridge')
    m.box('gantry', 6.5, 6.5, 14, 3, 3, 3.5, part='head')
    m.box('steel', 7.25, 7.25, 10.5, 1.5, 1.5, 3.5, part='head')
    m.box('steel_dark', 7, 7, 10, 2, 2, 0.75, part='head')
    m.box('glow_nozzle', 7.5, 7.5, 9.25, 1, 1, 0.75, part='head')



def butcher(m):
    # A low cabinet with a red lamp strip on its faces and the tier stripe on its top.
    base(m, 6)
    stripe(m, 0, 0, 6.25, 16, 2)
    panel(m, 'black', 4, 2, 8, 2)
    panel(m, 'glow_butcher', 4.5, 2.5, 7, 1, depth=0.4)
    # The loot hopper on the x- half: a funnel open at the top with meat lying in it.
    m.box('steel_dark', 1.5, 4.5, 6.25, 5, 7, 1.5)
    m.box('steel_mid', 1, 4, 7.75, 6, 8, 1.5)
    m.box('steel_mid', 0.5, 3.5, 9.25, 7, 1, 2.5)
    m.box('steel_mid', 0.5, 11.5, 9.25, 7, 1, 2.5)
    m.box('steel_mid', 0.5, 4.5, 9.25, 1, 7, 2.5)
    m.box('steel_mid', 6.5, 4.5, 9.25, 1, 7, 2.5)
    m.box('steel', 0.5, 3.5, 11.75, 7, 9, 0.25)
    m.box('black', 1.5, 4.5, 9.25, 5, 7, 0.25)
    m.box('meat', 2, 5.5, 9.5, 2.5, 2, 1)
    m.box('meat_light', 3.5, 8, 9.5, 2, 2.5, 0.75)
    # A post on the x+ half carries the arm, which swings about y; the sword hangs from its end.
    m.box('steel_dark', 9.5, 6.5, 6.25, 3.5, 3.5, 1)
    m.box('steel_mid', 10, 7, 7.25, 2.5, 2.5, 11)
    m.box('steel_dark', 9.5, 6.5, 18.25, 3.5, 3.5, 1.25)
    m.pivot('arm', 11.25, 8.25, 19)
    m.box('steel', 10.5, 7.5, 18.5, 7, 1.5, 1.25, part='arm')
    m.box('steel_dark', 15.75, 7.25, 17.25, 2, 2, 1.25, part='arm')
    m.box('sword_grip', 16.25, 7.75, 15.25, 1, 1, 2, part='arm')
    m.box('gun', 15.75, 6.75, 14.75, 2, 3, 0.5, part='arm')
    m.box('sword', 16.25, 7.5, 8.25, 1, 1.5, 6.5, part='arm')
    m.box('sword_edge', 16.25, 9, 8.75, 1, 0.25, 6, part='arm')


MODELS = {
    'repair': repair, 'kitchen': kitchen, 'fishery': fishery, 'feeder': feeder, 'shearer': shearer, 'apiary': apiary,
    'brewer': brewer, 'xp_tank': xp_tank, 'enchanter': enchanter, 'archeo': archeo, 'projector': projector,
    'builder': builder, 'butcher': butcher,
}
