'use strict';
// Item icons. Every mod item has its own 16×16 pixel drawing here, in the style of the base game's
// item icons; base-game items show the game's own textures when local/mc-assets.js is there.
// Loaded after data.js: it points RC.ITEMS at these pictures, and RC.icon (models.js) draws
// ['art', id] from RC.iconArt and ['mc', path] from the vanilla textures. Block icons rendered
// from the Blender models are left as they are.
(function () {
  const { shade, TIER_COLOR: T } = RC;

  // ---------------------------------------------------------------- palette
  // Fixed colours. Per icon, a is the main colour and e the accent; the letters around them are
  // their shades: k outline, d deep, b dark, a, c light, l lighter; f dark accent, g light accent.
  const FIXED = {
    x: '#15171c', w: '#ffffff', m: '#9aa3b2', n: '#5d6470', o: '#d8dce4',
    y: '#e2bd55', z: '#9a7a2a', r: '#e0564a', s: '#ff9a3c', t: '#ffe14a',
    h: '#a8793f', i: '#6b4a2b', u: '#56dbe2', v: '#b8f6fa', j: '#2f8f96', q: '#59d38c',
  };
  const palette = (a, e) =>
    Object.assign({}, FIXED, {
      k: shade(a, -0.72), d: shade(a, -0.5), b: shade(a, -0.28), a, c: shade(a, 0.3), l: shade(a, 0.6),
      e, f: shade(e, -0.35), g: shade(e, 0.45),
    });

  // ---------------------------------------------------------------- shapes
  // Rows of 16 characters; '.' is see-through. Shorter rows are padded on the right.
  const S = {
    ingot: [
      '', '', '', '',
      '......kkkkkkkkk.',
      '.....klccccccck.',
      '....kccccccccbk.',
      '...kccccccccbbk.',
      '..kaaaaaaaaabbk.',
      '..kalaaaaaaabbk.',
      '..kaaaaaaaaabk..',
      '..kdddddddddk...',
      '...kkkkkkkkk....',
    ],
    dust: [
      '', '', '', '', '', '',
      '.......kk.......',
      '.....kkcck......',
      '....kccaaakk....',
      '...kcaaaaabak...',
      '..kcaaabaaaabk..',
      '.kaaabaaaabaabk.',
      '.kbbbbbbbbbbbbk.',
      '..kkkkkkkkkkkk..',
    ],
    shard: [
      '',
      '..........kk....',
      '.........kvuk...',
      '........kvuujk..',
      '.......kvuuujk..',
      '......kvuuujk...',
      '.....kvuuujk....',
      '....kvuuujk.....',
      '...kvuuujk......',
      '..kuuuujk.......',
      '..kjuujk........',
      '...kjjk.........',
      '....kk..........',
    ],
    flask: [
      '',
      '.....khhhhk.....',
      '......kook......',
      '......kook......',
      '.....kooook.....',
      '....kooooook....',
      '...koggggggok...',
      '..kgeeeeeeeefk..',
      '..kgeeeeeeeefk..',
      '..keeeeeeeeefk..',
      '..keeeeeeeeefk..',
      '...keeeeeeefk...',
      '....kffffffk....',
      '.....kkkkkk.....',
    ],
    channel: [
      '', '', '', '',
      '..kkkkkkkkkkkk..',
      '.knoooooooooonk.',
      '.knggnggggnggnk.',
      '.kneeneeeeneenk.',
      '.kneeneeeeneenk.',
      '.knffnffffnffnk.',
      '.knmmmmmmmmmmnk.',
      '..kkkkkkkkkkkk..',
    ],
    wrench: [
      '..kk..kk........',
      '.kmk..kmk.......',
      '.kmk..kmk.......',
      '.kmmkkmmk.......',
      '..kmmmmok.......',
      '...kmmmmok......',
      '....kkmmmok.....',
      '......kcaak.....',
      '.......kcaak....',
      '........kcaak...',
      '.........kcaak..',
      '..........kabk..',
      '...........kk...',
    ],
    tablet: [
      '',
      '..kkkkkkkkkkkk..',
      '.kaaaaaaaaaaaak.',
      '.kaxxxxxxxxxxak.',
      '.kaxegxegxegxak.',
      '.kaxffxffxffxak.',
      '.kaxxxxxxxxxxak.',
      '.kaxegxegxegxak.',
      '.kaxffxffxffxak.',
      '.kaxxxxxxxxxxak.',
      '.kaxeeeeggxxxak.',
      '.kaxxxxxxxxxxak.',
      '.kaaaaacaaaaaak.',
      '.kbbbbbbbbbbbbk.',
      '..kkkkkkkkkkkk..',
    ],
    reaper: [
      '',
      '',
      '...kkkkkkkkkk...',
      '..kcacacacacak..',
      '..kabababababk..',
      '..kcacacacacak..',
      '...kkkkkkkkkk...',
      '....kn....nk....',
      '.kkkknkkkknkkkk.',
      '.kmmmmmmmmmmmmk.',
      '.knnnnnnnnnnnnk.',
      '.kkkkkkkkkkkkkk.',
      '..kok.kok.kok...',
      '...k...k...k....',
    ],
    crate: [
      '',
      '',
      '.kkkkkkkkkkkkkk.',
      '.kmhhhhhhhhhhmk.',
      '.khhhhhhhhhhhhk.',
      '.kiiiiiiiiiiiik.',
      '.khhhhhhhhhhhhk.',
      '.khhhhhhhhhhhhk.',
      '.kiiiiiiiiiiiik.',
      '.khhhhhhhhhhhhk.',
      '.khhhhhhhhhhhhk.',
      '.kiiiiiiiiiiiik.',
      '.khhhhhhhhhhhhk.',
      '.kmhhhhhhhhhhmk.',
      '.kkkkkkkkkkkkkk.',
    ],
    claw: [
      '',
      '......kkkk......',
      '......knnk......',
      '......kmmk......',
      '....kkkmmkkk....',
      '...kaaaaaaaak...',
      '...kbbbbbbbbk...',
      '..kmk......kmk..',
      '.kmk........kmk.',
      '.kmk........kmk.',
      '.kmk........kmk.',
      '..kmk......kmk..',
      '...kak....kak...',
      '....k......k....',
    ],
    bore: [
      '',
      '....kkkkkkkk....',
      '....kaaaaaak....',
      '....kbbbbbbk....',
      '...kkkkkkkkkk...',
      '...koomnoomnk...',
      '....kmnoomnk....',
      '....knoomnok....',
      '.....koomnk.....',
      '.....komnok.....',
      '......kmnk......',
      '......knok......',
      '.......kk.......',
    ],
    gun: [
      '', '', '',
      '.kkkkkkk........',
      'kaaaaaaak.......',
      'kacccccakkkkkkk.',
      'kaaraaaakmmmmmok',
      'kbbbbbbbknnnnnnk',
      'kbbbbbbbkkkkkkk.',
      '.kkbbkkk........',
      '...kbbk.........',
      '...kbbk.........',
      '...kkkk.........',
    ],
    helmet: [
      '', '', '',
      '...kkkkkkkkkk...',
      '..kcccccccccak..',
      '.kcaaaaaaaaaabk.',
      '.kaaaaaaaaaaabk.',
      '.kaakkkkkkkkabk.',
      '.kaak......kabk.',
      '.kbbk......kbbk.',
      '.kkk........kkk.',
    ],
    chestplate: [
      '',
      '.kkkk......kkkk.',
      'kcaak......kaabk',
      'kaaakkkkkkkkaabk',
      'kaaacaaaaaaaaabk',
      'kkkaaaaaaaaaakkk',
      '..kaaaaaaaaaak..',
      '..kaaaaaaaaaak..',
      '..kaaaaaaaaabk..',
      '..kaaaaaaaaabk..',
      '..kaaaaaaaaabk..',
      '..kbbbbbbbbbbk..',
      '..kkkkkkkkkkkk..',
    ],
    leggings: [
      '', '',
      '..kkkkkkkkkkkk..',
      '..kcaaaaaaaaak..',
      '..kaaaaaaaaaak..',
      '..kaaaaaaaaaak..',
      '..kaaaakkaaaak..',
      '..kaaak..kaaak..',
      '..kaabk..kaabk..',
      '..kaabk..kaabk..',
      '..kaabk..kaabk..',
      '..kaabk..kaabk..',
      '..kaabk..kaabk..',
      '..kkkkk..kkkkk..',
    ],
    boots: [
      '', '', '', '', '', '',
      '..kkkk....kkkk..',
      '..kcak....kcak..',
      '..kaak....kaak..',
      '..kaak....kaak..',
      '.kaaak...kaaak..',
      'kaaaak..kaaaak..',
      'kbbbbk..kbbbbk..',
      'kkkkkk..kkkkkk..',
    ],
    jetpack: [
      '',
      '....kk....kk....',
      '...kmmk..kmmk...',
      '..kaaaakkaaaak..',
      '..kcaaakkcaaak..',
      '..kcaaakkcaaak..',
      '..kaaaakkaaaak..',
      '..kaeaakkaeaak..',
      '..kbbbbkkbbbbk..',
      '...kmmk..kmmk...',
      '...knnk..knnk...',
      '...stts..stts...',
      '....ss....ss....',
      '....s......s....',
    ],
    backpack: [
      '',
      '.....kkkkkk.....',
      '....kk....kk....',
      '...kkkkkkkkkk...',
      '..kcccccccccak..',
      '..kaaaaaaaaaak..',
      '..kbbbbbbbbbbk..',
      '..kaaaayyaaaak..',
      '..kaaaazzaaaak..',
      '..kaaaaaaaaabk..',
      '.kkaaaaaaaaabkk.',
      '.kakaaaaaaaabkak',
      '.kbkbbbbbbbbbkbk',
      '.kkkkkkkkkkkkkkk',
    ],
    harpoon: [
      '..........kkkkk.',
      '...........kook.',
      '..........kmook.',
      '.........kmk.kk.',
      '........kak.....',
      '.......kak......',
      '......kak.......',
      '.....kak........',
      '....kak.........',
      '..kkak..........',
      '.khhk...........',
      'khhhhk..........',
      'khhhhk..........',
      '.kkkk...........',
    ],
    zipline: [
      '', '',
      '....kkk..kkk....',
      'nnnkmmmknkmmmknn',
      '....kkk..kkk....',
      '.....k....k.....',
      '....kaaaaaak....',
      '....kbbbbbbk....',
      '......kaak......',
      '......kaak......',
      '...kkkkaakkkk...',
      '...khhhhhhhhk...',
      '...kkkkkkkkkk...',
    ],
    plate: [
      '',
      '.........tt.....',
      '........tt......',
      '.......tttt.....',
      '........tt......',
      '.......tt.......',
      '......t.........',
      '.....kkkkkkkkkk.',
      '....kcccccccccck',
      '...kccccttcccck.',
      '..kcccttccccck..',
      '.kaaaaaaaaaaak..',
      '.kbbbbbbbbbbbk..',
      '.kkkkkkkkkkkkk..',
    ],
    book: [
      '',
      '..kkkkkkkkkkkk..',
      '..kbaaaaaaaawk..',
      '..kbaaaaaaaawk..',
      '..kbaaaaaaaawk..',
      '..kbaaaaaaaawk..',
      '..kbaaaaaaaawk..',
      '..kbaaaaaaaawk..',
      '..kbaaaaaaaawk..',
      '..kbaaaaaaaawk..',
      '..kbaaaaaaaawk..',
      '..kbaaaaaaaawk..',
      '..kbaaaaaaaawk..',
      '..kbbbbbbbbbwk..',
      '..kkkkkkkkkkkk..',
    ],
    hourglass: [
      '',
      '..kkkkkkkkkkkk..',
      '..khhhhhhhhhhk..',
      '..kkkkkkkkkkkk..',
      '...koyyyyyyok...',
      '....koyyyyok....',
      '.....koyyok.....',
      '......kook......',
      '......koyk......',
      '.....kooyok.....',
      '....kooyyook....',
      '...koyyyyyyok...',
      '..kkkkkkkkkkkk..',
      '..khhhhhhhhhhk..',
      '..kkkkkkkkkkkk..',
    ],
    // A heater shield: a frame (a) round a glowing field (e) with a white core in the middle.
    eshield: [
      '',
      '.kkkkkkkkkkkkkk.',
      '.kcccccccccccbk.',
      '.kcggggggggggbk.',
      '.kcgeeeeeeeefbk.',
      '.kcgeeeweeeefbk.',
      '.kcgeewwweeefbk.',
      '.kcgeeeweeeefbk.',
      '.kcgeeeeeeeefbk.',
      '.kageeeeeeeefbk.',
      '..kageeeeeefbk..',
      '...kageeeefbk...',
      '....kabeefbk....',
      '.....kabbbk.....',
      '......kkkk......',
    ],
    // A module: a circuit card with gold contacts and room for an 8×8 sign at (4, 3).
    module: [
      '',
      '.kkkkkkkkkkkkkk.',
      '.kcccccccccccbk.',
      '.kcaaaaaaaaaabk.',
      '.kcaaaaaaaaaabk.',
      '.kcaaaaaaaaaabk.',
      '.kcaaaaaaaaaabk.',
      '.kcaaaaaaaaaabk.',
      '.kcaaaaaaaaaabk.',
      '.kcaaaaaaaaaabk.',
      '.kcaaaaaaaaaabk.',
      '.kcaaaaaaaaaabk.',
      '.kbbbbbbbbbbbbk.',
      '.kxyxyxyxyxyxyk.',
      '.kkkkkkkkkkkkkk.',
    ],
  };

  // Signs for module cards, 8×8, in the accent colour.
  const SIGN = {
    speed: ['g...g...', 'ee..ee..', '.ee..ee.', '..ee..ee', '.ee..ee.', 'ee..ee..', 'f...f...', '........'],
    eco: ['....gge.', '..ggeee.', '.geeefe.', '.eeefee.', '.eefeee.', '..feee..', '.f......', 'f.......'],
    cap: ['eeeeeee.', 'e.....e.', 'eeeeeee.', 'e.....e.', 'eeeeeee.', 'e.....e.', 'eeeeeee.', '........'],
    stack: ['..gggg..', '..eeee..', '.gggggg.', '.eeeeee.', 'gggggggg', 'eeeeeeee', 'ffffffff', '........'],
    yield: ['...ee...', '...ee...', '.eeeeee.', '.eeeeee.', '...ee...', '...ee...', '......g.', '.....g.g'],
    store: ['...ff...', '.eeeeee.', '.e....e.', '.egggge.', '.e....e.', '.egggge.', '.e....e.', '.eeeeee.'],
    output: ['....ee..', '...ee...', '..ee....', '.eeeeee.', '....ee..', '...ee...', '..ee....', '........'],
    line: ['....e...', '....ee..', 'e.e.eee.', 'e.e.eeee', 'e.e.eee.', '....ee..', '....e...', '........'],
    scout: ['.eeee...', 'eg...e..', 'e....e..', 'e....e..', '.eeee...', '....ff..', '.....ff.', '......ff'],
    area: ['ee....ee', 'e......e', '..gggg..', '..g..g..', '..g..g..', '..gggg..', 'e......e', 'ee....ee'],
    storage: ['.ffffff.', 'fggggggf', 'feeeeeef', 'fffwwfff', 'feewweef', 'feeeeeef', 'ffffffff', '........'],
    rare: ['..eeee..', '.gwgeef.', 'eggeeeef', '.eeeeef.', '..eeef..', '...ef...', '........', '........'],
    pick: ['nmmmmmmn', 'mmeemmmm', 'mmegmmem', 'mmmmmmee', 'memmmmmm', 'meemmgmm', 'mmmmmeem', 'nmmmmmmn'],
    tunnel: ['eeeeeeee', 'effffffe', 'efxxxxfe', 'efxxxxfe', 'efxxxxfe', 'efxxxxfe', 'effffffe', 'eeeeeeee'],
    height: ['gggggggg', '...ee...', '..eeee..', '.eeeeee.', '...ee...', '...ee...', '...ee...', 'ffffffff'],
    lava: ['...ee..x', '..eeeex.', '.eegexe.', '.egexee.', '.eexeee.', '..xeee..', '.x......', 'x.......'],
    build: ['eeefeeef', 'eeefeeef', 'ffffffff', 'efeeefee', 'efeeefee', 'ffffffff', 'eeefeeef', 'eeefeeef'],
    radius: ['..eeee..', '.e....e.', 'e..gg..e', 'e.g..g.e', 'e.g..g.e', 'e..gg..e', '.e....e.', '..eeee..'],
    gear: ['e..ee..e', '.eeeeee.', '.ee..ee.', 'eeeffeee', 'eeeffeee', '.ee..ee.', '.eeeeee.', 'e..ee..e'],
    net: ['ee....ee', 'ee....ee', '..e..e..', '...gg...', '...gg...', '..e..e..', 'ee....ee', 'ee....ee'],
    up: ['...ee...', '..eeee..', '.eeeeee.', 'eeeeeeee', '...ee...', '...ee...', '...ee...', '...ff...'],
    person: ['..eeee..', '..eeee..', '...ee...', '.eeeeee.', '...ee...', '...ee...', '..e..e..', '.e....e.'],
    // Turret modules: effects (drop, skull, snowflake) and support (heart, shield, sword, bolt).
    bleed: ['...ee...', '...ee...', '..eeee..', '.geeeee.', 'egeeeeef', 'egeeeeef', '.eeeeff.', '..ffff..'],
    poison: ['.eeeeee.', 'eeeeeeee', 'exxeexxe', 'exxeexxe', 'eeeffeee', '.eeeeee.', '.efefef.', '........'],
    slow: ['e..e..e.', '.e.e.e..', '..eee...', 'eeegeee.', '..eee...', '.e.e.e..', 'e..e..e.', '........'],
    heal: ['.ee..ee.', 'egeeeeee', 'eeeeeeee', 'eeeeeeef', '.eeeeef.', '..eeef..', '...ef...', '........'],
    guard: ['eeeeeeee', 'eggeeeef', 'egeeeeef', 'egeeeeef', '.eeeeef.', '.eeeeef.', '..eeff..', '...ff...'],
    power: ['......ee', '.....ege', '....ege.', 'f..ege..', '.ffge...', '..ff....', '.f......', 'f.......'],
    haste: ['....gee.', '...eee..', '..eee...', '.eeeeee.', '...eee..', '..eee...', '.ee.....', 'e.......'],
  };
  // Emblems for book covers, 7×7 at (4, 4).
  const EMBLEM = {
    gear: ['e.ee.e.', '.eeee..', 'ee..ee.', 'ee..ee.', '.eeee..', 'e.ee.e.', '.......'],
    crystal: ['..gg...', '.geef..', '.geef..', '.geef..', '..ef...', '.......', '.......'],
    text: ['xxxxx..', '.......', 'xxxx...', '.......', 'xxxxx..', '.......', 'xxx....'],
    grid: ['xxxxxxx', 'xexexex', 'xxxxxxx', 'xexexex', 'xxxxxxx', 'xexexex', 'xxxxxxx'],
    star: ['...g...', '...g...', '.ggegg.', '...g...', '...g...', '.......', '.......'],
    gridgear: ['xxxx...', 'xexe...', 'xxxx...', '...mmm.', '..mnnm.', '..mnnm.', '...mm..'],
  };

  // ---------------------------------------------------------------- icons
  // Each icon: shape rows (or a draw function), its colours a and e, and overlays [x, y, rows].
  const ART = {};
  const def = (id, spec) => (ART[id] = spec);
  const module = (id, sign, e, board) => def(id, { rows: S.module, a: board || '#3b4a5a', e, over: [[4, 3, SIGN[sign]]] });
  const book = (id, a, emblem, e, extra) => def(id, { rows: S.book, a, e: e || '#e2bd55', over: [[4, 4, EMBLEM[emblem]]].concat(extra || []) });
  const dots = (pts, ch) => pts.map(([x, y]) => [x, y, [ch]]);

  // Alloys: one ingot, the tier colour and a mark of its own.
  def('a1', { rows: S.ingot, a: T[1], e: '#d8dce4', over: [[3, 9, ['.ooooooooo']]] });
  def('a2', { rows: S.ingot, a: T[2], e: '#ffe14a', over: [[11, 0, ['..t..', '.twt.', 'ttwtt', '.twt.', '..t..']]] });
  def('a3', { rows: S.ingot, a: T[3], e: '#b8f6fa', over: dots([[7, 5], [10, 7], [5, 9]], 'w').concat(dots([[8, 6], [6, 7], [8, 10]], 'v')) });
  def('a4', { rows: S.ingot, a: T[4], e: '#ffffff', over: [[10, 0, ['...w...', '...w...', '..www..', 'wwwgwww', '..www..', '...w...', '...w...']]] });
  // Dusts and the crystal shard.
  def('dust', { rows: S.dust, a: '#9a8a7a', e: '#d8a47a', over: dots([[6, 11], [10, 10], [8, 12]], 's').concat(dots([[5, 12], [11, 12], [9, 9]], 't')) });
  def('dust_cu', { rows: S.dust, a: '#e0824a', e: '#ffd0a8', over: dots([[8, 8], [12, 11], [4, 12]], 'l') });
  def('dust_au', { rows: S.dust, a: '#f5d54a', e: '#ffffff', over: dots([[8, 8], [12, 11], [4, 12], [13, 6]], 'w') });
  def('shard', { rows: S.shard, a: '#56dbe2', e: '#ffffff', over: dots([[3, 3], [13, 9], [6, 11]], 'w') });
  // Solution and catalysts: one flask each, the liquid's colour and its sign.
  def('f_sol', { rows: S.flask, a: '#5d6470', e: '#59d38c', over: dots([[6, 9], [9, 11], [10, 8], [5, 11]], 'w') });
  def('f_fast', { rows: S.flask, a: '#5d6470', e: '#ff7a2a', over: dots([[5, 8], [6, 9], [5, 10], [8, 8], [9, 9], [8, 10]], 'w') });
  def('f_eco', { rows: S.flask, a: '#5d6470', e: '#3fb8a0', over: [[6, 8, ['..ww', '.ww.', 'ww..', 'w...']]] });
  def('f_rich', { rows: S.flask, a: '#5d6470', e: '#e2bd55', over: [[6, 7, ['.w.', 'www', '.w.']], [10, 10, ['w']]] });
  // Pages about a group of materials show its first member.
  def('alloys', ART.a1);
  def('dusts', ART.dust);
  def('fluids', ART.f_sol);
  // Channels: a short tube with a window in the channel's colour and its sign.
  def('ch_item', { rows: S.channel, a: '#5d6470', e: '#ffa53c', over: [[6, 6, ['xxxx', 'xwwx', 'xwwx', 'xxxx']]] });
  def('ch_fluid', { rows: S.channel, a: '#5d6470', e: '#4c9bf5', over: [[6, 6, ['..w.', '.ww.', 'wwww', '.ww.']]] });
  def('ch_energy', { rows: S.channel, a: '#5d6470', e: '#ffe14a', over: [[6, 6, ['..xx', '.xx.', 'xxxx', '.xx.']]] });
  // Tools and gear.
  def('wrench', { rows: S.wrench, a: T[1], e: T[1] });
  def('tablet', { rows: S.tablet, a: T[4], e: '#5ad1e6' });
  def('tool_saw', { draw: saw, a: '#d8dce4', e: '#e2bd55' });
  def('tool_reaper', { rows: S.reaper, a: '#8fd35a', e: '#8fd35a' });
  def('tool_hold', { rows: S.crate, a: '#a8793f', e: '#a8793f' });
  def('tool_grab', { rows: S.claw, a: '#ffa53c', e: '#ffa53c' });
  def('tool_bore', { rows: S.bore, a: T[3], e: T[3] });
  def('tool_gun', { rows: S.gun, a: '#e0564a', e: '#e0564a' });
  def('tool_scoop', { draw: outlined(scoop), a: '#e08a3a', e: '#e08a3a' });
  def('tool_map', { draw: outlined(mapDrum), a: '#5ab0e6', e: '#7af0ff' });
  def('scout_compass', { draw: compass, a: '#8a919d', e: '#7af0ff' });
  def('jetpack', { rows: S.jetpack, a: T[4], e: '#ffe14a' });
  def('backpack', { rows: S.backpack, a: '#a8793f', e: T[2] });
  def('harpoon', { rows: S.harpoon, a: T[3], e: T[3] });
  def('zipline', { rows: S.zipline, a: T[2], e: T[2] });
  def('charge_plate', { rows: S.plate, a: T[4], e: '#ffe14a' });
  // Energy weapons: the hilt, frame or stock in the colour of the tier, the energy in cyan.
  def('blade', { draw: outlined(sword), a: T[2], e: '#62e6f2' });
  def('eshield', { rows: S.eshield, a: T[2], e: '#4fd2e6' });
  def('rifle', { draw: outlined(rifle), a: T[4], e: '#7af0ff' });
  // Technical armour: four pieces of the third tier with a charge strip. Several spellings of the
  // ids are covered, since the entries are still being written.
  const ARMOR = {
    helmet: { rows: S.helmet, a: T[3], e: '#ffe14a', over: [[6, 5, ['tttt']]] },
    chestplate: { rows: S.chestplate, a: T[3], e: '#ffe14a', over: [[6, 6, ['tttt']], [7, 7, ['tt']]] },
    leggings: { rows: S.leggings, a: T[3], e: '#ffe14a', over: [[5, 4, ['tttttt']]] },
    boots: { rows: S.boots, a: T[3], e: '#ffe14a', over: [[4, 8, ['t']], [12, 8, ['t']]] },
  };
  const ARMOR_IDS = { helmet: ['helmet', 'head'], chestplate: ['chestplate', 'chest', 'body'], leggings: ['leggings', 'legs'], boots: ['boots', 'feet'] };
  for (const piece in ARMOR_IDS) for (const name of ARMOR_IDS[piece]) def('armor_' + name, ARMOR[piece]);
  def('armor', ARMOR.chestplate);
  // Books.
  book('guide', T[1], 'gear');
  book('overview', '#2f8f96', 'crystal', '#b8f6fa');
  book('glossary', '#d8dce4', 'text');
  book('enchants', '#7a5ad0', 'star', '#ffffff', dots([[3, 2], [11, 12], [10, 3], [5, 12]], 'l'));
  book('mod_recipes', '#3558d6', 'grid', '#e6e8ee');
  book('mod_recipes_machines', '#c25a2a', 'gridgear', '#e6e8ee');
  // Start pages.
  def('quickstart', { rows: S.hourglass, a: '#6b7380', e: '#e2bd55' });
  def('xp', { draw: orb, a: '#7cfc5a', e: '#e6ff7a' });
  def('progress', { draw: stairs, a: '#6b7380', e: '#6b7380' });
  // Machine modules (spec 18.3).
  module('mod_speed', 'speed', '#ff6a3c');
  module('mod_eco', 'eco', '#8fe36a');
  module('mod_cap', 'cap', '#e6e8ee');
  module('mod_stack', 'stack', '#e2bd55');
  module('mod_yield', 'yield', '#7af0ff');
  module('mod_store_cap', 'store', '#ffe14a');
  module('mod_output', 'output', '#ffb23c');
  module('mod_line', 'line', '#c58aff');
  module('mod_scout', 'scout', '#e6e8ee');
  module('mod_area', 'area', '#7af0ff');
  module('mod_storage', 'storage', '#c9a45a');
  module('mod_rare', 'rare', '#5fe6dd');
  module('mod_pick', 'pick', '#f5d54a');
  module('mod_stratum', 'rare', '#d9c38a');
  module('mod_tunnel', 'tunnel', '#9aa3b2');
  module('mod_height', 'height', '#7af0ff');
  module('mod_lava', 'lava', '#ff7a2a');
  module('mod_build', 'build', '#d08060');
  module('mod_radius', 'radius', '#ffe14a');
  // Turret modules: effect cards on a dark red board, support cards on a dark green one.
  const EFFECT_BOARD = '#4a2a30', SUPPORT_BOARD = '#24453a';
  module('mod_bleed', 'bleed', '#e0303a', EFFECT_BOARD);
  module('mod_poison', 'poison', '#7cd04a', EFFECT_BOARD);
  module('mod_slow', 'slow', '#8fb8ff', EFFECT_BOARD);
  module('mod_heal', 'heal', '#ff6a8a', SUPPORT_BOARD);
  module('mod_guard', 'guard', '#c9d2e0', SUPPORT_BOARD);
  module('mod_power', 'power', '#ff8a3c', SUPPORT_BOARD);
  module('mod_haste', 'haste', '#ffe14a', SUPPORT_BOARD);
  // Pages that stand for a group of modules or chips.
  module('modules', 'gear', '#59d38c');
  module('orders', 'net', '#c58aff', '#3d3260');
  module('upgrades', 'up', '#ffd24a', '#6a4a2a');
  module('pmodules', 'person', '#b8f6fa', '#2f5a66');

  // ---------------------------------------------------------------- drawings by code
  // A circular saw blade: twelve teeth round a disc with a hub.
  function saw(put) {
    for (let y = 0; y < 16; y++)
      for (let x = 0; x < 16; x++) {
        const dx = x - 7.5, dy = y - 7.5, d = Math.hypot(dx, dy);
        const tooth = Math.floor(((Math.atan2(dy, dx) + Math.PI) / (2 * Math.PI)) * 12) % 2 === 0;
        if (d < 1.3) put(x, y, 'x');
        else if (d < 2.6) put(x, y, 'y');
        else if (d < 5.4) put(x, y, x + y < 14 ? 'o' : 'm');
        else if (d < 6.2) put(x, y, 'n');
        else if (d < 7.6 && tooth) put(x, y, d < 6.9 ? 'm' : 'k');
      }
  }
  // An experience orb.
  function orb(put) {
    for (let y = 0; y < 16; y++)
      for (let x = 0; x < 16; x++) {
        const d = Math.hypot(x - 7.5, y - 8), hi = Math.hypot(x - 6, y - 6.5);
        if (d >= 6) continue;
        put(x, y, d >= 5.1 ? 'k' : hi < 1.3 ? 'w' : hi < 2.6 ? 'g' : d < 3.5 ? 'a' : 'b');
      }
  }
  // A drawing filled by paint(dot) and outlined in k wherever a filled pixel meets an empty one.
  function outlined(paint) {
    return (put) => {
      const px = {};
      const dot = (x, y, ch) => (px[x + ',' + y] = ch);
      paint(dot);
      for (let y = 0; y < 16; y++)
        for (let x = 0; x < 16; x++) {
          const ch = px[x + ',' + y];
          if (ch) put(x, y, ch);
          else if (px[x - 1 + ',' + y] || px[x + 1 + ',' + y] || px[x + ',' + (y - 1)] || px[x + ',' + (y + 1)]) put(x, y, 'k');
        }
    };
  }
  const box = (dot, x0, y0, w, h, ch) => {
    for (let y = y0; y < y0 + h; y++) for (let x = x0; x < x0 + w; x++) dot(x, y, ch);
  };
  // An energy sword pointing to the top right, as the game's swords do: the blade runs along the
  // diagonal x + y = 15 with a white core, a light upper side and a darker lower side; the guard
  // crosses it, then the grip and the pommel.
  function sword(dot) {
    for (let x = 6; x <= 14; x++) dot(x, 15 - x, 'w');
    for (let x = 6; x <= 13; x++) dot(x, 14 - x, 'g');
    for (let x = 7; x <= 14; x++) dot(x, 16 - x, 'f');
    for (let x = 2; x <= 7; x++) dot(x, x + 5, 'a');
    for (let x = 3; x <= 7; x++) dot(x, x + 4, 'c');
    dot(5, 10, 't');
    for (let x = 2; x <= 4; x++) dot(x, 15 - x, 'x');
    dot(3, 13, 'n');
    dot(1, 14, 'c');
    dot(2, 14, 'b');
    dot(1, 15, 'b');
  }
  // A long rifle pointing right: stock and barrel rings in the colour of the tier, a steel
  // receiver with a glowing coil, a scope with its lens, the grip under it, the muzzle glowing.
  function rifle(dot) {
    box(dot, 1, 7, 3, 3, 'a');
    box(dot, 1, 7, 3, 1, 'c');
    box(dot, 1, 9, 3, 1, 'b');
    box(dot, 4, 6, 7, 3, 'n');
    box(dot, 4, 6, 7, 1, 'm');
    box(dot, 5, 7, 5, 1, 'e');
    dot(9, 7, 'g');
    box(dot, 11, 7, 4, 1, 'o');
    box(dot, 11, 8, 4, 1, 'm');
    dot(12, 7, 'c');
    dot(12, 8, 'a');
    dot(15, 7, 'g');
    dot(15, 8, 'e');
    box(dot, 6, 4, 4, 1, 'x');
    dot(10, 4, 'u');
    dot(7, 5, 'x');
    box(dot, 6, 9, 2, 3, 'b');
    dot(8, 9, 'x');
  }
  // An excavator bucket full of earth on the end of its steel boom, a pin where the boom holds it
  // and a row of steel teeth along its lip.
  function scoop(dot) {
    for (let i = 0; i < 5; i++) {
      dot(1 + i, 1 + i, 'm');
      dot(2 + i, 1 + i, 'o');
      dot(1 + i, 2 + i, 'n');
    }
    for (let x = 7; x <= 10; x++) dot(x, 4, 'h');
    for (let x = 5; x <= 12; x++) dot(x, 5, x % 3 ? 'i' : 'h');
    for (let x = 4; x <= 13; x++) dot(x, 6, 'c');
    for (let y = 7; y <= 10; y++) for (let x = 4; x <= 13; x++) dot(x, y, x === 4 ? 'c' : x === 13 ? 'b' : 'a');
    for (let x = 5; x <= 13; x++) dot(x, 11, x === 13 ? 'b' : 'a');
    for (let x = 6; x <= 13; x++) dot(x, 12, 'b');
    for (const x of [7, 9, 11, 13]) dot(x, 13, 'o');
    dot(5, 7, 'x');
    for (let x = 6; x <= 12; x++) dot(x, 9, 'b');
  }
  // A cartographer's drum: a cylinder wrapped in a map (land, water and a route) on a stand, with
  // an antenna and its glowing tip on top.
  function mapDrum(dot) {
    dot(8, 1, 'e');
    for (let y = 2; y <= 4; y++) dot(8, y, 'n');
    for (let x = 6; x <= 10; x++) if (x !== 8) dot(x, 3, 'm');
    for (let x = 4; x <= 11; x++) dot(x, 5, 'c');
    for (let x = 3; x <= 12; x++) dot(x, 6, x === 3 || x === 12 ? 'a' : 'c');
    for (let y = 7; y <= 11; y++)
      for (let x = 3; x <= 12; x++) dot(x, y, x === 3 ? 'a' : x === 12 ? 'b' : 'o');
    for (const [x, y] of [[5, 8], [6, 8], [5, 9], [6, 9], [7, 9], [9, 7], [10, 7], [10, 8], [11, 8]]) dot(x, y, 'q');
    for (const [x, y] of [[4, 11], [5, 11], [9, 10], [10, 10], [10, 11], [11, 11]]) dot(x, y, 'u');
    for (const [x, y] of [[7, 7], [8, 8], [8, 10]]) dot(x, y, 'r');
    for (let x = 4; x <= 11; x++) dot(x, 12, 'b');
    for (let x = 6; x <= 9; x++) dot(x, 13, 'n');
    for (let x = 5; x <= 10; x++) dot(x, 14, 'm');
  }
  // A tech compass: a steel case with a lug, a dark glass face in a cyan bezel, four ticks and a
  // needle, red to the north-east and white behind.
  function compass(put) {
    for (let x = 6; x <= 9; x++) put(x, 1, 'k');
    put(6, 2, 'k');
    put(7, 2, 'm');
    put(8, 2, 'm');
    put(9, 2, 'k');
    for (let y = 0; y < 16; y++)
      for (let x = 0; x < 16; x++) {
        const d = Math.hypot(x - 7.5, y - 8.5);
        if (d >= 6.5) continue;
        put(x, y, d >= 5.7 ? 'k' : d >= 4.6 ? (x + y < 15 ? 'c' : 'b') : d >= 3.9 ? 'f' : 'x');
      }
    for (const [x, y] of [[7, 5], [8, 5], [7, 12], [8, 12], [4, 8], [4, 9], [11, 8], [11, 9]]) put(x, y, 'e');
    put(5, 6, 'n');
    put(6, 6, 'n');
    put(5, 7, 'n');
    put(10, 6, 'r');
    put(9, 7, 'r');
    put(8, 8, 'm');
    put(7, 9, 'm');
    put(6, 10, 'w');
    put(5, 11, 'w');
  }
  // Four ingots climbing like stairs, one per tier.
  function stairs(put) {
    for (let t = 1; t <= 4; t++) {
      const x0 = 1 + (t - 1) * 3, y0 = 12 - (t - 1) * 3;
      for (let y = 0; y < 4; y++)
        for (let x = 0; x < 6; x++) {
          const edge = y === 0 || y === 3 || x === 0 || x === 5;
          put(x0 + x, y0 + y, edge ? 'x' : y === 1 ? 'L' + t : 'A' + t);
        }
    }
  }

  // ---------------------------------------------------------------- rendering
  const artCache = {};
  RC.iconArt = (id) => {
    const spec = ART[id];
    if (!spec) return null;
    if (artCache[id]) return artCache[id];
    const c = document.createElement('canvas');
    c.width = c.height = 16;
    const g = c.getContext('2d');
    const pal = palette(spec.a, spec.e);
    for (let t = 1; t <= 4; t++) {
      pal['A' + t] = T[t];
      pal['L' + t] = shade(T[t], 0.35);
    }
    const put = (x, y, ch) => {
      if (ch === '.' || ch === ' ' || x < 0 || y < 0 || x > 15 || y > 15) return;
      g.fillStyle = pal[ch] || '#ff00ff';
      g.fillRect(x, y, 1, 1);
    };
    const paint = (x0, y0, rows) => rows.forEach((row, y) => [...row].forEach((ch, x) => put(x0 + x, y0 + y, ch)));
    if (spec.draw) spec.draw(put);
    else paint(0, 0, spec.rows);
    for (const [x, y, rows] of spec.over || []) paint(x, y, rows);
    return (artCache[id] = c);
  };
  // Every id that has a drawing, for checks.
  RC.ICON_ART_IDS = Object.keys(ART);

  // ---------------------------------------------------------------- pointing items at pictures
  // Base-game items show the game's own texture when the client provided it.
  const VANILLA = {
    iron: 'iron_ingot', copper: 'copper_ingot', gold: 'gold_ingot', netherite: 'netherite_ingot', redstone: 'redstone',
    diamond: 'diamond', quartz: 'quartz', lapis: 'lapis_lazuli', pearl: 'ender_pearl', coal: 'coal', brick: 'brick',
    stick: 'stick', hopper: 'hopper', bucket: 'bucket', blaze: 'blaze_rod', glowstone: 'glowstone_dust', sugar: 'sugar',
    bonemeal: 'bone_meal', honey: 'honey_bottle', wart: 'nether_wart', string: 'string', leather: 'leather',
    book: 'book', compass: 'compass_00', pick: 'iron_pickaxe', shears: 'shears', bow: 'bow', water: 'water_bucket',
    slime: 'slime_ball', spider_eye: 'spider_eye', melon_glis: 'glistering_melon_slice',
  };
  // Base-game blocks are drawn as blocks, as in the game's inventory; their models in models.js
  // fall back to the hand-drawn look without the client.
  const BLOCKS = { table: 'table', vfurnace: 'vfurnace', dropper: 'dropper', wool: 'wool', obsidian: 'obsidian' };
  for (const id in RC.ITEMS) {
    const it = RC.ITEMS[id];
    if (BLOCKS[id]) it.icon = ['block', BLOCKS[id]];
    else if (VANILLA[id] && RC.mcImage('item/' + VANILLA[id])) it.icon = ['mc', 'item/' + VANILLA[id]];
    else if (ART[id] && it.icon[0] !== 'block') it.icon = ['art', id];
  }
})();
