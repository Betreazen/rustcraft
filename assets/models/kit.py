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
    'glow_crystal': '#56dbe2', 'glow_crystal_light': '#b8f6fa', 'glow_crystal_mid': '#8eeaf0', 'glow_crystal_deep': '#3fb8c0',
}


def shell(m, body='case', top=True):
    """The usual machine casing: a dark plinth, a body set half a pixel back between four corner
    posts, and a top slab with a lighter roof plate. top=False leaves the top open down to z 11."""
    m.box(body + '_dark', 0, 0, 0, 16, 16, 1.5)
    m.box(body, 0.5, 0.5, 1.5, 15, 15, 13 if top else 9.5)
    m.box(body, 0, 0, 1.5, 1.5, 1.5, 13 if top else 14.5, ring=True)
    if top:
        m.box(body, 0, 0, 14.5, 16, 16, 1.5)
        m.box(body + '_light', 0.5, 0.5, 16, 15, 15, 0.25)
        return
    # An open top: a rim, walls with a dark lining inside and a dark floor.
    m.box(body, 1.5, 0, 14.5, 13, 1.5, 1.5, ring=True)
    m.box(body + '_light', 0, 0, 16, 1.5, 1.5, 0.25, ring=True)
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


def port(m, rim, centre, z=16.25):
    """A round port on the top: a rim of four bars around a dark centre."""
    m.box(rim, 10, 5, z, 1, 6, 0.6, ring=True)
    m.box(centre, 6, 6, z, 4, 4, 0.3)
