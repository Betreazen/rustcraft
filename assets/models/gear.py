"""Gear worn and held by the player: technical armour (helmet, chest, legs, boots), jetpack,
zipline handle, and the energy weapons held in the hands (blade, shield, rifle, and a plain axe).

Gear is drawn over the player model in the same cell, so it is sized to a player of the usual
proportions: 1.8 blocks tall, a figure 32 of its own pixels high, so one pixel of the figure is
U = 1.8 / 32 block, or 0.9 pixel of a block here. The figure stands in the middle of its cell and
faces x+: legs 4 x 4 x 12 (y- and y+ of the middle), body 4 deep, 8 wide and 12 tall on them,
arms 4 x 4 x 12 on both sides of the body and a head of 8 x 8 x 8 on the top. Armour stands a
little out of the figure, as worn armour does. Limbs are parts of their own, with their origin
at the shoulder or the hip, so a pose can swing them: arm_y-, arm_y+, leg_y-, leg_y+, head.
The guide's scene axes are left-handed, so the figure's right arm is the y+ one.
"""
from kit import shade

U = 1.8 / 32 * 16
MID = 8
BODY = (MID - 2 * U, MID + 2 * U)
WIDE = (MID - 4 * U, MID + 4 * U)
HIP, NECK = 12 * U, 24 * U
TOP = 32 * U
SHOULDER = NECK - 2 * U

COLORS = {
    'jet': '#b8c0cc', 'jet_dark': shade('#b8c0cc', -0.35),
    'glow_flame': '#ff9a3c', 'glow_flame_core': '#fff3a8',
    'glow_visor': '#5ad1e6', 'glow_suit': '#7af0ff', 'grip': '#ffa53c',
    'glow_blade': '#62e6f2', 'glow_blade_core': '#e6fdff', 'glow_field': '#4fd2e6', 'glow_field_core': '#b8f6fa',
    'glow_flash': '#f0feff', 'wood': '#8a6a3a', 'wood_dark': shade('#8a6a3a', -0.35),
}
# Fists of the figure facing x+, in pixels of the block: the right one on the y+ side, the left
# one on the y- side, each in the middle of the lowest pixel and a half of the arm. A held item
# is modelled for an arm that hangs down; the guide turns or moves it as the arm swings forward
# about the shoulder, which is 8.5 figure pixels above the fist.
FIST_Z = HIP + 1.5 * U
FIST_R = (MID, MID + 6 * U, FIST_Z)
FIST_L = (MID, MID - 6 * U, FIST_Z)


def span(m, mat, x, y, z, part='body'):
    """A box from x[0] to x[1], y[0] to y[1] and z[0] to z[1], in pixels of the block."""
    m.box(mat, x[0], y[0], z[0], x[1] - x[0], y[1] - y[0], z[1] - z[0], part=part)


def sides(m, mat, x, y, z, part):
    """A box on the y- side of the figure and its mirror image on the y+ side; part is a name
    with {} for the side."""
    span(m, mat, x, y, z, part.format('y-'))
    span(m, mat, x, (16 - y[1], 16 - y[0]), z, part.format('y+'))


def limbs(m, part, x, z):
    """Puts the origin of both parts named `part` ({} for the side) at (x, y, z), y on each side."""
    m.pivot(part.format('y-'), x, MID - 2 * U if part.startswith('leg') else MID - 6 * U, z)
    m.pivot(part.format('y+'), x, MID + 2 * U if part.startswith('leg') else MID + 6 * U, z)


def armor_helmet(m):
    # A shell round the head with a glowing visor across the eyes, a crest in the colour of the
    # tier, sensor pods over the ears and an antenna.
    g = U
    out, back = WIDE[1] + g, WIDE[0] - g
    m.pivot('head', MID, MID, NECK)
    span(m, 'steel', (back, out), (back, out), (TOP, TOP + g), 'head')
    span(m, 'steel_mid', (back, WIDE[0]), (back, out), (NECK + 1.4, TOP), 'head')
    for y in ((back, WIDE[0]), (WIDE[1], out)):
        span(m, 'steel', (WIDE[0], out), y, (NECK + 1.4, TOP), 'head')
    span(m, 'steel', (WIDE[1], out), WIDE, (TOP - 1.6, TOP), 'head')
    span(m, 'glow_visor', (WIDE[1], out - 0.1), WIDE, (TOP - 3.6, TOP - 1.6), 'head')
    span(m, 'steel_mid', (WIDE[1], out), WIDE, (NECK + 0.9, NECK + 2), 'head')
    span(m, 'tint', (back + 0.9, out), (MID - 0.7, MID + 0.7), (TOP + g, TOP + g + 0.6), 'head')
    for y in ((back - 0.4, back), (out, out + 0.4)):
        span(m, 'tint', (MID - 1.5, MID + 1.5), y, (NECK + 2.9, NECK + 5.9), 'head')
    span(m, 'steel_dark', (MID - 3, MID - 2.4), (back - 0.5, back + 0.1), (TOP - 1.8, TOP + 2.7), 'head')


def armor_chest(m):
    # A breastplate with a glowing core, a back plate, a collar and shoulder pads with guards on
    # the forearms.
    g = U
    front, back = BODY[1] + g, BODY[0] - g
    span(m, 'steel', (BODY[1], front), WIDE, (HIP + 1.7, NECK))
    span(m, 'steel_mid', (back, BODY[0]), WIDE, (HIP + 1.7, NECK))
    span(m, 'steel_dark', (back, front), WIDE, (NECK, NECK + 0.7))
    span(m, 'tint', (back, front + 0.1), WIDE, (HIP + 0.9, HIP + 1.7))
    span(m, 'tint', (front, front + 0.2), (MID - 1.6, MID + 1.6), (HIP + 5.6, HIP + 8.8))
    span(m, 'glow_suit', (front, front + 0.35), (MID - 1, MID + 1), (HIP + 6.2, HIP + 8.2))
    for z in (HIP + 2.2, HIP + 3.8):
        span(m, 'steel_mid', (front, front + 0.2), (WIDE[0] + 0.8, WIDE[1] - 0.8), (z, z + 1))
    limbs(m, 'arm_{}', MID, SHOULDER)
    arm = (MID - 8 * U, MID - 4 * U)
    sides(m, 'steel', (back - 0.3, front + 0.3), (arm[0] - g, arm[1]), (SHOULDER - 0.8, NECK + 1), 'arm_{}')
    sides(m, 'tint', (back - 0.3, front + 0.3), (arm[0] - g - 0.2, arm[0] - g), (SHOULDER, SHOULDER + 1), 'arm_{}')
    sides(m, 'steel_mid', (back, front), (arm[0] - g + 0.45, arm[1]), (HIP + 1.7, HIP + 5.7), 'arm_{}')
    sides(m, 'glow_suit', (front, front + 0.2), (arm[0] + 0.8, arm[1] - 0.8), (HIP + 3.2, HIP + 3.8), 'arm_{}')


def armor_legs(m):
    # A belt with a buckle in the colour of the tier, and plates on the thighs and shins with
    # knee guards; the boots cover the feet.
    g = U / 2
    front, back = BODY[1] + g, BODY[0] - g
    span(m, 'steel_dark', (back, front), (WIDE[0] - g, WIDE[1] + g), (HIP - 1.2, HIP + 0.45))
    span(m, 'tint', (front, front + 0.35), (MID - 1, MID + 1), (HIP - 1, HIP + 0.25))
    limbs(m, 'leg_{}', MID, HIP)
    leg, low = (WIDE[0], MID), 4.5
    sides(m, 'steel', (BODY[1], front), leg, (low, HIP - 1.2), 'leg_{}')
    sides(m, 'steel_mid', BODY, (WIDE[0] - g, WIDE[0]), (low, HIP - 1.2), 'leg_{}')
    sides(m, 'steel_mid', (back, BODY[0]), leg, (low, HIP - 1.2), 'leg_{}')
    sides(m, 'tint', (front, front + 0.35), (WIDE[0] + 0.6, MID - 0.6), (low + 1.1, low + 2.9), 'leg_{}')


def armor_boots(m):
    # Boots with thick soles, toe caps in the colour of the tier and small glowing heel vents.
    g = U
    front, back = BODY[1] + g, BODY[0] - g
    limbs(m, 'leg_{}', MID, HIP)
    foot = (WIDE[0] - g, MID - 0.05)
    sides(m, 'black', (back - 0.3, front + 0.6), (foot[0] - 0.2, foot[1]), (0, 0.9), 'leg_{}')
    sides(m, 'steel_dark', (back, front), foot, (0.9, 4.5), 'leg_{}')
    sides(m, 'tint_light', (back, front), foot, (4.5, 4.9), 'leg_{}')
    sides(m, 'tint', (front, front + 0.6), (foot[0] + 0.2, foot[1] - 0.2), (0.9, 2.6), 'leg_{}')
    sides(m, 'glow_suit', (back - 0.4, back), (foot[0] + 1.4, foot[1] - 1.4), (1.3, 2.3), 'leg_{}')


def jetpack(m):
    # A pack on the back (x-) with two tanks, a nozzle under each and the flames flame_l (y-)
    # and flame_r (y+), which the guide shows while flying. Straps run over the shoulders.
    back = BODY[0] - U
    span(m, 'steel_mid', (back - 2.3, back), (MID - 2.8, MID + 2.8), (13.5, 20.5))
    span(m, 'glow_suit', (back - 2.45, back - 2.3), (MID - 0.6, MID + 0.6), (17, 19))
    for y, flame in (((MID - 4.1, MID - 1.1), 'flame_l'), ((MID + 1.1, MID + 4.1), 'flame_r')):
        x = (back - 3.3, back - 0.7)
        c = (y[0] + y[1]) / 2
        span(m, 'jet', x, y, (12.5, 21.5))
        span(m, 'tint', (x[0] + 0.3, x[1] - 0.3), (y[0] + 0.3, y[1] - 0.3), (21.5, 22.2))
        span(m, 'jet_dark', (x[0] - 0.1, x[1] + 0.1), (y[0] - 0.1, y[1] + 0.1), (15.5, 16.2))
        span(m, 'steel_dark', (x[0] + 0.3, x[1] - 0.3), (c - 1, c + 1), (11, 12.5))
        span(m, 'black', (x[0] + 0.6, x[1] - 0.6), (c - 0.7, c + 0.7), (10.8, 11))
        mid = (x[0] + x[1]) / 2
        m.pivot(flame, mid, c, 10.8)
        span(m, 'glow_flame_core', (mid - 0.9, mid + 0.9), (c - 0.9, c + 0.9), (9.3, 10.8), flame)
        span(m, 'glow_flame', (mid - 0.6, mid + 0.6), (c - 0.6, c + 0.6), (5.3, 9.3), flame)
        span(m, 'glow_flame', (mid - 0.3, mid + 0.3), (c - 0.3, c + 0.3), (3.8, 5.3), flame)
    for y in ((MID - 2.8, MID - 2), (MID + 2, MID + 2.8)):
        span(m, 'black', (back, BODY[1] + U + 0.1), y, (NECK, NECK + 0.6))
        span(m, 'black', (BODY[1] + U, BODY[1] + U + 0.3), y, (15, NECK))


def zip_handle(m):
    # The rope runs along x through y 8 at z 13. Two rollers ride on it, roller_a and roller_b,
    # turning about y; a side plate holds their axles and a hanger with a bridge under the rope
    # carries the grip the player holds, along y at the bottom of the cell.
    for part, x in (('roller_a', 4.5), ('roller_b', 11.5)):
        centre = (x, 8, 14.75)
        m.pivot(part, *centre)
        for a in (0, 45):
            m.box('zip_roller', x - 1.5, 7.25, 13.25, 3, 1.5, 3, part=part, tilt=('Y', a), pivot=centre)
        m.box('steel_dark', x - 0.5, 6.75, 14.25, 1, 2.5, 1, part=part)
    m.box('steel_mid', 3.5, 9.25, 14, 9, 0.75, 1.5)
    m.box('steel_mid', 7, 9.25, 11.5, 2, 0.75, 2.5)
    m.box('glow_lamp', 7.5, 10, 14.25, 1, 0.25, 1)
    m.box('steel', 7.25, 7.25, 11.5, 1.5, 2.75, 1)
    m.box('steel', 7.25, 7.25, 2.5, 1.5, 1.5, 9)
    m.box('grip', 6.75, 4, 0.75, 2.5, 8, 1.75)
    for y in (3.5, 12):
        m.box('steel_dark', 6.5, y, 0.5, 3, 0.5, 2.25)


def along(m, mat, fist, x, half_y, half_z, part, tilt=None, dz=0):
    """A box along x from fist[0] + x[0] to fist[0] + x[1], centred on the fist across and up
    (raised by dz), and turned by tilt about the fist."""
    fx, fy, fz = fist
    m.box(mat, fx + x[0], fy - half_y, fz + dz - half_z, x[1] - x[0], 2 * half_y, 2 * half_z,
          part=part, tilt=tilt, pivot=fist)


# A sword or an axe in a hanging fist points forward and up by this much.
HELD_TILT = ('Y', -40)


def swung(m, parts):
    """Puts the origin of the parts at the right shoulder, so the guide turns them with the arm."""
    for part in parts:
        m.pivot(part, MID, FIST_R[1], SHOULDER)


def blade(m):
    # The energy blade in the right fist. The hilt part has the pommel and the guard in the colour
    # of the tier, the grip, a charge lamp on the guard and the emitter; the edge part is the
    # glowing blade with a white core, which the guide hides while the blade has no charge.
    swung(m, ('hilt', 'edge'))
    t = HELD_TILT
    along(m, 'tint', FIST_R, (-3.6, -2.4), 0.75, 0.75, 'hilt', t)
    along(m, 'black', FIST_R, (-2.4, 2.1), 0.5, 0.5, 'hilt', t)
    along(m, 'tint', FIST_R, (2.1, 3.0), 0.65, 2.0, 'hilt', t)
    along(m, 'glow_suit', FIST_R, (2.25, 2.85), 0.75, 0.3, 'hilt', t, dz=1.2)
    along(m, 'steel', FIST_R, (3.0, 4.1), 0.6, 0.85, 'hilt', t)
    along(m, 'steel_dark', FIST_R, (4.1, 4.5), 0.5, 0.7, 'hilt', t)
    along(m, 'glow_blade', FIST_R, (4.5, 12.6), 0.3, 1.05, 'edge', t)
    along(m, 'glow_blade', FIST_R, (12.6, 13.6), 0.3, 0.55, 'edge', t)
    along(m, 'glow_blade_core', FIST_R, (4.5, 12.3), 0.38, 0.3, 'edge', t)


def axe(m):
    # A plain axe in the right fist, for the guide's scenes with the shield: a wooden handle and a
    # steel head on its top side, with a light edge.
    swung(m, ('hilt',))
    t = HELD_TILT
    along(m, 'wood', FIST_R, (-2.5, 8.0), 0.45, 0.45, 'hilt', t)
    along(m, 'wood_dark', FIST_R, (-2.9, -2.5), 0.5, 0.5, 'hilt', t)
    along(m, 'steel', FIST_R, (5.2, 7.6), 0.35, 1.7, 'hilt', t, dz=1.3)
    along(m, 'jet', FIST_R, (4.5, 5.2), 0.3, 2.1, 'hilt', t, dz=1.5)


def eshield(m):
    # The energy shield on the left fist, its face to x+ just in front of the fist. The shield
    # part is a steel body in the shape of a heater shield with a frame in the colour of the tier,
    # a handle to the fist and an emitter in the middle; the field part is the glowing face, which
    # the guide hides while the shield has no charge. Both move with the fist.
    for part in ('shield', 'field'):
        m.pivot(part, *FIST_L)
    fx, fy, fz = FIST_L
    back = fx + 2 * U + 0.6
    face = back + 0.8
    top = fz + 6.5
    # (bottom, top, half width) of the body, from the top down to the point.
    rows = ((top - 9, top, 4.6), (top - 10, top - 9, 4.0), (top - 11, top - 10, 3.2),
            (top - 12, top - 11, 2.2), (top - 12.8, top - 12, 1.1))
    # The field fills the frame: 0.9 in from the sides and the top, 0.6 up from the point.
    low = top - 11.4
    for z0, z1, half in rows:
        span(m, 'steel_mid', (back, face), (fy - half, fy + half), (z0, z1), 'shield')
        for y in ((fy - half, fy - half + 0.9), (fy + half - 0.9, fy + half)):
            span(m, 'tint', (face, face + 0.2), y, (z0, z1), 'shield')
        inner = half - 0.9
        if inner > 0.3:
            span(m, 'glow_field', (face, face + 0.25), (fy - inner, fy + inner), (max(z0, low), min(z1, top - 0.9)), 'field')
    span(m, 'tint', (face, face + 0.2), (fy - 4.6, fy + 4.6), (top - 0.9, top), 'shield')
    for z in (top - 3.2, top - 7.4):
        span(m, 'glow_field_core', (face + 0.25, face + 0.32), (fy - 3.2, fy + 3.2), (z, z + 0.35), 'field')
    mid = top - 5.3
    span(m, 'steel_dark', (face, face + 0.55), (fy - 1.1, fy + 1.1), (mid - 1.1, mid + 1.1), 'shield')
    span(m, 'glow_field_core', (face + 0.55, face + 0.75), (fy - 0.5, fy + 0.5), (mid - 0.5, mid + 0.5), 'field')
    span(m, 'black', (fx + 1.0, back), (fy - 0.5, fy + 0.5), (fz - 0.6, fz + 0.6), 'shield')


def rifle(m):
    # The energy rifle in the right fist, level and pointing forward: a grip in the fist, a
    # receiver with a glowing coil along both sides, a stock and barrel rings in the colour of the
    # tier, a scope with a lens, and the barrel. The muzzle part is the flash at the end of the
    # barrel, which the guide shows while the rifle fires. Both move with the fist.
    for part in ('gun', 'muzzle'):
        m.pivot(part, *FIST_R)
    fx, fy, fz = FIST_R

    # Lengths along x are drawn at 0.65: the whole rifle is 18 pixels long, a little over a block.
    def piece(mat, x, half_y, z, part='gun'):
        span(m, mat, (fx + 0.65 * x[0], fx + 0.65 * x[1]), (fy - half_y, fy + half_y), (fz + z[0], fz + z[1]), part)

    piece('steel_dark', (-0.8, 0.8), 0.55, (-1.2, 2.0))
    piece('steel_mid', (1.2, 2.6), 0.35, (0.8, 2.0))
    piece('steel', (-5.5, 7.5), 1.0, (2.0, 4.2))
    piece('steel_mid', (-5.5, 7.5), 1.05, (2.0, 2.45))
    piece('glow_suit', (-3.5, 6.0), 1.12, (2.9, 3.5))
    piece('tint', (-10.0, -5.5), 0.85, (0.6, 4.0))
    piece('tint_dark', (-10.4, -10.0), 0.9, (0.4, 4.2))
    piece('steel_dark', (7.5, 15.0), 0.55, (2.6, 3.8))
    for x in ((11.5, 12.3), (14.6, 15.4)):
        piece('tint', x, 0.75, (2.4, 4.0))
    piece('black', (1.0, 5.0), 0.45, (4.2, 4.6))
    piece('steel_dark', (0.0, 6.0), 0.65, (4.6, 5.9))
    piece('glow_visor', (6.0, 6.2), 0.45, (4.8, 5.7))
    piece('glow_flash', (15.4, 17.8), 0.4, (2.95, 3.45), 'muzzle')
    piece('glow_flash', (15.4, 16.2), 1.1, (2.1, 4.3), 'muzzle')


MODELS = {
    'armor_helmet': armor_helmet, 'armor_chest': armor_chest, 'armor_legs': armor_legs, 'armor_boots': armor_boots,
    'jetpack': jetpack, 'zip_handle': zip_handle,
    'blade': blade, 'eshield': eshield, 'rifle': rifle, 'axe': axe,
}
