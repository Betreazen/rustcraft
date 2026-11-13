"""Machines: furnace, crusher, crafter, alloy smelter, mixer, charger, placer, breaker, pump, miner, vein,
utilizer (trash) and disintegrator (spec 25)."""
from kit import family, firebox, frame, panel, port, shade, shell, tier

FIRE, ALLOY, SOL = '#ff9a3c', '#8a5645', '#59d38c'
COLORS = {
    'glow_fire': FIRE, 'glow_fire_light': shade(FIRE, 0.45), 'ember': shade(FIRE, -0.7),
    'glow_alloy_fire': '#ffb23c', 'glow_alloy_fire_light': shade('#ffb23c', 0.45), 'alloy_ember': shade('#ffb23c', -0.7),
    **family('alloy', ALLOY), 'mortar': shade(ALLOY, -0.55),
    'glow_grid': '#e6c15a', 'grid_dim': shade('#e6c15a', -0.55),
    'glow_sol': SOL, 'sol_dark': shade(SOL, -0.45),
    'glow_coil': '#ffe14a', 'coil_dark': shade('#ffe14a', -0.35),
    'placer': SOL, 'placer_dark': shade(SOL, -0.4), 'glow_led_green': '#59d38c', 'glow_led_red': '#e0564a',
    'breaker': '#e0564a', 'breaker_dark': shade('#e0564a', -0.4),
    'glow_water': '#4c9bf5', 'water_dark': shade('#4c9bf5', -0.45),
    'glow_eye': '#ff5a4a', 'miner_screen': shade('#ff8a5a', -0.55), 'glow_miner_screen': '#ff8a5a',
    **family('vein', '#5a3d6b'), 'ore_cyan': '#5fe6dd', 'ore_gold': '#f5d54a', 'ore_red': '#e53b3b',
    'glow_core': '#c58aff', 'glow_core_light': shade('#c58aff', 0.45), 'glow_ring': '#c58aff',
    # Utilizer: the fire in its pit and behind its grated mouths.
    'glow_burn': '#ff6a2a', 'glow_burn_light': shade('#ff6a2a', 0.5), 'burn_ember': shade('#ff6a2a', -0.72),
    # Disintegrator: the beam, the glass of the chamber, the pickaxe it splits and the ingots out.
    'glow_beam': '#ff5ad8', 'glow_beam_light': shade('#ff5ad8', 0.55),
    'glass': '#dcedf5', 'wood': '#8a6a3a', 'iron_item': '#d8d8d8', 'iron_item_dark': shade('#d8d8d8', -0.3),
}


def furnace(m):
    shell(m)
    tier(m)
    firebox(m, 'glow_fire', 'glow_fire_light', 'ember')
    port(m, 'case_light', 'ember')
    m.box('glow_fire', 6.5, 6.5, 16.25, 3, 3, 0.4, part='fire')


def crusher(m):
    shell(m, top=False)
    tier(m)
    # Jaws on the faces: a dark slot with two rows of teeth.
    panel(m, 'black', 3, 4, 10, 6)
    for y in (3.5, 5.5, 7.5, 9.5, 11.5):
        panel(m, 'steel', y, 8.5, 1, 1.5, depth=0.5)
    for y in (4.5, 6.5, 8.5, 10.5):
        panel(m, 'steel', y, 4, 1, 1.5, depth=0.5)
    # Two toothed rollers in the open top; they turn towards each other about x.
    for part, y in (('roll_a', 5.25), ('roll_b', 10.75)):
        m.pivot(part, 8, y, 12.75)
        for a in (0, 45):
            m.box('steel_mid', 2.5, y - 1.5, 11.25, 11, 3, 3, part=part, tilt=('X', a), pivot=(8, y, 12.75))
        for a in (0, 60, 120):
            m.box('steel', 2.75, y - 0.4, 10.4, 10.5, 0.8, 4.7, part=part, tilt=('X', a), pivot=(8, y, 12.75))


def crafter(m):
    shell(m)
    tier(m)
    # A 3 x 3 grid on the faces and on the top; on the top every cell has a lamp of its own.
    for i in range(3):
        for j in range(3):
            panel(m, 'black', 3 + 3.5 * i, 3 + 3.5 * j, 3, 3)
            panel(m, 'grid_dim', 4 + 3.5 * i, 4 + 3.5 * j, 1, 1, depth=0.4)
            x, y = 3 + 3.5 * i, 3 + 3.5 * j
            m.box('black', x, y, 16.25, 3, 3, 0.35)
            m.box('grid_dim', x + 1, y + 1, 16.6, 1, 1, 0.15)
            m.box('glow_grid', x + 0.75, y + 0.75, 16.6, 1.5, 1.5, 0.3, part='lit%d' % (3 * j + i))


def alloy(m):
    shell(m, 'alloy')
    tier(m)
    # Copper brick: mortar lines above the fire window.
    for z in (10.5, 12.75):
        panel(m, 'mortar', 0.5, z, 15, 0.4, depth=0.1)
    for y, z in ((4, 10.9), (11, 10.9), (7.5, 13.15)):
        panel(m, 'mortar', y, z, 0.4, 1.35 if z > 13 else 1.85, depth=0.1)
    firebox(m, 'glow_alloy_fire', 'glow_alloy_fire_light', 'alloy_ember')
    port(m, 'alloy_light', 'alloy_ember')
    m.box('glow_alloy_fire', 6.5, 6.5, 16.25, 3, 3, 0.4, part='fire')


def mixer(m):
    shell(m, top=False)
    tier(m)
    # A window on the faces with the solution behind it.
    frame(m, 'black', 4, 4, 8, 8)
    panel(m, 'glow_sol', 5, 5, 6, 3.5)
    panel(m, 'sol_dark', 5, 8.5, 6, 2.5)
    panel(m, 'white', 9.5, 9.5, 1, 1, depth=0.35)
    # The open vat: the solution and a stirrer under a bridge with its motor.
    m.box('glow_sol', 1.75, 1.75, 11.25, 12.5, 12.5, 2)
    m.box('steel_mid', 0.5, 7, 16, 15, 2, 1)
    m.box('steel_dark', 6.5, 6.5, 17, 3, 3, 2)
    m.pivot('stirrer', 8, 8, 12)
    m.box('steel', 7.5, 7.5, 12, 1, 1, 4.5, part='stirrer')
    for a in (0, 90):
        m.box('steel', 3, 7.25, 12.75, 10, 1.5, 1.25, part='stirrer', tilt=('Z', a), pivot=(8, 8, 12.75))


def charger(m):
    shell(m)
    tier(m)
    # Three coils on the faces and a charging pad on the top.
    for z in (3.5, 7, 10.5):
        panel(m, 'coil_dark', 2, z - 0.5, 12, 0.5, depth=0.4)
        panel(m, 'glow_coil', 2, z, 12, 1.5, depth=0.6)
    m.box('steel_mid', 2, 2, 16.25, 12, 12, 0.4)
    m.box('glow_coil', 13, 3, 16.65, 1, 10, 0.25, ring=True)
    m.box('steel_dark', 6, 6, 16.65, 4, 4, 0.25)


def placer(m):
    shell(m)
    tier(m)
    # A port on the x+ face with a piston that pushes out.
    m.box('placer', 15.5, 4, 4, 0.75, 8, 1)
    m.box('placer', 15.5, 4, 11, 0.75, 8, 1)
    m.box('placer', 15.5, 4, 5, 0.75, 1, 6)
    m.box('placer', 15.5, 11, 5, 0.75, 1, 6)
    m.box('black', 15.5, 5, 5, 0.25, 6, 6)
    m.box('steel_mid', 11.5, 7, 7, 4.5, 2, 2, part='piston')
    m.box('placer_dark', 15.6, 5.25, 5.25, 0.6, 5.5, 5.5, part='piston')
    m.box('steel', 15.75, 6.5, 6.5, 0.6, 3, 3, part='piston')
    status(m, 'glow_led_green', 'placer')


def status(m, led, mat):
    """A status panel on the three sides other than the front (x+); its lamp lights while the block works."""
    for quarter in (1, 2, 3):
        m.box('black', 15.5, 4, 6, 0.25, 8, 4, quarter=quarter)
        m.box(led, 15.5, 10, 8, 0.4, 1, 1, quarter=quarter)
        m.box(mat, 15.5, 5, 7, 0.4, 4, 0.75, quarter=quarter)


def breaker(m):
    shell(m)
    tier(m)
    # A red port on the x+ face with a drill that turns about x.
    for z in (4, 11):
        m.box('breaker', 15.5, 4, z, 0.75, 8, 1)
    for y in (4, 11):
        m.box('breaker', 15.5, y, 5, 0.75, 1, 6)
    m.box('black', 15.5, 5, 5, 0.25, 6, 6)
    m.box('steel_dark', 15.5, 5.5, 5.5, 1.5, 5, 5)
    m.pivot('drill', 17, 8, 8)
    for i, (size, turn) in enumerate(((4, 0), (3.25, 45), (2.5, 0), (1.5, 45), (0.75, 0))):
        mat = 'steel' if i % 2 == 0 else 'breaker_dark'
        m.box(mat, 17 + 0.8 * i, 8 - size / 2, 8 - size / 2, 0.8, size, size, part='drill', tilt=('X', turn), pivot=(17, 8, 8))
    status(m, 'glow_led_red', 'breaker')


def pump(m):
    shell(m)
    tier(m)
    # Vertical bars on the faces and a blue band that lights up while the pump works.
    for y in (4, 7, 10):
        panel(m, 'black', y, 3, 2, 8)
    m.box('glow_water', -0.25, -0.25, 12, 16.5, 16.5, 1)
    # A hose down into the water.
    m.box('steel_dark', 6, 6, -5, 4, 4, 5)
    for z in (-4, -2):
        m.box('steel_mid', 5.5, 5.5, z, 5, 5, 0.75)
    # A plunger in a guide frame on the top; it goes up and down.
    for y in (4, 11):
        m.box('steel_mid', 7, y, 16.25, 2, 1, 5)
    m.box('steel_mid', 6.5, 4, 21.25, 3, 8, 1)
    m.box('steel_dark', 6, 6, 16.25, 4, 4, 0.75)
    m.box('steel', 7.25, 7.25, 14, 1.5, 1.5, 7, part='plunger')
    m.box('water_dark', 6.25, 6.25, 18, 3.5, 3.5, 1.5, part='plunger')


def miner(m):
    # Legs leave room under the body for the drill.
    m.box('case_dark', 0, 0, 0, 3, 3, 5, ring=True)
    m.box('case_dark', 0, 0, 4, 16, 16, 1.5)
    m.box('case', 0.5, 0.5, 5.5, 15, 15, 9)
    m.box('case', 0, 0, 5.5, 1.5, 1.5, 9, ring=True)
    m.box('case', 0, 0, 14.5, 16, 16, 1.5)
    m.box('case_light', 0.5, 0.5, 16, 15, 15, 0.25)
    tier(m)
    # A red lens on the faces.
    panel(m, 'black', 5, 6, 6, 7)
    panel(m, 'black', 4, 7, 8, 5)
    panel(m, 'glow_eye', 6, 7, 4, 5, depth=0.5)
    panel(m, 'glow_eye', 5, 8, 6, 3, depth=0.5)
    panel(m, 'white', 6.5, 10.5, 1, 1, depth=0.6)
    # A screen on the top, an antenna and its scanner that turns.
    m.box('black', 2, 2, 16.25, 6, 5, 0.3)
    m.box('miner_screen', 2.5, 2.5, 16.55, 5, 4, 0.1)
    m.box('glow_miner_screen', 3, 4, 16.6, 3, 0.75, 0.15)
    m.box('steel_dark', 10, 10, 16.25, 3, 3, 1)
    m.box('steel_mid', 11, 11, 17.25, 1, 1, 4)
    m.pivot('scanner', 11.5, 11.5, 21)
    m.box('steel', 8, 11, 21, 7, 1, 0.75, part='scanner')
    m.box('steel_mid', 10.5, 10.5, 20.75, 2, 2, 1.25, part='scanner')
    for x in (7.5, 14.75):
        m.box('glow_eye', x, 10.75, 20.75, 0.75, 1.5, 1.25, part='scanner')
    # The drill under the body turns about z.
    m.box('steel_dark', 5.5, 5.5, 3.25, 5, 5, 0.75)
    m.pivot('drill', 8, 8, 0)
    for i, (size, turn) in enumerate(((4, 0), (3, 45), (2, 0), (1, 45))):
        m.box('steel' if i % 2 == 0 else 'steel_mid', 8 - size / 2, 8 - size / 2, 2.25 - i * 0.75, size, size, 1,
              part='drill', tilt=('Z', turn), pivot=(8, 8, 0))


def vein(m):
    # A rock base and four pillars with ore growing on them.
    m.box('vein_dark', 0, 0, 0, 16, 16, 3)
    m.box('vein', 0.5, 0.5, 3, 15, 15, 1)
    # The front pillar (x+ y+) is the lowest, so the core shows from the usual view.
    for x, y, h in ((0, 0, 10), (12, 0, 8), (12, 12, 3), (0, 12, 9)):
        m.box('vein', x, y, 4, 4, 4, h)
        m.box('vein_light', x + 0.5, y + 0.5, 4 + h, 3, 3, 0.5)
    for mat, x, y, z, s, tilt in (
            ('ore_cyan', 1, 1, 14.5, 2, ('X', 15)), ('ore_gold', 13, 1.5, 12.5, 1.75, ('Y', -15)),
            ('ore_red', 13, 13, 7.5, 2, ('X', -20)), ('ore_cyan', 1.5, 12.5, 13.5, 1.5, ('Y', 20)),
            ('ore_gold', 4.5, 2.5, 4, 1.5, ('Y', 25)), ('ore_red', 2.5, 6, 4, 1.5, ('X', -25)),
            ('ore_cyan', 12, 6.5, 4, 1.75, ('X', 20)), ('ore_red', 9, 12, 4, 1.25, ('Y', -20))):
        m.box(mat, x, y, z, s, s, s * 1.6, tilt=tilt)
    # A glowing ring on the pillars and the core over its pedestal; the core turns slowly.
    m.box('glow_ring', 4, 0.75, 6, 8, 1, 1, ring=True)
    m.box('vein_dark', 6, 6, 4, 4, 4, 1.5)
    m.pivot('core', 8, 8, 8.5)
    m.box('glow_core', 5.75, 5.75, 6.25, 4.5, 4.5, 4.5, part='core', tilt=('Z', 45), pivot=(8, 8, 6.25))
    m.box('glow_core_light', 6.75, 6.75, 10.75, 2.5, 2.5, 0.75, part='core', tilt=('Z', 45), pivot=(8, 8, 10.75))


def trash(m):
    # A squat bin, 12.75 pixels high: a solid lower body, walls round an open pit and a rim.
    m.box('case_dark', 0, 0, 0, 16, 16, 1.5)
    m.box('case', 0.5, 0.5, 1.5, 15, 15, 5)
    m.box('case', 1.5, 0.5, 6.5, 13, 1, 4.5, ring=True)
    m.box('case', 0, 0, 1.5, 1.5, 1.5, 9.5, ring=True)
    m.box('case_dark', 1.5, 1.5, 6.5, 13, 0.25, 4.5, ring=True)
    m.box('black', 1.5, 1.5, 6.5, 13, 13, 0.25)
    m.box('case', 0, 0, 11, 16, 1.5, 1.5, ring=True)
    m.box('case_light', 0, 0, 12.5, 16, 1.5, 0.25, ring=True)
    m.box('tint', 0, 0, 12.75, 16, 1.5, 0.25, part='tier')
    # A grated mouth on every face with the fire behind it; the guide hides the fire while it stands.
    frame(m, 'black', 3.5, 3, 9, 7)
    panel(m, 'burn_ember', 4.5, 4, 7, 5)
    panel(m, 'glow_burn', 4.5, 4, 7, 2.5, depth=0.4, part='fire')
    panel(m, 'glow_burn', 6, 6.5, 1.5, 1.5, depth=0.45, part='flame')
    panel(m, 'glow_burn_light', 9.5, 6, 1.25, 2, depth=0.45, part='flame')
    for y in (6.5, 9):
        panel(m, 'steel_dark', y, 4, 0.75, 5, depth=0.7)
    # The pit: a bed of embers, the fire over it and four tongues of flame.
    m.box('burn_ember', 1.75, 1.75, 6.75, 12.5, 12.5, 0.5)
    m.box('glow_burn', 2.5, 2.5, 7.25, 11, 11, 0.75, part='fire')
    for x, y, w, h in ((3.5, 4, 2, 2.5), (8, 3.5, 1.5, 3.25), (5, 9, 2.5, 2), (10, 9.5, 2, 2.75)):
        m.box('glow_burn_light' if h > 2.5 else 'glow_burn', x, y, 7.5, w, w, h, part='flame')
    # The grate across the mouth.
    for x in (5, 10):
        m.box('steel_dark', x, 1.5, 11.5, 1, 13, 0.75)
    m.box('steel_mid', 1.5, 7.5, 11.5, 13, 1, 0.75)


def disintegrator(m):
    # A low body and a glass chamber between four posts, open to the view under a top frame with a
    # bridge across it; the cutter hangs from the bridge.
    m.box('case_dark', 0, 0, 0, 16, 16, 1.5)
    m.box('case', 0.5, 0.5, 1.5, 15, 15, 4.5)
    m.box('case', 0, 0, 1.5, 1.5, 1.5, 13, ring=True)
    m.box('case_light', 0.5, 0.5, 6, 15, 15, 0.25)
    m.box('case_dark', 1.5, 0.5, 6.25, 13, 1, 0.75, ring=True)
    m.box('case', 0, 0, 14.5, 16, 1.5, 1.5, ring=True)
    m.box('case_light', 0, 0, 16, 16, 1.5, 0.25, ring=True)
    m.box('tint', 0, 0, 16.25, 16, 1.5, 0.25, part='tier')
    m.box('case', 1.5, 7, 15, 13, 2, 1)
    m.box('case_light', 1.5, 7, 16, 13, 2, 0.25)
    # Glass is suggested by two light streaks in every window, as on the tank.
    m.box('glass', 15.25, 3, 9, 0.25, 1, 4, ring=True)
    m.box('glass', 15.25, 4.5, 10.5, 0.25, 0.75, 2.5, ring=True)
    # A pedestal in a glowing ring with a pickaxe on it; its head splits into halves along y.
    m.box('steel_dark', 5.5, 5.5, 6.25, 5, 5, 1.75)
    m.box('glow_beam', 11, 5, 6.25, 0.5, 6, 0.5, ring=True)
    m.box('wood', 7.5, 7.5, 8, 1, 1, 3.5)
    for part, y0, tip in (('half_a', 4, 4), ('half_b', 8, 11)):
        m.box('iron_item', 7.25, y0, 10.75, 1.5, 4, 1.25, part=part)
        m.box('iron_item_dark', 7.25, tip, 9.75, 1.5, 1, 1, part=part)
    # The beam shines from the cutter hub down onto the split; the cutter turns about z.
    m.box('glow_beam_light', 7.4, 7.4, 12, 1.2, 1.2, 1.25, part='beam')
    m.box('glow_beam', 6.25, 7.8, 9.5, 3.5, 0.4, 3.75, part='beam')
    m.pivot('cutter', 8, 8, 13.5)
    m.box('steel_dark', 6.5, 6.5, 13.25, 3, 3, 1.75, part='cutter')
    for a in (0, 90):
        m.box('steel', 2.5, 7.25, 13.5, 11, 1.5, 0.5, part='cutter', tilt=('Z', a), pivot=(8, 8, 13.5))
        m.box('glow_beam', 2.5, 7.5, 13.4, 1, 1, 0.7, part='cutter', tilt=('Z', a), pivot=(8, 8, 13.5))
    # The output tray on the x+ face with two ingots in it; a lamp strip on the other three faces.
    m.box('black', 15.5, 4.75, 2.25, 0.1, 6.5, 3.25)
    m.box('steel_mid', 15.5, 4, 1.5, 1.25, 8, 0.75)
    for y in (4, 11.25):
        m.box('steel_mid', 15.5, y, 2.25, 1.25, 0.75, 2)
    m.box('steel_mid', 16.25, 4, 2.25, 0.5, 8, 1)
    for y in (5.25, 8.5):
        m.box('iron_item', 15.6, y, 2.25, 0.65, 2.25, 0.75)
    for quarter in (1, 2, 3):
        m.box('black', 15.5, 4, 2.5, 0.25, 8, 2.5, quarter=quarter)
        m.box('glow_beam', 15.5, 4.75, 3.25, 0.4, 6.5, 1, quarter=quarter)


MODELS = {
    'furnace': furnace, 'crusher': crusher, 'crafter': crafter, 'alloy': alloy, 'mixer': mixer, 'charger': charger,
    'placer': placer, 'breaker': breaker, 'pump': pump, 'miner': miner, 'vein': vein,
    'trash': trash, 'disintegrator': disintegrator,
}
