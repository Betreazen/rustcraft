"""Shared palette and parts of the RustCraft block models; build.py turns them into Blender objects.

Sizes are texture pixels, 16 to a block. Machines show their face on all four sides, as the box
textures of the guide do: a feature is drawn on the x+ face and repeated round with ring=True.
"""


def shade(color, f):
    """Lightens a '#rrggbb' colour towards white for f > 0, darkens it towards black for f < 0."""
    rgb = [int(color[i:i + 2], 16) for i in (1, 3, 5)]
    out = [c + (255 - c) * f if f >= 0 else c * (1 + f) for c in rgb]
    return '#' + ''.join('%02x' % max(0, min(255, round(c))) for c in out)


def family(name, color):
    """A colour with its lighter and darker shade, as the frames of the textures use them."""
    return {name: color, name + '_light': shade(color, 0.28), name + '_dark': shade(color, -0.38)}


# The palette comes from the textures and box models in docs/showcase/js/models.js.
CASE, AMBER = '#6b7380', '#ffcf5a'
COLORS = {
    **family('case', CASE),
    'steel': '#8a919d', 'steel_mid': '#5d6470', 'steel_dark': '#3a3f49',
    'plate': '#4a5060', 'plate_dark': '#3d4350', 'gun': '#2b3038', 'black': '#15171c', 'white': '#ffffff',
    'copper': '#c57f52',
    # Recoloured by the guide: the tier of a node, rail or machine stripe.
    'tint': '#c57f52', 'tint_light': shade('#c57f52', 0.28), 'tint_dark': shade('#c57f52', -0.38),
    'glow_lamp': AMBER, 'led_red': '#e0564a', 'led_green': '#59d38c', 'led_yellow': '#ffd24a',
    # 0.21 (spec 40): the light that runs up the side walls of a working machine.
    'glow_run': '#62ff9a',
    'glow_crystal': '#56dbe2', 'glow_crystal_light': '#b8f6fa', 'glow_crystal_mid': '#8eeaf0', 'glow_crystal_deep': '#3fb8c0',
}


def shell(m, body='case', top=True, bay=0.5):
    """The usual machine casing: a dark plinth, a body set `bay` pixels back (half a pixel by
    default) between four corner posts, and a top slab with a lighter roof plate. top=False
    leaves the top open down to z 11."""
    m.box(body + '_dark', 0, 0, 0, 16, 16, 1.5)
    m.box(body, bay, bay, 1.5, 16 - 2 * bay, 16 - 2 * bay, 13 if top else 9.5)
    m.box(body, 0, 0, 1.5, 1.5, 1.5, 13 if top else 14.5, ring=True)
    if top:
        m.box(body, 0, 0, 14.5, 16, 16, 1.5)
        m.box(body + '_light', 0.5, 0.5, 16, 15, 15, 0.25)
        return
    # An open top: a rim, walls with a dark lining inside and a dark floor. The light caps on the
    # posts stand a quarter pixel in from the outer faces, so a block put on top hides their
    # sides instead of fighting them (round 31).
    m.box(body, 1.5, 0, 14.5, 13, 1.5, 1.5, ring=True)
    m.box(body + '_light', 0.25, 0.25, 16, 1.25, 1.25, 0.25, ring=True)
    m.box(body, 1.5, 0.5, 11, 13, 1, 3.5, ring=True)
    m.box(body + '_dark', 1.5, 1.5, 11.25, 13, 0.25, 4.5, ring=True)
    m.box('black', 1.5, 1.5, 11, 13, 13, 0.25)


def tier(m):
    """The stripe a machine above the first tier wears on its top, in the colour of its alloy."""
    m.box('tint', 0, 0, 16, 16, 2, 0.5, part='tier')


def frame(m, mat, y, z, w, h, bar=1, depth=0.75):
    """A rectangle of bars on every side face: w x h pixels with its corner at (y, z) on the x+ face.
    The bars stand out of the body face by depth - 0.5, a quarter pixel past the corner posts."""
    x = 15.5
    m.box(mat, x, y, z, depth, w, bar, ring=True)
    m.box(mat, x, y, z + h - bar, depth, w, bar, ring=True)
    m.box(mat, x, y, z + bar, depth, bar, h - 2 * bar, ring=True)
    m.box(mat, x, y + w - bar, z + bar, depth, bar, h - 2 * bar, ring=True)


def panel(m, mat, y, z, w, h, depth=0.25, part='body'):
    """A flat plate on every side face, standing `depth` out of the body face."""
    m.box(mat, 15.5, y, z, depth, w, h, part=part, ring=True)


def firebox(m, fire, light, ember):
    """A fire window on every side face. The flames are the parts fire and flame; the guide
    hides them while the block stands, and the dark embers behind them stay."""
    frame(m, 'black', 4, 3, 8, 6)
    panel(m, ember, 5, 4, 6, 4)
    panel(m, fire, 5, 4, 6, 2, depth=0.4, part='fire')
    panel(m, fire, 8, 5.5, 1.5, 2.5, depth=0.45, part='flame')
    panel(m, light, 6, 5, 2, 2.5, depth=0.5, part='flame')
    panel(m, light, 9.5, 4.5, 1, 2.5, depth=0.5, part='flame')


FACES = ('x+', 'x-', 'y+', 'y-', 'z+', 'z-')
# The four sides in the order of quarter turns: m.box(..., quarter=n) takes a box on x+ to SIDES[n].
SIDES = ('x+', 'y+', 'x-', 'y-')


def face_box(m, mat, face, a, b, w, h, inset=0.0, depth=0.5, part='body'):
    """A box on one of the six faces of the block: (a, b) is its corner and (w, h) its size on
    the face, in the two other axes in x, y, z order; it is `depth` thick, and its outer side is
    `inset` pixels inside the face of the block (a negative inset stands out of it)."""
    axis = 'xyz'.index(face[0])
    at, size = [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]
    across = [i for i in range(3) if i != axis]
    at[axis], size[axis] = (16 - inset - depth if face[1] == '+' else inset), depth
    at[across[0]], size[across[0]] = a, w
    at[across[1]], size[across[1]] = b, h
    m.box(mat, *at, *size, part=part)


# 0.21 (spec 40, SA1): the status slot on the side walls. A dark slot on the corner post at the
# y- end of every face (the post of the usual shell, 0..1.5) and, as the part `run`, a green light
# that runs up it while the block works (anim_mc RUN). The slot stands from inside the post to 0.13
# out of the face and the light 0.1 further: past the face's frames, clear of the connectors, and
# off the quarter-pixel grid, so none of their faces lies in a plane of the machine's, a
# connector's or a neighbour's (zfight.rs). What shows in the slot stays 0.15 inside it at each
# end, so its top and bottom never meet the slot's.
SLOT_X, SLOT_DEPTH, SLOT_Y, SLOT_W = 15.9, 0.23, 0.2, 1.1
SLOT_Z, SLOT_H = 2.6, 10.8
RUN_LEN = 2.5
# How far the light runs: from just over the slot's floor to as far under its top.
RUN_TRAVEL = SLOT_H - 0.3 - RUN_LEN
# The tallest bar the slot holds (the cell's fill, full).
SLOT_BAR = SLOT_H - 0.3


def status_slot(m):
    """The dark slot on every side face; what shows in it is a part (`run`, or the cell's fill)."""
    m.box('black', SLOT_X, SLOT_Y, SLOT_Z, SLOT_DEPTH, SLOT_W, SLOT_H, ring=True)


def in_slot(m, mat, h, part):
    """A glowing bar in the slot of every side face, `h` pixels tall from just over its floor."""
    m.box(mat, SLOT_X + 0.05, SLOT_Y + 0.15, SLOT_Z + 0.15, SLOT_DEPTH + 0.05, SLOT_W - 0.3, h, part=part, ring=True)


def runner(m):
    """The status slot and its running light (part `run`)."""
    status_slot(m)
    in_slot(m, 'glow_run', RUN_LEN, 'run')


def port(m, rim, centre, z=16.25):
    """A round port on the top: a rim of four bars around a dark centre."""
    m.box(rim, 10, 5, z, 1, 6, 0.6, ring=True)
    m.box(centre, 6, 6, z, 4, 4, 0.3)


# Round 31: the connectors («присоски») a housing of the magistral plugs into. A housing's arm
# ends in a collar 6.5 pixels across (4.75..11.25, transport.py `pipe`); the connector is a bolted
# frame round it with a small energy window under it, in the style of the crystal's, the
# windmill's and the solar panel's connectors (energy.py). One part per side: link_x+, link_y+,
# link_x-, link_y- and link_z+ on the top; the game shows the ones whose side a housing touches
# (rustcraft_rules::connectors), the guide the sides in the object's `link`. Round 32: link_z-
# under the plinth, the top one mirrored, round the collar of a housing's arm that comes up from
# below; the blockstate has no `down` (it would double the states), so the block entity draws it
# (rustcraft_connectors::draw_bottom).
# Its sizes stay off the quarter-pixel grid the machines' faces are drawn on, so no face of a
# connector lies in a plane of the machine's or the housing's (zfight.rs checks it).
LINK_FRAME = ((3.8, 3.8, 8.4, 1), (3.8, 11.2, 8.4, 1), (3.8, 4.8, 1, 6.4), (11.2, 4.8, 1, 6.4))
LINK_BOLTS = (4.05, 11.45)


def links(m):
    """The six connectors of a machine, each its own part."""
    for quarter, side in enumerate(SIDES):
        part = 'link_' + side
        # The frame stands from inside the body to 0.15 out of the face, past the face's frames.
        for y, z, w, h in LINK_FRAME:
            m.box('case_dark', 15.4, y, z, 0.75, w, h, part=part, quarter=quarter)
        for y in LINK_BOLTS:
            for z in LINK_BOLTS:
                m.box('steel', 16.15, y, z, 0.3, 0.5, 0.5, part=part, quarter=quarter)
        m.box('glow_energy', 16.15, 7.1, 4.05, 0.15, 1.8, 0.5, part=part, quarter=quarter)
    # On the top the frame stands on the roof plate.
    for x, y, w, d in LINK_FRAME:
        m.box('case_dark', x, y, 15.85, w, d, 0.87, part='link_z+')
    for x in LINK_BOLTS:
        for y in LINK_BOLTS:
            m.box('steel', x, y, 16.72, 0.5, 0.5, 0.25, part='link_z+')
    m.box('glow_energy', 7.1, 4.05, 16.72, 1.8, 0.5, 0.15, part='link_z+')
    # Under the plinth: the top one mirrored, from inside the plinth to 0.72 below the block, round
    # the collar (z 15..16 of the housing below).
    for x, y, w, d in LINK_FRAME:
        m.box('case_dark', x, y, -0.72, w, d, 0.87, part='link_z-')
    for x in LINK_BOLTS:
        for y in LINK_BOLTS:
            m.box('steel', x, y, -0.97, 0.5, 0.5, 0.25, part='link_z-')
    m.box('glow_energy', 7.1, 4.05, -0.87, 1.8, 0.5, 0.15, part='link_z-')


def linked(draw):
    """`draw` with the connectors of `links` added."""
    def model(m):
        draw(m)
        links(m)
    return model
