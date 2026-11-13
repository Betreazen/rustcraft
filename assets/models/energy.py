"""Energy: crystal, crystal structure block, geode, hydro turbine, windmill, solar panel, fuel and lava
generators, battery, charge plate.

Sources a housing can join (crystal, windmill, solar panel) carry a connector part link_<side> on
every side a housing can touch; the guide shows the connectors of the linked sides only."""
from kit import SIDES, family, firebox, frame, panel, shade, shell, tier

LAVA, CHARGE, CELLS = '#ff6a1f', '#59d38c', '#3f6fd9'
COLORS = {
    'glow_water': '#4c9bf5', 'water_dark': shade('#4c9bf5', -0.45),
    'glow_fuel_fire': '#ff7a2a', 'glow_fuel_fire_light': shade('#ff7a2a', 0.45), 'fuel_ember': shade('#ff7a2a', -0.7), 'fuel_stripe': '#ff7a2a',
    **family('lavagen', '#5b4a4a'), 'glow_lava': LAVA, 'glow_lava_light': '#ffd24a', 'lava_crust': shade(LAVA, -0.6),
    'glow_charge': CHARGE, 'charge_dark': shade(CHARGE, -0.6),
    'solar': '#27324a', 'solar_frame': '#2b3040', 'glow_cell': shade(CELLS, -0.15), 'glow_cell_light': shade(CELLS, 0.45),
    'blade': '#e6e8ee', 'blade_dark': '#c9ccd4',
    'glow_energy': '#ffe14a',
    'geode': '#6d6f76', 'geode_dark': shade('#6d6f76', -0.25), 'geode_light': shade('#6d6f76', 0.2),
}


def crystal(m):
    # Socket: a plate with a raised seat and four copper clamps.
    m.box('plate', 2, 2, 0, 12, 12, 2)
    m.box('steel_mid', 3, 3, 2, 10, 10, 1)
    m.box('plate_dark', 4, 4, 3, 8, 8, 0.5)
    m.box('copper', 2, 2, 2, 2, 2, 2, ring=True)
    m.box('steel_dark', 4, 4, 3, 1, 1, 1.5, ring=True)
    # The main crystal narrows to a light tip.
    m.box('glow_crystal', 5, 5, 3, 6, 6, 8)
    m.box('glow_crystal_mid', 6, 6, 11, 4, 4, 2)
    m.box('glow_crystal_light', 7, 7, 13, 2, 2, 2)
    # Facets: a light and a deep strip on every side.
    m.box('glow_crystal_light', 10.8, 6, 5, 0.5, 1, 6, ring=True)
    m.box('glow_crystal_deep', 10.8, 9, 3, 0.5, 1, 5, ring=True)
    # Three leaning shards around it.
    for x, y, size, height, tilt in ((2.5, 8.5, 2.5, 5, ('Y', -16)), (10.5, 3, 2.5, 4, ('X', 16)), (10.5, 10.5, 2, 3, ('Y', 14))):
        pivot = (x + size / 2, y + size / 2, 3)
        m.box('glow_crystal', x, y, 3, size, size, height, tilt=tilt, pivot=pivot)
        m.box('glow_crystal_light', x + 0.5, y + 0.5, 3 + height, size - 1, size - 1, 1, tilt=tilt, pivot=pivot)
    # Connectors: on a side, a copper clamp from the socket out to a cup on the face with a
    # crystal stud in it; below, a copper rim round the socket; above, a copper cap on four
    # prongs over the tip.
    for quarter, side in enumerate(SIDES):
        part = 'link_' + side
        m.box('copper', 13.5, 6, 0.5, 2, 4, 2.5, part=part, quarter=quarter)
        m.box('copper', 15.5, 5, 0, 0.5, 6, 4, part=part, quarter=quarter)
        m.box('glow_crystal_light', 15.75, 6.5, 1, 0.5, 3, 2, part=part, quarter=quarter)
    m.box('copper', 1.25, 1.25, 0, 13.5, 13.5, 0.75, part='link_z-')
    m.box('glow_crystal', 14.6, 7, 0.15, 0.25, 2, 0.45, part='link_z-', ring=True)
    m.box('copper', 5, 5, 13, 1, 1, 2.25, part='link_z+', ring=True)
    m.box('copper', 5, 5, 15.25, 6, 6, 0.75, part='link_z+')
    m.box('glow_crystal_light', 7, 7, 16, 2, 2, 0.25, part='link_z+')


def crystalblock(m):
    # One cell of a merged structure: a crystal cube with facets on every side and on the top.
    m.box('glow_crystal', 0.5, 0.5, 0, 15, 15, 15.5)
    m.box('glow_crystal_light', 15, 3, 4, 0.75, 1.5, 10, ring=True)
    m.box('glow_crystal_deep', 15, 9, 1.5, 0.75, 1.5, 8, ring=True)
    m.box('glow_crystal_mid', 15, 12, 9, 0.75, 1, 5, ring=True)
    m.box('glow_crystal_mid', 3, 3, 15.5, 10, 10, 0.5)
    m.box('glow_crystal_light', 5, 5, 16, 6, 6, 0.5, tilt=('Z', 45), pivot=(8, 8, 16))


def geode(m):
    # A stone block with darker and lighter patches and blue crystals growing out of it.
    m.box('geode', 0, 0, 0, 16, 16, 16)
    for x, y, w, d in ((15.9, 2, 0.2, 4), (15.9, 9, 0.2, 3)):
        m.box('geode_dark', x, y, 3 + y / 2, w, d, 3, ring=True)
    m.box('geode_light', 2, 9, 15.9, 4, 3, 0.2)
    m.box('geode_dark', 9, 2, 15.9, 3, 3, 0.2)
    for x, y, size, height, tilt in ((3.2, 8.8, 3.5, 7, ('X', -14)), (8.8, 4.8, 4, 10, ('Y', 8)), (10, 11, 3, 5, ('X', 18))):
        pivot = (x + size / 2, y + size / 2, 16)
        m.box('glow_crystal', x, y, 16, size, size, height, tilt=tilt, pivot=pivot)
        m.box('glow_crystal_light', x + 0.75, y + 0.75, 16 + height, size - 1.5, size - 1.5, 1.25, tilt=tilt, pivot=pivot)
        m.box('glow_crystal_deep', x + size - 0.25, y + 0.5, 16, 0.5, 1, height * 0.7, tilt=tilt, pivot=pivot)
    # Crystals grow out of all four sides as well, so the druse reads as one with a block on it.
    for y, z, size, height, lean in ((4.5, 5, 3, 5, 70), (11.5, 10, 2.5, 4, 58), (11.5, 3.5, 1.8, 2.5, 82)):
        pivot = (15, y, z)
        m.box('glow_crystal', 15 - size / 2, y - size / 2, z, size, size, height, tilt=('Y', lean), pivot=pivot, ring=True)
        m.box('glow_crystal_light', 15.5 - size / 2, y + 0.5 - size / 2, z + height, size - 1, size - 1, 0.75,
              tilt=('Y', lean), pivot=pivot, ring=True)


def turbine(m):
    # The frame stands half a pixel out of the block, so water in the same cell stays inside it.
    m.box('case_dark', -0.5, -0.5, 0, 17, 17, 2)
    m.box('case', -0.5, -0.5, 0, 3.5, 3.5, 16, ring=True)
    m.box('case', -0.5, -0.5, 13, 17, 17, 3)
    m.box('case_light', 1, 1, 16, 14, 14, 0.5)
    # A blue band under the top and an intake grate over the rotor.
    m.box('glow_water', -0.75, -0.75, 12, 17.5, 17.5, 1)
    m.box('black', 4, 4, 16.5, 8, 8, 0.25)
    for y in (5, 7, 9, 11):
        m.box('steel', 4, y - 0.25, 16.5, 8, 0.5, 0.75)
    # Flaps: three slanted blades in every opening. The current passes between them.
    for y in (5, 8, 11):
        m.box('steel', 15.6, y - 1.5, 2, 0.8, 3, 10, tilt=('Z', 32), ring=True)
    # The rotor behind them.
    m.box('gun', 6, 6, 2, 4, 4, 11)
    m.box('glow_water', 3, 7.5, 4, 10, 1, 7)
    m.box('glow_water', 7.5, 3, 4, 1, 10, 7)
    m.box('water_dark', 5, 5, 3, 6, 6, 1)


def windmill(m):
    # One block on the ground, three blocks tall: a base, a mast, a nacelle and four blades in
    # front of it on the y+ side, all above the block itself. The space above must stay free.
    m.box('case_dark', 4, 4, 0, 8, 8, 1.5)
    m.box('case', 5, 5, 1.5, 6, 6, 2.5)
    m.box('steel', 6, 6, 4, 4, 4, 25)
    for z in (10, 18, 26):
        m.box('case', 5.5, 5.5, z, 5, 5, 1)
    m.box('case', 4.8, 4.8, 29, 6.4, 8, 6.4)
    m.box('case_light', 5.3, 5.3, 35.4, 5.4, 7, 0.4)
    m.box('case_dark', 6.5, 3.5, 30.5, 3, 1.3, 3.5)
    m.box('steel_mid', 6.9, 12.8, 31.3, 2.2, 1.2, 2.2)
    # The rotor turns about y through the hub; its blades reach 13.5 pixels out.
    hub = (8, 14, 32.4)
    m.pivot('rotor', *hub)
    m.box('steel_dark', 6.6, 13.5, 31, 2.8, 1.5, 2.8, part='rotor')
    for a in (0, 90, 180, 270):
        m.box('blade', 9, 13.75, 31.4, 12.5, 0.75, 2, part='rotor', tilt=('Y', a), pivot=hub)
        m.box('blade_dark', 9, 13.6, 31.6, 4, 1.05, 1.6, part='rotor', tilt=('Y', a), pivot=hub)
        m.box('led_red', 19.5, 13.7, 31.3, 2, 0.85, 2.2, part='rotor', tilt=('Y', a), pivot=hub)
    # Connectors: on a side, an arm from the base to a bolted flange with an energy window; below,
    # a bolted skirt under the base, which also stands on a housing.
    for quarter, side in enumerate(SIDES):
        part = 'link_' + side
        m.box('steel_mid', 11, 7, 2, 4, 2, 1.5, part=part, quarter=quarter)
        m.box('case_dark', 15, 5, 0.5, 1, 6, 4.5, part=part, quarter=quarter)
        m.box('glow_energy', 15.75, 7, 1.75, 0.5, 2, 2, part=part, quarter=quarter)
        for y in (5.5, 9.9):
            for z in (1, 4.4):
                m.box('steel', 15.75, y, z, 0.4, 0.6, 0.6, part=part, quarter=quarter)
    m.box('case', 2, 2, 0, 12, 12, 0.5, part='link_z-')
    m.box('steel', 2.5, 2.5, 0.5, 1, 1, 0.4, part='link_z-', ring=True)
    m.box('glow_energy', 13.9, 7, 0.1, 0.25, 2, 0.3, part='link_z-', ring=True)


def solar(m):
    # A panel of nine cells on a short post.
    m.box('plate_dark', 4.5, 4.5, 0, 7, 7, 1)
    m.box('plate', 6.5, 6.5, 1, 3, 3, 3.8)
    m.box('solar_frame', 0, 0, 4.8, 16, 16, 1.4)
    m.box('solar', 0.5, 0.5, 6.2, 15, 15, 0.3)
    for i in range(3):
        for j in range(3):
            x, y = 1 + 5 * i, 1 + 5 * j
            m.box('glow_cell', x, y, 6.5, 4, 4, 0.25)
            m.box('glow_cell_light', x, y, 6.75, 1, 1, 0.1)
    # Connectors: on a side, a cable conduit from the post to a light plate with a cell window,
    # tall enough to show under the panel; below, a skirt round the foot.
    for quarter, side in enumerate(SIDES):
        part = 'link_' + side
        m.box('solar_frame', 9.5, 7.25, 1.5, 5.5, 1.5, 1.5, part=part, quarter=quarter)
        m.box('steel_mid', 15, 5, 0.25, 1, 6, 4.3, part=part, quarter=quarter)
        m.box('plate_dark', 15.6, 6, 1, 0.5, 4, 2.8, part=part, quarter=quarter)
        m.box('glow_cell_light', 15.8, 7, 1.6, 0.45, 2, 1.6, part=part, quarter=quarter)
    m.box('solar_frame', 3, 3, 0, 10, 10, 0.4, part='link_z-')
    m.box('glow_cell', 12.9, 7, 0.05, 0.25, 2, 0.3, part='link_z-', ring=True)


def gen_fuel(m):
    shell(m)
    tier(m)
    firebox(m, 'glow_fuel_fire', 'glow_fuel_fire_light', 'fuel_ember')
    # A vent and a hazard stripe on the top, and a chimney.
    for y in (5, 7, 9):
        m.box('black', 3, y, 16.25, 6, 1, 0.25)
    m.box('fuel_stripe', 1, 12.5, 16.25, 14, 2, 0.3)
    for x in (1, 5, 9, 13):
        m.box('black', x, 12.5, 16.3, 2, 2, 0.3)
    m.box('steel_dark', 10.5, 4.5, 16.25, 3, 3, 5)
    m.box('steel_mid', 10, 4, 21.25, 4, 4, 0.75)
    m.box('black', 11, 5, 21.5, 2, 2, 0.6)


def gen_lava(m):
    shell(m, 'lavagen')
    tier(m)
    # A large window on the faces with lava behind it; a grate on the top.
    frame(m, 'black', 3, 3, 10, 10)
    panel(m, 'lava_crust', 4, 4, 8, 8)
    panel(m, 'glow_lava', 4, 4, 8, 6, depth=0.35)
    for y, z in ((5, 5), (8.5, 6.5), (6.5, 8.5), (10, 4.5)):
        panel(m, 'glow_lava_light', y, z, 1, 1, depth=0.45)
    m.box('black', 3, 3, 16.25, 10, 10, 0.25)
    m.box('glow_lava', 4, 4, 16.3, 8, 8, 0.3)
    for x in (5, 8, 11):
        m.box('lavagen_light', x - 0.5, 3, 16.5, 1, 10, 0.4)


def battery(m):
    shell(m)
    tier(m)
    # A charge gauge on the faces: a glowing bar in a dark frame under a scale of four ticks. The
    # bar is the part level_charge, which the guide stretches to the charge of the structure.
    frame(m, 'black', 5, 2.5, 6, 11, bar=0.75)
    panel(m, 'charge_dark', 5.75, 3.25, 4.5, 9.5)
    panel(m, 'glow_charge', 6.25, 3.75, 3.5, 8.5, depth=0.4, part='level_charge')
    for z in (5.45, 7.15, 8.85, 10.55):
        panel(m, 'black', 6.25, z, 3.5, 0.25, depth=0.45)
    # Terminals on the top.
    for x, y, cap in ((2.5, 2.5, 'led_red'), (10.5, 10.5, 'steel_dark')):
        m.box('steel_mid', x, y, 16.25, 3, 3, 0.75)
        m.box(cap, x + 0.5, y + 0.5, 17, 2, 2, 0.75)
    m.box('glow_charge', 6, 7.5, 16.25, 4, 1, 0.25)


def charge_plate(m):
    # A thin plate on the floor, like a pressure plate, laid over a housing: whoever stands on it
    # charges their gear quickly. Contacts in the corners and a glowing coil on the top.
    m.box('plate_dark', 1, 1, 0, 14, 14, 0.75)
    m.box('case', 1.5, 1.5, 0.75, 13, 13, 0.25)
    m.box('glow_energy', 3, 3, 1, 10, 0.75, 0.15, ring=True)
    m.box('glow_energy', 5.5, 5.5, 1, 5, 0.5, 0.15, ring=True)
    m.box('glow_energy', 7.25, 7.25, 1, 1.5, 1.5, 0.2)
    m.box('copper', 1.5, 1.5, 1, 1.25, 1.25, 0.2, ring=True)


MODELS = {
    'crystal': crystal, 'crystalblock': crystalblock, 'geode': geode, 'turbine': turbine, 'windmill': windmill,
    'solar': solar, 'gen_fuel': gen_fuel, 'gen_lava': gen_lava, 'battery': battery, 'charge_plate': charge_plate,
}
