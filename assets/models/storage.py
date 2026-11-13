"""Storage, worker docks and the rest: storage cell, terminal, docks, alloy block, crystal seed,
zipline anchor, turret, base anchor."""
from kit import family, panel, shade, shell, tier

VIOLET, TERM = '#c58aff', '#5ad1e6'
COLORS = {
    **family('cell', '#59616e'), 'drawer': shade('#59616e', -0.1), 'handle': '#c9a45a', 'glow_cell_lamp': '#59d38c',
    'term_screen': shade(TERM, -0.55), 'glow_term': TERM,
    'glow_unload': '#ffa53c', 'unload_dark': shade('#ffa53c', -0.6),
    'glow_load': '#4c9bf5', 'load_dark': shade('#4c9bf5', -0.6),
    'glow_coil': '#ffe14a', 'coil_dark': shade('#ffe14a', -0.35),
    'zip_post': '#e6e8ee', 'zip_base': '#9aa3b2', 'zip_roller': '#ffe14a', 'zip_roller_dark': '#c9a45a',
    'glow_eye': '#ff5a4a',
    **family('anchor', '#3b3550'), 'glow_ring': VIOLET, 'glow_core': VIOLET, 'glow_core_light': shade(VIOLET, 0.45),
    'core_dark': shade(VIOLET, -0.45),
}


def cell(m):
    shell(m, 'cell')
    # Two drawers with handles on the faces and a lamp between them.
    for z in (3, 9):
        panel(m, 'cell_dark', 2, z, 12, 4)
        panel(m, 'drawer', 3, z + 1, 10, 2, depth=0.4)
        panel(m, 'handle', 6, z + 1.75, 4, 0.5, depth=0.6)
    panel(m, 'black', 12.5, 7.25, 1.5, 1.5)
    panel(m, 'glow_cell_lamp', 12.75, 7.5, 1, 1, depth=0.45)


def terminal(m):
    shell(m)
    tier(m)
    # A screen on the faces: two lines of the storage list and three status lamps under it.
    panel(m, 'black', 2.5, 5, 11, 8.5)
    panel(m, 'term_screen', 3, 5.5, 10, 7.5, depth=0.35)
    for y, z, w in ((3.75, 10.5, 6), (3.75, 8.5, 8), (3.75, 6.5, 4.5)):
        panel(m, 'glow_term', y, z, w, 1, depth=0.45)
    for y, led in ((4, 'led_red'), (6, 'led_green'), (8, 'led_yellow')):
        panel(m, led, y, 3, 1, 1, depth=0.4)
    m.box('black', 4, 4, 16.25, 8, 8, 0.25)
    m.box('glow_term', 5, 5, 16.5, 6, 1, 0.15)


def dock(lit, dark):
    """A dock beside the rail: three arrows on the faces whose lights run along them."""
    def draw(m):
        shell(m)
        tier(m)
        panel(m, 'black', 2, 5, 12, 6)
        # Every arrow is two slanted arms; its light is a slightly larger copy over it.
        for i, y in enumerate((3, 7, 11)):
            for centre, a in ((8.9, -45), (7.1, 45)):
                pivot = (15.75, y + 1.25, centre)
                m.box(dark, 15.5, y, centre - 0.4, 0.35, 2.5, 0.8, tilt=('X', a), pivot=pivot, ring=True)
                m.box(lit, 15.5, y - 0.05, centre - 0.45, 0.5, 2.6, 0.9, part='lit%d' % i, tilt=('X', a), pivot=pivot, ring=True)
        for y in (5, 7, 9):
            m.box('black', 3, y, 16.25, 10, 1, 0.25)
    return draw


def dock_charge(m):
    shell(m)
    tier(m)
    # Coils on the faces and a contact plate on the top.
    for z in (4, 7.5, 11):
        panel(m, 'coil_dark', 3, z - 0.5, 10, 0.5, depth=0.4)
        panel(m, 'glow_coil', 3, z, 10, 1.5, depth=0.6)
    m.box('steel_mid', 3, 3, 16.25, 10, 10, 0.5)
    m.box('glow_coil', 6, 6, 16.75, 4, 4, 0.25)


def alloyblock(m):
    # A block of alloy in the colour of its tier: a bevelled frame and rivets on every side.
    m.box('tint_dark', 0, 0, 0, 16, 16, 1)
    m.box('tint', 0.5, 0.5, 1, 15, 15, 14)
    m.box('tint', 0, 0, 1, 1, 1, 14, ring=True)
    m.box('tint_light', 0, 0, 15, 16, 16, 1)
    m.box('tint_light', 15.5, 2, 13, 0.5, 12, 0.75, ring=True)
    for y in (2.5, 12.5):
        for z in (2.5, 11):
            m.box('tint_dark', 15.5, y, z, 0.6, 1, 1, ring=True)
    m.box('tint', 3, 3, 16, 10, 10, 0.25)


def seed(m):
    # A crystal seed on the lid of a tank, one part for every stage of its growth.
    shards = (
        ((6.75, 6.75, 2.5, 3.2, None),),
        ((6.75, 6.75, 2.5, 6.4, None), (3.2, 8.8, 2.8, 4.8, ('X', -12))),
        ((6.75, 6.75, 2.5, 9.6, None), (3.2, 8.8, 3.2, 7.2, ('X', -12)), (9.6, 4, 3.8, 9.6, ('Y', 10)),
         (9.9, 10.6, 2.9, 4.8, ('X', 16))),
    )
    for stage, crystals in enumerate(shards):
        part = 'stage%d' % stage
        m.box('steel_mid', 5.5, 5.5, 0, 5, 5, 0.75, part=part)
        for x, y, size, height, tilt in crystals:
            pivot = (x + size / 2, y + size / 2, 0.5)
            m.box('glow_crystal', x, y, 0.5, size, size, height, part=part, tilt=tilt, pivot=pivot)
            m.box('glow_crystal_light', x + 0.5, y + 0.5, 0.5 + height, size - 1, size - 1, 1, part=part, tilt=tilt, pivot=pivot)
            m.box('glow_crystal_deep', x + size - 0.3, y + 0.5, 0.5, 0.5, 1, height * 0.6, part=part, tilt=tilt, pivot=pivot)


def zipanchor(m):
    # A post with a pulley on the top; the pulley turns about y as the rope runs.
    m.box('zip_base', 5.5, 5.5, 0, 5, 5, 1.6)
    m.box('steel_mid', 6, 6, 1.6, 4, 4, 0.5)
    m.box('zip_post', 7, 7, 2.1, 2, 2, 11.9)
    m.box('steel_mid', 6.5, 6.5, 12.5, 3, 3, 1.5)
    for y in (6, 9.5):
        m.box('steel_dark', 7, y, 13.5, 2, 0.5, 3)
    m.pivot('roller', 8, 8, 15.5)
    for a in (0, 45):
        m.box('zip_roller', 6.25, 6.6, 13.75, 3.5, 2.8, 3.5, part='roller', tilt=('Y', a), pivot=(8, 8, 15.5))
    m.box('zip_roller_dark', 5.9, 7.4, 14.75, 4.2, 1.2, 1.5, part='roller')
    m.box('steel_dark', 7.5, 5.75, 15, 1, 4.5, 1)


def turret(m):
    # The base stays put.
    m.box('case_dark', 1, 1, 0, 14, 14, 2)
    m.box('case', 2, 2, 2, 12, 12, 3)
    m.box('case_light', 2, 2, 4.5, 2, 2, 1, ring=True)
    m.box('black', 4.5, 2.5, 5, 7, 1, 0.25, ring=True)
    m.box('steel_dark', 5, 5, 5, 6, 6, 1.5)
    # The head turns to the aim about the vertical axis of the block: armour, a sensor eye and
    # two cheeks that hold the barrel.
    head = 'aim_yaw'
    m.pivot(head, 8, 8, 6.5)
    m.box('plate', 4, 4.5, 6.5, 8, 7, 5, part=head)
    m.box('plate_dark', 5, 3.5, 7.5, 5, 1, 3, part=head)
    m.box('plate_dark', 5, 11.5, 7.5, 5, 1, 3, part=head)
    m.box('plate_dark', 2.5, 6, 7.5, 1.5, 4, 3, part=head)
    m.box('steel_mid', 5.5, 6, 11.5, 4, 4, 0.6, part=head)
    m.box('glow_eye', 12, 5.25, 10, 0.4, 1.5, 1, part=head)
    for y in (5.75, 9.5):
        m.box('plate_dark', 11.5, y, 8, 2.5, 0.75, 3, part=head)
    # The barrel tilts to the aim on the head and points to x+.
    barrel = 'aim_pitch'
    m.pivot(barrel, 13, 8, 9.5)
    m.box('steel_dark', 12, 6.5, 8, 2, 3, 3, part=barrel)
    m.box('gun', 14, 7.25, 8.75, 4.5, 1.5, 1.5, part=barrel)
    m.box('black', 18.5, 7, 8.5, 1, 2, 2, part=barrel)


def anchor(m):
    # An open cage: two slabs on four pillars.
    m.box('anchor_dark', 0, 0, 0, 16, 16, 3)
    m.box('anchor', 1, 1, 3, 14, 14, 1)
    m.box('anchor', 0, 0, 3, 3, 3, 10, ring=True)
    m.box('anchor', 0, 0, 13, 16, 16, 3)
    m.box('anchor_light', 0, 0, 15.5, 3, 3, 0.75, ring=True)
    # A glowing square ring in every opening and one more on the top.
    m.box('glow_ring', 14.5, 3, 12, 1, 10, 1, ring=True)
    m.box('glow_ring', 14.5, 3, 4, 1, 10, 1, ring=True)
    m.box('glow_ring', 14.5, 3, 5, 1, 1, 7, ring=True)
    m.box('glow_ring', 14.5, 12, 5, 1, 1, 7, ring=True)
    m.box('glow_ring', 3, 3, 16, 10, 1, 0.3, ring=True)
    # The core that holds the area: a cube on a pedestal, turned a quarter to the cage.
    m.box('anchor_dark', 6, 6, 4, 4, 4, 1.5)
    m.box('core_dark', 6.5, 6.5, 5.5, 3, 3, 1)
    m.box('glow_core', 5.5, 5.5, 6.5, 5, 5, 5, tilt=('Z', 45))
    m.box('glow_core_light', 6.75, 6.75, 11.5, 2.5, 2.5, 0.75, tilt=('Z', 45))


MODELS = {
    'cell': cell, 'terminal': terminal, 'dock_unload': dock('glow_unload', 'unload_dark'), 'dock_load': dock('glow_load', 'load_dark'),
    'dock_charge': dock_charge, 'alloyblock': alloyblock, 'seed': seed, 'zipanchor': zipanchor, 'turret': turret, 'anchor': anchor,
}
