'use strict';
// How the Blender models answer to the state of their objects. The fields of a pose are listed
// in player.js; part names come from the model files in assets/models. A block that works (on,
// busy, fed) glows and moves; a block that stands keeps still. Blocks without such a state in
// the guide (docks, terminal, portal dock, windmill) always work.
//
// Runtime props. Besides the poses below, player.js moves parts by their names after props a
// scene sets on the object itself, with s.put(id, type, x, y, z, props) or s.set(id, props):
//   aim    where the object looks: a point [x, y, z] in scene numbers (block corners are whole
//          numbers, so the middle of the block at 5, 1, 0 is [5.5, 1.5, 0.5]); or the id of
//          another object of the scene, which is then followed to its centre as it moves; or a
//          side 'x+', 'x-', 'y+', 'y-' (a point four blocks out). Parts aim_yaw* turn about the
//          vertical through their origin until their x+ looks at it; parts aim_pitch* ride on the
//          aim_yaw part with the same ending (aim_pitch on aim_yaw, aim_pitch_gun on aim_yaw_gun)
//          and tilt up or down to it, from -0.6 to 1.2 radians. No aim: they turn back to x+.
//          Models: turret (aim_yaw, aim_pitch), worker with tool 'gun' (aim_yaw_gun,
//          aim_pitch_gun), with tool 'saw' (aim_yaw_saw; the saw has no pitch) and with tool
//          'scoop' (aim_yaw_scoop; no pitch either).
//   reach  0..1: parts reach_* slide out along x+ of their model, or of the part they ride on,
//          by reach × 0.5 block. Worker saw: reach_saw carries the disc saw_disc; worker scoop:
//          reach_scoop carries the bucket scoop_bucket.
//   level  0..1: parts level_* stretch upright from their bottom, 0 empty and 1 full; their
//          colours stay as the pose tints them. Without a level the pose gives one (tank 0.75,
//          battery 0.6). Models: tank (level_fluid), battery (level_charge, on all four sides),
//          feeder (level_feed, 0.7), shearer (level_milk, 0.6), apiary (level_honey, 0.6),
//          brewer (level_brew, 0.7), xp_tank (level_xp, 0.65).
//   link   the sides where a housing touches the block, as ['x-', 'z-'] or 'x-z-': the
//          connector part link_<side> shows on each of them and no other. Models: crystal (all
//          six sides), windmill and solar (x-, x+, y-, y+, z-).
//   ease   seconds, 0.35 by default: how fast aim, reach and level follow a new value. They
//          glide from step to step as well; an object seen for the first time starts at its value.
// RC.CARRY lists the parts that ride on another part beyond the aim_pitch rule: { type: { part:
// carrier } }. A carried part takes every move of its carrier after its own.
// Gear worn or held by the player (armour, jetpack, zipline handle, and the weapons blade,
// eshield, rifle and axe) is a type of its own put in the player's cell. It faces like the
// figure (face, else the way it moves) and follows the figure's arms: give it the same face,
// arms and swing as the player (arms: 'down', 'forward', 'hang' or [right, left] quarter turns
// forward; swing: true strikes with the right arm; see RC.armTurns in models.js). The blade, the
// rifle and the axe sit in the right fist, the shield on the left one.
(function () {
  const P = RC.POSES, TIER = RC.TIER_COLOR, CH = RC.CH_COLOR, shade = RC.shade;
  RC.CARRY = {
    worker: { reach_saw: 'aim_yaw_saw', saw_disc: 'reach_saw', reach_scoop: 'aim_yaw_scoop', scoop_bucket: 'reach_scoop' },
    shearer: { blade_a: 'arm', blade_b: 'arm' },
    enchanter: { page_l: 'book', page_r: 'book' },
    builder: { head: 'bridge' },
  };
  const HIDE = { hide: true };
  const pulse = (t, speed) => 0.5 + 0.5 * Math.sin(t * (speed || 4));
  const flicker = (t) => 0.5 + 0.3 * Math.sin(t * 13) + 0.2 * Math.sin(t * 7.3);
  const px = (n) => n / 16;

  // A machine above the first tier wears a stripe in the colour of its alloy, as the boxes do.
  function machine(o, pose) {
    pose.tint = TIER[o.tier || 1];
    if (!(o.tier > 1)) (pose.parts = pose.parts || {}).tier = HIDE;
    return pose;
  }

  // ---- the first models
  P.crystal = (o, t) => ({ glow: 0.1 + 0.2 * pulse(t, 3) });
  P.node = (o, t) => ({ glow: o.bad ? 0 : 0.1 + 0.25 * pulse(t, 3), tint: o.bad ? '#e0564a' : TIER[o.tier || 1] });
  // The junction of the last tier: the rotors in its six windows turn while the line works.
  P.node_hub = (o, t) => {
    const c = o.bad ? '#e0564a' : TIER[o.tier || 4], a = o.bad ? 0 : t * 1.6;
    return {
      glow: o.bad ? 0 : 0.15 + 0.25 * pulse(t, 3),
      tint: c,
      tints: { light: shade(c, 0.35) },
      parts: { rotor_x: { axis: 'x', angle: a }, rotor_y: { axis: 'y', angle: a }, rotor_z: { axis: 'z', angle: a } },
    };
  };
  P.turbine = (o, t) => ({ glow: o.on ? 0.1 + 0.25 * pulse(t, 5) : 0 });
  // Recoil: a barrel that fires kicks back along itself. It aims by the aim prop (see above).
  const recoil = (t, length) => ({ move: [-px(length) * Math.max(0, Math.sin(t * 9)), 0, 0] });
  // A turret in support mode (support: true) does not fire: its eye glows slow and steady.
  P.turret = (o, t) => ({ glow: o.on ? 0.2 + 0.4 * pulse(t, 10) : o.support ? 0.2 + 0.2 * pulse(t, 3) : 0, parts: o.on ? { aim_pitch: recoil(t, 1) } : {} });
  P.station = (o, t) => ({ glow: 0.08 + 0.1 * pulse(t, 2) });
  P.anchor = (o, t) => ({ glow: o.on ? 0.1 + 0.3 * pulse(t, 3) : 0 });

  // ---- trunk and transport
  // Housing: an arm to every connected side, four channel strips coloured by the slots.
  const ARMS = ['x-', 'x+', 'y-', 'y+', 'z-', 'z+'];
  P.pipe = (o) => {
    const ch = ((o.ch || '') + '----').slice(0, 4), con = o.con || '';
    const parts = {};
    for (const a of ARMS) if (!con.includes(a)) parts[a] = HIDE;
    return { glow: o.bad ? 0 : 0.12, dim: o.bad ? 0.45 : 0, tints: { 0: CH[ch[0]], 1: CH[ch[1]], 2: CH[ch[2]], 3: CH[ch[3]] }, parts };
  };

  // Monorail: the model is a floor rail with a half to every side. On the ceiling it is turned
  // over about x, which swaps y- and y+; on a wall it stands on the wall and runs straight.
  const HALVES = ['x-', 'x+', 'y-', 'y+'];
  const opposite = (s) => s[0] + (s[1] === '+' ? '-' : '+');
  // A corner (exactly two halves at a right angle) is drawn as one rounded arc instead. A floor
  // rail with slope 'x+', 'x-', 'y+' or 'y-' is the climbing piece: it rises one block over its
  // cell towards that side, from the floor of its cell to the floor of the cell above the next
  // one, so the next rail stands one block higher; it ignores con.
  const ARCS = ['arc_x+y+', 'arc_y+x-', 'arc_x-y-', 'arc_y-x+'];
  const CLIMB = { 'x+': 0, 'y+': Math.PI / 2, 'x-': Math.PI, 'y-': -Math.PI / 2 };
  P.rail = (o) => {
    const col = TIER[o.tier || 1], dir = o.dir || 'x';
    const pose = { tint: col, tints: { light: shade(col, 0.3) }, parts: {} };
    for (const a of ARCS) pose.parts[a] = HIDE;
    if ((!o.face || o.face === 'floor') && o.slope in CLIMB) {
      pose.turn = [['z', CLIMB[o.slope]]];
      for (const h of HALVES.concat('body')) pose.parts[h] = HIDE;
      return pose;
    }
    pose.parts.slope = HIDE;
    let show;
    if (o.face === 'wy' || o.face === 'wx') {
      pose.turn = [o.face === 'wy' ? ['x', -Math.PI / 2] : ['y', Math.PI / 2]];
      // Standing up, the halves along y (on the y- wall) or along x (on the x- wall) run up.
      const along = o.face === 'wy' ? (dir === 'x' ? 'x' : 'y') : dir === 'y' ? 'y' : 'x';
      show = [along + '-', along + '+'];
    } else {
      show = o.con && o.con.length ? o.con : dir === 'x' ? ['x-', 'x+'] : ['y-', 'y+'];
      if (show.length === 1) show = [show[0], opposite(show[0])];
      if (o.face === 'ceil') {
        pose.turn = [['x', Math.PI]];
        show = show.map((s) => (s[0] === 'y' ? opposite(s) : s));
      }
      const arc = show.length === 2 && show[0][0] !== show[1][0] && ARCS.find((a) => a.includes(show[0]) && a.includes(show[1]));
      if (arc) {
        delete pose.parts[arc];
        show = [];
        pose.parts.body = HIDE;
      }
    }
    for (const h of HALVES) if (!show.includes(h)) pose.parts[h] = HIDE;
    return pose;
  };

  // Carts face where they travel, x+ while standing. The bracket for the mount is the only one
  // shown; wall brackets are turned back, so they keep to their wall whichever way the cart faces.
  const heading = (o) => (o.hx == null && !o.hy ? 0 : Math.atan2(o.hy || 0, o.hx || 0));
  const MOUNTS = ['below', 'above', 'side', 'sidex'];
  function cart(o, yaw) {
    const mount = o.tag || o.mount || 'below', parts = {};
    for (const m of MOUNTS) parts[m] = m !== mount ? HIDE : m === 'side' || m === 'sidex' ? { axis: 'z', angle: -yaw } : null;
    return parts;
  }
  P.capsule = (o, t) => {
    const yaw = heading(o);
    return { glow: 0.12 + 0.06 * pulse(t, 2), alpha: o.alpha, turn: [['z', yaw]], parts: cart(o, yaw) };
  };
  // The saw, the scoop and the gun turn to the aim prop; the saw and the scoop slide out by reach
  // (see above).
  const TOOLS = {
    saw: ['saw', 'aim_yaw_saw', 'reach_saw', 'saw_disc'], reaper: ['reaper', 'reaper_reel'], hold: ['hold'],
    grab: ['grab', 'grab_left', 'grab_right'], bore: ['bore', 'bore_bit'], gun: ['gun', 'aim_yaw_gun', 'aim_pitch_gun'],
    scoop: ['scoop', 'aim_yaw_scoop', 'reach_scoop', 'scoop_bucket'], map: ['map', 'map_drum'],
  };
  // How a tool moves while the worker is busy.
  const WORK = {
    saw: (t) => ({ saw_disc: { axis: 'y', angle: t * 14 } }),
    reaper: (t) => ({ reaper_reel: { axis: 'y', angle: t * 5 } }),
    grab: (t) => {
      const s = px(1.25) * pulse(t, 5);
      return { grab_left: { move: [0, s, 0] }, grab_right: { move: [0, -s, 0] } };
    },
    bore: (t) => ({ bore_bit: { axis: 'x', angle: t * 16 } }),
    gun: (t) => ({ aim_pitch_gun: recoil(t, 0.8) }),
    // The bucket digs in and curls back up; the cartographer's drum turns round and round.
    scoop: (t) => ({ scoop_bucket: { axis: 'y', angle: -0.7 * pulse(t, 3) } }),
    map: (t) => ({ map_drum: { axis: 'z', angle: t * 2.5 } }),
  };
  P.worker = (o, t) => {
    const yaw = heading(o), parts = cart(o, yaw);
    for (const k in TOOLS) if (k !== o.tool) for (const name of TOOLS[k]) parts[name] = HIDE;
    if (o.busy && WORK[o.tool]) Object.assign(parts, WORK[o.tool](t));
    return { glow: o.busy ? 0.2 + 0.2 * pulse(t, 8) : 0.1 + 0.08 * pulse(t, 2), turn: [['z', yaw]], parts };
  };

  const FLUID = { water: '#3f76e4', lava: '#f2731c', sol: '#59d38c', milk: '#f4f1e6' };
  // The fluid column is level_fluid; a tank without a level prop is three quarters full.
  P.tank = (o, t) => {
    const c = FLUID[o.fluid];
    if (!c) return { parts: { level_fluid: HIDE } };
    return { glow: o.fluid === 'water' ? 0 : 0.06 + 0.1 * pulse(t, 2), tint: c, tints: { top: shade(c, 0.3) }, level: 0.75 };
  };
  P.pdock = (o, t) => machine(o, { glow: 0.1 + 0.12 * pulse(t, 2), parts: { vortex: { axis: 'z', angle: t * 3 } } });

  // ---- machines
  const fire = (o, t) =>
    machine(o, o.on ? { glow: 0.06 + 0.16 * flicker(t), parts: { flame: { move: [0, 0, px(0.5) * Math.sin(t * 17)] } } } : { parts: { fire: HIDE, flame: HIDE } });
  P.furnace = fire;
  P.alloy = fire;
  P.crusher = (o, t) => machine(o, { parts: o.on ? { roll_a: { axis: 'x', angle: -t * 5 }, roll_b: { axis: 'x', angle: t * 5 } } : {} });
  // The lamps on the top light one after another, then all go out and start again.
  P.crafter = (o, t) => {
    const lit = o.on ? Math.floor(t * 4) % 11 : 0, parts = {};
    for (let i = lit; i < 9; i++) parts['lit' + i] = HIDE;
    return machine(o, { glow: o.on ? 0.2 : 0, parts });
  };
  P.mixer = (o, t) => machine(o, { glow: o.on ? 0.08 + 0.12 * pulse(t, 3) : 0, parts: o.on ? { stirrer: { axis: 'z', angle: t * 4 } } : {} });
  P.charger = (o, t) => machine(o, { glow: o.on ? 0.1 + 0.18 * pulse(t, 6) : 0 });
  P.placer = (o, t) => machine(o, { glow: o.on ? 0.15 + 0.2 * pulse(t, 6) : 0, parts: { piston: { move: [o.on ? px(4) * (0.5 - 0.5 * Math.cos(t * 3)) : 0, 0, 0] } } });
  // The drill looks to x+ as modelled; facing: 'x-' turns it round, as when it faces a placer.
  P.breaker = (o, t) =>
    machine(o, { glow: o.on ? 0.15 + 0.2 * pulse(t, 6) : 0, turn: o.facing === 'x-' ? [['z', Math.PI]] : [], parts: o.on ? { drill: { axis: 'x', angle: t * 14 } } : {} });
  P.pump = (o, t) => machine(o, { glow: o.on ? 0.1 + 0.15 * pulse(t, 5) : 0, parts: { plunger: { move: [0, 0, o.on ? px(-1 + 2.25 * pulse(t, 4)) : 0] } } });
  P.miner = (o, t) =>
    machine(o, { glow: o.on ? 0.06 + 0.12 * pulse(t, 6) : 0, parts: o.on ? { scanner: { axis: 'z', angle: t * 2.5 }, drill: { axis: 'z', angle: t * 12 } } : {} });
  P.vein = (o, t) => ({ glow: o.on ? 0.1 + 0.2 * pulse(t, 2) : 0, parts: o.on ? { core: { axis: 'z', angle: t * 0.8 } } : {} });
  // Utilizer (spec 25): while it burns (on) the fire in its pit and mouths flares and the flames
  // leap; standing, only the dark embers are left.
  P.trash = (o, t) =>
    machine(o, o.on ? { glow: 0.2 + 0.35 * flicker(t), parts: { flame: { move: [0, 0, px(0.75) * Math.sin(t * 15)] } } } : { parts: { fire: HIDE, flame: HIDE } });
  // Disintegrator (spec 25): while on, the cutter turns, the beam shines down onto the pickaxe and
  // the two halves of its head part along y and close again; standing, the beam is off.
  P.disintegrator = (o, t) => {
    if (!o.on) return machine(o, { parts: { beam: HIDE } });
    const gap = px(0.4 + 0.6 * pulse(t, 3));
    return machine(o, {
      glow: 0.2 + 0.25 * pulse(t, 9),
      parts: { cutter: { axis: 'z', angle: t * 9 }, half_a: { move: [0, -gap, 0] }, half_b: { move: [0, gap, 0] } },
    });
  };

  // ---- energy
  P.gen_fuel = fire;
  P.gen_lava = (o, t) => machine(o, { glow: o.on ? 0.08 + 0.18 * pulse(t, 2.5) : 0 });
  // The charge bar is level_charge; without a level prop it shows 0.6.
  P.battery = (o, t) => machine(o, { glow: o.on ? 0.12 + 0.15 * pulse(t, 3) : 0.05, level: 0.6 });
  // A solar panel lets light through (it casts no shadow in the game), so it is drawn see-through.
  P.solar = (o, t) => ({ glow: o.on ? 0.08 + 0.05 * pulse(t, 1.5) : 0, alpha: o.alpha != null ? o.alpha : 0.4 });
  // The connectors of the windmill glow faintly while it is linked.
  P.windmill = (o, t) => ({ glow: o.link && o.link.length ? 0.1 + 0.1 * pulse(t, 3) : 0, parts: { rotor: { axis: 'y', angle: t * 2.4 } } });
  // A charge plate glows while it charges whoever stands on it.
  P.charge_plate = (o, t) => ({ glow: o.on ? 0.2 + 0.3 * pulse(t, 6) : 0.05 });
  P.crystalblock = (o, t) => (o.fed ? { glow: 0.06 + 0.16 * pulse(t, 3) } : { dim: 0.35 });
  P.geode = (o, t) => ({ glow: 0.08 + 0.06 * pulse(t, 2) });

  // ---- storage, docks and the rest
  P.cell = (o, t) => ({ glow: 0.15 + 0.1 * pulse(t, 1.5) });
  P.terminal = (o, t) => machine(o, { glow: 0.1 + 0.08 * pulse(t, 2) });
  // A light runs along the three arrows.
  const dock = (o, t) => {
    const lit = Math.floor(t * 3) % 3, parts = {};
    for (let i = 0; i < 3; i++) if (i !== lit) parts['lit' + i] = HIDE;
    return machine(o, { glow: 0.25, parts });
  };
  P.dock_unload = dock;
  P.dock_load = dock;
  P.dock_charge = (o, t) => machine(o, { glow: 0.08 + 0.15 * pulse(t, 4) });
  P.alloyblock = (o) => {
    const c = TIER[o.tier || 1];
    return { tint: c, tints: { light: shade(c, 0.28), dark: shade(c, -0.38) } };
  };
  P.seed = (o, t) => {
    const parts = {};
    for (let s = 0; s < 3; s++) if (s !== (o.stage || 0)) parts['stage' + s] = HIDE;
    return { glow: 0.1 + 0.08 * pulse(t, 2), parts };
  };
  // A zipline anchor stands in the corner of its cell when `corner` is set; its outline before
  // placing is see-through and still.
  P.zipanchor = (o, t) => {
    const at = o.corner ? 0.32 : 0, move = [at, at, 0];
    return { alpha: o.alpha, parts: { body: { move }, roller: { axis: 'y', angle: o.alpha < 1 ? 0 : t * 3, move } } };
  };

  // ---- blocks for the Path-of-craft-tree branches (spec 22); all of them work while on
  // Repair stand: the grinding wheel turns and throws sparks, the vice clamp closes on the blade.
  P.repair = (o, t) =>
    machine(o, o.on
      ? { glow: 0.15 + 0.2 * flicker(t), parts: { wheel: { axis: 'y', angle: t * 14 }, clamp: { move: [0, -px(0.75), 0] }, sparks: Math.sin(t * 19) > -0.4 ? null : HIDE } }
      : { parts: { sparks: HIDE } });
  // Kitchen machine: the hob and the oven glow, the lid rattles and steam rises from the vent.
  P.kitchen = (o, t) =>
    machine(o, o.on
      ? { glow: 0.12 + 0.18 * pulse(t, 3), parts: { lid: { move: [0, 0, px(0.5) * Math.max(0, Math.sin(t * 9))] }, steam: { move: [0, 0, px(2) * ((t * 0.8) % 1)] } } }
      : { parts: { steam: HIDE } });
  // Fishing station: the winch lets the line down until the bobber dips, then winds it up.
  P.fishery = (o, t) => {
    const dip = o.on ? 0.5 - 0.5 * Math.cos(t * 1.2) : 0;
    return machine(o, {
      glow: o.on ? 0.15 + 0.12 * pulse(t, 2) : 0,
      parts: { winch: { axis: 'y', angle: dip * 3 }, line: { move: [0, 0, -px(3.5) * dip + (o.on ? px(0.3) * Math.sin(t * 5) : 0)] } },
    });
  };
  // Feeder: the stirrer in the hopper turns; the grain in the trough is level_feed.
  P.feeder = (o, t) => machine(o, { glow: o.on ? 0.12 + 0.15 * pulse(t, 3) : 0, level: 0.7, parts: o.on ? { auger: { axis: 'z', angle: t * 3 } } : {} });
  // Shearer-milker: the arm swings to and fro and its shears snip; the milk is level_milk.
  P.shearer = (o, t) => {
    const cut = o.on ? 0.3 * pulse(t, 10) : 0;
    return machine(o, {
      glow: o.on ? 0.12 + 0.15 * pulse(t, 4) : 0,
      level: 0.6,
      parts: { arm: { axis: 'z', angle: o.on ? 0.55 * Math.sin(t * 1.5) : 0 }, blade_a: { axis: 'x', angle: -cut }, blade_b: { axis: 'x', angle: cut } },
    });
  };
  // Apiary collector: bees fly round the hive while it works; the honey is level_honey.
  P.apiary = (o, t) =>
    machine(o, { glow: o.on ? 0.1 + 0.12 * pulse(t, 2) : 0.03, level: 0.6, parts: { bees: o.on ? { axis: 'z', angle: t * 2.2, move: [0, 0, px(0.75) * Math.sin(t * 5)] } : HIDE } });
  // Brewing cube: the brew (level_brew) bubbles while it brews; the bubbles keep to its surface.
  P.brewer = (o, t) => {
    const fill = o.level != null ? Math.max(0, Math.min(1, o.level)) : 0.7;
    const bubbles = o.on && fill > 0.05 ? { move: [0, 0, px(3) * (fill - 1) + px(0.6) * pulse(t, 6)] } : HIDE;
    return machine(o, { glow: o.on ? 0.12 + 0.15 * pulse(t, 3) : 0.04, level: 0.7, parts: { bubbles } });
  };
  // Experience condenser: the fluid is level_xp; the orb over it turns and floats while on.
  P.xp_tank = (o, t) =>
    machine(o, { glow: o.on ? 0.15 + 0.2 * pulse(t, 2.5) : 0.08, level: 0.65, parts: o.on ? { orb: { axis: 'z', angle: t * 1.5, move: [0, 0, px(0.75) * Math.sin(t * 2.5)] } } : {} });
  // Enchanting machine: while on, the book lies open, floats and turns, and the glyph ring turns
  // round it; off, the book stands half closed and the ring is gone.
  P.enchanter = (o, t) => {
    const open = o.on ? 0.2 + 0.06 * Math.sin(t * 6) : 1.3;
    return machine(o, {
      glow: o.on ? 0.2 + 0.2 * pulse(t, 3) : 0,
      parts: {
        book: o.on ? { axis: 'z', angle: t * 0.7, move: [0, 0, px(0.75) * Math.sin(t * 2)] } : null,
        page_l: { axis: 'x', angle: -open },
        page_r: { axis: 'x', angle: open },
        ring: o.on ? { axis: 'z', angle: -t * 1.2, move: [0, 0, px(0.5) * Math.sin(t * 2 + 1)] } : HIDE,
      },
    });
  };
  // Archaeology table: the brush sweeps along x over the sand.
  P.archeo = (o, t) => machine(o, { glow: o.on ? 0.15 + 0.15 * pulse(t, 4) : 0, parts: o.on ? { brush: { move: [px(3.5) * Math.sin(t * 3), 0, 0] } } : {} });
  // Holo projector: the cone of light and the turning hologram show while on.
  P.projector = (o, t) =>
    machine(o, o.on ? { glow: 0.25 + 0.15 * pulse(t, 3), parts: { holo: { axis: 'z', angle: t * 1.2, move: [0, 0, px(0.5) * Math.sin(t * 2)] } } } : { parts: { cone: HIDE, holo: HIDE } });
  // Building machine: the bridge runs along x, the head along y on it, the nozzle glows.
  P.builder = (o, t) =>
    machine(o, { glow: o.on ? 0.15 + 0.25 * flicker(t) : 0, parts: o.on ? { bridge: { move: [px(4) * Math.sin(t * 1.3), 0, 0] }, head: { move: [0, px(4) * Math.sin(t * 2.1), 0] } } : {} });

  // Butcher: while on, the arm winds up slowly and chops down fast with the sword, again and again.
  const CHOP = 1.4;
  const chop = (t) => {
    const ph = (t / CHOP) % 1;
    return ph < 0.7 ? -0.7 * (ph / 0.7) : -0.7 + ((ph - 0.7) / 0.3) * 1.05;
  };
  P.butcher = (o, t) => machine(o, { glow: o.on ? 0.15 + 0.2 * pulse(t, 5) : 0, parts: { arm: { axis: 'y', angle: o.on ? chop(t) : 0 } } });

  // ---- gear worn by the player
  // Gear is drawn over the player figure in the same cell (models.js) and faces the way the
  // figure does, by the same rule: face 'x+', 'x-', 'y+' or 'y-' if set, else the way it last
  // moved, else y+, towards the viewer. The models face x+.
  const FACE_YAW = { 'x+': 0, 'y+': Math.PI / 2, 'x-': Math.PI, 'y-': -Math.PI / 2 };
  const facing = (o) => {
    const moved = o.hx > 0 ? 'x+' : o.hx < 0 ? 'x-' : o.hy > 0 ? 'y+' : o.hy < 0 ? 'y-' : 'y+';
    return [['z', FACE_YAW[o.face in FACE_YAW ? o.face : moved]]];
  };
  // Technical armour wears the colour of its tier (3 by default) and glows while charged (on).
  const armor = (o, t) => {
    const c = TIER[o.tier || 3];
    return { tint: c, tints: { light: shade(c, 0.28), dark: shade(c, -0.38) }, glow: o.on ? 0.15 + 0.15 * pulse(t, 3) : 0.04, alpha: o.alpha, turn: facing(o) };
  };
  P.armor_helmet = armor;
  // The chest piece's sleeves follow the player's arms (`arms` and `swing`, as on the player
  // figure, RC.armTurns in models.js): quarter turns forward about the shoulder. The scene axes are
  // left-handed, so y+ is the right arm of a model facing x+.
  const sleeve = (q) => ({ axis: 'y', angle: (-q * Math.PI) / 2 });
  P.armor_chest = (o, t) => {
    const [right, left] = RC.armTurns(o, t);
    return Object.assign(armor(o, t), { parts: { 'arm_y+': sleeve(right), 'arm_y-': sleeve(left) } });
  };
  P.armor_legs = armor;
  P.armor_boots = armor;
  // The jetpack shows its two flames while flying (flying or on).
  P.jetpack = (o, t) => {
    const lit = o.flying || o.on;
    const flame = (k) => (lit ? { move: [0, 0, -px(0.6) * flicker(t + k)] } : HIDE);
    return { glow: lit ? 0.35 + 0.25 * flicker(t) : 0.05, alpha: o.alpha, turn: facing(o), parts: { flame_l: flame(0), flame_r: flame(1.7) } };
  };
  // The zipline handle: its two rollers roll on the rope while it moves (a path) or while on.
  P.zip_handle = (o, t) => {
    const roll = o.path || o.on ? t * 12 : 0;
    return { glow: o.path || o.on ? 0.2 : 0.05, alpha: o.alpha, turn: facing(o), parts: { roller_a: { axis: 'y', angle: roll }, roller_b: { axis: 'y', angle: roll } } };
  };

  // ---- weapons held by the player
  // A held weapon is a type of its own in the player's cell, as gear is, and is given the same
  // props as the player: face (or the path) to face with the figure, and arms / swing, so it
  // stays in the fist as the arm turns (RC.armTurns). The models are made for a hanging arm. A
  // sword or an axe turns with the arm about the shoulder; a shield or a rifle keeps upright or
  // level and moves with the fist, which hangs 8.5 figure pixels under the shoulder.
  const FIST = (8.5 * 1.8) / 32;
  const fistShift = (q) => {
    const a = (q * Math.PI) / 2;
    return [FIST * Math.sin(a), 0, FIST * (1 - Math.cos(a))];
  };
  // The energy blade (tier 2 by default) wears the colour of its tier on pommel and guard; while
  // charged (on) its blade glows, without charge the blade part is hidden.
  P.blade = (o, t) => {
    const arm = sleeve(RC.armTurns(o, t)[0]);
    return { tint: TIER[o.tier || 2], glow: o.on ? 0.3 + 0.12 * pulse(t, 7) : 0, alpha: o.alpha, turn: facing(o), parts: { hilt: arm, edge: o.on ? arm : HIDE } };
  };
  P.axe = (o, t) => ({ alpha: o.alpha, turn: facing(o), parts: { hilt: sleeve(RC.armTurns(o, t)[0]) } });
  // The energy shield on the left fist: its field shows while charged (on) and flashes while it
  // blocks a hit (hit). With the left arm raised (arms [r, l], l about 0.9) it covers the body,
  // so it also moves in towards the middle as the arm rises.
  P.eshield = (o, t) => {
    const q = RC.armTurns(o, t)[1], move = fistShift(q);
    move[1] = Math.min(1, q) * px(3);
    const at = { move };
    return {
      tint: TIER[o.tier || 2], glow: o.hit ? 0.25 + 0.55 * flicker(t * 1.6) : 0.18 + 0.1 * pulse(t, 3),
      alpha: o.alpha, turn: facing(o), parts: { shield: at, field: o.on ? at : HIDE },
    };
  };
  // The energy rifle in the right fist, level: aimed with arms 'forward'; while it fires (on) the
  // muzzle flashes and its coil glows.
  P.rifle = (o, t) => {
    const at = { move: fistShift(RC.armTurns(o, t)[0]) };
    return { tint: TIER[o.tier || 4], glow: o.on ? 0.35 + 0.4 * flicker(t * 1.4) : 0.12, alpha: o.alpha, turn: facing(o), parts: { gun: at, muzzle: o.on ? at : HIDE } };
  };
})();
