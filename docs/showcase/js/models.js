'use strict';
// Textures and block models for the guide scenes. Every look here is a concept sketch drawn by
// code; final models and textures replace them later. A model is a function (o, t, out) that
// calls out.box(o, dx, dy, dz, w, d, h, top, left, right, extra): a box in block coordinates,
// where top, left and right are a texture name or a colour for the top, the y+ side and the x+
// side, and extra is { alpha, glow, rot }. Blender models (assets/models, posed by poses.js)
// replace the boxes of the mod's blocks in scenes and icons; the boxes stay as the shapes a
// scene is framed by.
(function () {
  const { defTex, texMaker, MODELS: M, shade, rect, px } = RC;

  const CASE = '#6b7380';
  const TIER = (RC.TIER_COLOR = ['', '#c57f52', '#e2bd55', '#69d2d8', '#a184f2']);
  const CH = (RC.CH_COLOR = { i: '#ffa53c', f: '#4c9bf5', e: '#ffe14a', '-': '#15171c' });

  // ---- environment
  // Blocks of the base game wear the game's own textures when local/mc-assets.js is there (mc:
  // a path under assets/minecraft/textures); the rest of a definition is the hand-drawn fallback.
  const STONE = { base: '#8b8d91', noise: 0.1, speck: [['#77797d', 14]] };
  const DIRT = { base: '#8a5f3e', noise: 0.1, speck: [['#70492d', 12]] };
  const FARMLAND = { base: '#5f3c20', noise: 0.08, feat: 'rows' };
  const PLANKS = { base: '#b08a52', noise: 0.06, feat: 'planks' };
  const LOG = { base: '#6b4a2b', noise: 0.08, feat: 'bark' };
  const vanilla = (name, mc, def) => defTex(name, Object.assign({ mc }, def));
  vanilla('stone', 'block/stone', STONE);
  vanilla('deepslate', 'block/deepslate', { base: '#4e4f55', noise: 0.1, speck: [['#3c3d42', 14]] });
  vanilla('dirt', 'block/dirt', DIRT);
  vanilla('grass', 'block/grass_block_top', { base: '#6aa84f', noise: 0.12, speck: [['#588f41', 14]] });
  vanilla('grass_side', 'block/grass_block_side', DIRT);
  vanilla('farmland', 'block/farmland_moist', FARMLAND);
  vanilla('farmland_side', 'block/dirt', FARMLAND);
  vanilla('planks', 'block/oak_planks', PLANKS);
  vanilla('log', 'block/oak_log', LOG);
  vanilla('log_top', 'block/oak_log_top', LOG);
  vanilla('leaves', 'block/oak_leaves', { base: '#3f8f3a', noise: 0.16, speck: [['#2d6e2a', 26]] });
  vanilla('water', 'block/water_still', { base: '#3f76e4', noise: 0.06, speck: [['#6f9bf0', 8]] });
  vanilla('lava', 'block/lava_still', { base: '#f2731c', noise: 0.12, speck: [['#ffd24a', 10]] });
  vanilla('glass', 'block/glass', { base: '#b9d7e4', noise: 0.02, frame: true });
  vanilla('bricks', 'block/bricks', { base: '#9a5a44', noise: 0.08, paint(g) { for (const y of [3, 7, 11, 15]) rect(g, 0, y, 16, 1, '#c9b8a8'); for (const [x, y0] of [[7, 0], [15, 0], [3, 4], [11, 4], [7, 8], [15, 8], [3, 12], [11, 12]]) rect(g, x, y0, 1, 3, '#c9b8a8'); } });
  vanilla('bedrock', 'block/bedrock', { base: '#3a3a3e', noise: 0.25, speck: [['#1e1e22', 20]] });
  vanilla('wool', 'block/white_wool', { base: '#ededed', noise: 0.05 });
  vanilla('ct_top', 'block/crafting_table_top', { base: CASE, frame: true, feat: 'grid', accent: '#e6c15a' });
  vanilla('ct_front', 'block/crafting_table_front', PLANKS);
  vanilla('ct_side', 'block/crafting_table_side', PLANKS);
  // The furnace and the dropper of the base game; their fallback is the look the icons had.
  const FURNACE_FACE = { base: CASE, frame: true, feat: 'flame', accent: '#ff9a3c' };
  vanilla('vf_front', 'block/furnace_front', FURNACE_FACE);
  vanilla('vf_side', 'block/furnace_side', FURNACE_FACE);
  vanilla('vf_top', 'block/furnace_top', STONE);
  vanilla('dropper_front', 'block/dropper_front', { base: CASE, frame: true, feat: 'port', accent: '#59d38c' });
  vanilla('dropper_side', 'block/furnace_side', { base: CASE, frame: true, feat: 'port', accent: '#59d38c' });
  defTex('facade_blank', { base: '#9aa0aa', noise: 0.03, frame: true, feat: 'ring', accent: '#7f8692' });
  const ORES = { coal: '#24252a', iron: '#d8a47a', copper: '#e0824a', gold: '#f5d54a', redstone: '#e53b3b', lapis: '#3558d6', diamond: '#5fe6dd', emerald: '#3fd66a', debris: '#7a4a3a', quartz: '#efe6dc' };
  const ORE_MC = { quartz: 'block/nether_quartz_ore', debris: 'block/ancient_debris_side' };
  for (const k in ORES) vanilla('ore_' + k, ORE_MC[k] || 'block/' + k + '_ore', { base: '#8b8d91', noise: 0.1, speck: [[ORES[k], 16, true]] });

  // ---- mod blocks
  const casing = (extra) => Object.assign({ base: CASE, frame: true }, extra);
  defTex('casing', casing());
  defTex('casing_top', casing({ feat: 'vent' }));
  defTex('m_furnace', casing({ feat: 'flame', accent: '#ff9a3c' }));
  defTex('m_crusher', casing({ feat: 'jaws' }));
  defTex('m_furnace_top', casing({ feat: 'port', accent: '#ff9a3c' }));
  defTex('gen_fuel_top', casing({ feat: ['vent', 'stripe'], accent: '#ff7a2a' }));
  defTex('m_crafter', casing({ feat: 'grid', accent: '#e6c15a' }));
  defTex('m_alloy', { base: '#8a5645', frame: true, feat: 'flame', accent: '#ffb23c' });
  defTex('m_alloy_top', { base: '#8a5645', frame: true, feat: 'port', accent: '#ff9a3c' });
  defTex('m_mixer', casing({ feat: 'window', accent: '#59d38c' }));
  defTex('m_charger', casing({ feat: 'coil', accent: '#ffe14a' }));
  defTex('m_placer', casing({ feat: 'port', accent: '#59d38c' }));
  defTex('m_breaker', casing({ feat: 'port', accent: '#e0564a' }));
  defTex('m_pump', casing({ feat: 'bars' }));
  defTex('m_miner', casing({ feat: 'lens', accent: '#ff5a4a' }));
  defTex('m_miner_top', casing({ feat: 'screen', accent: '#ff8a5a' }));
  defTex('m_vein', { base: '#5a3d6b', noise: 0.12, frame: true, speck: [['#5fe6dd', 3, true], ['#f5d54a', 3, true], ['#e53b3b', 3, true]], feat: 'ring', accent: '#c58aff' });
  defTex('gen_fuel', casing({ feat: 'flame', accent: '#ff7a2a' }));
  defTex('gen_lava', { base: '#5b4a4a', frame: true, feat: 'window', accent: '#ff6a1f' });
  defTex('solar_top', { base: '#27324a', feat: 'cells', accent: '#3f6fd9' });
  defTex('battery', casing({ feat: 'coil', accent: '#59d38c' }));
  defTex('cell', { base: '#59616e', frame: true, feat: 'drawer', accent: '#c9a45a' });
  defTex('terminal', casing({ feat: 'screen', accent: '#5ad1e6' }));
  defTex('station', { base: '#4a5568', frame: true, feat: ['screen', 'stripe'], accent: '#ffcf5a' });
  defTex('dock_unload', casing({ feat: 'arrows', accent: '#ffa53c' }));
  defTex('dock_load', casing({ feat: 'arrows', accent: '#4c9bf5' }));
  defTex('dock_charge', casing({ feat: 'coil', accent: '#ffe14a' }));
  defTex('node', { base: '#3d4658', frame: true, feat: 'lens', accent: '#7af0ff' });
  defTex('anchor', { base: '#3b3550', frame: true, feat: 'ring', accent: '#c58aff' });
  defTex('crystal', { base: '#56dbe2', noise: 0.12, feat: 'facets' });
  defTex('crystal_dim', { base: '#3d8f96', noise: 0.12, feat: 'facets' });
  defTex('geode', { base: '#6d6f76', noise: 0.1, speck: [['#56dbe2', 6, true]] });
  vanilla('obsidian', 'block/obsidian', { base: '#2a2238', noise: 0.12, speck: [['#4a3a6a', 8]] });
  vanilla('netherrack', 'block/netherrack', { base: '#6b2f2f', noise: 0.14, speck: [['#4f2020', 14]] });
  defTex('pdock', casing({ feat: ['ring', 'arrows'], accent: '#b06cf0' }));
  const CHEST = { base: '#a8793f', noise: 0.06, feat: ['planks', 'latch'] };
  defTex('chest', CHEST);
  // Faces of the vanilla chest, cut out of its entity texture. The game draws that texture upside
  // down, so the side faces are flipped back.
  const chestPart = (name, at, flip) => vanilla('chest_' + name, { src: 'entity/chest/normal', at, flip }, CHEST);
  chestPart('lid_top', [28, 0, 14, 14], false);
  chestPart('lid_front', [42, 14, 14, 5], true);
  chestPart('lid_side', [0, 14, 14, 5], true);
  chestPart('base_front', [42, 33, 14, 10], true);
  chestPart('base_side', [0, 33, 14, 10], true);
  chestPart('lock', [1, 1, 2, 4], true);
  defTex('capsule', { base: '#e6e8ee', frame: true, feat: 'window', accent: '#8fd8f0' });
  defTex('chassis', { base: TIER[2], frame: true, feat: 'vent' });
  defTex('turbine', casing({ feat: 'bars', accent: '#4c9bf5', trim: '#4c9bf5' }));
  for (const [k, c] of Object.entries({ water: '#3f76e4', lava: '#f2731c', sol: '#59d38c', empty: '#2b3038' }))
    defTex('tank_' + k, { base: '#9fb4c2', noise: 0.03, frame: true, feat: 'window', accent: c });
  for (let t = 1; t <= 4; t++) defTex('alloy' + t, { base: TIER[t], noise: 0.05, frame: true });

  // Pipe textures show the four channel slots as coloured strips behind small windows.
  // 'pipe:h<slots>' runs the strips along u, 'pipe:v<slots>' along v, 'pipe:c<slots>' is the end cap.
  texMaker('pipe:', (code) => ({
    base: '#5d6470',
    noise: 0.04,
    paint(g) {
      const kind = code[0];
      const slots = (code.slice(1) + '----').slice(0, 4);
      if (kind === 'c') {
        rect(g, 0, 0, 6, 6, '#444a55');
        for (let i = 0; i < 4; i++) rect(g, 1 + (i % 2) * 2, 1 + ((i / 2) | 0) * 2, 2, 2, CH[slots[i]]);
        return;
      }
      for (let i = 0; i < 16; i++) {
        const rib = i % 4 === 3;
        for (let s = 0; s < 4; s++) {
          const col = rib ? '#444a55' : CH[slots[s]];
          if (kind === 'h') px(g, i, 1 + s, col);
          else px(g, 1 + s, i, col);
        }
      }
      if (kind === 'h') {
        rect(g, 0, 0, 16, 1, '#8a919d');
        rect(g, 0, 5, 16, 1, '#3a3f49');
      } else {
        rect(g, 0, 0, 1, 16, '#8a919d');
        rect(g, 5, 0, 1, 16, '#3a3f49');
      }
    },
  }));

  // ---- helpers
  const pulse = (t, speed) => 0.5 + 0.5 * Math.sin(t * (speed || 4));
  const cube = (top, left, right) => (o, t, out) =>
    out.box(o, 0, 0, 0, 1, 1, 1, o.top || top, o.left || left || top, o.right || right || left || top, { glow: o.on ? 0.06 + 0.1 * pulse(t) : 0, alpha: o.alpha });
  // A machine shows its face on both visible sides so it reads from any placement.
  // A machine above the first tier wears a stripe in the colour of its alloy.
  const machine = (front, top) => {
    const base = cube(top || 'casing_top', front, front);
    return (o, t, out) => {
      base(o, t, out);
      if (o.tier > 1) out.box(o, 0, 0, 1, 1, 0.16, 0.08, TIER[o.tier]);
    };
  };

  for (const n of ['stone', 'deepslate', 'dirt', 'planks', 'leaves', 'glass', 'bedrock', 'wool'].concat(Object.keys(ORES).map((k) => 'ore_' + k)))
    M[n] = cube(n);
  M.log = cube('log_top', 'log');
  M.farmland = cube('farmland', 'farmland_side');
  M.grass = cube('grass', 'grass_side');
  // Base-game blocks that recipes show: the furnace, the dropper and the crafting table, front
  // to the left as in the game's inventory.
  M.vfurnace = cube('vf_top', 'vf_front', 'vf_side');
  M.dropper = cube('vf_top', 'dropper_front', 'dropper_side');
  M.geode = (o, t, out) => {
    out.box(o, 0, 0, 0, 1, 1, 1, 'geode');
    out.box(o, 0.2, 0.55, 1, 0.22, 0.22, 0.45, '#b8f6fa', '#56dbe2', '#3fb8c0', { glow: 0.15 });
    out.box(o, 0.55, 0.3, 1, 0.26, 0.26, 0.62, '#b8f6fa', '#56dbe2', '#3fb8c0', { glow: 0.15 });
    out.box(o, 0.62, 0.68, 1, 0.18, 0.18, 0.3, '#b8f6fa', '#56dbe2', '#3fb8c0', { glow: 0.15 });
  };
  M.water = (o, t, out) => out.box(o, 0, 0, 0, 1, 1, 0.85, 'water', 'water', 'water', { alpha: 0.85 });
  M.lavapool = (o, t, out) => out.box(o, 0, 0, 0, 1, 1, 0.85, 'lava', 'lava', 'lava', { glow: 0.1 * pulse(t, 2) });
  // The vanilla chest: a 14×10×14 box, a lid five pixels thick over it and the lock on the front
  // (y+). The lid sticks out a hair so the overlapping pixel row does not flicker.
  M.chest = (o, t, out) => {
    if (!RC.mcImage('entity/chest/normal')) return out.box(o, 0.06, 0.06, 0, 0.88, 0.88, 0.85, 'planks', 'chest', 'chest');
    const p = 1 / 16, e = 0.002;
    out.box(o, p, p, 0, 14 * p, 14 * p, 10 * p, 'chest_base_side', 'chest_base_front', 'chest_base_side');
    out.box(o, p - e, p - e, 9 * p, 14 * p + 2 * e, 14 * p + 2 * e, 5 * p, 'chest_lid_top', 'chest_lid_front', 'chest_lid_side');
    out.box(o, 7 * p, 15 * p, 7 * p, 2 * p, p, 4 * p, 'chest_lock');
  };

  M.furnace = machine('m_furnace', 'm_furnace_top');
  M.crusher = machine('m_crusher', 'm_crusher');
  M.crafter = machine('m_crafter', 'm_crafter');
  M.alloy = machine('m_alloy', 'm_alloy_top');
  M.mixer = machine('m_mixer', 'm_mixer');
  M.charger = machine('m_charger', 'm_charger');
  M.placer = machine('m_placer', 'm_placer');
  M.breaker = machine('m_breaker', 'm_breaker');
  M.pump = machine('m_pump', 'm_pump');
  M.miner = machine('m_miner', 'm_miner_top');
  M.vein = cube('m_vein');
  M.gen_fuel = machine('gen_fuel', 'gen_fuel_top');
  M.gen_lava = machine('gen_lava', 'gen_lava');
  M.battery = machine('battery', 'battery');
  M.cell = cube('casing', 'cell', 'cell');
  M.terminal = machine('terminal', 'terminal');
  M.station = machine('station', 'casing');
  M.dock_unload = machine('dock_unload', 'dock_unload');
  M.dock_load = machine('dock_load', 'dock_load');
  M.dock_charge = machine('dock_charge', 'dock_charge');
  M.anchor = cube('anchor');
  M.casing = cube('casing');
  // A facade disguises a housing as a full block; `look` names the block it copies. A facade seen
  // with the wrench in hand is see-through, so the housing in the same cell shows.
  M.facade = (o, t, out) => {
    const look = o.look || 'facade_blank';
    out.box(o, 0, 0, 0, 1, 1, 1, look, look, look, { alpha: o.alpha });
  };
  M.bricks = cube('bricks');
  M.tank = (o, t, out) => out.box(o, 0, 0, 0, 1, 1, 1, 'tank_' + (o.fluid || 'empty'));
  M.alloyblock = (o, t, out) => out.box(o, 0, 0, 0, 1, 1, 1, 'alloy' + (o.tier || 1));

  M.node = (o, t, out) => {
    out.box(o, 0, 0, 0, 1, 1, 1, 'casing', 'node', 'node', { glow: o.bad ? 0 : 0.05 + 0.08 * pulse(t, 3) });
    out.box(o, 0.44, 0.44, 1, 0.12, 0.12, 0.3, o.bad ? '#e0564a' : TIER[o.tier || 1]);
  };
  M.solar = (o, t, out) => {
    out.box(o, 0.4, 0.4, 0, 0.2, 0.2, 0.3, '#4a5060');
    out.box(o, 0, 0, 0.3, 1, 1, 0.12, 'solar_top', '#2b3040', '#2b3040', { glow: o.on ? 0.08 : 0 });
  };
  M.crystal = (o, t, out) => {
    out.box(o, 0.15, 0.15, 0, 0.7, 0.7, 0.14, '#4a5060');
    out.box(o, 0.27, 0.27, 0.14, 0.46, 0.46, 0.7, 'crystal', 'crystal', 'crystal', { glow: 0.1 + 0.2 * pulse(t, 3) });
    out.box(o, 0.4, 0.4, 0.84, 0.2, 0.2, 0.14, '#b8f6fa', '#8eeaf0', '#8eeaf0');
  };
  // A crystal that is part of a merged structure fills its whole cell.
  M.crystalblock = (o, t, out) => {
    const tex = o.fed ? 'crystal' : 'crystal_dim';
    out.box(o, 0, 0, 0, 1, 1, 1, tex, tex, tex, { glow: o.fed ? 0.1 + 0.22 * pulse(t, 3) : 0 });
  };

  // Housing: con lists connected sides ('x+x-y+z-'), ch lists the four slots ('ie--').
  M.pipe = (o, t, out) => {
    const ch = o.ch || '----';
    const h = 'pipe:h' + ch, v = 'pipe:v' + ch, c = 'pipe:c' + ch;
    const a = 0.3125, s = 0.375, e = 0.6875, r = 1 - e;
    const bad = o.bad ? { glow: 0, alpha: 0.55 } : undefined;
    out.box(o, a, a, a, s, s, s, h, h, v, bad);
    const con = o.con || '';
    if (con.includes('x-')) out.box(o, 0, a, a, a, s, s, h, h, c, bad);
    if (con.includes('x+')) out.box(o, e, a, a, r, s, s, h, h, c, bad);
    if (con.includes('y-')) out.box(o, a, 0, a, s, a, s, v, c, h, bad);
    if (con.includes('y+')) out.box(o, a, e, a, s, r, s, v, c, h, bad);
    if (con.includes('z-')) out.box(o, a, a, 0, s, s, a, c, v, v, bad);
    if (con.includes('z+')) out.box(o, a, a, e, s, s, r, c, v, v, bad);
  };

  // Monorail piece. face: 'floor', 'ceil', 'wy' (wall on the y- side) or 'wx' (wall on the x- side).
  // These boxes frame the scene; the Blender model rail.glb draws it (poses.js).
  const RAIL_CLIMB = { 'x+': ['x', 1], 'x-': ['x', -1], 'y+': ['y', 1], 'y-': ['y', -1] };
  const RAIL_ARC = 6;
  M.rail = (o, t, out) => {
    const col = TIER[o.tier || 1], lt = shade(col, 0.3);
    const m = 0.4375, w = 0.125;
    const dir = o.dir || 'x';
    if (o.face === 'wy') return dir === 'x' ? out.box(o, 0, 0, m, 1, w, w, lt, col) : out.box(o, m, 0, 0, w, w, 1, lt, col);
    if (o.face === 'wx') return dir === 'y' ? out.box(o, 0, 0, m, w, 1, w, lt, col) : out.box(o, 0, m, 0, w, w, 1, lt, col);
    // A floor rail with slope 'x+', 'x-', 'y+' or 'y-' climbs one block over its cell towards that
    // side, as a staircase of four short pieces.
    const up = (!o.face || o.face === 'floor') && RAIL_CLIMB[o.slope];
    if (up) {
      for (let i = 0; i < 4; i++) {
        const a = up[1] > 0 ? i / 4 : 1 - (i + 1) / 4;
        if (up[0] === 'x') out.box(o, a, m, i / 4, 0.25, w, 0.25 + w, lt, col);
        else out.box(o, m, a, i / 4, w, 0.25, 0.25 + w, lt, col);
      }
      return;
    }
    const z = o.face === 'ceil' ? 0.875 : 0;
    // Floor and ceiling rails bend: a half piece runs from the cell centre to every neighbour.
    let con = o.con && o.con.length ? o.con : dir === 'x' ? ['x-', 'x+'] : ['y-', 'y+'];
    if (con.length === 1) con = [con[0], con[0][0] + (con[0][1] === '+' ? '-' : '+')];
    // A corner, two halves at a right angle, is a quarter circle round the corner of the cell.
    if (con.length === 2 && con[0][0] !== con[1][0]) {
      const cx = con.includes('x+') ? 1 : 0, cy = con.includes('y+') ? 1 : 0;
      const mid = Math.atan2(0.5 - cy, 0.5 - cx);
      for (let k = 0; k < RAIL_ARC; k++) {
        const a = mid + (Math.PI / 2) * ((k + 0.5) / RAIL_ARC - 0.5);
        out.box(o, cx + 0.5 * Math.cos(a) - w, cy + 0.5 * Math.sin(a) - w, z, 2 * w, 2 * w, w, lt, col);
      }
      return;
    }
    out.box(o, m, m, z, w, w, w, lt, col);
    if (con.includes('x-')) out.box(o, 0, m, z, m, w, w, lt, col);
    if (con.includes('x+')) out.box(o, m + w, m, z, m, w, w, lt, col);
    if (con.includes('y-')) out.box(o, m, 0, z, w, m, w, lt, col);
    if (con.includes('y+')) out.box(o, m, m + w, z, w, m, w, lt, col);
  };

  // Cabin that always stays upright; only the bracket to the rail moves. The mount comes from the
  // object or from the tag of the path segment it is on: 'below', 'above', 'side' (wall y-), 'sidex'.
  function cabin(o, out, top, side, extra) {
    const mount = o.tag || o.mount || 'below';
    const arm = '#3a3f49';
    if (mount === 'above') {
      out.box(o, 0.2, 0.2, 0.08, 0.6, 0.6, 0.62, top, side, side, extra);
      out.box(o, 0.44, 0.44, 0.7, 0.12, 0.12, 0.18, arm);
    } else if (mount === 'side') {
      out.box(o, 0.44, 0.12, 0.42, 0.12, 0.2, 0.12, arm);
      out.box(o, 0.2, 0.32, 0.12, 0.6, 0.6, 0.7, top, side, side, extra);
    } else if (mount === 'sidex') {
      out.box(o, 0.12, 0.44, 0.42, 0.2, 0.12, 0.12, arm);
      out.box(o, 0.32, 0.2, 0.12, 0.6, 0.6, 0.7, top, side, side, extra);
    } else {
      out.box(o, 0.4, 0.4, 0.12, 0.2, 0.2, 0.08, arm);
      out.box(o, 0.2, 0.2, 0.2, 0.6, 0.6, 0.66, top, side, side, extra);
    }
  }
  // Where the front of a cart is: carts face the way they travel, x+ when standing still.
  function nose(o, out, z, top, side, shift) {
    const hx = o.hx == null && !o.hy ? 1 : o.hx || 0, hy = o.hy || 0;
    const mount = o.tag || o.mount || 'below';
    const oy = mount === 'side' ? 0.12 : 0, ox = mount === 'sidex' ? 0.12 : 0;
    const x = (hx > 0 ? 0.8 : hx < 0 ? -0.02 : 0.39) + ox + (shift || 0) * hx;
    const y = (hy > 0 ? 0.8 : hy < 0 ? -0.02 : 0.39) + oy + (shift || 0) * hy;
    out.box(o, x, y, z, 0.22, 0.22, 0.22, top, side);
  }
  M.capsule = (o, t, out) => {
    cabin(o, out, '#cfeaf5', 'capsule', { alpha: o.alpha });
    nose(o, out, 0.3, '#fff3a8', '#ffe14a');
  };
  const TOOL = { saw: '#d8dce4', reaper: '#8fd35a', hold: '#a8793f', grab: '#ffa53c', bore: '#9aa3b2', gun: '#e0564a', scoop: '#e08a3a', map: '#5ab0e6' };
  M.worker = (o, t, out) => {
    cabin(o, out, TOOL[o.tool] || '#8a919d', 'chassis');
    const mount = o.tag || o.mount || 'below';
    if (o.tool && o.tool !== 'hold') nose(o, out, mount === 'above' ? 0.25 : 0.4, TOOL[o.tool], shade(TOOL[o.tool], -0.2), o.busy ? 0.06 * Math.sin(t * 18) : 0);
  };

  M.obsidian = cube('obsidian');
  M.netherrack = cube('netherrack');
  M.pdock = machine('pdock', 'pdock');
  // A portal two cells wide and three tall, standing along x.
  M.portal = (o, t, out) => {
    out.box(o, 0, 0.3, 0, 2, 0.4, 0.25, 'obsidian');
    out.box(o, 0, 0.3, 0.25, 0.25, 0.4, 2.5, 'obsidian');
    out.box(o, 1.75, 0.3, 0.25, 0.25, 0.4, 2.5, 'obsidian');
    out.box(o, 0, 0.3, 2.75, 2, 0.4, 0.25, 'obsidian');
    out.box(o, 0.25, 0.45, 0.25, 1.5, 0.1, 2.5, '#8a3df0', '#8a3df0', '#8a3df0', { alpha: 0.7, glow: 0.1 + 0.15 * pulse(t, 2) });
  };
  // A crystal seed growing into a druse; stage 0, 1 or 2.
  M.seed = (o, t, out) => {
    const g = { glow: 0.15 };
    const s = o.stage || 0;
    out.box(o, 0.42, 0.42, 0, 0.16, 0.16, 0.2 + 0.2 * s, '#b8f6fa', '#56dbe2', '#3fb8c0', g);
    if (s > 0) out.box(o, 0.2, 0.55, 0, 0.2, 0.2, 0.15 + 0.15 * s, '#b8f6fa', '#56dbe2', '#3fb8c0', g);
    if (s > 1) {
      out.box(o, 0.6, 0.25, 0, 0.24, 0.24, 0.6, '#b8f6fa', '#56dbe2', '#3fb8c0', g);
      out.box(o, 0.62, 0.66, 0, 0.18, 0.18, 0.3, '#b8f6fa', '#56dbe2', '#3fb8c0', g);
    }
  };
  M.zipanchor = (o, t, out) => {
    const c = o.corner ? 0.72 : 0.4, a = { alpha: o.alpha };
    out.box(o, c - 0.05, c - 0.05, 0, 0.3, 0.3, 0.1, '#9aa3b2', '#5d6470', '#5d6470', a);
    out.box(o, c + 0.04, c + 0.04, 0.1, 0.12, 0.12, 0.8, '#e6e8ee', '#c9ccd4', '#9aa3b2', a);
    out.box(o, c, c, 0.9, 0.2, 0.2, 0.14, '#ffe14a', '#e2bd55', '#c9a45a', a);
  };
  M.tree = (o, t, out) => {
    if (o.stage === 0) return out.box(o, 0.4, 0.4, 0, 0.2, 0.2, 0.45, '#4fae45');
    out.box(o, 0.36, 0.36, 0, 0.28, 0.28, 1.4, 'log_top', 'log', 'log');
    out.box(o, -0.2, -0.2, 1.4, 1.4, 1.4, 1.1, 'leaves');
  };

  // ---- plants of the base game
  // The scene shader has no see-through pixels, so a cut-out plant texture is drawn as boxes one
  // texture pixel wide and of zero depth: every column of the texture splits into runs of solid
  // pixels, and each run is a box that wears just that run (a 'cut:' texture). The planes stand
  // as in the game's block models: two planes crossed along the diagonals (cross), four in a #
  // (crop). Without local/mc-assets.js the same code cuts hand-drawn sprites (SPRITES) instead.
  const PX = 1 / 16;
  const SPRITES = {};
  const plantCache = {};
  // The solid runs of a 16-pixel plant texture: { d: RGBA, runs: [[column, row, rows], ...] },
  // from the game's texture, else from its sprite, else null.
  function plantPixels(path) {
    if (path in plantCache) return plantCache[path];
    let img = RC.mcImage(path);
    if (!img && SPRITES[path]) {
      img = document.createElement('canvas');
      img.width = img.height = 16;
      SPRITES[path](img.getContext('2d'));
    }
    let p = null;
    if (img) {
      const d = img.getContext('2d').getImageData(0, 0, 16, 16).data;
      const solid = (x, y) => d[(y * 16 + x) * 4 + 3] > 25;
      p = { d, runs: [] };
      for (let x = 0; x < 16; x++)
        for (let y = 0; y < 16; y++) {
          if (!solid(x, y)) continue;
          let n = 1;
          while (y + n < 16 && solid(x, y + n)) n++;
          p.runs.push([x, y, n]);
          y += n;
        }
    }
    return (plantCache[path] = p);
  }
  // 'cut:<path>:<column>:<row>:<rows>[:<tint>]' is one run of a plant texture, from its top row
  // in the top row of the canvas, multiplied by the colour tint (six hex digits) when given.
  texMaker('cut:', (code) => ({
    base: '#5f9a3a',
    make() {
      const [path, cx, cy, cn, tint] = code.split(':');
      const p = plantPixels(path);
      if (!p) return null;
      const k = tint ? RC.hex2rgb('#' + tint) : [255, 255, 255];
      const c = document.createElement('canvas');
      c.width = c.height = 16;
      const g = c.getContext('2d');
      for (let r = 0; r < +cn; r++) {
        const i = ((+cy + r) * 16 + +cx) * 4;
        g.fillStyle = 'rgb(' + [0, 1, 2].map((j) => Math.round((p.d[i + j] * k[j]) / 255)).join(',') + ')';
        g.fillRect(0, r, 16, 1);
      }
      return c;
    },
  }));
  // An upright plane of rows r0..r1 and columns c0..c1 of a plant texture: from (x, y, z) in the
  // cell, `w` blocks along dir ('x' or 'y'), one texture pixel to a pixel of height. flip puts
  // column c1 - 1 at the start; tint colours it; rot turns the whole plane.
  function plane(o, out, path, p) {
    const pix = plantPixels(path);
    if (!pix) return;
    const c0 = p.c0 || 0, c1 = p.c1 || 16, r0 = p.r0 || 0, r1 = p.r1 || 16;
    const cw = p.w / (c1 - c0), top = p.z + (r1 - r0) * PX;
    const extra = p.rot ? { rot: p.rot } : undefined;
    for (const [cx, cy, cn] of pix.runs) {
      const y0 = Math.max(cy, r0), n = Math.min(cy + cn, r1) - y0;
      if (cx < c0 || cx >= c1 || n <= 0) continue;
      const tex = 'cut:' + path + ':' + cx + ':' + y0 + ':' + n + (p.tint ? ':' + p.tint : '');
      const at = (p.flip ? c1 - 1 - cx : cx - c0) * cw, z = top - (y0 - r0 + n) * PX;
      if (p.dir === 'y') out.box(o, p.x, p.y + at, z, 0, cw, n * PX, tex, tex, tex, extra);
      else out.box(o, p.x + at, p.y, z, cw, 0, n * PX, tex, tex, tex, extra);
    }
  }
  // The game's cross model: two planes 14.4 pixels wide, turned 45° about the middle of the cell
  // and stretched back to the corners. p may set z, w, rows, columns and tint as for plane.
  const CROSS = (14.4 * Math.SQRT2) / 16;
  const DIAGONAL = { axis: 'z', at: [0.5, 0.5, 0], angle: Math.PI / 4 };
  function cross(o, out, path, p) {
    const q = Object.assign({ z: 0, w: CROSS, rot: DIAGONAL }, p);
    const a = 0.5 - q.w / 2;
    plane(o, out, path, Object.assign({}, q, { x: a, y: 0.5, dir: 'x' }));
    plane(o, out, path, Object.assign({}, q, { x: 0.5, y: a, dir: 'y' }));
  }
  // The game's crop model: four planes in a #, a quarter of the cell in from its sides.
  function crop(o, out, path) {
    for (const at of [0.25, 0.75]) {
      plane(o, out, path, { x: 0, y: at, z: 0, w: 1, dir: 'x' });
      plane(o, out, path, { x: at, y: 0, z: 0, w: 1, dir: 'y' });
    }
  }
  // o.age (or o.stage) clamped to 0..max; `def` when neither is set.
  const ageOf = (o, max, def) => {
    const a = o.age != null ? o.age : o.stage != null ? o.stage : def == null ? max : def;
    return Math.max(0, Math.min(max, Math.round(a)));
  };

  // Hand-drawn sprites, the fallback for the game's plant textures. Stems are drawn light grey,
  // because the models tint them as the game does.
  const sprite = (path, paint) => (SPRITES[path] = paint);
  // Upright stalks h rows tall, every other one two rows shorter, with a leaf pixel on each side.
  function tuft(g, h, stem, leaf) {
    [1, 4, 7, 10, 13].forEach((x, i) => {
      const n = Math.max(1, h - (i % 2) * 2), y = 16 - n;
      rect(g, x, y, 1, n, stem);
      if (n > 3) {
        px(g, x + 1, y + 1, leaf);
        px(g, x - 1, y + 2, leaf);
      }
    });
  }
  const roots = (g, col) => [2, 8, 13].forEach((x) => rect(g, x, 14, 2, 2, col));
  for (let s = 0; s < 8; s++) sprite('block/wheat_stage' + s, (g) => tuft(g, 2 + 2 * s, s < 7 ? '#4fae45' : '#c9a43c', s < 7 ? '#86c043' : '#e8cc5c'));
  for (let s = 0; s < 4; s++) {
    sprite('block/carrots_stage' + s, (g) => { tuft(g, 3 + 3 * s, '#3f8f2a', '#5fb53a'); if (s === 3) roots(g, '#e8862a'); });
    sprite('block/potatoes_stage' + s, (g) => { tuft(g, 3 + 3 * s, '#4a9a30', '#6cbf45'); if (s === 3) roots(g, '#c9a24a'); });
    sprite('block/beetroots_stage' + s, (g) => { tuft(g, 3 + 3 * s, '#8e2c3a', '#4f9a35'); if (s === 3) roots(g, '#7a1a2a'); });
    // A sweet berry bush: a green mound that grows with its age, berries from age 2.
    sprite('block/sweet_berry_bush_stage' + s, (g) => {
      const top = 12 - 3 * s;
      for (let y = top; y < 16; y++) {
        const w = Math.min(14, 4 + (y - top) * 2);
        for (let x = 8 - (w >> 1); x < 8 + (w >> 1); x++) if ((x * 7 + y * 3) % 5) px(g, x, y, (x + y) % 3 ? '#3f7f3a' : '#2d6e2a');
      }
      if (s >= 2) for (const [x, y] of [[4, 11], [9, 9], [11, 13], [6, 14]].slice(0, s === 3 ? 4 : 2)) rect(g, x, y, 2, 2, s === 3 ? '#d8333f' : '#7a3a2a');
    });
  }
  for (let s = 0; s < 3; s++) sprite('block/nether_wart_stage' + s, (g) => tuft(g, 4 + 4 * s, '#7a1f22', '#a83236'));
  for (let s = 0; s < 2; s++) sprite('block/torchflower_crop_stage' + s, (g) => { tuft(g, 3 + 3 * s, '#4f8a2a', '#6aa83a'); if (s) rect(g, 7, 8, 2, 2, '#f08a2a'); });
  sprite('block/torchflower', (g) => {
    rect(g, 7, 6, 2, 10, '#4f8a2a');
    rect(g, 4, 10, 3, 1, '#6aa83a');
    rect(g, 9, 12, 3, 1, '#6aa83a');
    rect(g, 5, 1, 6, 5, '#f08a2a');
    rect(g, 6, 2, 4, 3, '#ffd24a');
  });
  sprite('block/sugar_cane', (g) => {
    for (const x of [2, 7, 12]) {
      rect(g, x, 0, 2, 16, '#8cc35a');
      rect(g, x + 1, 0, 1, 16, '#6fa443');
      for (const y of [3, 8, 13]) rect(g, x, (y + x) % 16, 2, 1, '#c8e08a');
    }
    for (const [x, y] of [[4, 5], [5, 4], [10, 10], [11, 9]]) px(g, x, y, '#7fb64f');
  });
  // Cave vines: two strands with leaves; the lit ones carry glow berries; the tip ends early.
  for (const tip of ['', '_plant'])
    for (const lit of ['', '_lit'])
      sprite('block/cave_vines' + tip + lit, (g) => {
        const end = tip ? 16 : 12;
        for (const x of [4, 10]) {
          rect(g, x, 0, 1, end, '#4f7a2a');
          for (let y = 1; y < end; y += 3) px(g, x + (y % 2 ? 1 : -1), y, '#6a9a3a');
        }
        if (lit) for (const [x, y] of [[5, 4], [9, 9]]) rect(g, x, y, 2, 2, '#f0a33a');
      });
  for (const [path, from] of [['block/kelp', 6], ['block/kelp_plant', 0]])
    sprite(path, (g) => {
      for (let y = from; y < 16; y++) {
        px(g, 6 + ((y >> 2) & 1), y, '#5a8a3a');
        px(g, 10 - ((y >> 2) & 1), y, '#4a7a2a');
      }
    });
  sprite('block/bamboo_stage0', (g) => { rect(g, 7, 9, 2, 7, '#6a9a2a'); rect(g, 9, 10, 2, 1, '#86b83a'); });
  for (const [size, len] of [['small', 5], ['large', 9]])
    sprite('block/bamboo_' + size + '_leaves', (g) => {
      for (let i = 0; i < len; i++) {
        px(g, 7 - (i >> 1), 1 + i, '#5f9a2a');
        px(g, 8 + (i >> 1), i, '#6aa83a');
        if (size === 'large') px(g, 3 + (i >> 2), 6 + i % 7, '#4f8a22');
      }
    });
  for (const fruit of ['pumpkin', 'melon']) {
    sprite('block/' + fruit + '_stem', (g) => { rect(g, 7, 0, 2, 16, '#d0d0d0'); rect(g, 5, 3, 2, 1, '#e8e8e8'); rect(g, 9, 6, 2, 1, '#e8e8e8'); });
    sprite('block/attached_' + fruit + '_stem', (g) => { rect(g, 7, 7, 2, 9, '#d0d0d0'); rect(g, 0, 6, 8, 2, '#d0d0d0'); });
  }
  for (let s = 0; s < 3; s++) sprite('block/cocoa_stage' + s, (g) => { for (const [x, y] of [[14, 0], [15, 0], [13, 1], [14, 1], [12, 2], [13, 2], [12, 3]]) px(g, x, y, '#6a7a2a'); });

  // Full blocks and the textured boxes of plants.
  vanilla('pumpkin_side', 'block/pumpkin_side', { base: '#e08a26', noise: 0.06, feat: 'bark' });
  vanilla('pumpkin_top', 'block/pumpkin_top', { base: '#d9822a', noise: 0.06, paint: (g) => rect(g, 7, 7, 2, 2, '#5a7a2a') });
  vanilla('melon_side', 'block/melon_side', { base: '#7fb238', noise: 0.06, paint: (g) => [1, 5, 9, 13].forEach((x) => rect(g, x, 0, 2, 16, '#4f7f22')) });
  vanilla('melon_top', 'block/melon_top', { base: '#86b83c', noise: 0.08, paint: (g) => rect(g, 7, 7, 2, 2, '#5a7a2a') });
  vanilla('jungle_log', 'block/jungle_log', { base: '#5e4a2c', noise: 0.1, feat: 'bark', speck: [['#6d8a3a', 5]] });
  vanilla('jungle_log_top', 'block/jungle_log_top', { base: '#8a6a3a', noise: 0.08, feat: 'ring', accent: '#5e4a2c' });
  vanilla('soul_sand', 'block/soul_sand', { base: '#56402f', noise: 0.12, speck: [['#3a2a1e', 16, true]] });
  // The cactus is a box a pixel in from the sides of its cell, so its faces wear the textures
  // without their see-through rim.
  vanilla('cactus_side', { src: 'block/cactus_side', at: [1, 0, 14, 16] }, {
    base: '#5a8f32', noise: 0.06,
    paint(g) { for (const x of [2, 6, 10]) rect(g, x, 0, 1, 16, '#477a26'); for (const [x, y] of [[4, 3], [8, 9], [12, 5], [0, 12]]) px(g, x, y, '#e6e2b0'); },
  });
  vanilla('cactus_top', { src: 'block/cactus_top', at: [1, 1, 14, 14] }, {
    base: '#6aa040', noise: 0.06,
    paint(g) { for (const [x, y, w, h] of [[0, 0, 14, 1], [0, 13, 14, 1], [0, 0, 1, 14], [13, 0, 1, 14]]) rect(g, x, y, w, h, '#4f8a2e'); rect(g, 6, 6, 2, 2, '#8fc060'); },
  });
  // A bamboo stalk wears the first columns of its texture on the sides and a corner of it on top.
  vanilla('bamboo', 'block/bamboo_stalk', { base: '#7ea83a', noise: 0.05, paint: (g) => { rect(g, 0, 0, 1, 16, '#5f8a26'); rect(g, 0, 6, 3, 1, '#4f7a1e'); rect(g, 0, 13, 3, 1, '#4f7a1e'); } });
  vanilla('bamboo_top', { src: 'block/bamboo_stalk', at: [13, 0, 3, 3] }, { base: '#c8d27a', noise: 0.04 });
  // Cocoa pods by age, for a pod on the y- side of its log facing y+ (the game's south model):
  // the pod box [x, y, z, w, d, h] in pixels and the region of the texture its sides wear.
  const COCOA = [
    { box: [6, 11, 7, 4, 4, 5], side: [11, 4, 4, 5], col: '#8fb84a' },
    { box: [5, 9, 5, 6, 6, 7], side: [9, 4, 6, 7], col: '#c49a3a' },
    { box: [4, 7, 3, 8, 8, 9], side: [8, 4, 8, 9], col: '#b0602a' },
  ];
  COCOA.forEach((c, a) => {
    const pod = { base: c.col, noise: 0.08, paint: (g) => { rect(g, 0, 0, 16, 1, shade(c.col, 0.25)); for (const x of [1, 3, 5]) rect(g, x, 1, 1, 15, shade(c.col, -0.2)); } };
    vanilla('cocoa_side' + a, { src: 'block/cocoa_stage' + a, at: c.side }, pod);
    vanilla('cocoa_top' + a, { src: 'block/cocoa_stage' + a, at: [0, 0, c.box[3], c.box[3]] }, pod);
  });

  // Wheat; stage 0, 1 and 2 show the game's stages 1, 4 and 7.
  const WHEAT = [1, 4, 7];
  M.crop = (o, t, out) => crop(o, out, 'block/wheat_stage' + WHEAT[ageOf(o, 2, 0)]);
  // Crops by age, as the game counts it (default: ripe), and the texture stage of every age.
  const CROPS = { carrots: [0, 0, 1, 1, 2, 2, 2, 3], potatoes: [0, 0, 1, 1, 2, 2, 2, 3], beetroots: [0, 1, 2, 3], nether_wart: [0, 1, 1, 2] };
  for (const k in CROPS) M[k] = (o, t, out) => crop(o, out, 'block/' + k + '_stage' + CROPS[k][ageOf(o, CROPS[k].length - 1)]);
  // Torchflower: a crop of age 0 and 1, the flower at age 2 (default).
  M.torchflower = (o, t, out) => {
    const a = ageOf(o, 2);
    cross(o, out, a < 2 ? 'block/torchflower_crop_stage' + a : 'block/torchflower');
  };
  // Sugar cane: one cross per block, stacked into a column.
  M.sugar_cane = (o, t, out) => cross(o, out, 'block/sugar_cane');
  M.cactus = (o, t, out) => out.box(o, PX, PX, 0, 14 * PX, 14 * PX, 1, 'cactus_top', 'cactus_side', 'cactus_side');
  // Bamboo: a stalk two pixels thick (age 0) or three (age 1), with leaves: 'small', 'large' or
  // none; sapling: true is the shoot.
  M.bamboo = (o, t, out) => {
    if (o.sapling) return cross(o, out, 'block/bamboo_stage0');
    const s = o.age ? 3 : 2, a = 0.5 - (s * PX) / 2;
    out.box(o, a, a, 0, s * PX, s * PX, 1, 'bamboo_top', 'bamboo', 'bamboo');
    if (o.leaves !== 'small' && o.leaves !== 'large') return;
    const path = 'block/bamboo_' + o.leaves + '_leaves', from = 0.8 * PX, w = 14.4 * PX;
    plane(o, out, path, { x: from, y: 0.5, z: 0, w, dir: 'x' });
    plane(o, out, path, { x: 0.5, y: from, z: 0, w, dir: 'y' });
  };
  // Kelp: the top of its column, or with top: false a block under the top.
  M.kelp = (o, t, out) => cross(o, out, o.top === false ? 'block/kelp_plant' : 'block/kelp');
  M.pumpkin = cube('pumpkin_top', 'pumpkin_side');
  M.melon = cube('melon_top', 'melon_side');
  M.jungle_log = cube('jungle_log_top', 'jungle_log');
  M.soul_sand = cube('soul_sand');
  // Pumpkin and melon stems. A stem grows with its age 0..7 (default 7) and turns from green to
  // brown; with face 'x+', 'x-', 'y+' or 'y-' it is attached: bent towards the fruit on that side.
  const hex = (r, g, b) => ((1 << 24) | (r << 16) | (g << 8) | b).toString(16).slice(1);
  const ATTACHED = hex(224, 199, 28);
  const STEM_SIDE = { 'x-': [0, 0.5, 'x', false], 'x+': [7 * PX, 0.5, 'x', true], 'y-': [0.5, 0, 'y', false], 'y+': [0.5, 7 * PX, 'y', true] };
  const stem = (fruit) => (o, t, out) => {
    const path = 'block/' + fruit + '_stem', side = STEM_SIDE[o.face];
    if (side) {
      cross(o, out, path, { z: -PX, w: Math.SQRT2, r1: 8, tint: ATTACHED });
      plane(o, out, 'block/attached_' + fruit + '_stem', { x: side[0], y: side[1], z: 0, w: 9 * PX, dir: side[2], flip: side[3], c1: 9, tint: ATTACHED });
      return;
    }
    const a = ageOf(o, 7);
    cross(o, out, path, { z: -PX, w: Math.SQRT2, r1: 2 * (a + 1), tint: hex(a * 32, 255 - a * 8, a * 4) });
  };
  M.pumpkin_stem = stem('pumpkin');
  M.melon_stem = stem('melon');
  // Sweet berry bush: age 0..3, or berries: false for age 1 and true (default) for age 3.
  M.sweet_berry_bush = (o, t, out) => {
    const a = o.age != null || o.stage != null ? ageOf(o, 3) : o.berries === false ? 1 : 3;
    cross(o, out, 'block/sweet_berry_bush_stage' + a);
  };
  // Cave vines hang from the block above; tip: false is a block above the tip, berries: true
  // carries glow berries.
  M.cave_vines = (o, t, out) => cross(o, out, 'block/cave_vines' + (o.tip === false ? '_plant' : '') + (o.berries ? '_lit' : ''));
  // A cocoa pod of age 0..2 (default 2) on the side of a jungle log; face is the side of the pod's
  // cell the log is on: 'x+', 'x-', 'y+' (default) or 'y-'. A short stem joins it to the log.
  const COCOA_TURN = { 'y+': 0, 'x+': -Math.PI / 2, 'x-': Math.PI / 2, 'y-': Math.PI };
  M.cocoa = (o, t, out) => {
    const a = ageOf(o, 2), [x, y, z, w, d, h] = COCOA[a].box;
    const angle = COCOA_TURN[o.face] || 0;
    const rot = angle ? { axis: 'z', at: [0.5, 0.5, 0], angle } : undefined;
    out.box(o, x * PX, y * PX, z * PX, w * PX, d * PX, h * PX, 'cocoa_top' + a, 'cocoa_side' + a, 'cocoa_side' + a, rot && { rot });
    plane(o, out, 'block/cocoa_stage' + a, { x: 0.5, y: 12 * PX, z: 12 * PX, w: 4 * PX, dir: 'y', c0: 12, r1: 4, rot });
  };

  // ---- figures: Steve and the zombie
  // Vanilla proportions: the figure is 32 skin pixels tall and 1.8 blocks high; the head is 8
  // pixels, body, arms and legs 12, arms and legs 4 wide. Skins come from the game client; without
  // it the parts wear flat colours. Parts are given in figure numbers: u across (the figure's
  // right is u-), v forward, both from the cell centre, z up, all in skin pixels.
  const SP = 1.8 / 32;
  // Skin regions [x, y, w, h] of each face, and where the outer layer of the region lies.
  const SKIN = {
    head: { top: [8, 0, 8, 8], front: [8, 8, 8, 8], back: [24, 8, 8, 8], side: [0, 8, 8, 8], over: [32, 0] },
    body: { top: [20, 16, 8, 4], front: [20, 20, 8, 12], back: [32, 20, 8, 12], side: [16, 20, 4, 12], over: [0, 16] },
    rarm: { top: [44, 16, 4, 4], bottom: [48, 16, 4, 4], front: [44, 20, 4, 12], back: [52, 20, 4, 12], side: [40, 20, 4, 12], over: [0, 16] },
    larm: { top: [36, 48, 4, 4], bottom: [40, 48, 4, 4], front: [36, 52, 4, 12], back: [44, 52, 4, 12], side: [40, 52, 4, 12], over: [16, 0] },
    rleg: { top: [4, 16, 4, 4], front: [4, 20, 4, 12], back: [12, 20, 4, 12], side: [0, 20, 4, 12], over: [0, 16] },
    lleg: { top: [20, 48, 4, 4], front: [20, 52, 4, 12], back: [28, 52, 4, 12], side: [24, 52, 4, 12], over: [-16, 0] },
  };
  // legacy: the old skin layout, where the left limbs reuse the right ones.
  const FIGURES = {
    steve: { src: 'entity/player/wide/steve', head: '#b4846d', hair: '#4a3222', body: '#00a8a8', arm: '#b4846d', leg: '#3b3b9a', eye: '#4a3f9a' },
    zombie: { src: 'entity/zombie/zombie', legacy: true, head: '#4f8a3a', hair: '#3f7030', body: '#00a8a8', arm: '#4f8a3a', leg: '#3d4f8a', eye: '#15171c' },
  };
  // 'skin:<figure>:<part>:<face>' cuts one face out of the skin, nine canvas pixels to a skin
  // pixel in a canvas 160 wide, so that the canvas spans one block like every texture.
  texMaker('skin:', (code) => {
    const [who, part, face] = code.split(':');
    const f = FIGURES[who];
    const limb = f.legacy && part[0] === 'l' ? 'r' + part.slice(1) : part;
    const base = part === 'head' ? (face === 'top' ? f.hair : f.head) : part === 'body' ? f.body : part.endsWith('arm') ? f.arm : f.leg;
    const eyes = part === 'head' && face === 'front';
    return {
      base,
      noise: 0.04,
      mc: { src: f.src, at: SKIN[limb][face], over: f.legacy ? null : SKIN[limb].over, scale: 9, size: 160 },
      paint: eyes ? (g) => { px(g, 1, 4, '#ffffff'); px(g, 2, 4, f.eye); px(g, 4, 4, f.eye); px(g, 5, 4, '#ffffff'); rect(g, 0, 0, 8, 2, f.hair); } : null,
    };
  });

  // The way a figure looks: o.face, else the way it last moved, else towards the viewer (y+).
  const FACING = { 'y+': [1, 0, 0, 1], 'x+': [0, 1, -1, 0], 'y-': [-1, 0, 0, -1], 'x-': [0, -1, 1, 0] };
  function facing(o) {
    if (FACING[o.face]) return o.face;
    if (o.hx || o.hy) return o.hx > 0 ? 'x+' : o.hx < 0 ? 'x-' : o.hy > 0 ? 'y+' : 'y-';
    return 'y+';
  }
  // Draws a box given in figure pixels. tex: { top, depth, across } — depth for the faces that
  // look forward and back, across for the faces to the sides. extra.lift: [u, v, z, quarters]
  // turns the box forward about the across axis through the figure point (u, v, z), by
  // `quarters` quarter turns (1 when left out): 1 holds an arm out in front, as a zombie does,
  // 2 raises it straight up.
  function figBox(o, out, face, u, v, z, w, d, h, tex, extra) {
    const [a, b, c, e] = FACING[face];
    const xs = [a * u + b * v, a * (u + w) + b * (v + d)], ys = [c * u + e * v, c * (u + w) + e * (v + d)];
    const x0 = 0.5 + Math.min(...xs) * SP, y0 = 0.5 + Math.min(...ys) * SP;
    const alongY = face[0] === 'y';
    const bw = (alongY ? w : d) * SP, bd = (alongY ? d : w) * SP;
    let more = extra;
    if (extra && extra.lift) {
      const [lu, lv, lz, quarters] = extra.lift;
      const at = [0.5 + (a * lu + b * lv) * SP, 0.5 + (c * lu + e * lv) * SP, lz * SP];
      const angle = (quarters || 1) * { 'y+': Math.PI / 2, 'y-': -Math.PI / 2, 'x+': -Math.PI / 2, 'x-': Math.PI / 2 }[face];
      more = Object.assign({}, extra, { rot: { axis: alongY ? 'x' : 'y', at, angle } });
      delete more.lift;
    }
    out.box(o, x0, y0, z * SP, bw, bd, h * SP, tex.top, alongY ? tex.depth : tex.across, alongY ? tex.across : tex.depth, more);
  }
  // Arm poses: quarter turns forward of the right and the left arm about the shoulder (figure
  // point z 22, as in the game's model). down: both hang (default); forward: both reach out in
  // front, as a zombie; hang: the right arm straight up, holding on above the head.
  const ARMS = { down: [0, 0], forward: [1, 1], hang: [2, 0] };
  // A sword stroke of the right arm, SWING seconds long and repeated: the arm winds up over the
  // shoulder, strikes down in front and rests a moment. Quarter turns forward.
  const SWING = 0.9;
  function stroke(t) {
    const ph = (((t / SWING) % 1) + 1) % 1;
    if (ph < 0.45) return 0.2 + 1.2 * Math.sin(((ph / 0.45) * Math.PI) / 2);
    if (ph < 0.6) return 1.4 - (1.25 * (ph - 0.45)) / 0.15;
    return 0.15 + (0.05 * (ph - 0.6)) / 0.4;
  }
  // The arms of a figure at the moment t, as [right, left] quarter turns forward: o.arms is a
  // pose of ARMS or such a pair itself; swing: true swings the right arm in sword strokes. Gear
  // held or worn in the same cell (poses.js) reads the same, so it moves with the arms.
  RC.armTurns = (o, t) => {
    const base = Array.isArray(o.arms) ? o.arms : ARMS[o.arms] || ARMS.down;
    return o.swing ? [stroke(t), base[1]] : base;
  };
  // The extra of the right (0) or left (1) arm and of what is worn on it, in the pose `arms` (a
  // name of ARMS or a [right, left] pair of quarter turns).
  function armExtra(extra, arms, side) {
    const quarters = (Array.isArray(arms) ? arms : ARMS[arms] || ARMS.down)[side];
    return quarters ? Object.assign({}, extra, { lift: [side ? 6 : -6, 0, 22, quarters] }) : extra;
  }
  // A whole figure. The front faces of head and body are thin plates just in front of the part,
  // because a box shows one texture on both its front and back.
  function figure(o, out, who, extra, arms) {
    const face = facing(o);
    const T = (part, f) => 'skin:' + who + ':' + part + ':' + f;
    // A box shows one texture on its top and bottom: a raised arm shows the hand's end.
    const part = (name, u, v, z, w, d, h, plate, more) => {
      const up = more && more.lift && more.lift[3] === 2;
      const tex = { top: T(name, up ? 'bottom' : 'top'), depth: T(name, plate ? 'back' : 'front'), across: T(name, 'side') };
      figBox(o, out, face, u, v, z, w, d, h, tex, more || extra);
      if (plate) figBox(o, out, face, u, v + d + 0.04, z, w, 0, h, { top: tex.top, depth: T(name, 'front'), across: tex.across }, more || extra);
    };
    part('rleg', -4, -2, 0, 4, 4, 12);
    part('lleg', 0, -2, 0, 4, 4, 12);
    part('body', -4, -2, 12, 8, 4, 12, true);
    part('rarm', -8, -2, 12, 4, 4, 12, false, armExtra(extra, arms, 0));
    part('larm', 4, -2, 12, 4, 4, 12, false, armExtra(extra, arms, 1));
    part('head', -4, -4, 24, 8, 8, 8, true);
    return face;
  }
  const flat = (col) => ({ top: shade(col, 0.15), depth: col, across: shade(col, -0.1) });

  M.mob = (o, t, out) => {
    const hit = o.hit ? { glow: 0.5 * pulse(t, 20) } : undefined;
    figure(o, out, 'zombie', hit, 'forward');
  };
  // Steve. arms: 'down' (default), 'forward', 'hang' (see ARMS) or [right, left] quarter turns;
  // swing: true strikes with the right arm (RC.armTurns). The armour drawn with armor: true
  // follows the arms; the Blender gear in the same cell follows them when given the same props.
  M.player = (o, t, out) => {
    const arms = RC.armTurns(o, t);
    const face = figure(o, out, 'steve', o.flying ? { glow: 0.06 * pulse(t, 8) } : undefined, arms);
    const fb = (u, v, z, w, d, h, tex, extra) => figBox(o, out, face, u, v, z, w, d, h, typeof tex === 'string' ? flat(tex) : tex, extra);
    if (o.armor) {
      // Technical armour of the third tier: helmet, chestplate, leggings and boots a pixel
      // proud of the body.
      const A = TIER[3], D = shade(A, -0.3);
      fb(-4.5, -4.5, 28.5, 9, 9, 4, A);
      fb(-4.5, -2.5, 15, 9, 5, 9.5, A);
      fb(-8.5, -2.5, 19, 4.5, 5, 5.5, D, armExtra(undefined, arms, 0));
      fb(4, -2.5, 19, 4.5, 5, 5.5, D, armExtra(undefined, arms, 1));
      fb(-4.5, -2.5, 4, 9, 5, 8.5, D);
      fb(-4.5, -2.5, 0, 9, 5, 3.5, A);
      fb(-1.5, 2.6, 17, 3, 0, 4, '#ffe14a', { glow: 0.2 });
    }
    if (o.pack) {
      fb(-3.5, -5, 13, 7, 3, 10, '#a8793f');
      fb(-3.5, -5.5, 19, 7, 3.5, 4, '#7a5530');
    }
    if (o.flying) {
      // Jet nozzles on the back and their flames.
      fb(-3.5, -4.5, 16, 2.5, 2.5, 6, '#9aa3b2');
      fb(1, -4.5, 16, 2.5, 2.5, 6, '#9aa3b2');
      const fl = 4 + 2 * pulse(t, 14);
      fb(-3.2, -4.2, 16 - fl, 2, 2, fl, '#ffb23c', { alpha: 0.85, glow: 0.3 });
      fb(1.3, -4.2, 16 - fl, 2, 2, fl, '#ffb23c', { alpha: 0.85, glow: 0.3 });
    }
  };
  // A skeleton archer in flat colours, drawn by code: thin limbs, ribs and a bow held out in front
  // in the right hand, both arms forward. hit: true flashes it, as the zombie.
  const BONE = flat('#cfcfc6'), BONE_DARK = flat('#9c9c92'), SOCKET = flat('#2a2a2e'), BOW = flat('#7a5530'), STRING = flat('#e6e6e6');
  M.skeleton = (o, t, out) => {
    const face = facing(o), hit = o.hit ? { glow: 0.5 * pulse(t, 20) } : {};
    const fb = (u, v, z, w, d, h, tex, extra) => figBox(o, out, face, u, v, z, w, d, h, tex, Object.assign({}, hit, extra));
    for (const u of [-3, 1]) fb(u, -1, 0, 2, 2, 12, BONE);
    fb(-4, -1.5, 12, 8, 3, 1.5, BONE_DARK);
    fb(-1, -1, 13.5, 2, 2, 10.5, BONE);
    for (const z of [16, 18.5, 21]) fb(-4, -2, z, 8, 4, 1, BONE);
    fb(-4, -1.5, 22.5, 8, 3, 1.5, BONE_DARK);
    fb(-4, -4, 24, 8, 8, 8, BONE);
    for (const u of [-3, 1]) fb(u, 4.04, 28, 2, 0, 2, SOCKET);
    fb(-2, 4.04, 25.5, 4, 0, 1, SOCKET);
    // Arms out in front, turned a quarter forward about the shoulders. The bow turns with the
    // right arm: drawn under the hanging fist with its stave along v, it stands upright in front
    // of the fist, the grip nearest the target and the string nearest the archer.
    for (const u of [-6, 4]) fb(u, -1, 12, 2, 2, 12, BONE, { lift: [u + 1, 0, 22, 1] });
    const arm = { lift: [-5, 0, 22, 1] };
    fb(-5.5, -1.5, 10.4, 1, 3, 1.3, BOW, arm);
    for (const v of [-5, 1.5]) fb(-5.5, v, 11.1, 1, 3.5, 1, BOW, arm);
    for (const v of [-5.5, 4.5]) fb(-5.5, v, 12.1, 1, 1, 1.4, BOW, arm);
    fb(-5.25, -5, 13.3, 0.5, 10, 0.3, STRING, arm);
  };
  M.drop = (o, t, out) => out.box(o, 0.38, 0.38, 0.05 + 0.06 * pulse(t, 5), 0.24, 0.24, 0.24, o.color || '#d8a47a');
  // The Blender model turns its head and barrel to the aim prop (poses.js), so the room it needs
  // is the base and the square the barrel sweeps round the head, as high as it tilts up.
  M.turret = (o, t, out) => {
    out.box(o, 0.06, 0.06, 0, 0.88, 0.88, 0.41, 'casing_top', 'casing');
    out.box(o, -0.22, -0.22, 0.41, 1.44, 1.44, 0.62, '#4a5060', '#3d4350', '#3d4350', { glow: o.on ? 0.15 * pulse(t, 10) : 0 });
  };
  // Hydro turbine: one block that stands in water and lets the current through its flaps.
  M.turbine = (o, t, out) => {
    out.box(o, 0, 0, 0, 1, 1, 1, 'casing_top', 'turbine', 'turbine', { glow: o.on ? 0.05 + 0.08 * pulse(t, 5) : 0 });
  };
  // The crafting table of the base game.
  M.table = cube('ct_top', 'ct_front', 'ct_side');
  // Windmill: one block, a model three blocks tall (windmill.glb): a mast to 1.8 blocks, the
  // nacelle over it and four blades that turn in the x-z plane about the hub at y+ of the mast,
  // 0.84 of a block long.
  M.windmill = (o, t, out) => {
    out.box(o, 0.375, 0.375, 0, 0.25, 0.25, 1.81, '#8a919d', CASE);
    out.box(o, 0.3, 0.3, 1.81, 0.4, 0.5, 0.4, 'casing');
    for (let i = 0; i < 4; i++)
      out.box(o, 0.5, 0.84, 1.96, 0.84, 0.06, 0.125, '#e6e8ee', '#e6e8ee', '#c9ccd4', { rot: { axis: 'y', at: [0.5, 0.875, 2.025], angle: t * 2.4 + (i * Math.PI) / 2 } });
  };

  // Blocks and gear that only have Blender models (poses.js); these boxes are the room a scene
  // frames for them. Gear is worn over the player in the same cell.
  M.node_hub = (o, t, out) => out.box(o, 0, 0, 0, 1, 1, 1, 'casing', 'node', 'node', { glow: o.bad ? 0 : 0.08 });
  // The blocks of the Path-of-craft-tree branches (spec 22): one block each, as high as the model
  // stands (the shearer's arm and tank, the condenser's orb, the projector's cone).
  const room = (h) => (o, t, out) => out.box(o, 0, 0, 0, 1, 1, h, 'casing_top', 'casing', 'casing', { glow: o.on ? 0.06 : 0 });
  for (const [type, h] of [['repair', 1], ['kitchen', 1.15], ['fishery', 1.1], ['feeder', 1], ['shearer', 1.35], ['apiary', 1.1], ['brewer', 1],
    ['xp_tank', 1.35], ['enchanter', 1], ['archeo', 1.05], ['projector', 1.45], ['builder', 1.1], ['butcher', 1.25]])
    M[type] = room(h);
  // Utilizer and disintegrator (spec 25): the squat bin and the block with its output tray.
  for (const [type, h] of [['trash', 0.8], ['disintegrator', 1.05]]) M[type] = room(h);
  M.charge_plate = (o, t, out) => out.box(o, 0.06, 0.06, 0, 0.88, 0.88, 0.07, '#ffe14a', '#3d4350', '#3d4350', { glow: o.on ? 0.2 : 0 });
  M.jetpack = (o, t, out) => out.box(o, 0.12, 0.24, 0.24, 0.22, 0.52, 1.15, '#b8c0cc');
  M.zip_handle = (o, t, out) => out.box(o, 0.16, 0.22, 0.03, 0.68, 0.56, 1.1, '#5d6470');
  const armor = (z, h, wide) => (o, t, out) => out.box(o, 0.31, 0.5 - wide / 2, z, 0.38, wide, h, TIER[o.tier || 3], null, null, { alpha: o.alpha });
  M.armor_helmet = armor(1.35, 0.5, 0.6);
  M.armor_chest = armor(0.7, 0.72, 1.02);
  M.armor_legs = armor(0.25, 0.47, 0.54);
  M.armor_boots = armor(0, 0.3, 0.6);
  // Weapons held by the player (Blender models blade, eshield, rifle and axe; poses.js puts them
  // in the fists). Their boxes are the room by the hand that hangs down.
  const held = (dx, dz, w, h) => (o, t, out) => out.box(o, 0.5 + dx, 0.3, dz, w, 0.4, h, '#8a919d', null, null, { alpha: o.alpha });
  M.blade = held(-0.2, 0.5, 1, 0.9);
  M.axe = held(-0.15, 0.6, 0.6, 0.6);
  M.eshield = held(0.1, 0.35, 0.2, 0.85);
  M.rifle = held(-0.6, 0.65, 1.15, 0.35);

  // ---- the world round the blocks of the Path-of-craft-tree branches (spec 22)
  vanilla('sand', 'block/sand', { base: '#dbcf9c', noise: 0.08, speck: [['#c4b483', 14]] });
  vanilla('gravel', 'block/gravel', { base: '#857f7c', noise: 0.16, speck: [['#5f5a58', 16], ['#a8a2a0', 10]] });
  vanilla('clay', 'block/clay', { base: '#a0a6b4', noise: 0.06, speck: [['#8e94a2', 8]] });
  vanilla('sus_sand', 'block/suspicious_sand_0', { base: '#d6c48e', noise: 0.1, speck: [['#9a7a4e', 10, true]] });
  vanilla('sus_gravel', 'block/suspicious_gravel_0', { base: '#837d7a', noise: 0.16, speck: [['#9a7a4e', 8, true]] });
  vanilla('sandstone', 'block/sandstone_top', { base: '#d9cb94', noise: 0.04 });
  vanilla('sandstone_side', 'block/sandstone', { base: '#d4c48c', noise: 0.04, feat: 'rows' });
  vanilla('bookshelf', 'block/bookshelf', { base: '#8a6a43', noise: 0.06, paint: (g) => { for (const y of [1, 8]) for (let x = 1; x < 15; x += 2) rect(g, x, y, 2, 6, ['#8e2c3a', '#3558d6', '#3f8f3a', '#c9a43c'][(x + y) % 4]); } });
  const HIVE = { base: '#c69a4e', noise: 0.06, feat: 'planks' };
  vanilla('beehive_end', 'block/beehive_end', HIVE);
  vanilla('beehive_side', 'block/beehive_side', HIVE);
  vanilla('beehive_front', 'block/beehive_front', Object.assign({ paint: (g) => rect(g, 5, 7, 6, 4, '#3a2a1a') }, HIVE));
  vanilla('beehive_honey', 'block/beehive_front_honey', Object.assign({ paint: (g) => { rect(g, 5, 7, 6, 4, '#3a2a1a'); rect(g, 4, 11, 8, 3, '#f2a81d'); } }, HIVE));
  // Blocks a scoop digs out: gone is the scene moment the block vanishes.
  for (const n of ['sand', 'gravel', 'clay', 'sus_sand', 'sus_gravel']) {
    const draw = cube(n);
    M[n] = (o, t, out) => (o.gone != null && t >= o.gone ? null : draw(o, t, out));
  }
  M.sandstone = cube('sandstone', 'sandstone_side');
  M.bookshelf = cube('planks', 'bookshelf');
  // A beehive with its entrance on the y+ side; full (default) shows the honey dripping out.
  M.beehive = (o, t, out) => out.box(o, 0, 0, 0, 1, 1, 1, 'beehive_end', o.full === false ? 'beehive_front' : 'beehive_honey', 'beehive_side');
  // A cell of a projector's ghost: a see-through glowing block; bad: a wrong block stands in the
  // cell, and the ghost round it glows red.
  M.ghost = (o, t, out) => {
    const c = o.bad ? '#ff5a4a' : '#7af0ff', e = o.bad ? -0.03 : 0.05;
    out.box(o, e, e, e, 1 - 2 * e, 1 - 2 * e, 1 - 2 * e, c, shade(c, -0.15), shade(c, -0.3), { alpha: o.bad ? 0.45 : 0.32, glow: 0.2 + 0.15 * pulse(t, 3) });
  };
  // The scout compass in the right hand: on the fist held out in front (arms: 'forward' or a
  // raised right arm), else upright in the hanging hand. The needle points ahead.
  M.scout_compass = (o, t, out) => {
    const face = facing(o), up = RC.armTurns(o, t)[0] >= 0.5;
    const fb = (u, v, z, w, d, h, col, extra) => figBox(o, out, face, u, v, z, w, d, h, flat(col), extra);
    if (up) {
      fb(-7.75, 8, 24, 3.5, 3.5, 0.7, TIER[3]);
      fb(-7.25, 8.5, 24.7, 2.5, 2.5, 0.15, '#e6e8ee');
      fb(-6.25, 9, 24.85, 0.5, 2, 0.2, '#e0564a', { glow: 0.3 });
    } else {
      fb(-7.75, 2.1, 9.5, 3.5, 0.7, 3.5, TIER[3]);
      fb(-7.25, 2.8, 10, 2.5, 0.15, 2.5, '#e6e8ee');
      fb(-6.25, 2.95, 10.5, 0.5, 0.2, 1.8, '#e0564a', { glow: 0.3 });
    }
  };

  // ---- animals: boxes in skin pixels as the game's models, cut out of their skins
  // 'mob:<src>:<x>:<y>:<w>:<h>:<turn>:<size>:<colour>' is the region [x, y, w, h] of the entity
  // texture src, a quarter turned when turn is 1, 18 canvas pixels to a skin pixel in a canvas
  // `size` wide (320 for a grown animal, so a skin pixel is as big as Steve's); the colour is the
  // fallback without the game client.
  texMaker('mob:', (code) => {
    const [src, x, y, w, h, turn, size, col] = code.split(':');
    return {
      base: '#' + col,
      noise: 0.06,
      make() {
        const img = RC.mcImage(src);
        if (!img) return null;
        const c = document.createElement('canvas');
        c.width = c.height = +size;
        const g = c.getContext('2d'), k = 18;
        g.imageSmoothingEnabled = false;
        if (+turn) {
          g.translate(h * k, 0);
          g.rotate(Math.PI / 2);
        }
        g.drawImage(img, +x, +y, +w, +h, 0, 0, w * k, h * k);
        return c;
      },
    };
  });
  // One box of an animal: [u, v, z, w, d, h] in figure numbers (see figBox) times the scale k;
  // reg names the skin regions of its top, its ends (depth) and its sides (across). A long body
  // keeps its length upright in the skin: `long` turns the top when the animal looks along x,
  // `upright` turns the sides always, so the length runs along the box.
  function beastPart(o, out, face, k, src, col, box, reg, extra) {
    const T = (r, turn) => 'mob:' + src + ':' + r.join(':') + ':' + (turn ? 1 : 0) + ':' + Math.round(320 / k) + ':' + col.slice(1);
    const [u, v, z, w, d, h] = box.map((n) => n * k);
    figBox(o, out, face, u, v, z, w, d, h, { top: T(reg.top, reg.long && face[0] === 'x'), depth: T(reg.end), across: T(reg.side, reg.upright) }, extra);
  }
  const LEG = { top: [4, 16, 4, 4], end: [4, 20, 4, 12], side: [0, 20, 4, 12] };
  const COW = {
    head: { top: [6, 0, 8, 6], end: [6, 6, 8, 8], side: [0, 6, 6, 8] },
    body: { top: [50, 14, 12, 18], end: [28, 4, 12, 10], side: [18, 14, 10, 18], long: true, upright: true },
  };
  const SHEEP = {
    head: { top: [8, 0, 6, 8], end: [8, 8, 6, 6], side: [0, 8, 8, 6] },
    body: { top: [34, 14, 8, 16], end: [34, 8, 8, 6], side: [28, 14, 6, 16], long: true, upright: true },
    fleece: { top: [6, 0, 6, 6], end: [6, 6, 6, 6], side: [0, 6, 6, 6] },
  };
  // Animals face like figures (face, else the way they move); baby: true is a young one; a cow
  // vanishes at the scene moment gone (struck by the butcher).
  const BABY = 0.55;
  M.cow = (o, t, out) => {
    if (o.gone != null && t >= o.gone) return;
    const face = facing(o), k = o.baby ? BABY : 1, P = (box, reg, col) => beastPart(o, out, face, k, 'entity/cow/cow_temperate', col, box, reg);
    for (const [u, v] of [[-6, -10], [2, -10], [-6, 2], [2, 2]]) P([u, v, 0, 4, 4, 12], LEG, '#3d2c1e');
    P([-6, -11, 12, 12, 18, 10], COW.body, '#4a3626');
    P([-4, 6, 15, 8, 6, 8], COW.head, '#5a4430');
  };
  // A sheep; wool: false is a sheared one.
  M.sheep = (o, t, out) => {
    const face = facing(o), k = o.baby ? BABY : 1;
    const P = (box, reg, col) => beastPart(o, out, face, k, 'entity/sheep/sheep', col, box, reg);
    const W = (box, reg) => beastPart(o, out, face, k, 'entity/sheep/sheep_wool', '#e8e8e4', box, reg);
    for (const [u, v] of [[-4, -7], [0, -7], [-4, 3], [0, 3]]) P([u, v, 0, 4, 4, 12], LEG, '#d8c8b8');
    P([-3, 6, 16, 6, 8, 6], SHEEP.head, '#d8c0b0');
    if (o.wool === false) return P([-4, -8, 12, 8, 16, 6], SHEEP.body, '#c9a890');
    W([-5.75, -9.75, 10.25, 11.5, 19.5, 9.5], SHEEP.body);
    W([-3.6, 5.4, 15.4, 7.2, 7.2, 7.2], SHEEP.fleece);
  };
  // A bee hovering a quarter block up and bobbing, its wings flat on its back.
  const BEE = { top: [10, 0, 7, 10], end: [10, 10, 7, 7], side: [0, 10, 10, 7], long: true };
  M.bee = (o, t, out) => {
    const face = facing(o), k = 0.6, bob = 4 + 1.5 * Math.sin(t * 6 + (RC.hash(o.id) % 7));
    beastPart(o, out, face, k, 'entity/bee/bee', '#e8b830', [-3.5, -5, bob, 7, 10, 7], BEE);
    const wing = flat('#eef4fa'), lift = (7 + bob) * k;
    for (const u of [-4.5, 0.5]) figBox(o, out, face, u * k, -2 * k, lift, 4 * k, 5 * k, 0.3 * k, wing, { alpha: 0.75 });
  };

  // ---------------------------------------------------------------- item icons
  // 8x8 bitmaps: '.' empty, 'a' base colour, 'b' dark, 'c' light, 'k' outline.
  const ICONS = {
    ingot: ['........', '..kkkkk.', '.kcccak.', 'kcaaabk.', 'kaaabbk.', 'kbbbbk..', '.kkkk...', '........'],
    dust: ['........', '...k....', '..kck...', '.kcaak..', 'kcaaabk.', 'kaabbbk.', '.kkkkk..', '........'],
    gem: ['........', '..kkkk..', '.kccaak.', 'kcaaaabk', '.kaaabk.', '..kabk..', '...kk...', '........'],
    chip: ['........', '.kkkkkk.', '.kaaaak.', '.kacbak.', '.kabbak.', '.kaaaak.', '.kkkkkk.', '.k.kk.k.'],
    tube: ['........', '......kk', '....kkak', '..kkaabk', 'kkaabbkk', 'kabbkk..', 'kkkk....', '........'],
    gear: ['...kk...', '.k.aa.k.', '.kaaaak.', 'kaakkaak', 'kaakkaak', '.kaaaak.', '.k.aa.k.', '...kk...'],
    bottle: ['...kk...', '...kk...', '..kcck..', '.kcaaak.', '.kaaaak.', '.kaaabk.', '..kbbk..', '...kk...'],
    plate: ['........', '........', '.kkkkkk.', 'kccccaak', 'kaaaabbk', '.kkkkkk.', '........', '........'],
    chest: ['........', 'kk....kk', 'kakkkkak', 'kaaccaak', '.kaaaak.', '.kaaabk.', '.kabbbk.', '.kkkkkk.'],
    bag: ['..kkkk..', '.k....k.', '.kkkkkk.', 'kaaccaak', 'kaakkaak', 'kaaaaaak', 'kabbbbak', '.kkkkkk.'],
    hook: ['....kkk.', '...kaak.', '..kak.k.', '.kak....', 'kak..kk.', 'kak.kak.', '.kaaak..', '..kkk...'],
    wrench: ['.kk..kk.', 'kak..kak', 'kaakkaak', '.kaaaak.', '..kaak..', '..kaak..', '..kaak..', '..kkkk..'],
    book: ['........', '.kkkkkk.', 'kaaaaack', 'kaccaack', 'kaaaaack', 'kaccaack', 'kbbbbbbk', '.kkkkkk.'],
    tablet: ['........', 'kkkkkkkk', 'kbbbbbbk', 'kbccabbk', 'kbaaabbk', 'kbbbbbbk', 'kkkkkkkk', '...kk...'],
    stick: ['......kk', '.....kak', '....kak.', '...kak..', '..kak...', '.kak....', 'kak.....', 'kk......'],
    cube: ['........', '..kkkk..', '.kcccck.', 'kcccaaak', 'kaaaabbk', 'kaaabbbk', '.kabbbk.', '..kkkk..'],
    leaf: ['........', '....kkk.', '..kkcak.', '.kcaaak.', '.kaaabk.', '.kabbk..', 'kkkkk...', 'k.......'],
    drop: ['...kk...', '..kcak..', '..kaak..', '.kcaaak.', '.kaaaak.', '.kaaabk.', '..kbbk..', '...kk...'],
  };
  const iconCache = {};
  // Two kinds name a picture instead of a shape: ['mc', 'item/iron_ingot'] is a vanilla item
  // texture from local/mc-assets.js, ['art', id] the mod item's own drawing from icons.js. Both
  // fall back to the generic shape when the picture is missing.
  RC.icon = (kind, color) => {
    const key = kind + color;
    if (iconCache[key]) return iconCache[key];
    const own = kind === 'mc' ? RC.mcImage(color) : kind === 'art' && RC.iconArt ? RC.iconArt(color) : null;
    if (own) return (iconCache[key] = own);
    const c = document.createElement('canvas');
    c.width = c.height = 16;
    const g = c.getContext('2d');
    const pal = { a: color, b: shade(color, -0.35), c: shade(color, 0.45), k: '#15171c' };
    (ICONS[kind] || ICONS.cube).forEach((row, y) => {
      for (let x = 0; x < 8; x++) if (row[x] !== '.') rect(g, x * 2, y * 2, 2, 2, pal[row[x]]);
    });
    return (iconCache[key] = c);
  };
})();
