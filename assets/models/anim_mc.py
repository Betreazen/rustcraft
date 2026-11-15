"""The moving parts of the block models, for the game's block-entity renderer (slice 12).

The guide moves these parts in docs/showcase/js/poses.js; this table says the same in numbers the
game reads. export_mc.py leaves every part named here out of the block's model, writes it as a
standalone model `rustcraft:block/<model>__<part>` (API 151) and writes the table as Rust for the
rules crate (mod/rustcraft/rules/src/anim/parts.rs), which the client draws each frame.

A part: {'on': [motion, ...], 'off': 'rest' | 'hide' | [motion, ...], 'show': rule, 'carry': part,
'level': True}. Motions are applied in order, each about the part's pivot (m.pivot in the model
files; the middle of the block when none): ('rot', axis, base, amp, shape, freq, phase) turns by
base + amp * shape(freq * t + phase) radians; ('move', (x, y, z), base, amp, shape, freq, phase)
slides by that many pixels along the vector. Axes and vectors are the model's (z up). Shapes:
'lin' (the argument itself), 'sin', 'pos' (max(0, sin)), 'pulse' (0.5 + 0.5 sin), 'cos01'
(0.5 - 0.5 cos), 'saw' (the fraction), 'chop' (the butcher's wind-up and strike over 1.4 s),
'level' (the block's level 0..1). 'off' is what it does while the block stands: 'rest' draws it
still, 'hide' draws nothing, a list moves it so. 'always' blocks work all the time in the guide
(docks, portal dock, windmill). 'show' draws a part only some of the time: ('below', rate,
modulo, index) while index < floor(rate t) mod modulo (the autocrafter's lamps), ('eq', rate,
modulo, index) while floor(rate t) mod modulo == index (the docks' lamps), ('sin', freq,
threshold) while sin(freq t) > threshold (sparks). 'level' parts stretch upright from their bottom
by the level; 'carry' rides on another part, taking its moves after its own.
"""

LIN, SIN, POS, PULSE, COS01, SAW, CHOP, LEVEL = 'lin', 'sin', 'pos', 'pulse', 'cos01', 'saw', 'chop', 'level'


def spin(axis, speed):
    return ('rot', axis, 0.0, speed, LIN, 1.0, 0.0)


def wave_move(vec, amp, shape, freq, base=0.0, phase=0.0):
    return ('move', vec, base, amp, shape, freq, phase)


def wave_rot(axis, amp, shape, freq, base=0.0, phase=0.0):
    return ('rot', axis, base, amp, shape, freq, phase)


def still_rot(axis, angle):
    return ('rot', axis, angle, 0.0, LIN, 0.0, 0.0)


def still_move(vec, px):
    return ('move', vec, px, 0.0, LIN, 0.0, 0.0)


X, Y, Z = (1, 0, 0), (0, 1, 0), (0, 0, 1)


def flame(amp, freq):
    return {'on': [wave_move(Z, amp, SIN, freq)], 'off': 'hide'}


FIRE = {'on': [], 'off': 'hide'}

ANIM = {
    'furnace': {'fire': FIRE, 'flame': flame(0.5, 17)},
    'alloy': {'fire': FIRE, 'flame': flame(0.5, 17)},
    'gen_fuel': {'fire': FIRE, 'flame': flame(0.5, 17)},
    'trash': {'fire': FIRE, 'flame': flame(0.75, 15)},
    'crusher': {'roll_a': {'on': [spin('x', -5)], 'off': 'rest'}, 'roll_b': {'on': [spin('x', 5)], 'off': 'rest'}},
    'crafter': dict(('lit%d' % i, {'on': [], 'off': 'hide', 'show': ('below', 4, 11, i)}) for i in range(9)),
    'mixer': {'stirrer': {'on': [spin('z', 4)], 'off': 'rest'}},
    'placer': {'piston': {'on': [wave_move(X, 4, COS01, 3)], 'off': 'rest'}},
    'breaker': {'drill': {'on': [spin('x', 14)], 'off': 'rest'}},
    'pump': {'plunger': {'on': [wave_move(Z, 2.25, PULSE, 4, base=-1)], 'off': 'rest'}},
    'miner': {'scanner': {'on': [spin('z', 2.5)], 'off': 'rest'}, 'drill': {'on': [spin('z', 12)], 'off': 'rest'}},
    'disintegrator': {
        'cutter': {'on': [spin('z', 9)], 'off': 'rest'},
        'half_a': {'on': [wave_move((0, -1, 0), 0.6, PULSE, 3, base=0.4)], 'off': 'rest'},
        'half_b': {'on': [wave_move(Y, 0.6, PULSE, 3, base=0.4)], 'off': 'rest'},
        'beam': {'on': [], 'off': 'hide'},
    },
    'windmill': {'rotor': {'on': [spin('y', 2.4)], 'off': 'always'}},
    'battery': {'level_charge': {'on': [], 'off': 'rest', 'level': True}},
    'tank': {'level_fluid': {'on': [], 'off': 'rest', 'level': True, 'tinted': True}},
    'dock_unload': dict(('lit%d' % i, {'on': [], 'off': 'always', 'show': ('eq', 3, 3, i)}) for i in range(3)),
    'dock_load': dict(('lit%d' % i, {'on': [], 'off': 'always', 'show': ('eq', 3, 3, i)}) for i in range(3)),
    'pdock': {'vortex': {'on': [spin('z', 3)], 'off': 'always'}},
    'turret': {'aim_pitch': {'on': [wave_move((-1, 0, 0), 1, POS, 9)], 'off': 'rest', 'carry': 'aim_yaw'},
               'aim_yaw': {'on': [], 'off': 'rest'}},
    'repair': {
        'wheel': {'on': [spin('y', 14)], 'off': 'rest'},
        'clamp': {'on': [still_move((0, -1, 0), 0.75)], 'off': 'rest'},
        'sparks': {'on': [], 'off': 'hide', 'show': ('sin', 19, -0.4)},
    },
    'kitchen': {
        'lid': {'on': [wave_move(Z, 0.5, POS, 9)], 'off': 'rest'},
        'steam': {'on': [wave_move(Z, 2, SAW, 0.8)], 'off': 'hide'},
    },
    'fishery': {
        'winch': {'on': [wave_rot('y', 3, COS01, 1.2)], 'off': 'rest'},
        'line': {'on': [wave_move(Z, -3.5, COS01, 1.2), wave_move(Z, 0.3, SIN, 5)], 'off': 'rest'},
    },
    'feeder': {'auger': {'on': [spin('z', 3)], 'off': 'rest'}, 'level_feed': {'on': [], 'off': 'rest', 'level': True}},
    'shearer': {
        'arm': {'on': [wave_rot('z', 0.55, SIN, 1.5)], 'off': 'rest'},
        'blade_a': {'on': [wave_rot('x', -0.3, PULSE, 10)], 'off': 'rest', 'carry': 'arm'},
        'blade_b': {'on': [wave_rot('x', 0.3, PULSE, 10)], 'off': 'rest', 'carry': 'arm'},
        'level_milk': {'on': [], 'off': 'rest', 'level': True},
    },
    'apiary': {
        'bees': {'on': [spin('z', 2.2), wave_move(Z, 0.75, SIN, 5)], 'off': 'hide'},
        'level_honey': {'on': [], 'off': 'rest', 'level': True},
    },
    'brewer': {
        'bubbles': {'on': [wave_move(Z, 3, LEVEL, 0, base=-3), wave_move(Z, 0.6, PULSE, 6)], 'off': 'hide'},
        'level_brew': {'on': [], 'off': 'rest', 'level': True},
    },
    'xp_tank': {
        'orb': {'on': [spin('z', 1.5), wave_move(Z, 0.75, SIN, 2.5)], 'off': 'rest'},
        'level_xp': {'on': [], 'off': 'rest', 'level': True},
    },
    'enchanter': {
        'book': {'on': [spin('z', 0.7), wave_move(Z, 0.75, SIN, 2)], 'off': 'rest'},
        'page_l': {'on': [wave_rot('x', -0.06, SIN, 6, base=-0.2)], 'off': [still_rot('x', -1.3)], 'carry': 'book'},
        'page_r': {'on': [wave_rot('x', 0.06, SIN, 6, base=0.2)], 'off': [still_rot('x', 1.3)], 'carry': 'book'},
        'ring': {'on': [spin('z', -1.2), wave_move(Z, 0.5, SIN, 2, phase=1)], 'off': 'hide'},
    },
    'archeo': {'brush': {'on': [wave_move(X, 3.5, SIN, 3)], 'off': 'rest'}},
    'projector': {
        'holo': {'on': [spin('z', 1.2), wave_move(Z, 0.5, SIN, 2)], 'off': 'hide'},
        'cone': {'on': [], 'off': 'hide'},
    },
    'builder': {
        'bridge': {'on': [wave_move(X, 4, SIN, 1.3)], 'off': 'rest'},
        'head': {'on': [wave_move(Y, 4, SIN, 2.1)], 'off': 'rest', 'carry': 'bridge'},
    },
    'butcher': {'arm': {'on': [wave_rot('y', 1, CHOP, 1)], 'off': 'rest'}},
    'vein': {'core': {'on': [spin('z', 0.8)], 'off': 'rest'}},
}


def parts_of(model):
    return ANIM.get(model, {})
