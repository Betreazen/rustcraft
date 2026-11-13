"""Trunk and transport: node, junction node, housing (pipe), monorail, station, capsule, worker cart, tank,
portal dock."""
import math

from kit import FACES, face_box, family, frame, panel, shade, shell, tier

NODE, STATION, CAPSULE, CHASSIS, TANK = '#3d4658', '#4a5568', '#e6e8ee', '#e2bd55', '#9fb4c2'
TOOLS = {'saw': '#d8dce4', 'reaper': '#8fd35a', 'hold': '#a8793f', 'grab': '#ffa53c', 'bore': '#9aa3b2', 'gun': '#e0564a',
         'scoop': '#e08a3a', 'map': '#5ab0e6'}
COLORS = {
    **family('node', NODE), 'glow_lens': '#7af0ff',
    **family('station', STATION), 'hazard': '#ffcf5a', 'screen': shade('#ffcf5a', -0.55), 'glow_screen': '#ffcf5a',
    'pipe': '#5d6470', 'pipe_light': '#8a919d', 'pipe_dark': '#3a3f49',
    # Channel strips of a housing, recoloured by the guide after the four slots.
    'glow_tint_0': '#15171c', 'glow_tint_1': '#15171c', 'glow_tint_2': '#15171c', 'glow_tint_3': '#15171c',
    **family('capsule', CAPSULE), 'capsule_top': '#cfeaf5', 'glow_window': '#8fd8f0', 'glow_headlight': '#fff3a8',
    **family('chassis', CHASSIS), 'glow_status': '#59d38c',
    **{'tool_' + k: c for k, c in TOOLS.items()}, **{'tool_' + k + '_dark': shade(c, -0.3) for k, c in TOOLS.items()},
    **family('tank', TANK), 'glass': '#dcedf5', 'glow_tint': '#3f76e4', 'glow_tint_top': shade('#3f76e4', 0.3),
    'glow_map_tip': '#7af0ff', 'map_paper': '#e6e2cf', 'map_land': '#59d38c',
    'portal': '#b06cf0', 'portal_dark': shade('#b06cf0', -0.6), 'glow_portal': '#b06cf0', 'glow_portal_light': shade('#b06cf0', 0.45),
}


def lens(m, face):
    """A lens on one side of a node: a dark cross-shaped socket with a glowing eye."""
    face_box(m, 'black', face, 5, 4, 6, 8, inset=0.5)
    face_box(m, 'black', face, 4, 5, 8, 6, inset=0.5)
    face_box(m, 'glow_lens', face, 6, 5, 4, 6, depth=1)
    face_box(m, 'glow_lens', face, 5, 6, 6, 4, depth=1)
    face_box(m, 'white', face, 7, 9, 1, 1, inset=-0.2, depth=0.4)


def cage(m):
    """The frame of twelve beams a node is built in, lighter on the top corners."""
    m.box('node', 0, 0, 0, 16, 2, 2, ring=True)
    m.box('node', 0, 0, 14, 16, 2, 2, ring=True)
    m.box('node', 0, 0, 2, 2, 2, 12, ring=True)
    m.box('node_light', 0, 0, 15.5, 2, 2, 0.5, ring=True)


def node(m):
    # A dark core inside the frame, and the same lens on all six sides, the top and the bottom
    # too. Four pips round every lens wear the colour of the node's tier.
    m.box('node_dark', 1, 1, 1, 14, 14, 14)
    cage(m)
    for face in FACES:
        lens(m, face)
        for a in (2.5, 11.5):
            for b in (2.5, 11.5):
                face_box(m, 'tint', face, a, b, 2, 2, inset=0.5)


def node_hub(m):
    # The junction of the last tier joins all six sides, so it looks open: a deep window on every
    # side with a glowing rim, and in every window a rotor that turns things to any side. The
    # rotors of opposite windows are one part, turning about the axis through them: rotor_x,
    # rotor_y and rotor_z. A glowing heart sits behind them in the middle.
    cage(m)
    m.box('glow_lens', 5, 5, 5, 6, 6, 6)
    # Corner caps in the colour of the tier.
    for z in (-0.25, 13.75):
        m.box('tint', -0.25, -0.25, z, 2.5, 2.5, 2.5, ring=True)
    for face in FACES:
        for a, b, w, h in ((2, 2, 12, 2), (2, 12, 12, 2), (2, 4, 2, 8), (12, 4, 2, 8)):
            face_box(m, 'node_dark', face, a, b, w, h, inset=0.5, depth=1)
        for a, b, w, h in ((4, 4, 8, 0.5), (4, 11.5, 8, 0.5), (4, 4.5, 0.5, 7), (11.5, 4.5, 0.5, 7)):
            face_box(m, 'glow_lens', face, a, b, w, h, inset=0.25, depth=1)
        for a, b, w, h in ((4, 4, 8, 1.5), (4, 10.5, 8, 1.5), (4, 5.5, 1.5, 5), (10.5, 5.5, 1.5, 5)):
            face_box(m, 'node_dark', face, a, b, w, h, inset=1.5, depth=1.5)
        rotor = 'rotor_' + face[0]
        m.pivot(rotor, 8, 8, 8)
        face_box(m, 'tint', face, 5, 7.25, 6, 1.5, inset=2, depth=0.5, part=rotor)
        face_box(m, 'tint', face, 7.25, 5, 1.5, 6, inset=2, depth=0.5, part=rotor)
        face_box(m, 'tint_light', face, 5, 7.25, 1, 1.5, inset=1.9, depth=0.5, part=rotor)
        face_box(m, 'tint_light', face, 7.25, 10, 1.5, 1, inset=1.9, depth=0.5, part=rotor)
        face_box(m, 'white', face, 7.25, 7.25, 1.5, 1.5, inset=1.75, depth=0.75, part=rotor)


def station(m):
    # A plinth with a hazard band, as on the edge of a platform.
    m.box('station_dark', 0, 0, 0, 16, 16, 2)
    m.box('black', 0.25, 0.25, 2, 15.5, 15.5, 2)
    for y in (1, 5, 9, 13):
        m.box('hazard', 15.5, y, 2, 0.5, 2, 2, ring=True)
    m.box('station', 1, 1, 4, 14, 14, 8)
    # A route screen on every side: two lines of the scheme and three status lamps.
    m.box('black', 14.6, 3, 6, 0.8, 10, 5.2, ring=True)
    m.box('screen', 15.4, 3.5, 6.5, 0.2, 9, 4.2, ring=True)
    m.box('glow_screen', 15.5, 4.5, 9.3, 0.25, 5, 0.8, ring=True)
    m.box('glow_screen', 15.5, 4.5, 7.5, 0.25, 7, 0.8, ring=True)
    for y, led in ((4, 'led_red'), (6, 'led_green'), (8, 'led_yellow')):
        m.box(led, 14.9, y, 4.6, 0.4, 1, 0.9, ring=True)
    # The roof with a beacon.
    m.box('station', 0, 0, 12, 16, 16, 2)
    m.box('station_light', 1, 1, 14, 14, 14, 0.5)
    m.box('case', 3, 3, 14.5, 10, 10, 0.5)
    m.box('steel_dark', 7, 7, 15, 2, 2, 1)
    m.box('glow_lamp', 6.5, 6.5, 16, 3, 3, 1.5)


# A housing is 6 pixels across. Its four channel strips run inside, 1 pixel thick, placed as four
# queens on a 4 x 4 board, so every strip shows between the others from every side.
ROW = (6, 7, 8, 9)
QUEEN = (1, 3, 0, 2)


def along(axis, start, length, a, b, size=1):
    """A box that runs `length` pixels along `axis` from `start`; (a, b) is the corner of its
    cross-section on the other two axes in x, y, z order."""
    if axis == 'x':
        return start, a, b, length, size, size
    if axis == 'y':
        return a, start, b, size, length, size
    return a, b, start, size, size, length


def pipe(m):
    # The centre: a cage of twelve beams around the four channels, lighter on top.
    for a in (5, 10):
        for b in (5, 10):
            edge = 'pipe_light' if b == 10 else 'pipe_dark'
            m.box(edge, *along('x', 5, 6, a, b))
            m.box(edge, *along('y', 6, 4, a, b))
            m.box('pipe', *along('z', 6, 4, a, b))
    for k in range(4):
        m.box('glow_tint_%d' % k, ROW[k], ROW[k], ROW[QUEEN[k]], 1, 1, 1)
    # An arm to every side, one part each; the guide shows the arms to connected sides only.
    for axis in 'xyz':
        for sign, start in (('-', 0), ('+', 11)):
            arm = axis + sign
            for a in (5, 10):
                for b in (5, 10):
                    edge = 'pipe' if axis == 'z' else 'pipe_light' if b == 10 else 'pipe_dark'
                    m.box(edge, *along(axis, start, 5, a, b), part=arm)
            for k in range(4):
                m.box('glow_tint_%d' % k, *along(axis, start, 5, ROW[k], ROW[QUEEN[k]]), part=arm)
            # A collar where the arm meets the next block.
            m.box('pipe', *along(axis, 0 if start == 0 else 15, 1, 4.75, 4.75, 6.5), part=arm)


# The halves in the order of quarter turns about the block centre: quarter n takes x+ to HALVES[n].
HALVES = ('x+', 'y+', 'x-', 'y-')
# A corner arc runs round the corner of the cell between two halves, 8 pixels out, in this many
# straight pieces.
ARC_STEPS = 8
SLOPE = 16 * math.sqrt(2)


def rail_section(m, part, x, length, **turn):
    """A straight piece of rail along x from `x`: the dark foot, the rail in the tier colour and
    its light top, 2 pixels high in all, centred on y 8."""
    m.box('steel_dark', x, 6.5, 0, length, 3, 0.5, part=part, **turn)
    m.box('tint', x, 7, 0.5, length, 2, 1.25, part=part, **turn)
    m.box('tint_light', x, 7, 1.75, length, 2, 0.25, part=part, **turn)


def rail(m):
    # A floor rail: the hub in the cell centre and a half rail to every side, one part each. The
    # guide shows the halves to connected sides and turns the model onto walls and ceilings.
    m.box('steel_dark', 6.5, 6.5, 0, 3, 3, 0.5)
    m.box('tint', 7, 7, 0.5, 2, 2, 1.25)
    m.box('tint_light', 7, 7, 1.75, 2, 2, 0.25)
    for turn, half in enumerate(HALVES):
        m.box('steel_dark', 9.5, 6.5, 0, 6.5, 3, 0.5, part=half, quarter=turn)
        m.box('tint', 9, 7, 0.5, 7, 2, 1.25, part=half, quarter=turn)
        m.box('tint_light', 9, 7, 1.75, 7, 2, 0.25, part=half, quarter=turn)
        m.box('steel_mid', 12, 5, 0, 1.5, 6, 0.75, part=half, quarter=turn)
    # Corners: a quarter circle from one side to the next in place of the hub and two halves,
    # made of short straight pieces turned about the corner of the cell. arc_x+y+ joins x+ and
    # y+; the other three follow round.
    corner = (16, 16, 0)
    for turn, half in enumerate(HALVES):
        part = 'arc_' + half + HALVES[(turn + 1) % 4]
        for k in range(ARC_STEPS):
            rail_section(m, part, 14.9, 2.2, tilt=('Z', -(k + 0.5) * 90 / ARC_STEPS), pivot=corner, quarter=turn)
        for angle in (-22.5, -67.5):
            m.box('steel_mid', 15.25, 5, 0, 1.5, 6, 0.75, part=part, tilt=('Z', angle), pivot=corner, quarter=turn)
    # The climbing piece: at 45 degrees from the floor of the cell on the x- side up to the
    # floor of the cell above the next one on the x+ side, on two posts.
    climb = dict(tilt=('Y', -45), pivot=(0, 8, 0))
    rail_section(m, 'slope', -0.5, SLOPE + 1, **climb)
    for d in (4, 11, 18):
        m.box('steel_mid', d, 5, 0, 1.5, 6, 0.75, part='slope', **climb)
    for x, h in ((5.25, 5.25), (11.25, 11.25)):
        m.box('steel_mid', x, 7.25, 0, 1.5, 1.5, h, part='slope')
        m.box('steel_dark', x - 0.75, 6.5, 0, 3, 3, 0.5, part='slope')


def mounts(m, low, high):
    """Brackets from a cart to its rail, one part each: below (floor rail), above (ceiling rail),
    side (wall rail on the y- side) and sidex (wall rail on the x- side). `low` and `high` are
    the bottom and the top of the cart body. Each bogie clamps the rail web from both sides."""
    m.box('steel_mid', 4.5, 6, 2, 7, 4, 1, part='below')
    m.box('black', 5, 5.5, 0.75, 6, 0.75, 1.5, part='below')
    m.box('black', 5, 9.75, 0.75, 6, 0.75, 1.5, part='below')
    m.box('steel_dark', 7, 7, 3, 2, 2, low - 2.5, part='below')
    m.box('steel_mid', 4.5, 6, 13, 7, 4, 1, part='above')
    m.box('black', 5, 5.5, 13.75, 6, 0.75, 1.5, part='above')
    m.box('black', 5, 9.75, 13.75, 6, 0.75, 1.5, part='above')
    m.box('steel_dark', 7, 7, high - 0.5, 2, 2, 13.5 - high, part='above')
    for part, quarter in (('side', 0), ('sidex', 3)):
        m.pivot(part, 8, 8, 8)
        m.box('steel_mid', 4.5, 2, 6, 7, 1, 4, part=part, quarter=quarter)
        m.box('black', 5, 0.75, 5.5, 6, 1.5, 0.75, part=part, quarter=quarter)
        m.box('black', 5, 0.75, 9.75, 6, 1.5, 0.75, part=part, quarter=quarter)
        m.box('steel_dark', 7, 3, 7, 2, 1.5, 2, part=part, quarter=quarter)


def capsule(m):
    # The cabin faces x+ and stays level; the guide turns it to where it travels.
    m.box('capsule', 3, 4, 4, 10, 8, 8)
    m.box('capsule_top', 3.5, 4.5, 12, 9, 7, 0.75)
    m.box('steel_mid', 3.5, 4.5, 3.25, 9, 7, 0.75)
    m.box('capsule', 13, 5, 4.5, 1, 6, 7)
    m.box('capsule_dark', 2.25, 5, 5, 0.75, 6, 6)
    # Windows on both sides and a windscreen, a headlight in front and tail lamps behind.
    for y in (3.75, 11.75):
        m.box('glow_window', 4.5, y, 7.5, 3, 0.5, 3)
        m.box('glow_window', 8.5, y, 7.5, 3, 0.5, 3)
        m.box('capsule_dark', 7.75, y + 0.05, 4.75, 0.5, 0.4, 6.5)
    m.box('glow_window', 13.75, 5.75, 7.75, 0.5, 4.5, 3)
    m.box('glow_headlight', 13.75, 7, 5.25, 0.75, 2, 1.5)
    for y in (5.5, 9.5):
        m.box('led_red', 2, y, 9, 0.5, 1, 1)
    mounts(m, 3.25, 12.75)


def worker(m):
    # The chassis faces x+; the guide turns it to where it travels and shows the tool it wears.
    m.box('chassis_dark', 3, 4, 3.5, 10, 8, 1.5)
    m.box('chassis', 3.5, 4.5, 5, 9, 7, 5)
    m.box('chassis_light', 3, 4, 10, 10, 8, 0.75)
    for y in (4.25, 11.25):
        for z in (6.5, 8):
            m.box('black', 5, y, z, 5, 0.5, 0.5)
    m.box('steel_dark', 2.5, 5, 4.5, 0.5, 6, 2)
    m.box('steel_mid', 13, 5, 4.5, 1, 6, 5)
    # The status lamp on the deck.
    m.box('steel_dark', 3.75, 10, 10.75, 2, 2, 0.5)
    m.box('glow_status', 4, 10.25, 11.25, 1.5, 1.5, 1)
    mounts(m, 3.5, 10.75)
    tools(m)


def tools(m):
    """The eight tools of a worker, each its own parts, in front of the chassis (x+) or on its deck."""
    t = 'tool_'
    # Saw: a disc on an arm that slides out of a head. The head turns about the upright through
    # its turntable, the arm slides out along the head and the disc turns about y on its end.
    m.box('steel_dark', 13, 6.5, 5.5, 3, 3, 0.75, part='saw')
    m.pivot('aim_yaw_saw', 14.5, 8, 6.25)
    m.box('steel_mid', 13, 6.75, 6.25, 3, 2.5, 2.5, part='aim_yaw_saw')
    m.box('steel_dark', 16, 7, 6.75, 2, 2, 1.5, part='aim_yaw_saw')
    m.pivot('reach_saw', 14.5, 8, 6.25)
    m.box('steel', 14.5, 7.25, 7, 5, 1.5, 1, part='reach_saw')
    m.pivot('saw_disc', 20, 8, 7.5)
    for a in (0, 45, 90, 135):
        m.box(t + 'saw', 17, 7.75, 6.4, 6, 0.5, 2.2, part='saw_disc', tilt=('Y', a), pivot=(20, 8, 7.5))
    m.box('steel_dark', 19.25, 7.5, 6.75, 1.5, 1, 1.5, part='saw_disc')
    # Reaper: a reel of four bats across the front; it turns about y.
    for y in (2.5, 12.5):
        m.box('steel_mid', 14, y, 6, 3, 1, 1.5, part='reaper')
    m.pivot('reaper_reel', 17, 8, 6.75)
    m.box('steel_dark', 16.25, 3.5, 6, 1.5, 9, 1.5, part='reaper_reel')
    for a in (0, 90):
        for y in (3.5, 11.75):
            m.box(t + 'reaper_dark', 14, y, 6.5, 6, 0.75, 0.5, part='reaper_reel', tilt=('Y', a), pivot=(17, 8, 6.75))
    for dx, dz in ((2.5, 0), (-2.5, 0), (0, 2.5), (0, -2.5)):
        m.box(t + 'reaper', 16.5 + dx, 3, 6.25 + dz, 1, 10, 1, part='reaper_reel')
    # Hold: a cargo box on the deck with a lid and a latch; it does not move.
    m.box(t + 'hold', 6.25, 4.75, 11.25, 6.5, 6.5, 2.5, part='hold')
    m.box(t + 'hold_dark', 6, 4.5, 10.75, 7, 7, 0.5, part='hold')
    m.box(t + 'hold_dark', 6, 4.5, 13.75, 7, 7, 0.75, part='hold')
    m.box('steel', 12.75, 7.25, 12, 0.5, 1.5, 1.5, part='hold')
    # Grab: a boom with two jaws that close and open along y.
    m.box('steel_mid', 14, 7, 9, 3.5, 2, 1.5, part='grab')
    m.box('steel_dark', 16, 5.5, 7.5, 2, 5, 1.5, part='grab')
    for part, y, tip in (('grab_left', 5, 6), ('grab_right', 10, 9.25)):
        m.box(t + 'grab', 16.25, y, 3.5, 1.5, 1, 4, part=part)
        m.box(t + 'grab_dark', 16.25, tip, 3.5, 1.5, 0.75, 1, part=part)
    # Bore: a stepped drill bit that turns about x.
    m.box('steel_mid', 14, 5.5, 4.5, 1.5, 5, 5, part='bore')
    m.pivot('bore_bit', 15.5, 8, 7)
    for i, (size, turn) in enumerate(((4.5, 0), (3.5, 45), (2.5, 0), (1.5, 45), (0.75, 0))):
        mat = t + ('bore' if i % 2 == 0 else 'bore_dark')
        m.box(mat, 15.5 + i, 8 - size / 2, 7 - size / 2, 1, size, size, part='bore_bit', tilt=('X', turn), pivot=(16, 8, 7))
    # Gun: a turret on the deck. The head turns about the upright, the barrel tilts on it and
    # recoils along itself.
    m.box('steel_dark', 9, 6, 10.75, 4, 4, 0.75, part='gun')
    m.pivot('aim_yaw_gun', 11, 8, 11.5)
    m.box(t + 'gun', 9.25, 6.5, 11.5, 3.5, 3, 2.5, part='aim_yaw_gun')
    m.box(t + 'gun_dark', 9.25, 6.25, 12, 2, 0.25, 1.5, part='aim_yaw_gun')
    m.box(t + 'gun_dark', 9.25, 9.5, 12, 2, 0.25, 1.5, part='aim_yaw_gun')
    m.pivot('aim_pitch_gun', 12, 8, 13)
    m.box('gun', 11.5, 7.25, 12.25, 5.5, 1.5, 1.5, part='aim_pitch_gun')
    m.box('black', 17, 7, 12, 0.75, 2, 2, part='aim_pitch_gun')
    # Scoop: an excavator bucket on a boom, built as the saw: the head turns about the upright
    # through its turntable, the boom slides out along the head and the bucket curls about y on
    # its end, mouth forward.
    m.box('steel_dark', 13, 6.5, 5.5, 3, 3, 0.75, part='scoop')
    m.pivot('aim_yaw_scoop', 14.5, 8, 6.25)
    m.box('steel_mid', 13, 6.5, 6.25, 3, 3, 2.5, part='aim_yaw_scoop')
    m.box(t + 'scoop_dark', 15.5, 6.75, 6.75, 1.5, 2.5, 1.5, part='aim_yaw_scoop')
    m.pivot('reach_scoop', 14.5, 8, 6.25)
    m.box(t + 'scoop', 14.5, 7.25, 7, 5.5, 1.5, 1.25, part='reach_scoop')
    m.box('steel_dark', 19.5, 7, 6, 1, 2, 2, part='reach_scoop')
    m.pivot('scoop_bucket', 20, 8, 6.5)
    m.box(t + 'scoop', 20, 5, 3, 0.75, 6, 4.25, part='scoop_bucket')
    m.box(t + 'scoop', 20, 5, 3, 3.5, 6, 0.75, part='scoop_bucket')
    for y in (5, 10.5):
        m.box(t + 'scoop_dark', 20.75, y, 3.75, 2.25, 0.5, 2.75, part='scoop_bucket')
    for y in (5.5, 7.5, 9.5):
        m.box('steel', 23.5, y, 3, 0.75, 1, 0.5, part='scoop_bucket')
    # Map: a cartographer's drum on a mast on the deck. The drum, with the map round it and an
    # antenna on top, turns about the upright while the worker charts its way.
    m.box('steel_dark', 8.5, 6.5, 10.75, 3, 3, 0.75, part='map')
    m.box('steel_mid', 9.5, 7.5, 11.5, 1, 1, 2, part='map')
    m.pivot('map_drum', 10, 8, 13.5)
    for a in (0, 45):
        m.box(t + 'map', 8, 6, 13.5, 4, 4, 2.5, part='map_drum', tilt=('Z', a), pivot=(10, 8, 13.5))
        m.box(t + 'map_dark', 7.9, 5.9, 15.75, 4.2, 4.2, 0.5, part='map_drum', tilt=('Z', a), pivot=(10, 8, 15.75))
    m.box('map_paper', 11.9, 6.75, 13.9, 0.4, 2.5, 1.6, part='map_drum')
    m.box('map_land', 12, 7, 14.2, 0.4, 1.25, 0.9, part='map_drum')
    m.box('steel', 9.75, 7.75, 16.25, 0.5, 0.5, 3.25, part='map_drum')
    m.box('steel_mid', 8, 7.75, 18.25, 4, 0.5, 0.5, part='map_drum')
    m.box('glow_map_tip', 9.5, 7.5, 19.5, 1, 1, 1, part='map_drum')


def tank(m):
    # A frame of steel-blue posts and plates; the fluid inside is the part `level_fluid`.
    m.box('tank_dark', 0, 0, 0, 16, 16, 1.5)
    m.box('black', 1.5, 1.5, 1.5, 13, 13, 0.25)
    m.box('tank', 0, 0, 1.5, 2, 2, 13, ring=True)
    m.box('tank', 0, 0, 14.5, 16, 16, 1.5)
    m.box('tank_light', 0.5, 0.5, 16, 15, 15, 0.25)
    m.box('steel_mid', 6, 6, 16.25, 4, 4, 0.75)
    m.box('tank_dark', 2, 0.5, 7.5, 12, 0.75, 1, ring=True)
    # Glass is suggested by two light streaks in every window.
    m.box('glass', 15.25, 3.5, 9.5, 0.25, 1, 4, ring=True)
    m.box('glass', 15.25, 5, 11, 0.25, 0.75, 2.5, ring=True)
    # The column is full at level 1; the guide stretches it to the level it shows.
    m.box('glow_tint', 2, 2, 1.75, 12, 12, 11.75, part='level_fluid')
    m.box('glow_tint_top', 2, 2, 13.5, 12, 12, 0.5, part='level_fluid')


def pdock(m):
    shell(m)
    tier(m)
    # A violet ring with transfer arrows on the faces.
    frame(m, 'glow_portal', 3, 3, 10, 10)
    panel(m, 'portal_dark', 4, 4, 8, 8)
    for y in (5, 7.5, 10):
        panel(m, 'glow_portal_light', y, 6.5, 1, 1, depth=0.5)
        panel(m, 'glow_portal_light', y + 0.5, 7.5, 1, 1, depth=0.5)
        panel(m, 'glow_portal_light', y, 8.5, 1, 1, depth=0.5)
    # On the top, a ring round a vortex that turns.
    m.box('portal', 12, 3, 16.25, 1, 10, 0.75, ring=True)
    m.box('portal_dark', 4, 4, 16.25, 8, 8, 0.25)
    m.pivot('vortex', 8, 8, 16.5)
    for a in (0, 60, 120):
        m.box('glow_portal', 4.5, 7.5, 16.5, 7, 1, 0.3, part='vortex', tilt=('Z', a), pivot=(8, 8, 16.5))
    m.box('glow_portal_light', 7, 7, 16.5, 2, 2, 0.5, part='vortex', tilt=('Z', 45), pivot=(8, 8, 16.5))


MODELS = {'node': node, 'node_hub': node_hub, 'station': station, 'pipe': pipe, 'rail': rail, 'capsule': capsule, 'worker': worker, 'tank': tank, 'pdock': pdock}
