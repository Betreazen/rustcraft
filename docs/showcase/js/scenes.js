'use strict';
// Animated scenes of the guide. A scene is a list of steps; every step adds to the scene state
// and carries one caption. Scene ids match entry ids from data.js.
(function () {
  const S = (RC.SCENES = {});
  const IT = '#ffa53c', EN = '#ffe14a', FL = '#4c9bf5', SOL = '#59d38c', LAVA = '#f2731c';
  const step = (text, run, dur) => ({ text, run, dur });

  const ENV = new Set(['bricks', 'stone', 'deepslate', 'dirt', 'grass', 'planks', 'log', 'leaves', 'bedrock', 'glass', 'lava', 'lavapool', 'water', 'farmland', 'geode', 'rail', 'capsule', 'worker', 'tree', 'crop', 'mob', 'player', 'drop', 'alloyblock',
    'carrots', 'potatoes', 'beetroots', 'nether_wart', 'torchflower', 'sugar_cane', 'cactus', 'bamboo', 'kelp', 'pumpkin', 'melon', 'pumpkin_stem', 'melon_stem',
    'sweet_berry_bush', 'cave_vines', 'cocoa', 'jungle_log', 'soul_sand']);
  const DIRS = [['x+', 1, 0, 0], ['x-', -1, 0, 0], ['y+', 0, 1, 0], ['y-', 0, -1, 0], ['z+', 0, 0, 1], ['z-', 0, 0, -1]];
  // Housings connect on their own to every neighbouring housing and machine.
  function link(s) {
    const at = new Map();
    for (const o of s.objs.values()) if (!o.path) at.set(o.x + ',' + o.y + ',' + o.z, o);
    for (const o of s.objs.values()) {
      if (o.type === 'rail' && (!o.face || o.face === 'floor' || o.face === 'ceil')) {
        o.con = DIRS.slice(0, 4).filter(([, dx, dy]) => {
          const n = at.get(o.x + dx + ',' + (o.y + dy) + ',' + o.z);
          return n && n.type === 'rail' && (n.face || 'floor') === (o.face || 'floor') && (n.tier || 1) === (o.tier || 1);
        }).map((d) => d[0]);
      }
      if (o.type !== 'pipe') continue;
      o.con = DIRS.filter(([, dx, dy, dz]) => {
        const n = at.get(o.x + dx + ',' + (o.y + dy) + ',' + (o.z + dz));
        return n && !ENV.has(n.type) && !n.type.startsWith('ore_');
      }).map((d) => d[0]).join('');
    }
  }
  // Name tags: one per kind of block, on the first object of that kind.
  function autoTag(s) {
    const kindOf = (o) => (o.type === 'worker' ? 'worker:' + o.tool : o.type);
    s.tags = s.tags.filter((t) => s.objs.has(t.obj) && kindOf(s.objs.get(t.obj)) === t.kind);
    const have = new Set(s.tags.map((t) => t.kind));
    for (const o of s.objs.values()) {
      const kind = kindOf(o);
      if (!RC.TAGGED.has(o.type) || have.has(kind)) continue;
      const info = RC.info(o);
      have.add(kind);
      s.tags.push({ obj: o.id, kind, text: info.name, lift: info.lift });
    }
  }
  const scene = (id, steps) => {
    S[id] = { steps: steps.map((st) => ({ text: st.text, dur: st.dur, run: (s) => { st.run(s); link(s); autoTag(s); } })) };
  };
  const floor = (s, x0, y0, nx, ny, type, z) => {
    for (let i = 0; i < nx; i++) for (let j = 0; j < ny; j++) s.put('fl' + (x0 + i) + '_' + (y0 + j) + '_' + (z || -1), type || 'stone', x0 + i, y0 + j, z == null ? -1 : z);
  };

  // The usual layout: blocks stand in a row at y = 0, one trunk of housings runs in front of them
  // at y = 1, a power source sits on the left end and the node on the right end.
  function bus(s, blocks, o) {
    o = o || {};
    const n = blocks.length;
    floor(s, -1, 0, n + 2, 2);
    blocks.forEach((b, i) => b && s.put('b' + i, b[0], i, 0, 0, b[1]));
    for (let i = 0; i < n; i++) s.put('p' + i, 'pipe', i, 1, 0, { ch: o.ch || 'ie--' });
    if (o.src !== null) s.put('src', o.src || 'crystal', -1, 1, 0, o.srcProps);
    s.put('node', 'node', n, 1, 0, { tier: o.tier || 1 });
  }
  const r = (a, b) => [[a, 0, 0], [a, 1, 0], [b, 1, 0], [b, 0, 0]];
  const fromSrc = (b) => [[-1, 1, 0], [b, 1, 0], [b, 0, 0]];
  const power = (s, ...targets) => targets.forEach((b) => s.flow('en' + b, fromSrc(b), { color: EN, n: 2 }));
  // A second crystal that stands apart from the first one, so the two do not merge into a structure.
  const crystal2 = (s, target) => {
    s.put('flc2', 'stone', 1, 2, -1); s.put('src2', 'crystal', 1, 2, 0);
    if (target != null) s.flow('en2', [[1, 2, 0], [1, 1, 0], [target, 1, 0], [target, 0, 0]], { color: EN, n: 2 });
  };

  // A cliff edge for monorail scenes: three cells of floor, then a wall behind the path over a
  // lava drop, then a ceiling. The capsule travels straight through all three at the same height.
  function canyon(s, x0, y) {
    for (let i = 0; i < 9; i++) {
      const x = x0 + i;
      if (i < 3) { s.put('cf' + i, 'stone', x, y, -1); s.put('cg' + i, 'stone', x, y, -2); }
      else { s.put('cl' + i, 'lavapool', x, y, -2); s.put('cm' + i, 'stone', x, y - 1, -2); }
      if (i >= 3 && i < 6) { s.put('cw' + i, 'stone', x, y - 1, -1); s.put('cv' + i, 'stone', x, y - 1, 0); }
      if (i >= 6) { s.put('cc' + i, 'stone', x, y, 1); s.put('cd' + i, 'stone', x, y - 1, 1); }
    }
  }
  const canyonRail = (s, x0, y, from, to) => {
    for (let i = from; i < to; i++) s.put('cr' + i, 'rail', x0 + i, y, 0, { face: i < 3 ? 'floor' : i < 6 ? 'wy' : 'ceil', dir: 'x' });
  };
  const canyonRide = (x0, y) => [[x0, y, 0, 'below'], [x0 + 3, y, 0, 'side'], [x0 + 6, y, 0, 'above'], [x0 + 8, y, 0, 'above']];

  const TRUNK = 'Проложи вдоль машины магистраль: корпуса с каналами предметов и энергии. На концах стоят кристалл и узел магистрали.';

  // ================================================================ start
  scene('overview', [
    step('Внутри базы всё связывает одна магистраль: предметы и энергия идут в одном корпусе.', (s) => {
      bus(s, [['chest'], ['crusher', { on: true }], ['furnace', { on: true }], ['chest']]);
      s.flow('a', r(0, 1)); s.flow('b', r(1, 2), { color: '#d8a47a' }); s.flow('c', r(2, 3), { color: '#d8d8d8' }); power(s, 1); crystal2(s, 2);
    }, 7),
    step('Когда что-то не работает, блок сам называет причину.', (s) => {
      s.del('src', 'src2'); s.flows = []; s.set('b1', { on: false }).set('b2', { on: false });
      s.badge('d', 1, 0, 0, 'Нет энергии');
    }, 5),
    step('Между базами ведёт монорельс. На поворотах он скруглён, вверх идёт лесенкой под 45°. Капсула всегда остаётся ровной.', (s) => {
      s.objs.clear(); s.badges = []; s.flows = []; climb(s, 0, 0);
      s.put('cap', 'capsule', 0, 4, 0).move('cap', climbRide(0, 0), 1.6, 'ping');
    }, 12),
    step('Рельс крепится к полу, стене и потолку. Меняется только то, с какой стороны капсула держится за рельс.', (s) => {
      s.objs.clear(); canyon(s, -2, 1); canyonRail(s, -2, 1, 0, 9);
      s.put('cap', 'capsule', -2, 1, 0).move('cap', canyonRide(-2, 1), 1.8, 'ping');
    }, 10),
  ]);

  scene('quickstart', [
    step('Найди под землёй друзу кристаллов и добудь осколки. Кристаллики растут на ней со всех сторон.', (s) => {
      floor(s, 0, 0, 3, 3); s.put('g', 'geode', 1, 1, 0); s.put('pl', 'player', 0, 2, 0);
      s.put('d', 'drop', 2, 2, 0, { color: '#56dbe2' });
    }),
    step('Сделай на верстаке медносталь из меди и железа. Это первый сплав, из него делаются все блоки первой ступени.', (s) => {
      s.objs.clear(); floor(s, -1, 0, 5, 2); s.put('tb', 'table', 1, 0, 0); s.put('pl', 'player', 0, 0, 0, { face: 'x+' });
      s.badge('a', 1, 0, 0, 'медь + железо → 2 медностали', 'info');
    }, 8),
    step('Сделай два кристалла, узел магистрали, корпуса и каналы в каждый корпус. Кристаллы ставь порознь.', (s) => {
      s.objs.clear(); s.badges = []; bus(s, [['chest'], ['crusher'], ['furnace'], ['chest']]); crystal2(s);
    }, 8),
    step('Положи руду в левый сундук. Она сама станет слитками в правом.', (s) => {
      s.set('b1', { on: true }).set('b2', { on: true });
      s.flow('a', r(0, 1)); s.flow('b', r(1, 2), { color: '#d8a47a' }); s.flow('c', r(2, 3), { color: '#d8d8d8' }); power(s, 1); crystal2(s, 2);
    }, 8),
    step('Позже, на второй ступени, правый сундук сменит склад: ячейки и терминал на той же магистрали. Слитки уходят прямо на склад.', (s) => {
      s.del('b3'); s.put('b3', 'terminal', 3, 0, 0); floor(s, 3, -1, 2, 1);
      for (const x of [3, 4]) for (const z of [0, 1]) s.put('qc' + x + z, 'cell', x, -1, z);
    }, 9),
  ]);

  scene('progress', [
    step('Четыре ступени, у каждой свой сплав: медносталь, искросталь, кристаллит, звёздная сталь.', (s) => {
      for (let t = 1; t <= 4; t++) { s.put('a' + t, 'alloyblock', -2, t - 1, 0, { tier: t }); s.badge('n' + t, -2, t - 1, -0.9, String(t), 'info'); }
    }, 7),
    step('Рельсы из сплава своей ступени быстрее: 8, 14, 22 и 32 блока в секунду.', (s) => {
      for (let t = 1; t <= 4; t++) {
        const y = t - 1;
        for (let x = 0; x < 7; x++) { s.put('fl' + t + x, 'stone', x, y, -1); s.put('r' + t + x, 'rail', x, y, 0, { tier: t, dir: 'x' }); }
        s.put('c' + t, 'capsule', 0, y, 0).move('c' + t, [[0, y, 0], [6, y, 0]], [0, 1, 1.75, 2.75, 4][t], 'ping');
      }
    }, 10),
  ]);

  scene('xp', [
    step('Добытчик поставил игрок. Он считается владельцем машины.', (s) => {
      bus(s, [['chest'], ['miner', { on: true }]], { ch: 'ie--' }); s.put('pl', 'player', 3, 0, 0); floor(s, 3, 0, 1, 1);
      s.flow('o', r(1, 0)); power(s, 1); s.badge('x', 3, 0, 0.2, 'владелец', 'info');
    }),
    step('За каждую добытую руду владелец получает половину опыта, как за добычу руками.', (s) => s.badge('x', 3, 0, 0.2, '+50% опыта', 'ok'), 6),
    step('Пользоваться машиной при этом может любой игрок: в RustCraft всё общее.', (s) => {
      s.put('fl2', 'stone', -1, 2, -1); s.put('pl2', 'player', -1, 2, 0, { armor: true }); s.badge('y', -1, 2, 0.2, 'другой игрок берёт руду', 'info');
    }, 6),
  ]);

  // ================================================================ pipe
  // A source that a housing can join (crystal, solar panel, windmill) shows its connector only on
  // the sides where a housing or a node touches it: plugScene sets their link prop every step.
  const PLUGS = new Set(['crystal', 'solar', 'windmill']), HOUSINGS = new Set(['pipe', 'node', 'node_hub']);
  function plug(s) {
    const at = new Set();
    for (const o of s.objs.values()) if (!o.path && HOUSINGS.has(o.type)) at.add(o.x + ',' + o.y + ',' + o.z);
    for (const o of s.objs.values())
      if (PLUGS.has(o.type)) o.link = DIRS.filter(([, dx, dy, dz]) => at.has(o.x + dx + ',' + (o.y + dy) + ',' + (o.z + dz))).map((d) => d[0]);
  }
  const plugScene = (id, steps) => scene(id, steps.map((st) => step(st.text, (s) => { st.run(s); plug(s); }, st.dur)));

  const slots = (s, text) => s.badge('g', 2, 1, 0, 'гнёзда: ' + text, 'info');
  plugScene('housing', [
    step('Корпус — пустая труба с четырьмя гнёздами. Сам по себе он ничего не несёт.', (s) => {
      floor(s, 0, 0, 5, 2); s.put('b0', 'chest', 0, 0, 0); s.put('b4', 'furnace', 4, 0, 0);
      for (let i = 0; i < 5; i++) s.put('p' + i, 'pipe', i, 1, 0, { ch: '----' });
      slots(s, 'пусто · пусто · пусто · пусто');
    }),
    step('Вставь предметный канал в каждый корпус линии. Он занял первое гнездо, и предметы пошли.', (s) => {
      for (let i = 0; i < 5; i++) s.set('p' + i, { ch: 'i---' });
      s.flow('i', r(0, 4)); slots(s, 'предметы · пусто · пусто · пусто');
    }, 7),
    step('Добавь энергетический канал во второе гнездо и поставь кристалл: печь заработала без отдельного провода.', (s) => {
      for (let i = 0; i < 5; i++) s.set('p' + i, { ch: 'ie--' });
      s.put('src', 'crystal', -1, 1, 0); floor(s, -1, 1, 1, 1); s.flow('e', [[-1, 1, 0], [4, 1, 0], [4, 0, 0]], { color: EN }); s.set('b4', { on: true });
      slots(s, 'предметы · энергия · пусто · пусто');
    }, 7),
    step('Третье гнездо — жидкость. Три вида ресурсов идут в одном блоке.', (s) => {
      for (let i = 0; i < 5; i++) s.set('p' + i, { ch: 'ief-' });
      s.flow('f', [[0, 1, 0], [4, 1, 0]], { color: FL, n: 2 }); slots(s, 'предметы · энергия · жидкость · пусто');
    }, 7),
    step('В четвёртое гнездо встаёт любой канал, например второй предметный: по нему идёт отдельный поток.', (s) => {
      for (let i = 0; i < 5; i++) s.set('p' + i, { ch: 'iefi' });
      s.flow('back', r(4, 0), { color: '#d8d8d8', n: 2 }); slots(s, 'предметы · энергия · жидкость · предметы');
    }, 7),
  ]);

  plugScene('node', [
    step('Корпус работает, пока по трубе до ближайшего узла не больше его предела. У узла первой ступени это 32 блока; в сцене линия укорочена.', (s) => {
      floor(s, 0, 0, 12, 1); s.put('node', 'node', 0, 0, 0);
      for (let i = 1; i < 7; i++) s.put('p' + i, 'pipe', i, 0, 0, { ch: 'ie--' });
      for (let i = 7; i < 10; i++) s.put('p' + i, 'pipe', i, 0, 0, { ch: 'ie--', bad: true });
      s.flow('f', [[0, 0, 0], [6, 0, 0]], { color: EN }); s.badge('b', 8, 0, 0, 'Дальше предела узла');
    }, 9),
    step('Поставь второй узел прямо в линию. Он продлил её ещё на свой предел.', (s) => {
      s.badges = []; s.put('p7', 'node', 7, 0, 0);
      for (let i = 8; i < 12; i++) s.put('p' + i, 'pipe', i, 0, 0, { ch: 'ie--' });
      s.flow('f', [[0, 0, 0], [11, 0, 0]], { color: EN, n: 5 });
    }, 8),
    step('У узла первых трёх ступеней грани связаны попарно, и линия идёт сквозь него только прямо. Вторая магистраль пересекает первую через тот же узел и не смешивается с ней.', (s) => {
      for (const y of [-2, -1, 1, 2]) { s.put('fy' + y, 'stone', 7, y, -1); s.put('q' + y, 'pipe', 7, y, 0, { ch: 'f---' }); }
      s.flow('g', [[7, -2, 0], [7, 2, 0]], { color: FL, n: 3 }); s.badge('x', 7, 0, 0, 'две линии, не смешиваются', 'info');
    }, 9),
    step('Узел четвёртой ступени — развилка: все шесть граней связаны. Предметы, вода и энергия сворачивают в нём туда, где их ждёт приёмник.', (s) => {
      junction(s);
      s.flow('i', [[1, -1, 0], [1, 0, 0], [4, 0, 0], [4, 3, 0]]); s.flow('f', [[2, -1, 0], [2, 0, 0], [4, 0, 0], [4, -3, 0]], { color: FL, n: 3 });
      s.flow('e', [[0, 0, 0], [7, 0, 0]], { color: EN, n: 4 });
    }, 9),
    step('Из сундука идут железная руда и алмазы. Руду берёт дробилка в конце линии, а алмазы она не принимает: на развилке они сворачивают в сундук.', (s) => {
      s.put('end', 'crusher', 7, 0, 0, { on: true }); s.flow('i', null);
      s.flow('ore', [[1, -1, 0], [1, 0, 0], [7, 0, 0]], { color: '#d8a47a', n: 4 }); s.flow('gem', [[1, -1, 0], [1, 0, 0], [4, 0, 0], [4, 3, 0]], { color: '#5fe6dd', n: 3 });
      s.badge('o', 7, 0, 0, 'сюда руда', 'info'); s.badge('d', 4, 3, 0, 'сюда алмазы', 'info');
    }, 9),
  ]);
  // A cross of housings round the junction at (4, 0): an energy crystal on the left end, a chest
  // and a water tank beside the line, a chest on the y+ branch, a tank on the y- branch and a
  // furnace at the far end. The order of puts picks the objects that carry the name tags (the
  // first of each kind), so the tags spread over the picture.
  function junction(s) {
    s.objs.clear(); s.flows = []; s.badges = [];
    floor(s, 0, -3, 8, 7);
    s.put('hub', 'node_hub', 4, 0, 0);
    for (const x of [2, 1, 3, 5, 6]) s.put('m' + x, 'pipe', x, 0, 0, { ch: 'ife-' });
    for (const y of [-2, -1, 1, 2]) s.put('n' + y, 'pipe', 4, y, 0, { ch: 'ife-' });
    s.put('src', 'crystal', 0, 0, 0); s.put('co', 'chest', 4, 3, 0); s.put('to', 'tank', 4, -3, 0, { fluid: 'water', level: 0.4 });
    s.put('ci', 'chest', 1, -1, 0); s.put('tw', 'tank', 2, -1, 0, { fluid: 'water', level: 0.8 }); s.put('end', 'furnace', 7, 0, 0, { on: true });
  }

  scene('ch_item', [
    step('Предметный канал переносит предметы из источника в приёмники. Одинаковые приёмники получают их по очереди.', (s) => {
      bus(s, [['chest'], ['chest'], ['chest'], null], { ch: 'i---', src: null });
      s.flow('a', r(0, 1)); s.flow('b', r(0, 2), { n: 2 });
      s.badge('s', 0, 0, 0, 'источник', 'info'); s.badge('t', 2, 0, 0, 'приёмник', 'info');
    }, 8),
    step('Приёмник с более высоким приоритетом заполняется первым. Приоритет задаётся ключом на стыке.', (s) => {
      s.badges = []; s.flow('a', null); s.flow('b', r(0, 2), { n: 3 });
      s.badge('p', 2, 0, 0, 'приоритет 2', 'info'); s.badge('q', 1, 0, 0, 'приоритет 1', 'info');
    }, 8),
    step('Когда приёмникам некуда класть, предмет остаётся в источнике. В трубе ничего не застревает и на землю не выпадает.', (s) => {
      s.flows = []; s.badges = []; s.badge('f', 2, 0, 0, 'Приёмник полон');
    }, 8),
    step('Дробилке нужна руда, а алмазы она не принимает. Они уходят в сундук со стыком «в сундук»; без такого сундука алмазы остались бы в источнике.', (s) => {
      s.badges = []; s.put('b1', 'crusher', 1, 0, 0, { on: true }); s.put('k', 'crystal', 1, 0, 1);
      s.flow('ore', r(0, 1), { color: '#d8a47a', n: 2 }); s.flow('gem', r(0, 2), { color: '#5fe6dd', n: 2 }); s.flow('dust', r(1, 2), { color: '#c9a27a', n: 1 });
      s.flow('en', [[1, 0, 1], [1, 0, 0]], { color: EN, n: 1, speed: 0.8 }); s.badge('t', 2, 0, -0.9, 'стык «в сундук»', 'info');
    }, 9),
  ]);

  // A pump over a pool of water at the origin. pumpLine adds a line of housings with a fluid and
  // an energy channel at y = 1 from x = 0 to n - 1, the node on its left end and an energy crystal
  // standing on the node, so nothing stands in front of the blocks at y = 0.
  const POOL = [['w', 0, 0], ['w2', -1, 0], ['w3', -1, -1], ['w4', 0, -1]];
  function pumpPool(s, n) {
    floor(s, -1, 0, n + 1, 2); s.del('fl0_0_-1', 'fl-1_0_-1');
    for (const [id, x, y] of POOL) s.put(id, 'water', x, y, -1);
    s.put('b0', 'pump', 0, 0, 0);
  }
  function pumpLine(s, n) {
    for (let i = 0; i < n; i++) s.put('p' + i, 'pipe', i, 1, 0, { ch: 'fe--' });
    s.put('node', 'node', -1, 1, 0); s.put('src', 'crystal', -1, 1, 1);
  }
  const pumpPower = (s, ...targets) => targets.forEach((b) => s.flow('en' + b, [[-1, 1, 1], [-1, 1, 0], [b, 1, 0], [b, 0, 0]], { color: EN, n: 2 }));
  plugScene('ch_fluid', [
    step('Жидкостный канал первой ступени несёт одну жидкость. Насос над водой качает её в бак.', (s) => {
      pumpPool(s, 9); pumpLine(s, 9); s.set('b0', { on: true });
      s.put('tw', 'tank', 6, 0, 0, { fluid: 'water', level: 0.3, ease: 2 }); s.flow('w', r(0, 6), { color: FL }); pumpPower(s, 0);
    }, 8),
    step('Канал второй ступени несёт две жидкости сразу. Лава из бака идёт в лавовый генератор, вода в свой бак, и они не смешиваются.', (s) => {
      s.put('tl', 'tank', 2, 0, 0, { fluid: 'lava', level: 0.9, ease: 2 }); s.put('gl', 'gen_lava', 4, 0, 0, { on: true });
      s.set('tw', { level: 0.5 }); s.flow('l', r(2, 4), { color: LAVA, n: 2 }); s.badge('c', 7, 1, 0, 'канал 2-й ступени: 2 жидкости', 'info');
    }, 9),
    step('Смеситель хочет отдать раствор — третью жидкость. В канал второй ступени она не помещается, и канал показывает диагноз.', (s) => {
      s.put('mx', 'mixer', 5, 0, 0); s.put('ts', 'tank', 8, 0, 0, { fluid: 'empty', level: 0 });
      s.set('tw', { level: 0.6 }).set('tl', { level: 0.75 }); s.badges = []; s.badge('d', 5, 1, 0.6, 'В канале нет места для новой жидкости');
    }, 8),
    step('Канал третьей ступени несёт четыре жидкости, и раствор пошёл в свой бак. Можно и иначе: второй жидкостный канал в соседнем гнезде.', (s) => {
      s.set('mx', { on: true }).set('ts', { fluid: 'sol', level: 0.5 }).set('tw', { level: 0.7 }).set('tl', { level: 0.6 });
      s.flow('wm', r(0, 5), { color: FL, n: 2 }); s.flow('sol', r(5, 8), { color: SOL, n: 2 }); pumpPower(s, 5);
      s.badges = []; s.badge('c', 7, 1, 0, 'канал 3-й ступени: до 4 жидкостей', 'info');
    }, 9),
  ]);
  plugScene('pump', [
    step('Насос ставится прямо над источником воды или лавы.', (s) => pumpPool(s, 3)),
    step('Подведи магистраль с жидкостным и энергетическим каналом и поставь бак.', (s) => { pumpLine(s, 3); s.put('b2', 'tank', 2, 0, 0, { fluid: 'empty', level: 0 }); }, 7),
    step('Насос забирает воду под собой и отдаёт её в магистраль. Бак наполняется, а вода под насосом не кончается.', (s) => {
      s.set('b0', { on: true }).set('b2', { fluid: 'water', level: 0.8, ease: 2.5 }); s.flow('f', r(0, 2), { color: FL }); pumpPower(s, 0);
    }, 8),
    step('Над лавовым озером насос качает лаву. Лавовое озеро конечно.', (s) => {
      for (const [id, x, y] of POOL) s.put(id, 'lavapool', x, y, -1);
      s.set('b2', { fluid: 'lava', level: 0.4 }); s.flow('f', r(0, 2), { color: LAVA });
    }, 7),
    step('Озеро выкачано: насос останавливается и называет причину.', (s) => {
      for (const [id, x, y] of POOL) { s.del(id); s.put(id + 'b', 'stone', x, y, -2); }
      s.set('b0', { on: false }).set('b2', { level: 0.7 }); s.flow('f', null); s.flow('en0', null); s.badge('d', 0, 0, 0, 'Нет жидкости под насосом');
    }, 7),
  ]);
  plugScene('tank', [
    step('Баку нужен источник жидкости, например насос над водой. Бак подключается жидкостным каналом.', (s) => {
      pumpPool(s, 5); pumpLine(s, 5); s.put('b2', 'tank', 2, 0, 0, { fluid: 'empty', level: 0 });
    }, 7),
    step('Насос качает воду, и уровень в баке растёт. Его видно сквозь стенки и в окне бака.', (s) => {
      s.set('b0', { on: true }).set('b2', { fluid: 'water', level: 0.85, ease: 2.5 }); s.flow('f', r(0, 2), { color: FL }); pumpPower(s, 0);
    }, 8),
    step('Поставь второй бак вплотную, и баки сольются в один большой. Жидкость в нём общая: нижний блок полон, верхний наполняется.', (s) => {
      s.set('b2', { level: 1 }); s.put('b3', 'tank', 2, 0, 1, { fluid: 'water', level: 0.35, ease: 2.5 });
    }, 8),
    step('Бак держит одну жидкость. Для воды и раствора ставят два бака: смеситель берёт воду из линии и наполняет бак раствора.', (s) => {
      s.put('mx', 'mixer', 3, 0, 0, { on: true }); s.put('b4', 'tank', 4, 0, 0, { fluid: 'sol', level: 0.45, ease: 2.5 }); s.set('b3', { level: 0.6 });
      s.flow('m', r(0, 3), { color: FL, n: 2 }); s.flow('sol', r(3, 4), { color: SOL, n: 2 }); pumpPower(s, 3);
      s.badge('c', 4, 1, -0.6, 'канал 2-й ступени: вода и раствор', 'info');
    }, 9),
  ]);

  // The node window shows the balance of the line: what the sources give and the machines take.
  const balance = (s, x, give, take, kind) => s.badge('bal', x, 1, 0, 'узел: дают ' + give + ' · ' + (kind === 'warn' ? 'просят ' : 'тратят ') + take + ' Е/с', kind || 'info');
  plugScene('ch_energy', [
    step('Энергетический канал несёт энергию от источника к машинам. Кристалл даёт 4 Е/с: этого хватает одной печи в экономном режиме.', (s) => {
      bus(s, [['furnace', { on: true }], null, null, null, null]); power(s, 0); balance(s, 5, 4, '2,8');
    }, 8),
    step('Машин стало больше, и энергии не хватает. Все машины получают равную долю и работают медленнее, но магистраль не останавливается.', (s) => {
      s.put('b1', 'crusher', 1, 0, 0, { on: true }); s.put('b2', 'furnace', 2, 0, 0, { on: true });
      [0, 1, 2].forEach((b) => s.flow('en' + b, fromSrc(b), { color: EN, n: 1, speed: 0.5 }));
      balance(s, 5, 4, '8,4', 'warn'); s.badge('w', 1, 0, 0, 'Не хватает энергии: машины замедлены', 'warn');
    }, 8),
    step('Добавь источник, и скорость вернётся. Окно узла показывает баланс: сколько линия производит и сколько тратит.', (s) => {
      s.put('b3', 'gen_fuel', 3, 0, 0, { on: true }); s.badge('w', 0, 0, 0, null);
      [0, 1, 2].forEach((b) => s.flow('en' + b, r(3, b), { color: EN, n: 2, speed: 1.8 })); balance(s, 5, 20, '8,4', 'ok');
    }, 8),
    step('Без канала тоже можно: кристалл питает машину, которая стоит к нему вплотную.', (s) => {
      s.objs.clear(); s.flows = []; s.badges = []; floor(s, -1, 0, 6, 2);
      s.put('k', 'crystal', 0, 0, 0); s.put('f1', 'furnace', 1, 0, 0, { on: true }); s.flow('e', [[0, 0, 0], [1, 0, 0]], { color: EN, n: 1, speed: 0.6 });
    }, 7),
    step('Машина машину не питает. Вторая печь касается только первой и стоит без энергии.', (s) => {
      s.put('f2', 'furnace', 2, 0, 0); s.badge('d', 2, 0, 0, 'Нет энергии');
    }, 7),
  ]);

  plugScene('facade', [
    step('Магистраль идёт вдоль стены и вся на виду.', (s) => {
      floor(s, -1, 0, 7, 2);
      for (let x = -1; x < 6; x++) for (let z = 0; z < 3; z++) if (!(z === 1 && x >= 0 && x < 6)) s.put('w' + x + '_' + z, 'bricks', x, 0, z);
      s.put('b0', 'furnace', 0, 1, 1, { on: true }); s.put('fb0', 'stone', 0, 1, 0);
      for (let x = 0; x < 5; x++) s.put('p' + x, 'pipe', x, 0, 1, { ch: 'ie--' });
      s.put('src', 'crystal', 4, 1, 1); s.put('fsrc', 'stone', 4, 1, 0); s.put('nd', 'node', 5, 0, 1);
      s.flow('e', [[4, 1, 1], [4, 0, 1], [0, 0, 1], [0, 1, 1]], { color: EN, n: 3 });
    }, 7),
    step('Кликни фальш-панелью по каждому корпусу. Пока вид не задан, они выглядят как гладкие серые блоки.', (s) => {
      for (let x = 0; x < 5; x++) s.put('f' + x, 'facade', x, 0, 1);
    }, 8),
    step('Кликни по панелям кирпичом. Стена снова целая, а магистраль внутри работает как раньше.', (s) => {
      for (let x = 0; x < 5; x++) s.set('f' + x, { look: 'bricks' });
    }, 8),
    step('Возьми в руку ключ: фальш-панели становятся прозрачными, видно корпуса и стыки.', (s) => {
      for (let x = 0; x < 5; x++) s.set('f' + x, { alpha: 0.12 });
      s.put('pl', 'player', 2, 1, 0); s.badge('k', 2, 1, 0.4, 'ключ в руке', 'info');
    }, 8),
  ]);

  scene('wrench', [
    step('Там, где корпус касается машины, образуется стык. Клик ключом открывает его настройки.', (s) => {
      bus(s, [['chest'], ['furnace'], ['chest']]);
    }),
    step('Направление задаётся для каждого канала отдельно. Здесь предметы и энергия идут в печь.', (s) => { s.flow('in', r(0, 1)); power(s, 1); s.set('b1', { on: true });
    }, 9),
    step('На стыке правого сундука стоит фильтр: он принимает только слитки.', (s) => { s.flow('out', r(1, 2), { color: '#d8d8d8' });
    }, 9),
    step('Ключ с зажатым приседанием снимает машину: она падает предметом вместе с содержимым и настройками.', (s) => {
      s.badges = []; s.flows = []; s.del('b1'); s.put('d', 'drop', 1, 0, 0, { color: '#6b7380' }); s.badge('k', 1, 0, -0.6, 'машина снята, настройки внутри', 'ok');
    }, 7),
  ]);

  // Orders travel the whole line (spec section 19): dust → electric furnace → ingots → autocrafter
  // (coppersteel) → alloy smelter (sparksteel) → autocrafter → storage. orderLine is in storage.
  scene('orders', [
    step('На линии стоит узел третьей ступени, и она принимает заказы. Автокрафтер сам просит у склада то, чего ему не хватает.', (s) => {
      orderLine(s, ['furnace', 'crafter', 'alloy', 'crafter']);
      s.set('b2', { on: true }); s.flow('a', r(0, 2), { color: '#b08a52', n: 2 }); s.badge('q', 2, 0, 0, 'запрос: 8 досок', 'info');
    }, 9),
    step('Заказ из терминала проходит через все машины линии. На складе только пыль, а для заказа нужна искросталь.', (s) => {
      s.flows = []; s.badges = []; s.set('b2', { on: false }).set('b1', { on: true });
      s.badge('t', 0, 0, 0, 'заказ: искросталь из пыли', 'info'); s.flow('d', r(0, 1), { color: '#d8a47a', n: 2 });
    }, 9),
    step('Пыль плавится в электропечи. Слитки идут прямо в автокрафтер, и он делает медносталь.', (s) => {
      s.set('b2', { on: true }); s.flow('i', r(1, 2), { color: '#d8d8d8', n: 2 });
    }, 8),
    step('Медносталь идёт в плавильню сплавов, искросталь — во второй автокрафтер. Готовый предмет уходит на склад.', (s) => {
      s.set('b3', { on: true }).set('b4', { on: true });
      s.flow('a1', r(2, 3), { color: RC.TIER_COLOR[1], n: 2 }); s.flow('a2', r(3, 4), { color: RC.TIER_COLOR[2], n: 2 }); s.flow('out', r(4, 0), { color: '#f2f2f2', n: 2 });
    }, 9),
    step('«Держать N»: когда предмета на складе меньше N, склад сам заказывает разницу.', (s) => {
      s.flows = []; ['b1', 'b2', 'b3', 'b4'].forEach((b) => s.set(b, { on: false })); s.set('b2', { on: true });
      s.badge('t', 0, 0, 0, 'держать 64 · есть 50 → заказ 14', 'ok'); s.flow('k', r(0, 2), { color: '#b08a52', n: 2 }); s.flow('kb', r(2, 0), { color: '#f2f2f2', n: 1 });
    }, 8),
  ]);

  // ================================================================ energy
  plugScene('crystal', [
    step('Кристалл энергии сразу даёт энергию, топливо ему не нужно. Машину, которая стоит к нему вплотную, он питает и без магистрали.', (s) => {
      floor(s, -1, 0, 3, 2); s.put('k', 'crystal', -1, 0, 0); s.put('b0', 'furnace', 0, 0, 0, { on: true });
      s.flow('d', [[-1, 0, 0], [0, 0, 0]], { color: EN, n: 1, speed: 0.6 }); s.badge('e', -1, 0, 0, '4 Е/с', 'ok');
    }, 8),
    step('К дальним машинам энергия идёт по магистрали. Поставь кристалл вплотную к корпусу: присоска появится на той грани, которой он касается.', (s) => {
      s.del('k'); s.flow('d', null); s.badges = []; bus(s, [['furnace', { on: true }], null]); power(s, 0);
    }, 8),
    step('Кристалл можно поставить и прямо на корпус: он подключается снизу. Одного кристалла хватает на одну машину в экономном режиме.', (s) => {
      s.del('src'); s.put('pe', 'pipe', -1, 1, 0, { ch: 'ie--' }); s.put('src', 'crystal', -1, 1, 1);
      s.flow('en0', [[-1, 1, 1], [-1, 1, 0], [0, 1, 0], [0, 0, 0]], { color: EN, n: 2 }); s.badge('e', -1, 1, 1, '4 Е/с', 'ok');
    }, 8),
    step('Не ставь два кристалла вплотную, пока нет раствора: они сразу сольются в структуру, а она без раствора даёт столько же, сколько один.', (s) => {
      s.del('src'); s.put('c1', 'crystalblock', -1, 1, 1); s.put('fc', 'stone', -1, 0, 0); s.put('c2', 'crystalblock', -1, 0, 1);
      s.badge('e', -1, 1, 1, 'Нет раствора: 4 Е/с');
    }, 8),
  ]);
  const dropKind = (s, ...types) => { for (const o of [...s.objs.values()]) if (types.includes(o.type)) s.del(o.id); };
  plugScene('structure', [
    step('Одиночный кристалл даёт 4 Е/с.', (s) => {
      floor(s, 0, 0, 6, 4); s.put('k', 'crystal', 1, 1, 0); s.badge('e', 1, 1, 0, '4 Е/с', 'ok');
    }),
    step('Поставь второй кристалл вплотную. Блок 2×1 полный, и кристаллы сразу слились в одну цельную структуру.', (s) => {
      s.del('k'); s.put('s1', 'crystalblock', 1, 1, 0); s.put('s2', 'crystalblock', 2, 1, 0); s.badge('e', 1, 1, 0, 'Нет раствора: 4 Е/с');
    }, 8),
    step('Размер 3×2 не из списка: кристаллы не сливаются, и структура называет причину.', (s) => {
      dropKind(s, 'crystalblock'); s.fill('k', 'crystal', 0, 1, 0, 3, 2); s.badge('e', 1, 1, 1.2, 'Нарушена форма структуры');
    }, 8),
    step('Убери два лишних, и квадрат 2×2 слился. С питательным раствором из смесителя структура даёт 32 Е/с.', (s) => {
      dropKind(s, 'crystal'); s.fill('s', 'crystalblock', 1, 1, 0, 2, 2, { fed: true });
      s.put('mix', 'mixer', 5, 1, 0, { on: true }); s.put('p3', 'pipe', 3, 1, 0, { ch: 'f---' }); s.put('p4', 'pipe', 4, 1, 0, { ch: 'f---' });
      s.flow('f', [[5, 1, 0], [2, 1, 0]], { color: SOL }); s.badge('e', 1, 1, 0, '2×2: 32 Е/с', 'ok');
    }, 9),
    step('Структура 3×3 даёт 108 Е/с. Самая большая, 3×3×3, даёт 561 Е/с: на неё нужно 27 кристаллов.', (s) => {
      dropKind(s, 'crystalblock'); for (let z = 0; z < 3; z++) s.fill('s' + z + '_', 'crystalblock', 0, 0, z, 3, 3, { fed: true });
      s.badge('e', 1, 1, 1, '3×3×3: 561 Е/с', 'ok');
    }, 8),
  ]);
  // Solar: a line of housings with an energy channel at y = 1, the panel standing on the first one,
  // a battery and a furnace at y = 0. At night the panel gives a quarter and the battery the rest.
  const solarFlows = (s, night) => {
    const sun = [[0, 1, 1], [0, 1, 0]];
    s.flow('sb', night ? null : sun.concat([[1, 1, 0], [1, 0, 0]]), { color: EN, n: 2 });
    s.flow('sf', sun.concat([[3, 1, 0], [3, 0, 0]]), { color: EN, n: night ? 1 : 2, speed: night ? 0.5 : 1.6 });
    s.flow('bf', night ? r(1, 3) : null, { color: EN, n: 2 });
  };
  plugScene('solar', [
    step('Солнечная панель ставится прямо на корпус магистрали и подключается снизу.', (s) => {
      floor(s, -1, 0, 5, 2); for (let i = 0; i < 4; i++) s.put('p' + i, 'pipe', i, 1, 0, { ch: 'e---' });
      s.put('node', 'node', -1, 1, 0); s.put('sun', 'solar', 0, 1, 1); s.put('b1', 'battery', 1, 0, 0, { level: 0.3, ease: 3 }); s.put('b2', 'furnace', 3, 0, 0);
    }),
    step('Днём панель даёт 2 Е/с: питает машину и заряжает накопитель.', (s) => {
      s.set('sun', { on: true }).set('b1', { on: true, level: 0.8 }).set('b2', { on: true }); solarFlows(s, false); s.badge('d', 0, 1, 1, 'день: 2 Е/с', 'ok');
    }, 8),
    step('Ночью панель даёт четверть дневного, 0,5 Е/с. Чего не хватает машине, отдаёт накопитель.', (s) => {
      s.tint = 'rgba(8,12,40,0.5)'; s.set('b1', { level: 0.45 }); solarFlows(s, true); s.badge('d', 0, 1, 1, 'ночь: 0,5 Е/с', 'warn');
    }, 8),
    step('Корпус под панелью можно спрятать фальш-панелью. Панель стоит на нём и работает как раньше.', (s) => {
      s.tint = null; s.set('b1', { level: 0.7 }); solarFlows(s, false); s.put('fc', 'facade', 0, 1, 0, { look: 'bricks' }); s.badge('d', 0, 1, 1, 'день: 2 Е/с', 'ok');
    }, 8),
    step('Панели вплотную в один слой сразу образуют структуру любой формы. Ей хватает одного подключения к магистрали, отдача — сумма панелей.', (s) => {
      s.objs.clear(); s.badges = []; s.flows = []; s.tint = null;
      for (let x = 0; x < 4; x++) for (let y = 0; y < 3; y++) {
        s.put('fl' + x + y, 'farmland', x, y, -1); s.put('cr' + x + y, 'crop', x, y, 0, { stage: 0 });
        s.put('sp' + x + y, 'solar', x, y, 2, { on: true });
      }
      for (let z = 0; z < 3; z++) s.put('pc' + z, 'pipe', -1, 0, z, { ch: 'e---' });
      s.put('fu', 'furnace', -2, 0, 0, { on: true }); s.put('fs', 'stone', -2, 0, -1); s.put('ps', 'stone', -1, 0, -1);
      s.flow('e', [[0, 0, 2], [-1, 0, 2], [-1, 0, 0], [-2, 0, 0]], { color: EN, n: 3 });
      s.badge('st', 1.5, 1, 3, 'структура: 12 панелей · 24 Е/с', 'ok');
    }, 9),
    step('Панель пропускает свет и не даёт тени. Под структурой посевы растут, как под открытым небом, поэтому её поднимают прямо над полем.', (s) => {
      for (let x = 0; x < 4; x++) for (let y = 0; y < 3; y++) s.set('cr' + x + y, { stage: 2 });
      s.badge('sh', 1.5, 1, 1, 'тени нет', 'info');
    }, 9),
  ]);
  // Battery: four batteries touching are one structure; every one shows the charge of all.
  const BATS = ['b1', 'c1', 'c2', 'c3'];
  const charge = (s, level, on) => BATS.forEach((id) => s.set(id, { level, on }));
  scene('battery', [
    step('Накопитель ставится в магистраль рядом с источником и машинами.', (s) => {
      floor(s, 0, 0, 5, 2); for (let i = 0; i < 5; i++) s.put('p' + i, 'pipe', i, 1, 0, { ch: 'e---' }); s.put('node', 'node', 3, 0, 0);
      s.put('g', 'gen_fuel', 0, 0, 0, { on: true }); s.put('b1', 'battery', 1, 0, 0, { level: 0.4, ease: 3 }); s.put('f', 'furnace', 4, 0, 0, { on: true });
      s.flow('gf', r(0, 4), { color: EN, n: 2 });
    }),
    step('Накопители вплотную сразу образуют одну структуру любой формы. Запас общий: индикатор на каждом показывает заряд всей структуры.', (s) => {
      s.put('c1', 'battery', 2, 0, 0, { ease: 3 }); s.put('c2', 'battery', 1, 0, 1, { ease: 3 }); s.put('c3', 'battery', 2, 0, 1, { ease: 3 }); charge(s, 0.1, false);
      s.badge('st', 1, 0, 1, 'одна структура', 'info');
    }, 9),
    step('Пока источник даёт больше, чем тратят машины, структура заряжается. Генератор даёт 16 Е/с, печь тратит 8.', (s) => {
      charge(s, 0.9, true); s.flow('gb', r(0, 1), { color: EN, n: 2 }); s.badge('st', 1, 0, 1, 'заряжается', 'ok');
    }, 9),
    step('Топливо кончилось. Теперь структура отдаёт запас, и печь работает дальше.', (s) => {
      s.set('g', { on: false }); s.flow('gf', null); s.flow('gb', null); charge(s, 0.45, true); s.flow('bf', r(2, 4), { color: EN, n: 2 });
      s.badge('ng', 0, 0, 0, 'Нет топлива'); s.badge('st', 1, 0, 1, 'отдаёт запас', 'warn');
    }, 9),
    step('Модуль в любом накопителе действует на всю структуру. Модуль ёмкости добавляет ей половину запаса.', (s) => {
      charge(s, 0.3, true); s.badge('st', 1, 0, 1, 'модуль ёмкости: +50% всей структуре', 'info');
    }, 8),
  ]);
  scene('gen_fuel', [
    step('Топливный генератор берёт топливо из сундука по предметному каналу. Класть его можно и руками.', (s) => bus(s, [['chest'], ['gen_fuel'], ['furnace'], null], { src: null })),
    step('Уголь сгорает, генератор даёт 16 Е/с. Окно генератора показывает баланс всей цепи.', (s) => {
      s.set('b1', { on: true }).set('b2', { on: true }); s.flow('a', r(0, 1), { color: '#2a2b30' }); s.flow('b', r(1, 2), { color: EN, n: 2 });
      s.badge('w', 1, 0, 0.6, 'окно: даёт 16 · тратит 8 Е/с', 'info');
    }, 8),
    step('Машину, которая стоит вплотную, генератор питает и без магистрали: энергетический канал ей не нужен.', (s) => {
      for (let i = 0; i < 4; i++) s.set('p' + i, { ch: 'i---' });
      s.flow('b', [[1, 0, 0], [2, 0, 0]], { color: EN, n: 1, speed: 0.8 });
    }, 8),
  ]);
  scene('gen_lava', [
    step('Лавовый генератор получает лаву из бака по жидкостному каналу. В бак её качает насос над лавовым озером.', (s) => {
      bus(s, [['tank', { fluid: 'lava', level: 0.9, ease: 3 }], ['gen_lava'], ['furnace'], null], { src: null, ch: 'fe--' });
    }),
    step('Лава сгорает, генератор даёт 40 Е/с. В окне виден баланс всей цепи.', (s) => {
      s.set('b0', { level: 0.6 }).set('b1', { on: true }).set('b2', { on: true }); s.flow('a', r(0, 1), { color: LAVA }); s.flow('b', r(1, 2), { color: EN, n: 2 });
      s.badge('w', 1, 0, 0.6, 'окно: даёт 40 · тратит 8 Е/с', 'info');
    }, 8),
    step('Машину вплотную генератор питает и без магистрали: энергетический канал можно не ставить.', (s) => {
      for (let i = 0; i < 4; i++) s.set('p' + i, { ch: 'f---' });
      s.set('b0', { level: 0.4 }); s.flow('b', [[1, 0, 0], [2, 0, 0]], { color: EN, n: 1, speed: 0.8 });
    }, 8),
  ]);
  // A river along x at y = 1, three blocks deep (z = -1..-3) and cut open on the near side; it
  // flows to x+. The bank at y = 0 and y = -1 carries the housings, the furnace and the node.
  function river(s) {
    for (let x = 0; x < 6; x++) {
      for (let z = -3; z <= -1; z++) { s.put('w' + x + z, 'water', x, 1, z); s.put('k' + x + z, 'stone', x, 0, z); }
      s.put('bd' + x, 'stone', x, 1, -4); s.put('bk' + x, 'stone', x, 0, -4); s.put('fl' + x, 'stone', x, -1, -1);
    }
    s.put('p1', 'pipe', 0, 0, 0, { ch: 'e---' }); s.put('p2', 'pipe', 1, 0, 0, { ch: 'e---' }); s.put('b2', 'furnace', 2, 0, 0); s.put('node', 'node', 1, -1, 0);
  }
  const current = (s, z) => s.flow('wf', z == null ? null : [[0, 1, z], [5, 1, z]], { color: '#9fc0ff', n: 4, speed: 1.2 });
  const turbinePower = (s, z, slow) => s.flow('e', [[0, 1, z], [0, 1, 0], [0, 0, 0], [2, 0, 0]], { color: EN, n: slow ? 1 : 3, speed: slow ? 0.5 : 1.6 });
  // What each turbine of a chain of four gives: E12, 6 E/s in a current, +10% per block of depth,
  // +5% per turbine upstream. The first and the last turbine show their numbers (all four would
  // overlap); the sum stands over the water.
  const chain = (s, z, depth) => {
    let sum = 0;
    for (let i = 0; i < 4; i++) {
      const e = 6 * (1 + 0.1 * depth) * (1 + 0.05 * i);
      sum += e;
      s.put('t' + i, 'turbine', i, 1, z, { on: true });
      if (i === 0 || i === 3) s.badge('tb' + i, i, 1, z - 1.7, (i ? 'ниже по течению: ' : 'первая: ') + e.toFixed(1).replace('.', ',') + ' Е/с', 'ok');
    }
    s.badge('sum', 5, 1, -1, 'вся цепочка: ' + (Math.round(sum * 10) / 10).toString().replace('.', ',') + ' Е/с', 'info');
  };
  scene('turbine', [
    step('Гидротурбина ставится прямо в воду: в реку, в канал или в пруд. В течении она даёт 6 Е/с.', (s) => {
      river(s); s.put('t0', 'turbine', 0, 1, -1, { on: true }); s.put('p0', 'pipe', 0, 1, 0, { ch: 'e---' }); s.set('b2', { on: true });
      current(s, -1); turbinePower(s, -1); s.badge('e', 0, 1, -0.6, 'течение: 6 Е/с', 'ok');
    }, 8),
    step('В стоячей воде створки улавливают совсем немного, но турбина всё равно работает, днём и ночью.', (s) => {
      current(s, null); turbinePower(s, -1, true); s.badge('e', 0, 1, -0.6, 'стоячая вода: 1 Е/с', 'warn');
    }, 7),
    step('Турбины вплотную в воде образуют структуру. Турбина ниже по течению даёт на 5% больше за каждую турбину выше.', (s) => {
      s.badges = []; current(s, -1); turbinePower(s, -1); chain(s, -1, 0);
    }, 9),
    step('Опусти цепочку на два блока ко дну: глубина добавляет по 10% за блок. Вместе четыре турбины дают 31 Е/с.', (s) => {
      for (let i = 0; i < 4; i++) s.del('t' + i);
      s.put('pa', 'pipe', 0, 1, -1, { ch: 'e---' }); s.put('pb', 'pipe', 0, 1, -2, { ch: 'e---' }); current(s, -3); turbinePower(s, -3); chain(s, -3, 2);
    }, 9),
  ]);
  plugScene('windmill', [
    step('Ветряк ставится одним блоком, но выглядит как мачта в три блока высотой. Над ним и вокруг лопастей должно быть свободно.', (s) => {
      floor(s, -1, 0, 5, 2); s.put('wm', 'windmill', 0, 1, 0);
    }, 7),
    step('Поставь ветряк прямо на корпус магистрали: он подключается снизу. У моря он даёт 4 Е/с.', (s) => {
      for (let i = 0; i < 3; i++) s.put('p' + i, 'pipe', i, 1, 0, { ch: 'e---' });
      s.put('node', 'node', -1, 1, 0); s.put('b2', 'furnace', 2, 0, 0, { on: true }); s.put('wm', 'windmill', 0, 1, 1);
      s.flow('e', [[0, 1, 1], [0, 1, 0], [2, 1, 0], [2, 0, 0]], { color: EN, n: 2 }); s.badge('a', 0, 1, 2.5, '4 Е/с', 'ok');
    }, 8),
    step('Чем выше стоит ветряк, тем больше даёт: до 10 Е/с высоко в горах. Поставь его на башню из корпусов, энергия идёт по ним вниз.', (s) => {
      for (let z = 1; z < 4; z++) s.put('pz' + z, 'pipe', 0, 1, z, { ch: 'e---' });
      s.put('wm', 'windmill', 0, 1, 4); s.flow('e', [[0, 1, 4], [0, 1, 0], [2, 1, 0], [2, 0, 0]], { color: EN, n: 3 }); s.badge('a', 0, 1, 5.5, '7 Е/с', 'ok');
    }, 8),
    step('Спрячь корпуса башни фальш-панелями. Башня выглядит как обычная постройка, а ветряк работает как раньше.', (s) => {
      for (let z = 0; z < 4; z++) s.put('fc' + z, 'facade', 0, 1, z, { look: 'bricks' });
    }, 8),
  ]);

  // ================================================================ machines
  // Its own alloy goes round over the smelter: out of the output slots and back in.
  const SELF = [[1, 0, 0.55], [1.35, 0, 0.95], [0.65, 0, 0.95], [1, 0, 0.55]];
  plugScene('alloy', [
    step('Плавильня сплавов собирается из медностали, печи и меди. Работает она на любом топливе, как обычная печь.', (s) => { floor(s, -1, 0, 5, 2); s.put('b1', 'alloy', 1, 0, 0); }),
    step('У неё четыре ячейки ввода и три ячейки готового. Медносталь, золото и редстоун с углём дают искросталь.', (s) => {
      s.set('b1', { on: true }); s.badge('a', 1, 0, 0, 'медносталь + золото + редстоун → искросталь', 'info');
    }, 8),
    step('Если для следующего рецепта есть алмаз, кварц и осколок, плавильня сама берёт свою искросталь из ячейки готового и делает кристаллит.', (s) => {
      s.flow('self', SELF, { color: RC.TIER_COLOR[2], n: 2, speed: 0.8 }); s.badge('a', 1, 0, 0.3, 'своя искросталь + алмаз + кварц + осколок → кристаллит', 'ok');
    }, 9),
    step('С магистралью плавильня работает на энергии, без топлива. Сырьё приходит из сундука, готовый кристаллит забирает магистраль.', (s) => {
      s.badges = []; bus(s, [['chest'], ['alloy', { on: true }], ['chest'], null]); power(s, 1);
      s.flow('in', r(0, 1), { color: RC.TIER_COLOR[1] }); s.flow('out', r(1, 2), { color: RC.TIER_COLOR[3] });
    }, 9),
    step('Если искросталь нужна тебе самой, выключи в окне «брать свой сплав». Тогда искросталь уходит по линии.', (s) => {
      s.flow('self', null); s.flow('out', r(1, 2), { color: RC.TIER_COLOR[2] }); s.badge('a', 1, 0, 0.3, '«брать свой сплав»: выключено', 'info');
    }, 8),
  ]);
  S.alloys = S.alloy;
  plugScene('furnace', [
    step('Электропечь ставится как обычный блок.', (s) => { floor(s, -1, 0, 5, 2); s.put('b1', 'furnace', 1, 0, 0); }),
    step(TRUNK, (s) => bus(s, [['chest'], ['furnace'], ['chest'], null]), 7),
    step('Руда или пыль идёт из левого сундука, слитки уходят в правый. Топливо не нужно.', (s) => {
      s.set('b1', { on: true }); s.flow('in', r(0, 1)); s.flow('out', r(1, 2), { color: '#d8d8d8' }); power(s, 1);
    }, 7),
    step('Можно и без магистрали: поставь кристалл прямо к печи. В экономном режиме ей хватает одного кристалла.', (s) => {
      s.objs.clear(); s.flows = []; floor(s, -1, 0, 5, 2); s.put('k', 'crystal', 0, 0, 0); s.put('b1', 'furnace', 1, 0, 0, { on: true });
      s.flow('e', [[0, 0, 0], [1, 0, 0]], { color: EN, n: 1, speed: 0.6 }); s.badge('m', 1, 0, 0, 'экономный режим', 'info');
    }, 8),
  ]);
  plugScene('crusher', [
    step('Дробилка дробит железную, медную и золотую руду в пыль. Остальные руды она не принимает.', (s) => { floor(s, -1, 0, 5, 2); s.put('b1', 'crusher', 1, 0, 0); }),
    step(TRUNK, (s) => bus(s, [['chest'], ['crusher'], ['chest'], null]), 7),
    step('Руда идёт из левого сундука. Из одной руды выходят две пыли, из каждой пыли получится слиток.', (s) => {
      s.set('b1', { on: true }); s.flow('in', r(0, 1)); s.flow('out', r(1, 2), { color: '#d8a47a', n: 5 }); power(s, 1);
    }, 7),
  ]);
  scene('dusts', [
    step('Дробилка и электропечь стоят вплотную на одной магистрали. Двум машинам нужны два кристалла.', (s) => { bus(s, [['chest'], ['crusher'], ['furnace'], ['chest']]); crystal2(s); }, 7),
    step('Руда → две пыли → два слитка.', (s) => {
      s.set('b1', { on: true }).set('b2', { on: true });
      s.flow('a', r(0, 1)); s.flow('b', r(1, 2), { color: '#d8a47a', n: 4 }); s.flow('c', r(2, 3), { color: '#d8d8d8', n: 4 }); power(s, 1); crystal2(s, 2);
    }, 8),
  ]);
  plugScene('crafter', [
    step('В автокрафтере страницы. На каждой странице один рецепт: выложи образец в сетку 3×3.', (s) => {
      floor(s, -1, 0, 6, 2); s.put('b1', 'crafter', 1, 0, 0);
    }, 8),
    step('Автокрафтеру хватает одного стыка. Через него магистраль кладёт тростник и забирает готовую бумагу.', (s) => {
      bus(s, [['chest'], ['crafter', { on: true }], null, ['chest'], null]); s.flow('a', r(0, 1), { color: '#8fd35a' }); s.flow('b', r(1, 3), { color: '#f2f2f2' }); power(s, 1);
      s.badge('j', 1, 0, 0, 'один стык: туда и обратно', 'info');
    }, 8),
    step('В сундуке есть и булыжник. Автокрафтеру он не нужен, поэтому проходит мимо, в сундук со стыком «в сундук» в конце линии.', (s) => {
      s.badges = []; s.flow('c', r(0, 3), { color: '#8b8d91', n: 2 }); s.badge('t', 3, 0, -0.9, 'стык «в сундук»', 'info');
    }, 9),
    step('Бумагу просит второй автокрафтер: он делает из неё и кожи книги. В первый автокрафтер бумага не возвращается.', (s) => {
      s.badges = []; s.flow('c', null); s.put('b2', 'crafter', 2, 0, 0, { on: true });
      s.flow('b', r(1, 2), { color: '#f2f2f2' }); s.flow('k', r(2, 3), { color: '#8a4a2a' }); s.flow('d', r(0, 2), { color: '#a8643a', n: 1 }); power(s, 1, 2);
    }, 9),
    step('Когда готовое некуда отдать, например на складе достигнут лимит книг, автокрафтер останавливается сам.', (s) => {
      s.flows = []; s.set('b1', { on: false }).set('b2', { on: false }); s.badge('f', 2, 0, 0, 'Готовое некуда отдать', 'warn');
    }, 8),
  ]);
  // Placer and breaker share one work cell: a placer at x = 0 looks at the cell x = 1, breakers at
  // x = 2 look back at it (facing x-), and housings at x = 3 behind them take what they break to a
  // chest at x = 4; the node stands at y = -1 and an energy crystal on top of the housings. `high`
  // breakers and housings stand in a column, for a tree trunk. The placer, when there is one,
  // takes energy from a crystal beside it, without a housing; it is put first, so its crystal
  // carries the name tag.
  const POWDER = '#f0b27a', CONCRETE = '#e0782a', WOOD = '#8a6a43', COBBLE = '#8b8d91';
  const placerAt = (s, on, ky) => { s.put('pl', 'placer', 0, 0, 0, { on }); s.put('k1', 'crystal', 0, ky == null ? 1 : ky, 0); };
  function workCell(s, high, placerOn) {
    s.objs.clear(); s.flows = []; s.badges = []; floor(s, -1, -1, 6, 3);
    if (placerOn != null) placerAt(s, placerOn);
    for (let z = high - 1; z >= 0; z--) { s.put('br' + z, 'breaker', 2, 0, z, { facing: 'x-' }); s.put('q' + z, 'pipe', 3, 0, z, { ch: 'ie--' }); }
    s.put('ch', 'chest', 4, 0, 0); s.put('nd', 'node', 3, -1, 0); s.put('k2', 'crystal', 3, 0, high);
  }
  const breakOut = (s, color, fromZ) => s.flow('o', [[1, 0, fromZ || 0], [3, 0, fromZ || 0], [3, 0, 0], [4, 0, 0]], { color, n: 4 });
  function concreteCell(s, hard) {
    workCell(s, 1, true); s.put('wt', 'water', 1, -1, 0);
    const c = hard ? CONCRETE : POWDER;
    s.put('cc', 'wool', 1, 0, 0, { top: c, left: c, right: c });
    if (!hard) return s.move('cc', [[0.35, 0, 0], [1, 0, 0]], 0.4, 'once');
    s.set('br0', { on: true }); breakOut(s, CONCRETE);
  }
  function treeFarm(s, grown) {
    workCell(s, 4, !grown);
    if (!grown) return s.put('sp', 'tree', 1, 0, 0, { stage: 0 });
    for (let z = 0; z < 4; z++) { s.put('lg' + z, 'log', 1, 0, z); s.set('br' + z, { on: true }); }
    for (let dx = -1; dx <= 1; dx++) for (let dy = -1; dy <= 1; dy++) s.put('lf' + dx + dy, 'leaves', 1 + dx, dy, 4);
    s.put('lft', 'leaves', 1, 0, 5); breakOut(s, WOOD, 3);
  }
  function cobbleCell(s, on) {
    workCell(s, 1); s.put('lv', 'lavapool', 0, 0, 0); s.put('wt', 'water', 1, -1, 0); s.put('cb', 'stone', 1, 0, 0);
    if (on) { s.set('br0', { on: true }); breakOut(s, COBBLE); }
  }
  plugScene('placer', [
    step('Установщик ставит блок из своего инвентаря в клетку перед собой. Здесь это сухой бетон, а рядом с клеткой вода.', (s) => concreteCell(s, false), 7),
    step('Вода сразу превращает смесь в бетон. Разрушитель ломает готовый бетон и отдаёт в магистраль, а установщик ставит следующий.', (s) => concreteCell(s, true), 9),
    step('Ферма деревьев без монорельса: установщик сажает саженец, а вдоль будущего ствола стоит столб разрушителей.', (s) => treeFarm(s, false), 8),
    step('Дерево выросло. Разрушители рубят ствол и отдают брёвна в магистраль, установщик сажает новый саженец.', (s) => treeFarm(s, true), 9),
    step('Без разрушителя установщик заделывает дыру в стене: как только клетка перед ним освободилась, он ставит в неё блок.', (s) => {
      s.objs.clear(); s.flows = []; s.badges = []; floor(s, -1, -2, 4, 3); placerAt(s, true, -1);
      for (let y = -2; y <= 0; y++) for (let z = 0; z < 2; z++) if (y || z) s.put('wl' + y + z, 'stone', 1, y, z);
      s.put('fill', 'stone', 1, 0, 0).move('fill', [[0.3, 0, 0], [1, 0, 0]], 0.35, 'ping'); s.badge('h', 1, 0, -0.9, 'дыра в стене', 'info');
    }, 8),
  ]);
  plugScene('breaker', [
    step('Булыжник без кирки: лава и вода сходятся перед разрушителем, и в клетке встаёт булыжник.', (s) => cobbleCell(s, false), 7),
    step('Разрушитель ломает булыжник и отдаёт его в магистраль. На месте сломанного тут же встаёт новый.', (s) => cobbleCell(s, true), 8),
    step('Со столбом разрушителей и установщиком получается ферма деревьев: ствол рубится целиком, брёвна уходят в сундук.', (s) => treeFarm(s, true), 9),
    step('Бетон без ручной работы: установщик ставит сухой бетон к воде, он твердеет, а разрушитель ломает готовый.', (s) => concreteCell(s, true), 9),
  ]);
  function mixerScene(id, c) {
    scene(id, [
      step(c[0], (s) => {
        bus(s, [['tank', { fluid: 'water' }], ['chest'], ['mixer'], ['tank', { fluid: 'empty' }]], { ch: 'ifef' });
        s.badge('g', 1, 1, 0, 'гнёзда: предметы · вода · энергия · раствор', 'info');
      }, 8),
      step(c[1], (s) => { s.badges = []; s.set('b2', { on: true }); s.flow('w', r(0, 2), { color: FL, n: 2 }); s.flow('i', r(1, 2), { color: '#e8e6da', n: 2 }); power(s, 2); }, 7),
      step(c[2], (s) => { s.set('b3', { fluid: 'sol' }); s.flow('o', r(2, 3), { color: SOL, n: 2 }); }, 7),
    ]);
  }
  plugScene('mixer', [
    step('Смеситель берёт ингредиенты по предметному каналу, а воду по жидкостному.', (s) => {
      bus(s, [['tank', { fluid: 'water', level: 0.9, ease: 3 }], ['chest'], ['mixer'], ['tank', { fluid: 'empty', level: 0, ease: 3 }], null], { ch: 'ife-' });
    }, 7),
    step('Костная мука с редстоуном и вода пришли, смеситель работает.', (s) => {
      s.set('b2', { on: true }).set('b0', { level: 0.7 }); s.flow('w', r(0, 2), { color: FL, n: 2 }); s.flow('i', r(1, 2), { color: '#e8e6da', n: 2 }); power(s, 2);
    }, 7),
    step('Готовый раствор уходит в бак по тому же жидкостному каналу: канал второй ступени несёт две жидкости сразу.', (s) => {
      s.set('b3', { fluid: 'sol', level: 0.6 }).set('b0', { level: 0.5 }); s.flow('o', r(2, 3), { color: SOL, n: 2 });
      s.badge('c', 1, 3, -2.1, 'канал 2-й ступени: вода и раствор', 'info');
    }, 8),
    step('С каналом первой ступени поставь второй жидкостный канал в соседнее гнездо: по нему уйдёт раствор.', (s) => {
      for (let i = 0; i < 5; i++) s.set('p' + i, { ch: 'ifef' });
      s.set('b3', { level: 0.8 }).set('b0', { level: 0.35 }); s.badge('c', 1, 3, -2.1, 'гнёзда: предметы · вода · энергия · раствор', 'info');
    }, 8),
  ]);
  mixerScene('fluids', ['Раствор и катализаторы делает смеситель из воды и предметов.', 'Вода и ингредиенты приходят по магистрали.', 'Готовая жидкость хранится в баке. Отсюда она идёт к кристаллам или в машины.']);
  scene('charger', [
    step('Зарядник подключается к магистрали с энергетическим каналом.', (s) => bus(s, [null, ['charger'], null])),
    step('Встань на него в технической броне, и батареи начнут заряжаться.', (s) => { s.put('pl', 'player', 1, 0, 1, { armor: true }); s.set('b1', { on: true }); power(s, 1); s.badge('c', 1, 0, 1.2, 'Заряд 64%', 'ok'); }, 7),
  ]);
  // A charge plate lies on the floor over a housing set into the floor; a battery feeds the line.
  scene('charge_plate', [
    step('Зарядная плита тонкая, как нажимная. Её кладут на пол прямо над корпусом с энергетическим каналом.', (s) => {
      for (let x = -1; x < 5; x++) for (const y of [0, 2, 3]) s.put('fl' + x + '_' + y, 'stone', x, y, -1);
      for (let x = 0; x < 4; x++) s.put('p' + x, 'pipe', x, 1, -1, { ch: 'e---' });
      s.put('bt', 'battery', -1, 1, -1, { on: true, level: 0.8 }); s.put('node', 'node', 4, 1, -1);
      s.put('cp1', 'charge_plate', 1, 1, 0); s.put('cp2', 'charge_plate', 2, 1, 0);
    }, 7),
    step('Встань на плиту. Пока ты на ней стоишь, заряжаются все части брони, джетпак и снаряжение в руках: снимать ничего не нужно.', (s) => {
      const path = [[1, 3, 0], [1, 1, 0]];
      s.put('pl', 'player', 1, 3, 0, { armor: true }).move('pl', path, 1.2, 'once'); s.put('jp', 'jetpack', 1, 3, 0).move('jp', path, 1.2, 'once');
      s.set('cp1', { on: true }); s.flow('e1', [[-1, 1, -1], [1, 1, -1], [1, 1, 0]], { color: EN, n: 3 }); s.badge('c1', 1, 1, 1.2, 'заряд 45% · +128 Е/с', 'ok');
    }, 9),
    step('Плит можно положить несколько рядом, например у входа на базу или у станции монорельса: заряжается вся команда.', (s) => {
      s.put('pl2', 'player', 2, 3, 0, { armor: true }).move('pl2', [[2, 3, 0], [2, 1, 0]], 1.2, 'once');
      s.set('cp2', { on: true }); s.flow('e2', [[-1, 1, -1], [2, 1, -1], [2, 1, 0]], { color: EN, n: 3 });
      s.badge('c1', 1, 1, 1.2, 'заряд 100%', 'ok'); s.badge('c2', 2, 1, 0.2, 'заряд 30% · +128 Е/с', 'ok');
    }, 9),
  ]);
  plugScene('modules', [
    step('Модуль вставляется в гнездо в окне машины. На первой и второй ступени у машины три гнезда, на третьей и четвёртой — четыре.', (s) => {
      bus(s, [['chest'], ['furnace', { on: true }], ['chest'], null]); s.flow('in', r(0, 1), { n: 2, speed: 1 }); s.flow('out', r(1, 2), { color: '#d8d8d8', n: 2, speed: 1 }); power(s, 1);
      s.badge('m', 1, 0, 0, 'гнёзд: 3', 'info');
    }, 8),
    step('Модуль скорости подходит любой машине: она работает вдвое быстрее и тратит в 2,5 раза больше энергии.', (s) => {
      s.badge('m', 1, 0, 0, 'скорость: работа ×2, энергия ×2,5', 'info');
      s.flow('in', r(0, 1), { n: 4, speed: 2 }); s.flow('out', r(1, 2), { color: '#d8d8d8', n: 4, speed: 2 }); s.flow('en1', fromSrc(1), { color: EN, n: 5, speed: 2.5 });
    }, 8),
    step('Модуль экономии: энергии уходит на 40% меньше, а работа медленнее всего на 15%. Он выручает там, где энергии мало.', (s) => {
      s.badge('m', 1, 0, 0, 'экономия: энергия ×0,6, работа ×0,85', 'info');
      s.flow('in', r(0, 1), { n: 2, speed: 0.85 }); s.flow('out', r(1, 2), { color: '#d8d8d8', n: 2, speed: 0.85 }); s.flow('en1', fromSrc(1), { color: EN, n: 1, speed: 1 });
    }, 8),
  ]);

  // ================================================================ storage
  const store = (s) => {
    floor(s, 0, 0, 7, 3);
    for (let x = 0; x < 3; x++) for (let z = 0; z < 2; z++) s.put('c' + x + z, 'cell', x, 0, z);
    s.put('t', 'terminal', 3, 0, 0);
  };
  function storageScene(id, c, panel) {
    scene(id, [
      step(c[0], (s) => { floor(s, 0, 0, 7, 3); s.put('c0', 'cell', 0, 0, 0); s.put('t', 'terminal', 1, 0, 0); }),
      step(c[1], (s) => { s.del('c0'); store(s); }, panel ? 9 : 6),
      step(c[2], (s) => { s.del('c11'); s.badge('h', 1, 0, 1, 'Нарушена форма склада'); }, 7),
      step(c[3], (s) => {
        s.put('c11', 'cell', 1, 0, 1); s.badges = [];
        for (let x = 3; x < 6; x++) s.put('p' + x, 'pipe', x, 1, 0, { ch: 'i---' });
        s.put('t2', 'terminal', 6, 1, 0); s.put('node', 'node', 5, 2, 0); s.put('pl', 'player', 6, 2, 0);
      }, 7),
    ].concat(c[4] ? [step(c[4], (s) => s.badge('o', 3, 0, 0, 'поршень · можно заказать', 'info'), 8)] : []));
  }
  storageScene('cell', ['Склад начинается с одной ячейки и терминала.', 'Достраивай ячейки сплошным блоком любого размера. Каждая добавляет 27 слотов.', 'Дыра в форме: терминал покажет, какой ячейки не хватает. Предметы не теряются.', 'К складу подводится магистраль с предметным каналом: по ней предметы приходят и уходят.']);
  storageScene('terminal', ['Терминал ставится вплотную к ячейкам склада.', 'Он открывает весь склад одним окном: поиск, вкладки фильтра, все слоты, лимит для каждого предмета.', 'Если форма склада нарушена, терминал назовёт причину.', 'Второй терминал ставится там, где удобно: достаточно дотянуть магистраль с предметным каналом.',
    'С книгой рецептов поиск находит и то, чего на складе нет. Такие предметы бледные, их можно заказать.'], true);
  scene('mod_stack', [
    step('Обычный стак на складе — 64 предмета: 200 слитков занимают четыре слота.', (s) => { store(s); s.badge('m', 3, 0, 0, 'стак 64 · 200 слитков в 4 слотах', 'info'); }, 8),
    step('Модуль стака вставляется в окно терминала и действует на весь склад. Модуль из искростали даёт стак 256: те же слитки в одном слоте.', (s) => { s.badge('m', 3, 0, 0, 'стак 256 · 1 слот', 'ok'); }, 9),
    step('Модули из кристаллита и звёздной стали дают стак 1024 и 4096. В машине модуль даёт 256, в рюкзаке — до 1024.', (s) => { s.badge('m', 3, 0, 0, 'стак 1024 или 4096', 'ok'); s.put('pl', 'player', 5, 1, 0); }, 8),
  ]);
  scene('tablet', [
    step('Обычно к складу подходят через терминал.', (s) => { store(s); s.put('node', 'node', 4, 0, 0, { tier: 4 }); s.put('pl', 'player', 3, 1, 0); }),
    step('С планшетом склад открывается из любого места базы, без проводов.', (s) => { s.set('pl', { x: 6, y: 2 }); s.badge('m', 6, 2, 0.4, 'планшет', 'ok'); }, 8),
    step('Планшет работает в пределах дальности узла магистрали. Между базами груз возит монорельс.', (s) => { s.badge('m', 6, 2, 0.4, 'до узла 5 из 256 блоков', 'info'); }, 7),
  ]);

  // A store with its terminal on the left of a line of machines with a node of the third tier:
  // cells 2×2×2 behind the terminal at x = 0, machines at x = 1.., housings at y = 1, the crystal
  // in front of the first housing.
  function orderLine(s, blocks) {
    const n = blocks.length;
    floor(s, -3, -1, n + 5, 4);
    for (const x of [-2, -1]) for (const y of [-1, 0]) for (const z of [0, 1]) s.put('oc' + x + y + z, 'cell', x, y, z);
    s.put('t', 'terminal', 0, 0, 0);
    blocks.forEach((b, i) => b && s.put('b' + (i + 1), b, i + 1, 0, 0));
    for (let x = 0; x <= n; x++) s.put('p' + x, 'pipe', x, 1, 0, { ch: 'ie--' });
    s.put('src', 'crystal', 0, 2, 0); s.put('node', 'node', n + 1, 1, 0, { tier: 3 });
  }
  const PISTON = [['#b08a52', 'доски'], ['#8b8d91', 'булыжник'], ['#d8d8d8', 'железо'], ['#e53b3b', 'редстоун']];
  scene('mod_recipes', [
    step('Книга рецептов вставляется в гнездо терминала склада. Она знает рецепты верстака и всех машин.', (s) => {
      orderLine(s, [null, 'crafter', null]); s.badge('t', 0, 0, 0, 'книга рецептов', 'info');
    }, 8),
    step('Поиск терминала находит и то, чего на складе нет. Поршень показан бледным с пометкой «можно заказать».', (s) => {
      s.badge('t', 0, 0, 0, 'поршень · можно заказать', 'info');
    }, 8),
    step('Закажи 4 поршня. У автокрафтера появилась временная страница, она идёт раньше обычных страниц.', (s) => {
      s.badge('t', 0, 0, 0, 'заказ: поршень ×4', 'info'); s.badge('pg', 2, 0, 0, 'временная страница: поршень', 'ok');
    }, 8),
    step('Ингредиенты под заказ приходят со склада по магистрали: доски, булыжник, железо и редстоун.', (s) => {
      s.set('b2', { on: true }); PISTON.forEach(([color], i) => s.flow('in' + i, r(0, 2), { color, n: 1, speed: 1.1 + 0.25 * i }));
    }, 8),
    step('Готовые поршни уходят на склад, временная страница исчезает.', (s) => {
      s.flows = []; s.set('b2', { on: false }); s.badges = [];
      s.flow('out', r(2, 0), { color: '#c9b28a', n: 4 }); s.badge('t', 0, 0, 0, 'на складе: поршень ×4', 'ok');
    }, 7),
  ]);

  // ================================================================ rail
  // A rounded corner: the path of a cart through the floor cell (x, y), coming in by side `from`
  // and leaving by side `to`, on the quarter circle the rail draws round the cell corner they share.
  function arc(x, y, z, from, to) {
    const cx = from === 'x+' || to === 'x+' ? 1 : 0, cy = from === 'y+' || to === 'y+' ? 1 : 0;
    const edge = (side) => (side[0] === 'x' ? [side[1] === '+' ? 1 : 0, 0.5] : [0.5, side[1] === '+' ? 1 : 0]);
    const angle = (side) => Math.atan2(edge(side)[1] - cy, edge(side)[0] - cx);
    const a0 = angle(from), sweep = Math.atan2(Math.sin(angle(to) - a0), Math.cos(angle(to) - a0));
    const pts = [];
    for (let k = 0; k <= 4; k++) {
      const a = a0 + (sweep * k) / 4;
      pts.push([x + cx + 0.5 * Math.cos(a) - 0.5, y + cy + 0.5 * Math.sin(a) - 0.5, z]);
    }
    return pts;
  }
  // A rail that ends beside a station turns into it (spec 18.6). link() counts rails only, so the
  // side of the station is added to the end piece after it.
  function stationTurns(s) {
    const at = new Map();
    for (const o of s.objs.values()) if (!o.path) at.set(o.x + ',' + o.y + ',' + o.z, o);
    for (const o of s.objs.values()) {
      if (o.type !== 'rail' || o.slope || (o.face && o.face !== 'floor') || !o.con || o.con.length !== 1) continue;
      const side = DIRS.slice(0, 4).find(([d, dx, dy]) => d[0] !== o.con[0][0] && (at.get(o.x + dx + ',' + (o.y + dy) + ',' + o.z) || {}).type === 'station');
      if (side) o.con = [o.con[0], side[0]];
    }
  }
  function railScene(id, steps) {
    scene(id, steps);
    for (const st of S[id].steps) {
      const run = st.run;
      st.run = (s) => { run(s); stationTurns(s); };
    }
  }
  // A track that bends and climbs: two rounded corners on the floor, a 45° climb of two blocks over
  // a staircase of stone, a straight piece and a rounded corner on top. ox, oy shift it.
  const CLIMB = [[0, 4, 0], [0, 3, 0], [0, 2, 0], [1, 2, 0], [2, 2, 0], [2, 1, 0, 'y-'], [2, 0, 1, 'y-'], [2, -1, 2], [2, -2, 2], [3, -2, 2], [4, -2, 2]];
  function climb(s, ox, oy) {
    for (let x = 0; x < 5; x++) for (let y = -2; y < 5; y++) s.put('cg' + x + '_' + y, 'grass', ox + x, oy + y, -1);
    CLIMB.forEach(([x, y, z, slope], i) => {
      s.put('cr' + i, 'rail', ox + x, oy + y, z, slope ? { slope } : { dir: 'y' });
      for (let k = 0; k < z; k++) s.put('cs' + i + '_' + k, 'stone', ox + x, oy + y, k);
    });
  }
  // The capsule's way along that track: through the arcs, and up the climb one block per block.
  function climbRide(ox, oy) {
    const p = (x, y, z) => [ox + x, oy + y, z];
    return [p(0, 4, 0), p(0, 2.5, 0)].concat(arc(ox, oy + 2, 0, 'y+', 'x+'), [p(1.5, 2, 0)], arc(ox + 2, oy + 2, 0, 'x-', 'y-'),
      [p(2, 0.5, 1), p(2, -0.5, 2), p(2, -1.5, 2)], arc(ox + 2, oy - 2, 2, 'y+', 'x+'), [p(4, -2, 2)]);
  }
  scene('monorail', [
    step('На полу монорельс лежит как обычный рельс.', (s) => { canyon(s, 0, 1); canyonRail(s, 0, 1, 0, 3); }),
    step('Пол кончился, внизу лава. Путь продолжается по стене: рельс теперь сбоку.', (s) => canyonRail(s, 0, 1, 3, 6), 6),
    step('Стена кончилась. Путь уходит под потолок: рельс теперь сверху.', (s) => canyonRail(s, 0, 1, 6, 9), 6),
    step('Капсула едет прямо и не наклоняется. Меняется только то, с какой стороны она держится за рельс.', (s) => { s.put('cap', 'capsule', 0, 1, 0).move('cap', canyonRide(0, 1), 1.8, 'ping'); }, 12),
    step('Форму рельс выбирает сам. На повороте он скруглён, подъём идёт лесенкой под 45°: каждый следующий рельс на блок выше.', (s) => {
      s.objs.clear(); climb(s, 2, -2); s.put('cap', 'capsule', 2, 2, 0).move('cap', climbRide(2, -2), 1.6, 'ping');
    }, 12),
  ]);
  function netScene(id, c) {
    railScene(id, [
      step(c[0], (s) => {
        for (let x = 0; x < 9; x++) { s.put('fa' + x, 'stone', x, 3, -1); s.put('ra' + x, 'rail', x, 3, 0, { dir: 'x' }); }
        for (let y = 0; y < 3; y++) { s.put('fb' + y, 'stone', 4, y, -1); s.put('rb' + y, 'rail', 4, y, 0, { dir: 'y' }); }
        s.put('fs0', 'stone', 0, 4, -1); s.put('s0', 'station', 0, 4, 0); s.put('fs1', 'stone', 8, 4, -1); s.put('s1', 'station', 8, 4, 0);
        s.put('fs2', 'stone', 5, 0, -1); s.put('s2', 'station', 5, 0, 0);
        s.badge('n0', 0, 4, -0.4, 'Дом', 'info'); s.badge('n1', 8, 4, -0.4, 'Шахта', 'info'); s.badge('n2', 5, 0, -0.4, 'Ферма', 'info');
      }, 8),
      step(c[1], (s) => { s.put('pl', 'player', 0, 5, 0); s.put('fp', 'stone', 0, 5, -1); }, 9),
      step(c[2], (s) => {
        s.del('pl');
        s.put('cap', 'capsule', 0, 3, 0).move('cap', arc(0, 3, 0, 'y+', 'x+').concat([[4, 3, 0], [4, 0.5, 0]], arc(4, 0, 0, 'y+', 'x+')), 1.5, 'once');
        s.put('w', 'worker', 8, 3, 0, { tool: 'hold' }).move('w', arc(8, 3, 0, 'y+', 'x-').concat(arc(0, 3, 0, 'x+', 'y+')), 1.5, 'once');
      }, 8),
      step(c[3], (s) => { s.del('cap', 'w'); s.put('fq', 'stone', 5, 1, -1); s.put('pl', 'player', 5, 1, 0); }, 6),
    ]);
  }
  netScene('station', ['Три станции с названиями стоят сбоку от путей, и рельс поворачивает к каждой. Станции одной сети видят друг друга.', 'Открой станцию: на схеме видны все станции, расстояние и время в пути. Выбери, куда ехать.', 'Станция списала энергию и отправила капсулу. Развилку капсула проходит сама, встречную вагонетку проезжает насквозь.', 'Ты на месте. Капсула исчезла.']);
  netScene('capsule', ['Капсулу не делают и не носят с собой. Её вызывает станция.', 'Выбери станцию назначения на схеме.', 'Капсула появилась на станции и поехала кратчайшим путём. Встречную вагонетку она проезжает насквозь.', 'По прибытии капсула исчезает и не занимает путь.']);
  scene('lift', [
    step('Лифт — это монорельс, проложенный вертикально по стене.', (s) => {
      for (let z = 0; z < 5; z++) { for (let x = -1; x < 2; x++) s.put('w' + x + z, 'stone', x, 0, z); s.put('r' + z, 'rail', 0, 1, z, { face: 'wy', dir: 'z' }); }
      floor(s, -1, 0, 3, 3);
    }),
    step('На этажах у рельса стоят станции с названиями.', (s) => {
      s.put('s0', 'station', -1, 1, 0); s.put('fl2', 'planks', 1, 1, 2); s.put('s1', 'station', 1, 1, 3);
      s.badge('a', -1, 1, -0.4, 'Подвал', 'info'); s.badge('b', 1, 1, 2.6, 'Мастерская', 'info');
    }, 6),
    step('Этаж выбирается в схеме станции. Капсула едет вверх, оставаясь ровной.', (s) => {
      s.put('cap', 'capsule', 0, 1, 0, { mount: 'side' }).move('cap', [[0, 1, 0, 'side'], [0, 1, 3, 'side']], 1.5, 'ping');
    }, 10),
  ]);

  // ================================================================ workers
  // How long a step stays on screen (stepTime in player.js); a move that should last the whole
  // step takes its speed from this.
  function hold(text, dur) {
    return Math.max(dur || 5, 3 + text.length / 14);
  }
  // A move that waits `delay` seconds of the step on its first point, then starts.
  function later(s, id, pts, speed, delay) {
    s.set(id, { path: [pts[0]].concat(pts), speed, mode: 'once', t0: s.now + Math.max(0, delay) });
  }
  // A block goes out of sight at moment `at` of the step: it sinks `depth` blocks into the block
  // under it (a plant into its soil, a column into its base), fast.
  function sink(s, id, depth, at) {
    const o = s.objs.get(id);
    later(s, id, [[o.x, o.y, o.z], [o.x, o.y, o.z - depth]], 8, at);
  }
  // A name tag set by hand, for blocks the guide has no model or card of their own for.
  function name(s, id, text, lift) {
    const o = s.objs.get(id);
    s.tags = s.tags.filter((t) => t.obj !== id);
    if (o) s.tags.push({ obj: id, kind: o.type === 'worker' ? 'worker:' + o.tool : o.type, text, lift });
  }

  // A straight working track at y = 1 with an unloading dock on the left and a charger on the right.
  function track(s, n, groundType) {
    for (let x = 0; x < n; x++) { s.put('ft' + x, groundType || 'grass', x, 1, -1); s.put('rt' + x, 'rail', x, 1, 0, { tier: 2, dir: 'x' }); }
  }
  function docks(s, n) {
    s.put('fd0', 'stone', 0, 2, -1); s.put('du', 'dock_unload', 0, 2, 0); s.put('fd1', 'stone', -1, 2, -1); s.put('dc', 'chest', -1, 2, 0);
    s.put('fd2', 'stone', n - 1, 2, -1); s.put('dch', 'dock_charge', n - 1, 2, 0); s.put('fd3', 'stone', n, 2, -1); s.put('dk', 'crystal', n, 2, 0);
  }
  function workerScene(id, tool, c, panel) {
    scene(id, [
      step(c[0], (s) => {
        track(s, 7);
        if (tool === 'reaper') for (let x = 0; x < 7; x++) { s.put('cf' + x, 'farmland', x, 0, -1); s.put('cc' + x, 'crop', x, 0, 0, { stage: 2 }); }
        s.put('w', 'worker', 3, 1, 0, { tool });
      }, panel ? 9 : 5),
      step(c[1], (s) => { s.move('w', [[0, 1, 0], [6, 1, 0]], 1.5, 'ping'); s.set('w', { busy: true }); }, 8),
      step(c[2], (s) => { docks(s, 7); s.set('w', { path: null, x: 0, busy: false }); s.flow('u', [[0, 1, 0], [0, 2, 0], [-1, 2, 0]]); s.badge('t', 0, 1, 0, 'трюм полон → разгрузка', 'info'); }, 7),
      step(c[3], (s) => { s.flow('u', null); s.set('w', { x: 6 }); s.badge('t', 6, 1, 0, 'заряд 20% → зарядка', 'warn'); s.flow('e', [[7, 2, 0], [6, 2, 0], [6, 1, 0]], { color: EN, n: 2 }); }, 7),
    ]);
  }
  workerScene('chassis', 'saw', ['Шасси ставится на монорельс. В его окне выбирается один инструмент и улучшения.', 'Работник сам ездит по своему участку и работает вдоль него.', 'Когда трюм полон, он сам едет к блоку разгрузки.', 'Когда садится батарея, он сам едет к блоку зарядки.'], true);
  workerScene('dock_unload', 'reaper', ['Работник ездит по участку и собирает урожай в трюм.', 'Трюм постепенно заполняется.', 'Блок разгрузки стоит вплотную к пути. Он забирает груз и отдаёт его в сундук или магистраль.', 'Блок зарядки стоит на том же участке.']);
  workerScene('dock_charge', 'reaper', ['У работника есть батарея.', 'Пока заряда хватает, он работает.', 'Разгрузка и зарядка стоят на одном участке пути.', 'При заряде ниже 20% работник сам едет к блоку зарядки. Блок питается от магистрали или источника рядом.']);
  workerScene('upgrades', 'saw', ['Улучшения вставляются в окно шасси рядом с инструментом.', 'Скорость ускоряет работу, радиус расширяет зону вдоль пути.', 'Ёмкость увеличивает трюм: к блоку разгрузки работник ездит реже.', 'Батарея увеличивает запас: к блоку зарядки работник ездит реже.'], true);

  // A tree of blocks, four logs and a crown of leaves, so the saw can take it from the top down.
  function blockTree(s, id, x, y) {
    for (let z = 0; z < 4; z++) s.put(id + 'l' + z, 'log', x, y, z);
    for (let i = -1; i <= 1; i++) for (let j = -1; j <= 1; j++) {
      if (i || j) s.put(id + 'a' + i + j, 'leaves', x + i, y + j, 3);
      if (!i || !j) s.put(id + 'b' + i + j, 'leaves', x + i, y + j, 4);
    }
  }
  const crown = (s, id, z) => { for (const o of [...s.objs.values()]) if (o.id.startsWith(id) && o.type === 'leaves' && o.z === z) s.del(o.id); };
  scene('tool_saw', [
    step('Вдоль пути растут деревья. Лесоруб едет по участку и останавливается у дерева.', (s) => {
      track(s, 9); for (let x = 0; x < 9; x++) s.put('g' + x, 'grass', x, 0, -1);
      blockTree(s, 'ta', 3, 0); blockTree(s, 'tb', 7, 0);
      s.put('w', 'worker', 0, 1, 0, { tool: 'saw' }).move('w', [[0, 1, 0], [2.6, 1, 0]], 1, 'once');
    }, 6),
    step('Пила поворачивается к дереву и выдвигается. Первой пропадает верхушка кроны.', (s) => {
      s.set('w', { busy: true, aim: 'tal1', reach: 0.45 }); crown(s, 'ta', 4);
    }, 6),
    step('Дерево исчезает сверху вниз. Брёвна и саженцы сразу попадают в трюм, листва пропадает.', (s) => {
      crown(s, 'ta', 3); s.del('tal3', 'tal2');
    }, 6),
    step('Срублено последнее бревно. Пила убирается, а на месте дерева лесоруб сажает саженец из трюма.', (s) => {
      s.del('tal1', 'tal0'); s.put('ts', 'tree', 3, 0, 0, { stage: 0 }); s.set('w', { busy: false, aim: null, reach: 0 });
    }, 7),
    step('Брёвна лесоруб отвозит на блок разгрузки. Потом он едет к следующему дереву.', (s) => {
      docks(s, 9); s.move('w', [[2.6, 1, 0], [0, 1, 0]], 1, 'once'); s.flow('u', [[0, 1, 0], [0, 2, 0], [-1, 2, 0]], { color: '#8a6a3a', n: 2 });
    }, 8),
  ]);

  // Plants of the base game (spec section 17), each with a name tag set by hand.
  const BUSHES = [8, 9];
  // Ages of the cocoa pods up the x+ side of the jungle trunk, from z 1: the reaper takes the ripe ones.
  const COCOA_AGES = [2, 1, 2];
  // Wheat along the front row; behind the track wheat, sugar cane, a pumpkin with its stem and berry
  // bushes (behind, so what sinks out of sight stays hidden under the floor); a jungle trunk with
  // cocoa and a rail up its side at the end of the track.
  const WHEAT = [[1, 0], [2, 0]].concat([1, 2, 3, 4, 5, 6, 7, 8].map((x) => [x, 2]));
  function farm(s) {
    track(s, 10);
    for (let x = 0; x < 10; x++) for (const y of [0, 2]) s.put('fg' + x + y, 'grass', x, y, -1);
    for (const [x, y] of WHEAT) { s.put('fg' + x + y, 'farmland', x, y, -1); s.put('wh' + x + y, 'crop', x, y, 0, { stage: 2 }); }
    for (const x of [4, 5]) for (let z = 0; z < 3; z++) s.put('cn' + x + z, 'sugar_cane', x, 0, z);
    s.put('pk', 'pumpkin', 6, 0, 0); s.put('fg70', 'farmland', 7, 0, -1); s.put('st', 'pumpkin_stem', 7, 0, 0, { face: 'x-' });
    for (const x of BUSHES) s.put('bb' + x, 'sweet_berry_bush', x, 0, 0, { berries: true });
    floor(s, 10, 0, 3, 2, 'grass');
    for (let z = 0; z < 4; z++) { s.put('jl' + z, 'jungle_log', 11, 0, z); s.put('jr' + z, 'rail', 11, 1, z, { face: 'wy', dir: 'z', tier: 2 }); }
    COCOA_AGES.forEach((age, i) => s.put('cc' + (i + 1), 'cocoa', 12, 0, i + 1, { face: 'x-', age }));
    for (const [x, y] of [[11, 0], [10, 0], [12, 0], [11, -1], [11, 1]]) s.put('jf' + x + '_' + y, 'leaves', x, y, 4);
    name(s, 'fg22', 'Пшеница', 1.9); name(s, 'cn40', 'Тростник', 3.2); name(s, 'fg70', 'Тыква на стебле', 2.1);
    name(s, 'fg90', 'Сладкие ягоды', 2.3); name(s, 'jl1', 'Какао на стволе джунглей', 1);
  }
  // Ripe wheat sinks into its farmland as the reaper passes, a young sprout rises in its place.
  function reapWheat(s, x0, when) {
    for (const [x, y] of WHEAT) {
      if (x <= x0 || x > x0 + 3) continue;
      sink(s, 'wh' + x + y, 1, when(x));
      s.put('wy' + x + y, 'crop', x, y, -1, { stage: 0 }); later(s, 'wy' + x + y, [[x, y, -1], [x, y, 0]], 3, when(x) + 0.3);
    }
  }
  const REAP = [
    'Пшеницу жатка срывает зрелой и сразу засевает снова семенами из урожая. Она работает по обе стороны пути.',
    'Тростник она срезает выше нижнего блока, основание растёт дальше. У тыквы ломает плод, стебель остаётся.',
    'Сладкие ягоды она снимает, куст остаётся и растит новые.',
  ];
  // The reaper drives on without stopping or turning: each step takes it three blocks on, and every
  // plant changes as the reaper passes it.
  const reapStep = (i, harvest) => step(REAP[i], (s) => {
    const x0 = 3 * i, v = 3 / hold(REAP[i]);
    s.set('w', { busy: true }).move('w', [[x0, 1, 0], [x0 + 3, 1, 0]], v, 'once');
    const when = (x) => (x - 0.6 - x0) / v;
    reapWheat(s, x0, when); harvest(s, when);
  });
  scene('tool_reaper', [
    step('Одна жатка понимает все растения, настраивать её не нужно. Вдоль пути пшеница, тростник, тыква, ягоды и какао.', (s) => {
      farm(s); s.put('w', 'worker', 0, 1, 0, { tool: 'reaper' });
    }, 8),
    reapStep(0, () => {}),
    reapStep(1, (s, when) => {
      for (const x of [4, 5]) { sink(s, 'cn' + x + '2', 2, when(x)); sink(s, 'cn' + x + '1', 1, when(x)); }
      // Without its pumpkin the bent stem turns back into a straight one, as in the game: the
      // straight stem waits in the farmland and rises as the bent one sinks.
      sink(s, 'pk', 1.3, when(6)); sink(s, 'st', 1, when(6));
      s.put('su', 'pumpkin_stem', 7, 0, -1, { age: 7 }); later(s, 'su', [[7, 0, -1], [7, 0, 0]], 8, when(6) + 0.05);
    }),
    reapStep(2, (s, when) => {
      s.del('st'); name(s, 'cn40', 'Основание тростника', 1.2); name(s, 'fg70', 'Стебель тыквы', 1.9);
      for (const x of BUSHES) {
        sink(s, 'bb' + x, 1.3, when(x));
        s.put('bx' + x, 'sweet_berry_bush', x, 0, -1.3, { berries: false }); later(s, 'bx' + x, [[x, 0, -1.3], [x, 0, 0]], 8, when(x) + 0.05);
      }
    }),
    step('Монорельс идёт и по стволу джунглей. Жатка срывает зрелые какао-бобы и сажает их снова на тот же ствол.', (s) => {
      s.put('w2', 'worker', 11, 1, 0, { tool: 'reaper', busy: true, mount: 'side' }).move('w2', [[11, 1, 0, 'side'], [11, 1, 3, 'side']], 0.8, 'ping');
      COCOA_AGES.forEach((age, i) => { if (age === 2) s.set('cc' + (i + 1), { age: 0 }); });
    }, 9),
  ]);
  function holdScene(id, c) {
    scene(id, [
      step(c[0], (s) => {
        track(s, 9, 'stone'); s.put('f0', 'stone', 0, 2, -1); s.put('dl', 'dock_load', 0, 2, 0); s.put('f1', 'stone', -1, 2, -1); s.put('c0', 'chest', -1, 2, 0);
        s.put('f2', 'stone', 8, 2, -1); s.put('du', 'dock_unload', 8, 2, 0); s.put('f3', 'stone', 9, 2, -1); s.put('c1', 'chest', 9, 2, 0);
        s.badge('a', -1, 2, 0.5, 'База', 'info'); s.badge('b', 9, 2, 0.5, 'Форпост', 'info');
      }, 7),
      step(c[1], (s) => { s.put('w', 'worker', 0, 1, 0, { tool: 'hold' }); s.flow('l', [[-1, 2, 0], [0, 2, 0], [0, 1, 0]]); }, 6),
      step(c[2], (s) => { s.flow('l', null); s.move('w', [[0, 1, 0], [8, 1, 0]], 2.2, 'once'); }, 6),
      step(c[3], (s) => { s.flow('u', [[8, 1, 0], [8, 2, 0], [9, 2, 0]]); }, 7),
    ]);
  }
  holdScene('tool_hold', ['База и форпост далеко друг от друга. Тянуть магистраль долго и дорого, между ними монорельс.', 'Блок погрузки загружает грузовую вагонетку.', 'Вагонетка везёт груз по монорельсу.', 'Блок разгрузки принимает груз на другом конце. Вагонетка возвращается за следующим.']);
  holdScene('dock_load', ['Блок погрузки стоит вплотную к пути и берёт предметы из сундука или магистрали.', 'Когда подъезжает грузовая вагонетка, он загружает её.', 'Гружёная вагонетка уезжает.', 'На другом конце её ждёт блок разгрузки.']);

  // The ring round a living vein: a rail loop with rounded corners, and the grabber's way along it.
  const RING = [[1, 0, 0]].concat(arc(4, 0, 0, 'x-', 'y+'), arc(4, 4, 0, 'y-', 'x-'), arc(0, 4, 0, 'x+', 'y-'), arc(0, 0, 0, 'y+', 'x+'), [[1, 0, 0]]);
  const RING_LEN = RC.pathLength(RING), RING_SPEED = 2;
  // How far along the ring the grabber is closest to the cell (x, y).
  const ringNear = (x, y) => {
    let best = 0, near = Infinity;
    for (let d = 0; d < RING_LEN; d += 0.05) {
      const p = RC.pointOnPath(RING, d), far = Math.hypot(p[0] - x, p[1] - y);
      if (far < near) { near = far; best = d; }
    }
    return best;
  };
  function veinScene(id, c) {
    const spots = [[1, 2, 'iron'], [3, 2, 'coal'], [3, 1, 'copper'], [2, 3, 'diamond']].map(([x, y, o]) => [x, y, o, ringNear(x, y)]);
    scene(id, [
      step(c[0], (s) => {
        floor(s, 0, -3, 5, 8); s.put('v', 'vein', 2, 2, 0, { on: true });
        // Stone and energy come up from a trunk in a trench under the floor.
        for (let y = -2; y < 3; y++) { s.del('fl2_' + y + '_-1'); s.put('vp' + y, 'pipe', 2, y, -1, { ch: 'ie--' }); }
        s.put('vq', 'pipe', 2, -2, 0, { ch: 'ie--' }); s.put('vc', 'chest', 3, -2, 0); s.put('vk', 'crystal', 1, -2, 0); s.put('vn', 'node', 2, -3, 0);
        s.flow('st', [[3, -2, 0], [2, -2, 0], [2, -2, -1], [2, 2, -1], [2, 2, 0]], { color: '#8b8d91', n: 2 });
        s.flow('en', [[1, -2, 0], [2, -2, 0], [2, -2, -1], [2, 2, -1], [2, 2, 0]], { color: EN, n: 2 });
      }, 7),
      step(c[1], (s) => { spots.forEach(([x, y, o], i) => s.put('o' + i, 'ore_' + o, x, y, 0)); }, 7),
      step(c[2], (s) => {
        for (let i = 0; i < 5; i++) for (let j = 0; j < 5; j++) if (i === 0 || i === 4 || j === 0 || j === 4) s.put('r' + i + j, 'rail', i, j, 0, { tier: 4 });
        s.put('w', 'worker', 1, 0, 0, { tool: 'grab', busy: true }).move('w', RING, RING_SPEED, 'loop'); s.ring0 = s.now;
      }, 7),
      // Each ore sinks out of sight as the grabber passes it; then the vein grows a new one.
      step(c[3], (s) => {
        const at = ((s.now - s.ring0) * RING_SPEED) % RING_LEN;
        spots.forEach(([x, y, o, d], i) => sink(s, 'o' + i, 1.3, (((d - at) % RING_LEN) + RING_LEN) % RING_LEN / RING_SPEED));
        s.put('o4', 'ore_redstone', 1, 3, -1.3); later(s, 'o4', [[1, 3, -1.3], [1, 3, 0]], 2, RING_LEN / RING_SPEED + 0.5);
      }, 10),
    ]);
  }
  veinScene('vein', ['Живая жила потребляет камень и энергию. Они приходят по магистрали под полом.', 'В свободных клетках вокруг неё вырастают блоки руды. Какая руда вырастет, решает случай.', 'По кольцу вокруг жилы ездит вагонетка с захватом. На углах рельс скруглён.', 'Вагонетка собирает руду, когда проезжает мимо, а жила уже растит следующую.']);
  veinScene('tool_grab', ['Захват нужен там, где руда появляется рядом с путём, например у живой жилы.', 'Жила вырастила несколько блоков руды.', 'Вагонетка-сборщик едет по кольцу вокруг неё.', 'Проезжая мимо, она ломает блоки руды в соседних клетках и забирает их в трюм.']);

  // The bore digs one cell at a time as it gets there: the rock of the cell sinks into the floor
  // under it and the new rail rises out of it.
  const BORE = [
    'Впереди сплошная порода. Проходчик стоит на конце пути, в трюме рельсы и немного блоков.',
    'Без модулей он бурит тоннель 1×1 и сам кладёт в нём рельс.',
    'Модуль коридора расширяет тоннель до 3×3, следующие модули — до 9×9.',
    'Модуль высоты держит путь на одном уровне, даже если порода над ним поднимается.',
    'Впереди лава. Модуль лавы убирает её сам, блоки из трюма на неё не тратятся.',
  ];
  const boreStep = (i, x0, x1, wide, more) => step(BORE[i], (s) => {
    const v = (x1 - x0) / hold(BORE[i], 7);
    s.badges = []; if (more) more(s);
    s.set('w', { busy: true }).move('w', [[x0, 1, 0], [x1, 1, 0]], v, 'once');
    for (let x = x0 + 1; x <= x1; x++) {
      const at = (x - 1.2 - x0) / v;
      for (const y of [0, 1]) for (let z = 0; z < 3; z++) if ((wide || (y === 1 && z === 0)) && s.objs.has('m' + x + '_' + y + '_' + z)) sink(s, 'm' + x + '_' + y + '_' + z, z + 1, at);
      s.put('rt' + x, 'rail', x, 1, -1, { tier: 2 }); later(s, 'rt' + x, [[x, 1, -1], [x, 1, 0]], 6, at + 0.2);
    }
  }, 7);
  scene('tool_bore', [
    step(BORE[0], (s) => {
      floor(s, 0, 0, 12, 3);
      for (let x = 0; x < 3; x++) s.put('rt' + x, 'rail', x, 1, 0, { tier: 2 });
      for (let x = 3; x < 12; x++) for (const y of [0, 1]) for (let z = 0; z < (x > 7 ? 4 : 3); z++) s.put('m' + x + '_' + y + '_' + z, 'stone', x, y, z);
      s.del('m10_1_0'); s.put('lv', 'lavapool', 10, 1, 0);
      s.put('w', 'worker', 2, 1, 0, { tool: 'bore' });
    }, 7),
    boreStep(1, 2, 5, false),
    boreStep(2, 5, 7, true, (s) => s.badge('m', 7, 1, 1, 'модуль коридора: 3×3', 'info')),
    boreStep(3, 7, 9, true, (s) => s.badge('m', 9, 1, 2, 'модуль высоты', 'info')),
    boreStep(4, 9, 10, true, (s) => { s.badge('m', 10, 1, 1, 'модуль лавы', 'info'); s.del('lv'); }),
  ]);
  S.mod_tunnel = S.mod_height = S.mod_lava = S.tool_bore;

  // The mob walks in and stops in range. The guard follows it with head and barrel (aim) from the
  // moment it shows up; the shot is a beam from the barrel to the mob.
  function guardScene(id, c) {
    const muzzle = [3.5, 1.9, 0.85];
    scene(id, [
      step(c[0], (s) => {
        floor(s, 0, 0, 7, 4, 'grass');
        for (let x = 0; x < 7; x++) s.put('r' + x, 'rail', x, 1, 0, { tier: 3, dir: 'x' });
        s.put('w', 'worker', 0, 1, 0, { tool: 'gun' }).move('w', [[0, 1, 0], [6, 1, 0]], 1.2, 'ping');
      }, 6),
      step(c[1], (s) => {
        s.put('m', 'mob', 6, 3, 0).move('m', [[6, 3, 0], [3.5, 3, 0]], 0.5, 'once');
        const p = s.position(s.objs.get('w'), s.now);
        s.set('w', { aim: 'm' }).move('w', [[p[0], 1, 0], [3, 1, 0]], 1.2, 'once');
      }, 7),
      step(c[2], (s) => {
        s.set('m', { hit: true, path: null, x: 3.5, y: 3, face: 'x-' }); s.beam('b', muzzle, [4, 3.5, 1.1]);
        s.set('w', { busy: true });
      }, 6),
      step(c[3], (s) => {
        s.del('m'); s.beam('b', null); s.put('d1', 'drop', 3, 3, 0, { color: '#8a4a2a' }); s.put('d2', 'drop', 4, 3, 0, { color: '#d8d8d8' });
        s.badge('x', 3.5, 3, 0, 'добыча и опыт — владельцу', 'ok');
        s.set('w', { busy: false, aim: null });
      }, 8),
      step(c[4], (s) => {
        s.badges = []; s.del('d1', 'd2');
        s.put('pl', 'player', 6, 3, 0).move('pl', [[6, 3, 0], [3, 3, 0]], 0.8, 'once');
        s.move('w', [[3, 1, 0], [6, 1, 0], [0, 1, 0]], 1.2, 'once');
      }, 7),
    ]);
  }
  guardScene('tool_gun', ['Вагонетка-страж патрулирует свой участок монорельса у базы.', 'К базе идёт враждебный моб. Страж останавливается и поворачивает дуло за ним.', 'Страж стреляет. Каждый выстрел тратит заряд его батареи, боеприпасы не нужны.',
    'Моб убит. Добыча и опыт достаются владельцу стража, как за своё убийство. Добычу подберёт вагонетка с захватом.', 'По игрокам, питомцам и мирным мобам страж не стреляет. Он снова патрулирует участок.']);

  // The turret on a field of 13×13 blocks, so the zone of a support turret, 6 blocks round it,
  // fits on the ground (spec section 27). Attack first, then the effect modules, then support.
  const TX = 6, TY = 6, FIELD = 13, SUPPORT_R = 6, HEAL = '#ff8aa8';
  const turretField = (s) => {
    floor(s, 0, 0, FIELD, FIELD, 'grass');
    s.put('t', 'turret', TX, TY, 0); s.put('k', 'crystal', TX - 1, TY, 0);
  };
  // The tip of the barrel of a turret at (x, y) that aims at the middle of the cell (tx, ty).
  const muzzle = (x, y, tx, ty) => {
    const dx = tx - x, dy = ty - y, d = Math.hypot(dx, dy) || 1;
    return [x + 0.5 + (0.72 * dx) / d, y + 0.5 + (0.72 * dy) / d, 0.62];
  };
  // The zone of a support turret: a ring of beams on the ground, r blocks round the middle of the
  // turret's block; r = 0 takes it away.
  const RING_SEGMENTS = 32;
  function zone(s, r) {
    const at = (k) => {
      const a = (k / RING_SEGMENTS) * 2 * Math.PI;
      return [TX + 0.5 + r * Math.cos(a), TY + 0.5 + r * Math.sin(a), 0.03];
    };
    for (let i = 0; i < RING_SEGMENTS; i++) s.beam('zone' + i, r ? at(i) : null, at(i + 1), SOL);
  }
  // The first step of a scene that starts in the middle of the story lays the field first.
  const withField = (st) => step(st.text, (s) => { turretField(s); st.run(s); }, st.dur);

  const TURRET_ATTACK = [
    step('Турель стоит на месте. Энергию ей даёт кристалл рядом или магистраль.', turretField, 6),
    step('К базе идёт враждебный моб. Турель поворачивает голову и дуло за ним.', (s) => {
      s.put('m', 'mob', 12, 9, 0).move('m', [[12, 9, 0], [9.5, 8, 0]], 0.5, 'once');
      s.set('t', { aim: 'm' });
    }, 7),
    step('Моб вошёл в радиус: турель стреляет. Каждый выстрел тратит 40 Е.', (s) => {
      s.set('m', { hit: true, path: null, x: 9.5, y: 8, face: 'x-' }).set('t', { on: true });
      s.beam('b', muzzle(TX, TY, 9.5, 8), 'm');
    }, 6),
    step('Моб убит. Владелец турели получает добычу и опыт, как будто убил его сам. Добыча лежит на месте.', (s) => {
      s.del('m'); s.beam('b', null); s.put('d1', 'drop', 9, 8, 0, { color: '#8a4a2a' }); s.put('d2', 'drop', 10, 8, 0, { color: '#d8d8d8' });
      s.badge('x', 9.5, 8, 0, 'добыча и опыт — владельцу', 'ok');
      s.set('t', { on: false, aim: null });
    }, 8),
    step('По игрокам, питомцам и мирным мобам турель не стреляет.', (s) => {
      s.badges = []; s.del('d1', 'd2');
      s.put('pl', 'player', 12, 11, 0).move('pl', [[12, 11, 0], [8, 10, 0]], 0.8, 'once');
    }, 7),
  ];
  const TURRET_EFFECTS = [
    step('В гнёздах турели модули кровотечения, яда и замедления. Разные модули действуют вместе: каждый выстрел и кровоточит, и травит, и замедляет. Каждый модуль добавляет к выстрелу 10 Е.', (s) => {
      s.put('m2', 'mob', 12, 2, 0, { hit: true }).move('m2', [[12, 2, 0], [9.5, 3.5, 0]], 0.25, 'once');
      s.set('t', { aim: 'm2', on: true }); s.beam('b', muzzle(TX, TY, 9.5, 3.5), 'm2');
      s.badge('shot', TX, TY, 0.7, 'выстрел: 40 + 3 × 10 Е', 'info');
    }, 10),
    step('Замедленный моб едва идёт. Одинаковые модули складываются, но замедление не больше 60%. Зомби — нежить, яд на него не действует. Элиты и боссы SoulsCraft получают эффекты вдвое короче.', (s) => {
      s.set('m2', { path: null, x: 9.5, y: 3.5, face: 'x-' });
      s.badge('shot', null);
      s.badge('fx', 9.5, 3.5, 0, 'кровоточит · −20% скорости', 'bad');
      s.badge('fx2', 9.5, 3.5, 0.65, 'яд: нежить не травится', 'warn');
    }, 10),
  ];
  const TURRET_SUPPORT = [
    step('Модуль поддержки переводит турель в режим поддержки. Она больше не стреляет по мобам, а действует на игроков в радиусе 6 блоков.', (s) => {
      s.del('m2'); s.beam('b', null); s.badges = [];
      s.set('t', { on: false, support: true, aim: null });
      // The name tag is made again, so it names the turret by its new role.
      s.tags = s.tags.filter((tg) => tg.obj !== 't');
      s.put('p1', 'player', 10, 3, 0, { face: 'x-' }); s.put('pl', 'player', 8, 10, 0, { face: 'y-' });
      zone(s, SUPPORT_R);
      s.badge('r', TX + SUPPORT_R, TY, 0, 'радиус 6 блоков', 'info');
    }, 9),
    step('Лечение: раз в 3 секунды полсердца самому раненому игроку в радиусе. Луч показывает, кого турель лечит. Пока есть кого лечить, модуль тратит 8 Е/с.', (s) => {
      s.badge('r', null);
      s.set('t', { aim: 'p1' }); s.beam('h', muzzle(TX, TY, 10, 3), 'p1', HEAL);
      s.badge('hp1', 10, 3, 0, 'здоровье 6 из 20 · +1 раз в 3 с', 'ok');
      s.badge('hp2', 8, 10, 0, 'здоровье 18 из 20', 'info');
    }, 10),
    step('Защита и сила действуют на всех игроков в радиусе. Вышел из радиуса — бафы спадают через 2 секунды.', (s) => {
      s.badge('hp1', 10, 3, 0, 'защита −10% урона · сила +10% урона', 'ok');
      s.badge('hp2', null);
      s.move('pl', [[8, 10, 0], [12, 12, 0]], 0.8, 'once');
      s.badge('out', 12, 12, 0, 'вне радиуса: бафы спадут через 2 с', 'warn');
    }, 10),
    step('По мобам турель поддержки не стреляет. Одна турель — одна роль: на волну у алтаря SoulsCraft ставь рядом турели атаки.', (s) => {
      s.badges = [];
      s.put('ta', 'turret', 1, 11, 0, { aim: 'm3', on: true }); s.put('k2', 'crystal', 0, 11, 0);
      s.put('m3', 'mob', 0, 1, 0, { hit: true }).move('m3', [[0, 1, 0], [3, 3, 0]], 0.4, 'once');
      s.beam('b', muzzle(1, 11, 3, 3), 'm3');
      s.badge('t', TX, TY, 0.7, 'по мобам не стреляет', 'info');
    }, 10),
  ];
  scene('turret', TURRET_ATTACK.concat(TURRET_EFFECTS, TURRET_SUPPORT));
  scene('mod_bleed', [withField(TURRET_EFFECTS[0])].concat(TURRET_EFFECTS.slice(1)));
  S.mod_poison = S.mod_slow = S.mod_bleed;
  scene('mod_heal', [withField(TURRET_SUPPORT[0])].concat(TURRET_SUPPORT.slice(1)));
  S.mod_guard = S.mod_power = S.mod_haste = S.mod_heal;

  // ================================================================ mining
  scene('miner', [
    step('Добытчик ставится на поверхность и просматривает породу под собой до самого бедрока.', (s) => {
      const ores = { '1,-2': 'iron', '3,-3': 'diamond', '0,-4': 'coal', '4,-1': 'copper' };
      for (let x = 0; x < 5; x++) for (const y of [1, 2]) {
        s.put('g' + x + y, 'grass', x, y, 0);
        for (let z = -4; z < 0; z++) s.put('s' + x + y + z, y === 2 && ores[x + ',' + z] ? 'ore_' + ores[x + ',' + z] : z < -2 ? 'deepslate' : 'stone', x, y, z);
      }
      s.put('m', 'miner', 2, 1, 1, { on: true }); s.beam('b', [2.5, 1.5, 1], [1.5, 2.5, -1.5]);
    }, 7),
    step('Руда вытянута, на её месте та же порода, что вокруг. Добытое идёт в сундук, если он стоит вплотную.', (s) => {
      s.put('s12-2', 'stone', 1, 2, -2); s.put('s42-1', 'stone', 4, 2, -1); s.beam('b', [2.5, 1.5, 1], [3.5, 2.5, -2.5]);
      s.put('c', 'chest', 3, 1, 1); s.flow('o', [[2, 1, 1], [3, 1, 1]], { n: 2 });
    }, 8),
    step('Сундука рядом нет: руда уходит в магистраль.', (s) => {
      s.del('c'); s.put('p', 'pipe', 3, 1, 1, { ch: 'i---' }); s.put('c2', 'chest', 4, 1, 1); s.put('nd', 'node', 3, 1, 2);
      s.flow('o', [[2, 1, 1], [4, 1, 1]], { n: 2 });
    }, 7),
    step('Нет и магистрали: руда ждёт в блоке погрузки, откуда её увозит грузовая вагонетка.', (s) => {
      s.del('p', 'c2', 'nd'); s.put('dl', 'dock_load', 3, 1, 1);
      for (let x = 0; x < 5; x++) s.put('rr' + x, 'rail', x, 2, 1, { tier: 2 });
      s.put('w', 'worker', 0, 2, 1, { tool: 'hold' }).move('w', [[0, 2, 1], [3, 2, 1]], 1, 'once');
      s.flow('o', [[2, 1, 1], [3, 1, 1], [3, 2, 1]], { n: 2 });
    }, 8),
    step('Руда в области кончилась, и добытчик говорит об этом. Его переносят на новое место или ведут туда монорельс.', (s) => {
      s.put('s32-3', 'deepslate', 3, 2, -3); s.put('s02-4', 'deepslate', 0, 2, -4); s.beam('b', null); s.flow('o', null); s.set('m', { on: false });
      s.badge('d', 2, 1, 1, 'Область выработана', 'warn');
    }, 8),
  ]);

  // ================================================================ base
  // Chunks drawn as plots of 3×3 blocks, grass and dirt in turn; the anchor stands in the middle one.
  scene('anchor', [
    step('Ты ушёл с базы. Без игрока рядом чанки выгружаются, и машины в них замирают. Чанк в сцене уменьшен до 3×3 блоков.', (s) => {
      for (let x = 0; x < 9; x++) for (let y = 0; y < 9; y++) s.put('ag' + x + '_' + y, (((x / 3) | 0) + ((y / 3) | 0)) % 2 ? 'dirt' : 'grass', x, y, -1);
      s.put('f', 'furnace', 4, 3, 0); s.put('k', 'crystal', 4, 4, 0);
      s.put('c', 'crusher', 1, 7, 0); s.put('k2', 'crystal', 2, 7, 0); s.put('f2', 'furnace', 7, 1, 0); s.put('k3', 'crystal', 7, 2, 0);
      s.badge('z', 4, 3, 0, 'замерло', 'warn');
    }, 9),
    step('Простой якорь есть уже на первой ступени. Он держит прогруженным один чанк, в котором стоит, пока ты на сервере.', (s) => {
      s.put('a', 'anchor', 3, 4, 0, { on: true }); s.set('f', { on: true });
      s.badges = []; s.badge('ok', 3, 4, 0, 'держит 1 чанк', 'ok'); s.badge('z', 1, 7, 0, 'замерло', 'warn');
    }, 9),
    step('Модуль радиуса расширяет область до 3×3 чанков, второй модуль — до 5×5. Якорь тратит энергию за каждый чанк.', (s) => {
      s.set('c', { on: true }).set('f2', { on: true }); s.badges = []; s.badge('ok', 3, 4, 0, 'модуль радиуса: 3×3 чанка', 'ok');
    }, 9),
    step('Когда ты выходишь с сервера, якорь отключается, и машины снова замирают.', (s) => {
      ['a', 'f', 'c', 'f2'].forEach((id) => s.set(id, { on: false }));
      s.badges = []; s.badge('ok', 3, 4, 0, 'Владелец вышел с сервера', 'warn');
    }, 7),
  ]);
  S.mod_radius = S.anchor;

  // ================================================================ gear
  // Steve in technical armour: the four parts stand in his cell and move and face with him.
  const SUIT = { ah: 'armor_helmet', ac: 'armor_chest', al: 'armor_legs', ab: 'armor_boots' };
  function suit(s, x, y, z, props, gear) {
    s.put('pl', 'player', x, y, z, props);
    for (const id in SUIT) s.put(id, SUIT[id], x, y, z, Object.assign({}, props, gear));
    name(s, 'ac', 'Техническая броня', 2.25);
  }
  const suitSet = (s, props) => Object.keys(SUIT).forEach((id) => s.set(id, props));
  const suitMove = (s, pts, speed, delay) => ['pl', 'jp'].concat(Object.keys(SUIT)).forEach((id) => s.objs.has(id) && later(s, id, pts, speed, delay || 0));
  scene('armor', [
    step('Техническая броня — четыре части: шлем, нагрудник, поножи и ботинки. Они надеваются в обычные слоты брони.', (s) => {
      floor(s, 0, 0, 5, 3); suit(s, 1, 1, 0, {}, { tier: 3 });
    }, 8),
    step('У каждой части своя батарея и по два гнезда под модули игрока. Модули работают, пока часть заряжена.', (s) => {
      suitSet(s, { on: true }); s.badge('a', 1, 1, 0.3, 'заряд 100%', 'ok');
    }, 9),
    step('Заряд кончился: модули выключились. Броню заряжает зарядник.', (s) => {
      suitSet(s, { on: false }); s.badge('a', 1, 1, 0.3, 'Заряд 0%', 'warn');
    }, 7),
    step('Броня из звёздной стали — четвёртая ступень: батарея больше, модули и чары остаются. Быстрее всего её заряжает зарядная плита.', (s) => {
      s.put('cp', 'charge_plate', 3, 1, 0, { on: true }); s.put('k', 'crystal', 4, 1, 0); suitSet(s, { tier: 4, on: true });
      suitMove(s, [[1, 1, 0], [3, 1, 0]], 1, 0.5); s.badge('a', 3, 1, 0.3, 'заряжается', 'ok');
    }, 9),
  ]);
  scene('pmodules', [
    step('Модуль игрока вставляется в гнездо своей части брони. В каждой части два гнезда.', (s) => { floor(s, 0, 0, 8, 2); suit(s, 0, 1, 0, { face: 'x+' }, { tier: 3, on: true }); }, 8),
    step('Рывок в поножах: двойное нажатие бросает тебя на 4 блока вперёд.', (s) => {
      suitMove(s, [[0, 1, 0], [4, 1, 0]], 6, 1.5); s.badge('a', 2, 1, 0.7, 'рывок: 4 блока', 'ok');
    }, 6),
    step('Бег в поножах: 5 секунд после начала бега ты бежишь на 50% быстрее.', (s) => {
      suitMove(s, [[4, 1, 0], [7, 1, 0]], 1.6, 0.5); s.badge('a', 5, 1, 0.7, 'бег +50%', 'ok');
    }, 7),
    step('Демпфер в нагруднике: 5 секунд после полученного урона урон по тебе меньше на 20%.', (s) => {
      s.put('m', 'mob', 7, 0, 0, { face: 'y+' }); s.badge('a', 7, 1, 0.7, 'демпфер: урон −20%', 'ok');
    }, 8),
  ]);
  const JET = [
    'Джетпак — модуль нагрудника технической брони. На земле он выключен.',
    'Взлёт: из двух сопел бьёт пламя, и ты поднимаешься.',
    'Можно зависнуть на месте или лететь в любую сторону. Каждая секунда полёта тратит 30 Е заряда.',
    'Посадка: ты плавно опускаешься на землю.',
    'Пламя погасло. Когда кончается заряд, полёт выключается сам: заряди нагрудник на заряднике.',
  ];
  scene('jetpack', [
    step(JET[0], (s) => {
      floor(s, -1, 0, 5, 3, 'grass'); suit(s, 2, 1, 0, { face: 'x-' }, { tier: 4 }); s.put('jp', 'jetpack', 2, 1, 0, { face: 'x-' }); s.tags = s.tags.filter((tg) => tg.obj !== 'ac');
    }, 6),
    step(JET[1], (s) => { s.set('jp', { on: true }); suitSet(s, { on: true }); suitMove(s, [[2, 1, 0], [2, 1, 2.5]], 0.9, 0.8); }, 6),
    step(JET[2], (s) => { suitMove(s, [[2, 1, 2.5], [0, 1, 2.5]], 0.8, 2.5); s.badge('a', 2, 1, 2.8, '30 Е/с', 'info'); }, 8),
    step(JET[3], (s) => { s.badges = []; suitMove(s, [[0, 1, 2.5], [0, 1, 0]], 2.5 / (hold(JET[3], 6) - 0.4), 0.2); }, 6),
    step(JET[4], (s) => { s.set('jp', { on: false }); suitSet(s, { on: false }); s.badge('a', 0, 1, 0.3, 'Заряд 0%', 'warn'); }, 7),
  ]);
  scene('backpack', [
    step('Рюкзак из искростали лежит в обычном слоте инвентаря и не занимает слот брони.', (s) => {
      floor(s, 0, 0, 3, 3, 'grass'); s.put('pl', 'player', 1, 1, 0); s.badge('a', 1, 1, 0.3, 'в инвентаре: рюкзак на 27 слотов', 'info');
    }, 7),
    step('Он открывается клавишей или правым кликом. В окне рюкзака есть гнездо под модуль стака: с ним в слоте до 1024 предметов.', (s) => {
      s.badge('a', 1, 1, 0.3, 'модуль стака: до 1024', 'ok');
    }, 8),
    step('Рюкзак из звёздной стали надевается в свой слот на спине. Нагрудник остаётся свободным, поэтому броня с джетпаком носится вместе с рюкзаком.', (s) => {
      s.badges = []; suit(s, 1, 1, 0, { pack: true, face: 'x-' }, { tier: 4 }); name(s, 'pl', 'Рюкзак на спине', 1.45);
    }, 8),
  ]);
  scene('harpoon', [
    step('Гарпун пока проверяется. Задумка такая: перед тобой уступ, гарпун бьёт на 20 блоков.', (s) => {
      floor(s, 0, 1, 3, 1, 'grass'); for (let z = -1; z < 3; z++) { s.put('c' + z, 'stone', 3, 1, z); s.put('d' + z, 'stone', 4, 1, z); }
      s.put('pl', 'player', 0, 1, 0);
    }, 7),
    step('Выстрел цепляется за блок.', (s) => s.beam('h', [0.5, 1.5, 1], [3.5, 1.5, 2.9], '#c9ccd4'), 5),
    step('Гарпун подтягивает тебя к блоку. Над ним свободно, и ты забираешься наверх.', (s) => { s.beam('h', null); s.move('pl', [[0, 1, 0], [2.4, 1, 2.6], [3, 1, 3]], 3, 'once'); }, 7),
  ]);

  // ---- energy weapons
  // Steve in technical armour with weapons in his hands: every weapon is a type of its own in his
  // cell (wp in the right hand, sh the shield in the left) and is given his face, arms and swing,
  // so it stays in his fist (poses.js). The armour's name tag gives way to the weapon's.
  const KIT = ['pl', 'wp', 'sh'].concat(Object.keys(SUIT));
  const kitSet = (s, props) => KIT.forEach((id) => s.set(id, props));
  const kitMove = (s, pts, speed, delay) => KIT.forEach((id) => s.objs.has(id) && later(s, id, pts, speed, delay || 0));
  function armed(s, x, y, z, props, gear, weapons) {
    suit(s, x, y, z, props, gear);
    s.tags = s.tags.filter((tg) => tg.obj !== 'ac');
    for (const id in weapons) s.put(id, weapons[id][0], x, y, z, Object.assign({}, props, weapons[id][1]));
  }
  // A zombie walks up from x0 to x1 along y and stops there, facing the way it came.
  const zombie = (s, x0, x1, y, speed) => s.put('m', 'mob', x0, y, 0).move('m', [[x0, y, 0], [x1, y, 0]], speed || 0.8, 'once');
  const stopZombie = (s, x, y, props) => s.set('m', Object.assign({ path: null, x, y, face: s.objs.get('m').x < x ? 'x+' : 'x-' }, props));
  scene('blade', [
    step('Энергоклинок — меч на заряде своей батареи. Прочности у него нет, сломаться он не может.', (s) => {
      floor(s, -1, 0, 7, 3, 'grass');
      armed(s, 1, 1, 0, { face: 'x+' }, { tier: 3, on: true }, { wp: ['blade', { tier: 3, on: true }] });
      zombie(s, 5, 2.6, 1); s.badge('c', 1, 1, 0.3, 'заряд 20 000 Е', 'ok');
    }, 7),
    step('Удар попал по зомби: клинок потратил 25 Е. Промах заряд не тратит.', (s) => {
      stopZombie(s, 2.6, 1, { hit: true }); kitSet(s, { swing: true }); s.badge('c', 1, 1, 0.3, '−25 Е за удар · 19 975 Е', 'info');
    }, 7),
    step('Заряд кончился: клинок погас и бьёт как рука, пока его не зарядят.', (s) => {
      s.set('wp', { on: false }); s.badge('c', 1, 1, 0.3, 'Заряд 0 Е', 'warn');
    }, 7),
    step('Клинок заряжается на заряднике и на зарядной плите, как броня.', (s) => {
      s.del('m'); kitSet(s, { swing: false }); s.put('cp', 'charge_plate', 1, 2, 0, { on: true }); s.put('k', 'crystal', 0, 2, 0); name(s, 'k', 'Кристалл энергии', 1);
      kitMove(s, [[1, 1, 0], [1, 2, 0]], 1.2, 0.3); s.set('wp', { on: true }); s.badge('c', 1, 2, 0.3, 'заряжается', 'ok');
    }, 8),
  ]);
  // The shield is on the left arm, so Steve faces y+, towards the viewer: the shield is on his
  // side nearest the camera and its face looks at the attackers and at the viewer. The arrow flies
  // from the skeleton's bow to the face of the raised shield and back.
  const ARROW = [[2.28, 4.34, 0.75], [2.15, 0.67, 0.75], [2.28, 4.34, 0.75]];
  const walkUp = (s, id, type, y0, y1) => s.put(id, type, 1.6, y0, 0).move(id, [[1.6, y0, 0], [1.6, y1, 0]], 0.8, 'once');
  scene('eshield', [
    step('Энергощит в левой руке. Пока в нём есть заряд, его поле светится.', (s) => {
      floor(s, 0, -1, 5, 7, 'grass');
      armed(s, 2, 0, 0, { face: 'y+' }, { tier: 3 }, { sh: ['eshield', { tier: 3, on: true }] });
      walkUp(s, 'm', 'mob', 4, 1.4);
    }, 7),
    step('Зомби бьёт, щит блокирует удар. За каждую единицу урона уходит 10 Е, но не меньше 10 Е за блок.', (s) => {
      s.set('m', { path: null, y: 1.4, face: 'y-' }); kitSet(s, { arms: [0, 0.9] }); s.set('sh', { hit: true });
      s.badge('c', 2, 0, 0.3, 'блок: 3 урона → −30 Е', 'info');
    }, 8),
    step('Щит четвёртой ступени отражает стрелы и другие снаряды обратно в стрелка.', (s) => {
      s.del('m'); suitSet(s, { tier: 4 }); s.set('sh', { tier: 4 });
      s.put('sk', 'skeleton', 2, 5, 0, { face: 'y-', hit: true });
      s.flow('ar', ARROW, { color: '#e6e6e6', n: 1, speed: 3 });
      s.badge('c', 2, 5, 0, 'стрела вернулась в стрелка', 'ok');
    }, 8),
    step('Удар топором выключает щит: на 5 секунд на второй ступени, на 3 на третьей и на полторы на четвёртой.', (s) => {
      s.del('sk'); s.flow('ar', null); kitSet(s, { arms: 'down' }); s.set('sh', { on: false, hit: false });
      s.put('p2', 'player', 1.6, 1.4, 0, { face: 'y-', swing: true }); s.put('ax', 'axe', 1.6, 1.4, 0, { face: 'y-', swing: true });
      s.badge('c', 2, 0, 0.3, 'поле выключено на 1,5 с', 'warn');
    }, 9),
  ]);
  // Steve aims along x from the cell (0, 1); with arms 'forward' the muzzle is 1.6 blocks ahead
  // of his cell corner, by his right hand (y + 0.84) and 1.44 blocks up.
  const MUZZLE = [1.6, 1.84, 1.44], SHOT = '#7af0ff';
  scene('rifle', [
    step('Энерговинтовка — оружие четвёртой ступени. Она бьёт по прямой на 64 блока.', (s) => {
      floor(s, -1, 0, 11, 3, 'grass');
      armed(s, 0, 1, 0, { face: 'x+' }, { tier: 4, on: true }, { wp: ['rifle', { tier: 4 }] });
      zombie(s, 9, 8, 1, 0.5);
    }, 7),
    step('Выстрел попадает сразу, без полёта и падения, в первое существо на линии.', (s) => {
      stopZombie(s, 8, 1, { hit: true }); kitSet(s, { arms: 'forward' }); s.set('wp', { on: true });
      s.beam('b', MUZZLE, [8.45, 1.5, 1.2], SHOT); s.badge('d', 8, 1, 0, '16 урона', 'warn'); s.badge('c', 0, 1, 0.3, '−400 Е', 'info');
    }, 7),
    step('Между выстрелами 1,25 секунды. Каждый выстрел тратит 400 Е, батареи хватает на 100 выстрелов.', (s) => {
      s.set('m', { hit: false }); s.set('wp', { on: false }); s.beam('b', null); s.badges = [];
      s.badge('c', 0, 1, 0.3, 'до выстрела 1,25 с · 39 600 Е', 'info');
    }, 8),
    step('Второй выстрел добил зомби. Когда заряд кончается, винтовка не стреляет, пока её не зарядят.', (s) => {
      s.del('m'); s.put('d1', 'drop', 8, 1, 0, { color: '#8a4a2a' }); s.badge('c', 0, 1, 0.3, 'заряд 39 200 Е', 'info');
    }, 8),
  ]);
  scene('guide', [
    step('Справочник показывает каждый блок в действии, как эта сцена.', (s) => bus(s, [['chest'], ['furnace'], ['chest']], { src: null })),
    step('Блок сам называет причину, когда не работает.', (s) => s.badge('d', 1, 0, 0, 'Нет энергии'), 6),
    step('Устрани причину, и диагноз исчезнет.', (s) => { s.badges = []; s.put('src', 'crystal', -1, 1, 0); s.set('b1', { on: true }); s.flow('in', r(0, 1)); s.flow('out', r(1, 2), { color: '#d8d8d8' }); power(s, 1); }, 7),
  ]);
  scene('enchants', [
    step('Технические чары накладываются обычным столом зачарований на части технической брони.', (s) => { floor(s, 0, 0, 3, 3); s.put('pl', 'player', 1, 1, 0, { armor: true }); }, 7),
    step('Ёмкость увеличивает батарею, бережливость снижает расход заряда.', (s) => s.badge('a', 1, 1, 0.7, 'Ёмкость III: батарея +75%', 'ok'), 7),
  ]);

  // ================================================================ round 14
  const swapFlows = (s, speed) => { s.flow('in', r(0, 1), { speed }); s.flow('out', r(1, 2), { color: '#d8d8d8', speed }); };
  plugScene('swap', [
    step('Электропечь первой ступени работает. У неё три гнезда под модули.', (s) => {
      bus(s, [['chest'], ['furnace', { on: true }], ['chest'], null]); swapFlows(s, 1.2); power(s, 1);
      s.put('fp', 'stone', 1, 2, -1); s.put('pl', 'player', 1, 2, 0); s.badge('m', 1, 0, 0, 'ступень 1 · гнёзд: 3', 'info');
    }, 7),
    step('Возьми печь второй ступени и кликни правой кнопкой по старой. Новая встала на её место, старая оказалась в руке.', (s) => {
      s.set('b1', { tier: 2 }); swapFlows(s, 1.8); s.badge('m', 1, 0, 0, 'ступень 2 · быстрее · гнёзд: 3', 'ok');
    }, 9),
    step('Предметы, жидкости, модули и настройки остались в новой машине. Линия не останавливалась.', (s) => { s.badge('m', 1, 0, 0, 'содержимое на месте', 'ok'); }, 7),
    step('У печи третьей ступени уже четыре гнезда. Так же заменяются узел магистрали, каналы в корпусах, монорельс и шасси работника.', (s) => {
      s.set('b1', { tier: 3 }).set('node', { tier: 3 }); swapFlows(s, 2.4); s.badge('m', 1, 0, 0, 'ступень 3 · гнёзд: 4', 'ok');
    }, 9),
  ]);
  plugScene('grow', [
    step('Кликни осколком кристалла по верху бака с питательным раствором. Осколок встал как затравка.', (s) => {
      bus(s, [['mixer', { on: true }], ['tank', { fluid: 'sol', level: 0.8, ease: 3 }], null], { ch: 'f-e-' }); s.put('sd', 'seed', 1, 0, 1, { stage: 0 }); s.flow('f', r(0, 1), { color: SOL, n: 2 });
    }, 8),
    step('Затравка пьёт раствор из бака и медленно растёт: на одну друзу уходит 4 ведра за 10 минут.', (s) => s.set('sd', { stage: 1 }).set('b1', { level: 0.5 }), 7),
    step('Раствор кончился, и рост стоит. Он продолжится, когда раствор снова появится.', (s) => {
      s.set('b0', { on: false }).set('b1', { level: 0.02 }); s.flow('f', null); s.badge('g', 1, 0, 0.6, 'Нет раствора в баке');
    }, 7),
    step('Раствор снова есть, и друза выросла. Сломай её и получи от двух до четырёх осколков.', (s) => {
      s.set('b0', { on: true }).set('b1', { level: 0.6 }).set('sd', { stage: 2 }); s.flow('f', r(0, 1), { color: SOL, n: 2 }); s.badge('g', 1, 0, 0.6, '2–4 осколка', 'ok');
    }, 8),
  ]);
  scene('portal_dock', [
    step('У портала в каждом мире стоит портальный док. Доки связаны в пару ключом.', (s) => {
      for (let x = 0; x < 4; x++) for (let y = 0; y < 3; y++) { s.put('n' + x + y, 'netherrack', x, y, -1); s.put('o' + x + y, 'grass', x + 6, y, -1); }
      s.put('pa', 'portal', 0, 0, 0); s.put('pb', 'portal', 6, 0, 0); s.put('da', 'pdock', 2, 0, 0); s.put('db', 'pdock', 8, 0, 0);
      s.badge('wa', 1, 2, -1.6, 'Незер', 'info'); s.badge('wb', 7, 2, -1.6, 'Обычный мир', 'info');
    }, 8),
    step('В Незере грузовая вагонетка сдаёт груз в док.', (s) => {
      for (let x = 0; x < 4; x++) s.put('r' + x, 'rail', x, 1, 0, { tier: 2, dir: 'x' });
      s.put('w', 'worker', 0, 1, 0, { tool: 'hold' }).move('w', [[0, 1, 0], [2, 1, 0]], 1.5, 'once'); s.flow('u', [[2, 1, 0], [2, 0, 0]]);
    }, 7),
    step('Груз выходит из парного дока в обычном мире и идёт дальше по магистрали.', (s) => {
      s.beam('link', [2.5, 0.5, 0.9], [8.5, 0.5, 0.9], '#b06cf0');
      s.put('p0', 'pipe', 8, 1, 0, { ch: 'i---' }); s.put('p1', 'pipe', 9, 1, 0, { ch: 'i---' }); s.put('c', 'chest', 9, 0, 0); s.put('nd', 'node', 9, 2, 0);
      s.flow('o', [[8, 0, 0], [8, 1, 0], [9, 1, 0], [9, 0, 0]]);
    }, 8),
    step('Если другой мир сейчас выгружен, док копит груз в буфере и передаст его, когда мир загрузится.', (s) => {
      s.flow('o', null); s.beam('link', null); s.badge('b', 2, 0, 0, 'буфер: 12 из 27', 'warn');
    }, 8),
  ]);
  // The rope runs along x at y 1.5, from the anchor on the ledge down towards the viewer to the one
  // on the ground. The handle carries the rope through its rollers (model point y .5, z .8125) and
  // its grip under them (z .05 to .16, across the rope). The player hangs by the raised right hand
  // (arms: 'hang'): the fist, 1.69 blocks over the feet and 0.34 to the y+ side of the figure when
  // it faces x+, closes round the middle of the grip.
  const ROPE = [[-1.5, 1.5, 5.95], [8.5, 1.5, 0.95]];
  const handleZ = (x) => ROPE[1][2] + ((x + 0.5 - ROPE[1][0]) * (ROPE[0][2] - ROPE[1][2])) / (ROPE[0][0] - ROPE[1][0]) - 0.8125;
  const HANG = 1.59, BESIDE = 1 - 0.34, RIDE = [0, 4.25];
  const ZIP_RIDE = 'Ты едешь по тросу до другого конца. По тросу ездят в обе стороны и другие игроки, он держится 5 минут.';
  scene('zipline', [
    step('Возьми зиплайн в руку и прицелься в блок. Виден контур якоря: так ты заранее знаешь, куда встанет трос.', (s) => {
      floor(s, -2, 1, 11, 2, 'grass');
      for (let x = -2; x < 0; x++) for (let y = 1; y < 3; y++) for (let z = 0; z < 5; z++) s.put('c' + x + y + z, 'stone', x, y, z);
      s.put('pl', 'player', -1, 2, 5, { face: 'x+' }); s.put('zb', 'zipanchor', -2, 1, 5, { alpha: 0.4 }); s.put('za', 'zipanchor', 8, 1, 0, { alpha: 0.4 });
    }, 8),
    step('Правый клик: якорь встаёт на блок, и от тебя до него натягивается трос. Потрачен один заряд из шестнадцати.', (s) => {
      s.set('zb', { alpha: 1 }).set('za', { alpha: 1 }); s.beam('rope', ROPE[0], ROPE[1], '#e6e8ee'); s.badge('z', -1, 2, 5.3, 'зарядов: 15 из 16', 'info');
    }, 8),
    step('Кликни по тросу. Ты держишься одной рукой за ручку, её ролики стоят на тросе.', (s) => {
      s.badges = []; s.put('zh', 'zip_handle', RIDE[0], 1, handleZ(RIDE[0]), { face: 'x+' });
      s.set('pl', { arms: 'hang' }).move('pl', [[-1, 2, 5], [RIDE[0], BESIDE, handleZ(RIDE[0]) - HANG]], 2, 'once');
    }, 7),
    step(ZIP_RIDE, (s) => {
      const [a, b] = RIDE, v = Math.hypot(a - b, handleZ(a) - handleZ(b)) / (hold(ZIP_RIDE, 9) - 1.5);
      s.move('zh', [[a, 1, handleZ(a)], [b, 1, handleZ(b)]], v, 'once');
      s.move('pl', [[a, BESIDE, handleZ(a) - HANG], [b, BESIDE, handleZ(b) - HANG]], v, 'once');
    }, 9),
  ]);
  scene('shard', S.quickstart.steps.slice(0, 1).concat([step('Из одной друзы выпадает от двух до четырёх осколков.', (s) => s.badge('a', 2, 1, -0.6, '2–4 осколка', 'ok'), 6)]));

  // ================================================================ blocks for the tree branches (spec 22)
  const MILK = '#f4f1e6', HONEY = '#f2a81d', WHEAT_C = '#d9c25a', IRON_C = '#d8d8d8', SAND_C = '#e3d39a', PLANK_C = '#b08a52', XP_C = '#9cff5a';
  const OWNER = (branch) => '+опыт ' + branch + ' · владельцу 50%';
  const keepPower = (s) => { s.flows = s.flows.filter((f) => f.id.startsWith('en')); };

  plugScene('repair', [
    step('Ремонтный стенд стоит на магистрали. В левом сундуке изношенная железная кирка, в соседнем железные слитки.', (s) => {
      bus(s, [['chest'], ['chest'], ['repair', { tier: 2 }], ['chest']]);
      s.badge('k', 0, 0, 0.6, 'кирка: прочность 20%', 'warn'); s.badge('i', 1, 1, -1.5, 'слитки ×4', 'info');
    }, 8),
    step('Кирка и слитки приходят по предметному каналу. Стенд сам берёт материал, из которого сделан предмет, здесь железо.', (s) => {
      s.set('b2', { on: true }); s.flow('t', r(0, 2), { color: '#9aa3b2', n: 1 }); s.flow('m', r(1, 2), { color: IRON_C }); power(s, 2);
      s.badge('k', 2, 0, 0, 'прочность 20% → 45%', 'info');
    }, 8),
    step('Один слиток возвращает 25% прочности, как на наковальне. Чары остаются, уровни опыта не нужны.', (s) => {
      s.flow('t', null); s.badge('k', 2, 0, 0, 'прочность 95% · чары на месте', 'info'); s.badge('i', 1, 1, -1.5, 'слитки ×1', 'info');
    }, 8),
    step('Починенная кирка уходит в правый сундук. Каждая починка даёт опыт кузнецу, владелец получает половину.', (s) => {
      s.flow('m', null); s.flow('o', r(2, 3), { color: '#9aa3b2', n: 1 }); s.badge('i', 0, 0, 0, null);
      s.badge('k', 3, 0, 0, 'кирка: прочность 100%', 'ok'); s.badge('x', 1, 1, -1.5, OWNER('кузнеца'), 'ok');
    }, 8),
    step('Пришёл изношенный меч, а слитков больше нет. Стенд показывает диагноз и ждёт материал.', (s) => {
      keepPower(s); s.set('b2', { on: false }); s.badges = []; s.badge('d', 2, 0, 0, 'Нет материала для починки', 'warn');
    }, 7),
  ]);

  plugScene('kitchen', [
    step('Кухонный комбайн готовит еду по страницам рецептов, как автокрафтер. На первой странице торт, сетку раскладывать не нужно.', (s) => {
      bus(s, [['tank', { fluid: 'milk', level: 0.8, ease: 3 }], ['chest'], ['kitchen', { tier: 2 }], ['chest']], { ch: 'ief-' });
      s.badge('p', 2, 0, 0, 'страница 1: торт', 'info');
    }, 8),
    step('Пшеница, сахар и яйца приходят по предметному каналу, молоко — из бака по жидкостному.', (s) => {
      s.set('b2', { on: true }); s.flow('i', r(1, 2), { color: WHEAT_C }); s.flow('f', r(0, 2), { color: MILK, n: 2 }); power(s, 2);
    }, 7),
    step('Где рецепт просит ведро молока, комбайн берёт из бака одно ведро, пустые вёдра не нужны. Готовый торт уходит в сундук.', (s) => {
      s.set('b0', { level: 0.5 }); s.flow('o', r(2, 3), { color: '#f3e1c8', n: 2 }); s.badge('m', 0, 0, 0, 'молоко: 3 ведра на торт', 'info');
    }, 9),
    step('На второй странице хлеб, страницы работают по очереди. Каждое блюдо даёт опыт повару, владелец получает половину.', (s) => {
      s.badge('m', 0, 0, 0, null); s.badge('p', 2, 0, 0, 'страница 2: хлеб', 'info'); s.flow('o2', r(2, 3), { color: '#c8873a', n: 2 });
      s.badge('x', 1, 1, -1.5, OWNER('повара'), 'ok');
    }, 9),
    step('Молоко в баке кончилось. Торт ждёт, а хлеб комбайн печёт дальше: на него всего хватает.', (s) => {
      s.set('b0', { level: 0 }); s.flow('f', null); s.flow('o', null); s.badges = []; s.badge('d', 2, 0, 0, 'Нет молока в баке', 'warn');
    }, 8),
  ]);

  // A pond nx × ny blocks of water two blocks deep, in a ring of grass bank at the same height.
  function pond(s, nx, ny) {
    for (let x = -1; x <= nx; x++) for (let y = -1; y <= ny; y++) {
      const wet = x >= 0 && y >= 0 && x < nx && y < ny;
      s.put('pb' + x + '_' + y, wet ? 'water' : 'grass', x, y, 0);
      s.put('pd' + x + '_' + y, wet ? 'water' : 'dirt', x, y, -1);
    }
  }
  const CATCH = '#7a9ab8';
  scene('fishery', [
    step('Рыболовная станция ставится на поверхность воды. Под ней нужна открытая вода: не меньше 5×5 блоков и глубже 2 блоков.', (s) => {
      pond(s, 5, 5); s.put('f', 'fishery', 2, 4, 1, { tier: 2 }); s.put('c', 'chest', 2, 5, 1);
      s.badge('w', 3, 2, -2.1, 'открытая вода 5×5', 'info');
    }, 8),
    step('В гнездо кладётся удочка, её чары работают. Раз в 30–60 секунд леска уходит в воду, и станция вытягивает улов.', (s) => {
      s.set('f', { on: true }); s.badge('w', 3, 2, -2.1, 'удочка: приманка II', 'info');
    }, 8),
    step('Улов идёт по той же таблице, что у удочки в руках: рыба, иногда мусор, изредка сокровище. Он уходит в сундук вплотную.', (s) => {
      s.flow('c', [[2, 4, 1], [2, 5, 1]], { color: CATCH, n: 1, speed: 0.5 }); s.badge('c', 3, 6, -1.2, 'треска · лосось · мусор', 'ok');
    }, 9),
    step('Удочка теряет единицу прочности за улов. Каждый улов даёт опыт рыбаку, владелец получает половину.', (s) => {
      s.badge('w', 3, 2, -2.1, 'удочка: 61 из 64', 'info'); s.badge('c', 3, 6, -1.2, OWNER('рыбака'), 'ok');
    }, 8),
    step('Здесь пруд 3×2: открытой воды мало, и станция не ловит. Каждой станции нужен свой участок 5×5.', (s) => {
      s.objs.clear(); s.flows = []; s.badges = []; pond(s, 3, 2); s.put('f', 'fishery', 1, 1, 1, { tier: 2 }); s.put('c', 'chest', 1, 2, 1);
      s.badge('d', 1, 1, 1, 'Мало открытой воды', 'warn');
    }, 8),
  ]);

  // A pen of grass; the feeder stands at (3, 1). The pair of cows meets head to head at y 3.
  const HERD = [[0, 0], [2, 0], [4, 0], [6, 0], [0, 1], [0, 2], [2, 2], [4, 2], [6, 2], [0, 3], [6, 3], [1, 4], [5, 4]];
  scene('feeder', [
    step('Кормушка стоит в загоне, пшеница лежит в её гнезде. Первых двух коров в загон приводит игрок.', (s) => {
      floor(s, -1, 0, 9, 6, 'grass'); s.put('fd', 'feeder', 3, 1, 0, { tier: 2 });
      s.put('c1', 'cow', 6, 5, 0).move('c1', [[6, 5, 0], [2, 5, 0], [2, 3, 0]], 1.2, 'once');
      s.put('c2', 'cow', 7, 5, 0).move('c2', [[7, 5, 0], [4, 5, 0], [4, 3, 0]], 1, 'once');
      s.put('pl', 'player', 7, 4, 0, { face: 'x-' }); s.badge('k', 3, 1, 0, 'корм: пшеница', 'info');
    }, 8),
    step('Кормушка находит пару взрослых коров одного вида и кормит обеих.', (s) => {
      s.set('c1', { path: null, x: 2, y: 3, face: 'x+' }).set('c2', { path: null, x: 4, y: 3, face: 'x-' }).set('fd', { on: true });
      s.flow('a', [[3, 1, 0], [2, 2, 0], [2, 3, 0]], { color: WHEAT_C, n: 1 }); s.flow('b', [[3, 1, 0], [4, 2, 0], [4, 3, 0]], { color: WHEAT_C, n: 1 });
    }, 7),
    step('У пары появляется телёнок. Каждое разведение даёт опыт животноводу, владелец получает половину.', (s) => {
      s.flows = []; s.put('cb', 'cow', 3, 4, 0, { baby: true, face: 'y+' });
      s.badge('k', 3, 1, 0, 'коров: 3 из 16', 'info'); s.badge('x', 4, 6, -1.6, OWNER('животновода'), 'ok');
    }, 8),
    step('У обеих коров теперь перерыв 5 минут, как в игре. Телёнка кормушка подкармливает, и он растёт быстрее.', (s) => {
      s.badge('x', 4, 6, -1.6, 'перерыв у пары: 5 минут', 'info'); s.flow('c', [[3, 1, 0], [3, 2, 0], [3, 4, 0]], { color: WHEAT_C, n: 1 });
    }, 8),
    step('Когда коров в радиусе 16, кормушка их больше не разводит. Лишних уводят в другой загон.', (s) => {
      s.flows = []; s.del('pl'); s.set('fd', { on: false });
      HERD.forEach(([x, y], i) => s.put('h' + i, 'cow', x, y, 0, { face: i % 2 ? 'x-' : 'x+', baby: i === 11 }));
      s.badges = []; s.badge('k', 3, 1, 0, 'коров: 16 из 16', 'info'); s.badge('d', 4, 6, -1.6, 'Предел животных', 'warn');
    }, 8),
  ]);

  // The shearer on its line; the sheep and the cow stand in front of the line, in its radius.
  plugScene('shearer', [
    step('Стригаль-доильщик стоит у загона с овцой и коровой. Ножницы в нём встроены, класть их не нужно.', (s) => {
      bus(s, [['chest'], ['shearer', { tier: 2, level: 0.2 }], ['tank', { fluid: 'milk', level: 0.2, ease: 3 }]], { ch: 'ief-' });
      floor(s, -1, 2, 6, 2, 'grass'); s.put('sh', 'sheep', 0, 3, 0, { face: 'x+' }); s.put('cw', 'cow', 3, 3, 0, { face: 'x-' });
    }, 8),
    step('Шерсть у овцы отросла, и машина её стрижёт. С овцы выпадает 1–3 шерсти, они уходят в магистраль.', (s) => {
      s.set('b1', { on: true }).set('sh', { wool: false }); power(s, 1);
      s.flow('w', [[0, 3, 0], [1, 2, 0], [1, 1, 0], [0, 1, 0], [0, 0, 0]], { color: '#ededed', n: 2 }); s.badge('w', 0, 0, 0, 'шерсть ×2', 'ok');
    }, 8),
    step('Корову машина доит в бак жидкостным каналом: одно ведро молока с коровы раз в 5 минут.', (s) => {
      s.flow('w', null); s.set('b1', { level: 0.5 }).set('b2', { level: 0.6 });
      s.flow('m', [[3, 3, 0], [2, 2, 0], [1, 2, 0], [1, 1, 0], [2, 1, 0], [2, 0, 0]], { color: MILK, n: 2 }); s.badge('c', 3, 3, -0.6, 'следующее доение через 5 минут', 'info');
    }, 8),
    step('Каждая стрижка и каждое доение дают опыт животноводу. Владелец получает половину.', (s) => {
      s.badge('w', 0, 0, 0, null); s.badge('x', 1, 0, 0.6, OWNER('животновода'), 'ok');
    }, 7),
    step('Бака нет: молоко девать некуда, и доение стоит.', (s) => {
      s.flow('m', null); s.del('b2'); s.set('b1', { level: 1 }); s.badges = []; s.badge('d', 1, 0, 0, 'Некуда сливать молоко', 'warn');
    }, 7),
  ]);

  // Two beehives and torchflowers in front of the line; bees fly from the flowers to the hives.
  const BEES = [
    [[4, 2, 0.5], [2.6, 3.6, 1.1], [0, 3.8, 0.2], [1.2, 3.4, 0.9], [4, 2, 0.5]],
    [[4, 3, 0.4], [3, 3.9, 0.8], [2, 3.8, 0.2], [3.2, 3.2, 1], [4, 3, 0.4]],
    [[4.2, 2.4, 0.6], [3.6, 2.2, 1.2], [4.4, 3.2, 1], [4.2, 2.4, 0.6]],
  ];
  plugScene('apiary', [
    step('Пасечный сборщик ставится не дальше 4 блоков от ульев. Пчёлы носят пыльцу в ульи и копят мёд.', (s) => {
      bus(s, [['chest'], ['apiary', { tier: 2 }], ['chest']]); floor(s, -1, 2, 6, 2, 'grass');
      s.put('hv1', 'beehive', 0, 3, 0); s.put('hv2', 'beehive', 2, 3, 0); s.put('tf1', 'torchflower', 4, 2, 0); s.put('tf2', 'torchflower', 4, 3, 0);
      BEES.forEach((pts, i) => s.put('be' + i, 'bee', pts[0][0], pts[0][1], pts[0][2]).move('be' + i, pts, 1 + 0.2 * i, 'loop'));
      name(s, 'hv1', 'Улей', 1.2);
    }, 8),
    step('Улей полон, и сборщик забирает мёд. Пустые бутылки для него приходят по предметному каналу.', (s) => {
      s.set('b1', { on: true }).set('hv1', { full: false }); power(s, 1);
      s.flow('g', r(0, 1), { color: '#cfe7ef', n: 2 }); s.flow('h', [[0, 3, 0], [1, 2, 0], [1, 1, 0], [1, 0, 0]], { color: HONEY, n: 2 });
    }, 8),
    step('Бутылочки мёда уходят в сундук. Пчёлы не злятся, костёр под ульем не нужен. Каждый сбор даёт опыт животноводу.', (s) => {
      s.flow('h', null); s.flow('o', r(1, 2), { color: HONEY, n: 2 }); s.badge('x', 1, 0, 0.6, OWNER('животновода'), 'ok');
    }, 9),
    step('Пустые бутылки кончились, и мёд разливать не во что.', (s) => {
      keepPower(s); s.set('b1', { on: false }); s.badges = []; s.badge('d', 1, 0, 0, 'Нет пустых бутылок', 'warn');
    }, 7),
    step('В окне можно выбрать соты: бутылки для них не нужны. С полного улья выходят 3 сота.', (s) => {
      s.set('b1', { on: true }).set('hv2', { full: false }); s.badges = []; s.badge('s', 1, 0, 0, 'сбор: соты, 3 с улья', 'info');
      s.flow('h', [[2, 3, 0], [1, 2, 0], [1, 1, 0], [1, 0, 0]], { color: '#e8a020', n: 2 }); s.flow('o', r(1, 2), { color: '#e8a020', n: 3 });
    }, 8),
  ]);

  // The brewer on its line; in front of the line nether wart grows on soul sand by a monorail.
  plugScene('brewer', [
    step('Варочный куб варит зелья по страницам, на каждой странице одно зелье. Здесь выбрано зелье стремительности.', (s) => {
      bus(s, [['chest'], ['chest'], ['brewer', { tier: 3 }], ['chest']]);
      s.badge('p', 2, 0, 0, 'страница: стремительность', 'info');
    }, 8),
    step('Бутылки воды, адский нарост и сахар приходят по предметному каналу. Огненный порошок — топливо, одной порции хватает на 20 варок.', (s) => {
      s.set('b2', { on: true }); s.flow('a', r(0, 2), { color: '#5a8af0', n: 2 }); s.flow('b', r(1, 2), { color: '#a83236', n: 2 }); power(s, 2);
      s.badge('p', 2, 0, 0, 'огненный порошок: на 20 варок', 'info');
    }, 9),
    step('Куб сам проходит все шаги: сначала нарост, потом сахар. Шаг идёт 20 секунд, за варку выходят 3 бутылки.', (s) => {
      s.flow('o', r(2, 3), { color: '#7cc8e8', n: 3 }); s.badge('p', 2, 0, 0, 'варка: 3 бутылки', 'ok');
    }, 9),
    step('Каждая варка даёт опыт алхимику, владелец получает половину. Адский нарост для зелий собирает вагонетка с жаткой.', (s) => {
      s.badge('p', 2, 0, 0, OWNER('алхимика'), 'ok');
      for (let x = -1; x < 5; x++) { s.put('ss' + x, 'soul_sand', x, 2, -1); s.put('nw' + x, 'nether_wart', x, 2, 0, { age: x % 2 ? 3 : 1 }); s.put('fr' + x, 'stone', x, 3, -1); s.put('rr' + x, 'rail', x, 3, 0, { tier: 2, dir: 'x' }); }
      s.put('w', 'worker', -1, 3, 0, { tool: 'reaper', busy: true }).move('w', [[-1, 3, 0], [4, 3, 0]], 1, 'ping');
    }, 9),
    step('Огненный порошок кончился: куб ждёт топливо.', (s) => {
      for (const id of ['a', 'b', 'o']) s.flow(id, null);
      s.set('b2', { on: false }); s.badges = []; s.badge('d', 2, 0, 0, 'Нет огненного порошка', 'warn');
    }, 7),
  ]);

  // Steve stands to the left of the condenser, the second player to its right; a crystal behind
  // the condenser feeds it.
  scene('xp_tank', [
    step('Конденсатор опыта хранит уровни. Подойди к нему и открой его окно.', (s) => {
      floor(s, 0, -2, 5, 4); s.put('t', 'xp_tank', 2, 0, 0, { tier: 3, level: 0.05, ease: 2 }); s.put('k', 'crystal', 2, -1, 0);
      s.put('pl', 'player', 1, 1, 0, { face: 'x+' }); s.badge('p', 1, 1, -0.2, 'уровень 30', 'info');
    }, 7),
    step('Сдай уровни: по одному, по десять или все сразу. Опыт хранится в очках, поэтому возвращается без потерь.', (s) => {
      s.set('t', { on: true, level: 0.6 }); s.flow('x', [[1, 1, 0.3], [2, 0, 0.3]], { color: XP_C, n: 3 });
      s.badge('p', 1, 1, -0.2, 'уровень 0', 'info'); s.badge('t', 2, 0, 0.6, 'хранит 1395 очков', 'ok');
    }, 9),
    step('Конденсатор общий: уровни, которые сдал один игрок, может забрать другой.', (s) => {
      s.flow('x', null); s.put('p2', 'player', 3, -1, 0, { face: 'y+', armor: true }); s.flow('y', [[2, 0, 0.3], [3, -1, 0.3]], { color: XP_C, n: 3 });
      s.set('t', { level: 0.55 }); s.badge('q', 3, -1, 0.3, 'уровень 0 → 10', 'info'); s.badge('t', 2, 0, 0.6, 'осталось 1235 очков', 'info');
    }, 8),
    step('Зачаровательный автомат вплотную берёт уровни отсюда. Сам конденсатор опыта дерева не даёт.', (s) => {
      s.flow('y', null); s.del('p2'); s.badge('q', 0, 0, 0, null);
      s.put('e', 'enchanter', 3, 0, 0, { tier: 4, on: true }); s.put('k2', 'crystal', 4, 0, 0); s.flow('z', [[2, 0, 0.3], [3, 0, 0.3]], { color: XP_C, n: 2 });
      s.set('t', { level: 0.45 }); s.badge('t', 2, 0, 0.6, 'опыт дерева: нет', 'info');
    }, 8),
  ]);

  plugScene('enchanter', [
    step('Зачаровательный автомат стоит вплотную к конденсатору опыта. Книги и лазурит приходят по предметному каналу.', (s) => {
      bus(s, [['chest'], ['xp_tank', { tier: 3, on: true, level: 0.6, ease: 2 }], ['enchanter', { tier: 4 }], ['chest']]);
      s.badge('c', 0, 0, 0.6, 'книги ×8 · лазурит ×15', 'info'); s.badge('t', 1, 1, -1.5, 'уровней: 40', 'info');
    }, 8),
    step('В окне выбран уровень чар 30. Книжные полки не нужны: автомат работает как стол с 15 полками.', (s) => {
      s.set('b2', { on: true }); s.flow('k', r(0, 2), { color: '#8a5a3a', n: 2 }); s.flow('l', r(0, 2), { color: '#3558d6', n: 2, speed: 1.2 });
      s.flow('x', [[1, 0, 0.4], [2, 0, 0.4]], { color: XP_C, n: 2 }); power(s, 2); s.badge('e', 2, 0, 0, 'уровень чар: 30', 'info');
    }, 8),
    step('Чары уровня 21–30 стоят 3 уровня и 3 лазурита. Какие чары выпадут, решает случай, как у стола.', (s) => {
      s.flow('o', r(2, 3), { color: '#c58aff', n: 2 }); s.set('b1', { level: 0.55 });
      s.badge('t', 1, 1, -1.5, 'уровней: 40 → 37', 'info'); s.badge('c', 0, 0, 0.6, 'лазурит: 15 → 12', 'info');
    }, 8),
    step('Автомат зачаровывает каждую книгу, пока хватает уровней и лазурита. Каждое зачарование даёт опыт зачарователю.', (s) => {
      s.set('b1', { level: 0.48 }); s.badge('t', 1, 1, -1.5, 'уровней: 31', 'info'); s.badge('c', 0, 0, 0.6, 'лазурит ×6', 'info');
      s.badge('e', 2, 0, 0, OWNER('зачарователя'), 'ok');
    }, 9),
    step('Осталось 28 уровней, меньше выбранного уровня чар 30. Автомат ждёт, пока в конденсатор сдадут уровни.', (s) => {
      keepPower(s); s.set('b2', { on: false }).set('b1', { level: 0.44 }); s.badges = [];
      s.badge('t', 1, 1, -1.5, 'уровней: 28', 'info'); s.badge('d', 2, 0, 0, 'Мало уровней в конденсаторе', 'warn');
    }, 9),
  ]);

  // A beach track: the rail at y 1 on sand, a strip to dig on each side (three blocks deep, over a
  // stone floor), the sea beyond the y 2 strip and an unloading dock at the start.
  const DIG = { 0: ['sand', 'sand', 'gravel'], 2: ['sand', 'clay', 'clay'] };
  function beach(s) {
    for (let x = -1; x < 9; x++) {
      for (let y = 0; y < 3; y++) s.put('bf' + x + '_' + y, 'stone', x, y, -4);
      for (const y of [3, 4]) { s.put('sw' + x + '_' + y, 'water', x, y, -1); s.put('sb' + x + '_' + y, 'sand', x, y, -2); }
      for (let z = -3; z < 0; z++) {
        s.put('br' + x + '_' + z, z === -1 ? 'sand' : 'stone', x, 1, z);
        for (const y of [0, 2]) s.put('dg' + x + '_' + y + '_' + z, x < 1 ? 'stone' : DIG[y][-1 - z], x, y, z);
      }
      if (x >= 0) s.put('rt' + x, 'rail', x, 1, 0, { tier: 2, dir: 'x' });
    }
    s.put('du', 'dock_unload', 0, 2, 0); s.put('dc', 'chest', -1, 2, 0);
  }
  // The strip on side y vanishes column by column as the scoop passes at speed v from x0 (the gone
  // prop of sand, gravel and clay in models.js).
  function digStrip(s, y, x0, v) {
    for (let x = 1; x < 9; x++) for (let z = -3; z < 0; z++) s.set('dg' + x + '_' + y + '_' + z, { gone: s.now + Math.abs(x - x0) / v + 0.3 - 0.15 * z });
  }
  const SCOOP = [
    'Копатель — вагонетка-работник с ковшом. Его путь идёт вдоль пляжа: рядом песок, глубже гравий и глина.',
    'Ковш поворачивается вбок и копает полосу вдоль пути: на 1 клетку вбок и до 3 блоков вглубь.',
    'Блок под рельсом копатель не трогает, поэтому путь не обваливается. Обратно он копает другую сторону.',
  ];
  scene('tool_scoop', [
    step(SCOOP[0], (s) => { beach(s); s.put('w', 'worker', 0, 1, 0, { tool: 'scoop' }); }, 7),
    step(SCOOP[1], (s) => {
      const v = 8 / (hold(SCOOP[1]) - 1);
      s.set('w', { busy: true, aim: 'y-', reach: 0.6 }).move('w', [[0, 1, 0], [8, 1, 0]], v, 'once'); digStrip(s, 0, 0, v);
    }),
    step(SCOOP[2], (s) => {
      const v = 8 / (hold(SCOOP[2]) - 1);
      s.set('w', { aim: 'y+' }).move('w', [[8, 1, 0], [0, 1, 0]], v, 'once'); digStrip(s, 2, 8, v);
      s.badge('r', 4, 1, -2, 'блок под рельсом цел', 'info');
    }),
    step('Выкопанное лежит в трюме, копатель сгружает его на разгрузке. Каждый блок засчитывается как копание лопатой.', (s) => {
      s.set('w', { busy: false, aim: null, reach: 0 }); s.flow('u', [[0, 1, 0], [0, 2, 0], [-1, 2, 0]], { color: SAND_C, n: 3 });
      s.badge('r', 4, 1, -2, OWNER('копателя'), 'ok');
    }, 8),
    step('Вокруг пути всё выкопано, и копатель показывает диагноз.', (s) => {
      s.flow('u', null); s.badges = []; s.badge('d', 0, 1, 0, 'Рядом с путём нечего копать', 'warn');
    }, 6),
  ]);

  plugScene('archeo', [
    step('Игрок приносит с раскопок подозрительный песок и гравий. Кирка и лопата такие блоки разрушают.', (s) => {
      bus(s, [['chest'], ['archeo', { tier: 3 }], ['chest']]); floor(s, -1, 2, 5, 2, 'sand');
      s.put('ss', 'sus_sand', 0, 3, 0); s.put('sg', 'sus_gravel', 1, 2, 0); s.put('pl', 'player', 3, 3, 0, { face: 'x-' });
      name(s, 'ss', 'Подозрительный песок', 1.3); name(s, 'sg', 'Подозрительный гравий', 1.3);
    }, 8),
    step('Блоки лежат в сундуке, в гнезде стола кисточка. Блоки приходят по предметному каналу, стол расчищает каждый за 10 секунд.', (s) => {
      s.del('ss', 'sg'); s.set('b1', { on: true }); s.flow('i', r(0, 1), { color: SAND_C, n: 2 }); power(s, 1);
      s.badge('b', 1, 0, 0, 'кисточка: −1 прочности за блок', 'info');
    }, 9),
    step('Находка выпадает по таблице того места, где лежал блок: черепки, яйцо нюхача, кузнечные шаблоны. От блока остаётся обычный песок.', (s) => {
      s.flow('o1', r(1, 2), { color: '#b0603a', n: 1 }); s.flow('o2', r(1, 2), { color: SAND_C, n: 2, speed: 1.2 }); s.badge('c', 2, 0, -0.6, 'черепок · песок', 'ok');
    }, 10),
    step('Каждый расчищенный блок даёт опыт копателю. Владелец получает половину, пока он на сервере.', (s) => {
      s.badge('b', 1, 0, 0.5, OWNER('копателя'), 'ok');
    }, 7),
    step('Кисточки в гнезде нет: стол ждёт новую.', (s) => {
      keepPower(s); s.set('b1', { on: false }); s.badges = []; s.badge('d', 1, 0, 0, 'Нет кисточки', 'warn');
    }, 6),
  ]);

  // The building both projector scenes use: 3 × 2 cells, two blocks high, at x 3..5, y 0..1 (or
  // shifted by dx, dy); the cell list goes from the bottom up.
  const HUT = [];
  for (let z = 0; z < 2; z++) for (let y = 0; y < 2; y++) for (let x = 0; x < 3; x++) HUT.push([x, y, z]);
  const ghost = (s, dx, dy) => HUT.forEach(([x, y, z], i) => s.put('gh' + i, 'ghost', x + dx, y + dy, z));
  scene('projector', [
    step('Задай ключом два угла области, не больше 32×32×32, и нажми «Запомнить». Проектор запомнил постройку.', (s) => {
      floor(s, -1, -1, 8, 5); s.put('pj', 'projector', 1, 1, 0, { tier: 3 }); s.put('k', 'crystal', 1, 0, 0);
      HUT.forEach(([x, y, z], i) => s.put('hb' + i, 'planks', x + 3, y, z));
      s.badge('a', 2.5, -0.5, -0.15, 'угол 1', 'info'); s.badge('b', 5.5, 1.5, -2.15, 'угол 2', 'info');
    }, 8),
    step('Сними проектор ключом и поставь в другом месте: постройка остаётся в нём. Рядом появился призрак её блоков.', (s) => {
      s.objs.clear(); s.badges = []; floor(s, -2, -1, 9, 5, 'grass');
      s.put('pj', 'projector', 1, 1, 0, { tier: 3, on: true }); s.put('k', 'crystal', 1, 0, 0); ghost(s, 3, 0);
    }, 8),
    step('Окно показывает, каких блоков не хватает. Проектор стоит на магистрали склада, и кнопка «В руку» кладёт тебе стак досок.', (s) => {
      s.put('tm', 'terminal', -1, 1, 0); s.put('ce', 'cell', -2, 1, 0); s.put('ce2', 'cell', -1, 0, 0); s.put('pp', 'pipe', 0, 1, 0, { ch: 'i---' }); s.put('nd', 'node', 0, 0, 0);
      s.put('pl', 'player', 2, 3, 0, { face: 'x-' }); s.flow('h', [[-1, 1, 0], [1, 1, 0], [1, 3, 0], [2, 3, 0]], { color: PLANK_C, n: 2 });
      s.badge('n', 1, 1, 0.8, 'не хватает: доски ×12', 'info'); s.badge('p', 2, 3, -0.2, 'в руку: доски ×64', 'ok');
    }, 9),
    step('Ты ставишь блоки, и верно поставленный блок в призраке гаснет. Опыт строителя за каждый блок получаешь ты.', (s) => {
      s.flow('h', null); s.badges = []; s.set('pl', { face: null }).move('pl', [[2, 3, 0], [4, 3, 0], [4, 2, 0]], 1.5, 'once');
      HUT.slice(0, 6).forEach(([x, y, z], i) => { if (i === 5) return; s.del('gh' + i); s.put('hb' + i, 'planks', x + 3, y, z); });
      s.put('wrong', 'stone', 5, 1, 0); s.set('gh5', { bad: true }); s.badge('p', 4, 2, 0, '+опыт строителя за свои блоки', 'ok');
    }, 9),
    step('Клетку с чужим блоком призрак подсвечивает красным. Сам проектор опыта не даёт.', (s) => {
      s.badges = []; s.badge('w', 5, 1, -0.6, 'чужой блок', 'warn');
    }, 7),
  ]);

  // The builder between the storage line and the projector; the building rises behind the
  // projector, to its right.
  // Each block flies from the builder's head to its cell, one after another.
  function buildCells(s, from, to, delay) {
    for (let i = from; i < to; i++) {
      const [x, y, z] = HUT[i];
      s.put('hb' + i, 'planks', 1, 0, 1.1); later(s, 'hb' + i, [[1, 0, 1.1], [x + 3, y - 2, z]], 3, delay + (i - from) * 0.8);
    }
  }
  plugScene('builder', [
    step('Строительный автомат ставится вплотную к голопроектору с постройкой и подключается к магистрали склада.', (s) => {
      floor(s, -3, -3, 10, 5); s.put('ce', 'cell', -2, 0, 0); s.put('ce2', 'cell', -2, -1, 0); s.put('tm', 'terminal', -1, 0, 0);
      s.put('pp', 'pipe', 0, 0, 0, { ch: 'ie--' }); s.put('nd', 'node', 0, -1, 0); s.put('k', 'crystal', 0, 0, 1);
      s.put('bd', 'builder', 1, 0, 0, { tier: 3 }); s.put('pj', 'projector', 2, 0, 0, { tier: 3, on: true }); ghost(s, 3, -2);
    }, 8),
    step('Автомат ставит блоки по призраку снизу вверх, слой за слоем. Блоки он берёт со склада.', (s) => {
      s.set('bd', { on: true }); s.flow('b', [[-1, 0, 0], [1, 0, 0]], { color: PLANK_C, n: 2 }); buildCells(s, 0, 6, 0.5);
      s.badge('l', 1, 0, 0, 'слой 1 из 2', 'info');
    }, 8),
    step('Опыт строителя автомат даёт за каждую клетку постройки один раз: снести и поставить снова — опыта нет.', (s) => {
      buildCells(s, 6, 9, 0.3); s.badge('l', 1, 0, 0, 'слой 2 из 2', 'info'); s.badge('x', 4, 0, -1.2, OWNER('строителя'), 'ok');
    }, 9),
    step('Доски на складе кончились. Стройка ждёт, а окно автомата пишет, чего и сколько не хватает.', (s) => {
      s.flow('b', null); s.set('bd', { on: false }); s.badges = [];
      s.badge('d', 1, 0, 0, 'Нет блоков на складе', 'warn'); s.badge('n', -1, 0, 0.6, 'не хватает: доски ×3', 'info');
    }, 8),
    step('Доски снова появились на складе, и стройка продолжилась сама.', (s) => {
      s.set('bd', { on: true }); s.flow('b', [[-1, 0, 0], [1, 0, 0]], { color: PLANK_C, n: 2 }); buildCells(s, 9, 12, 0.3);
      s.badges = []; s.badge('d', 4, -1, 0.3, 'постройка готова', 'ok');
    }, 8),
  ]);

  // Ground in chunks of 3 × 3 blocks: plains on the left, a desert with a village on the right; the
  // monorail runs along y 4, through the middle row of chunks.
  function mapWorld(s) {
    for (let x = 0; x < 12; x++) for (let y = 0; y < 9; y++) {
      const odd = (((x / 3) | 0) + ((y / 3) | 0)) % 2;
      s.put('mg' + x + '_' + y, x < 6 ? (odd ? 'dirt' : 'grass') : odd ? 'sandstone' : 'sand', x, y, -1);
    }
    for (let x = 0; x < 12; x++) s.put('mr' + x, 'rail', x, 4, 0, { tier: 3, dir: 'x' });
    for (const [x, y] of [[8, 0], [9, 0], [8, 1], [9, 1], [10, 7]]) { s.put('vh' + x + y, 'sandstone', x, y, 0); s.put('vr' + x + y, 'planks', x, y, 1); }
  }
  scene('tool_map', [
    step('Картограф — вагонетка-работник с картографом. Он ездит по своему участку монорельса, как любой работник.', (s) => {
      mapWorld(s); s.put('w', 'worker', 0, 4, 0, { tool: 'map' }); s.put('st', 'station', 0, 5, 0);
    }, 7),
    step('Он наносит на схему станций чанки, через которые идёт путь, и по одному чанку с каждой стороны. Чанк в сцене — 3×3 блока.', (s) => {
      s.set('w', { busy: true }).move('w', [[0, 4, 0], [11, 4, 0]], 1.4, 'ping'); s.badge('n', 0, 5, 0, 'на схеме: 12 чанков', 'info');
    }, 9),
    step('Найдены новый биом и постройка: пустыня и деревня. Они появляются на схеме в окне любой станции сети.', (s) => {
      s.badge('v', 8, 0, 0, 'деревня', 'ok'); s.badge('b', 8, 6, -2, 'пустыня', 'ok'); s.badge('n', 0, 5, 0, '12 чанков · 2 биома · 1 постройка', 'info');
    }, 9),
    step('Опыт исследователя — за каждое место, где ни владелец, ни его картограф ещё не были. Каждое место один раз.', (s) => {
      s.badges = []; s.badge('n', 0, 5, 0, OWNER('исследователя'), 'ok');
    }, 8),
    step('Вокруг пути всё нанесено, и езда по тому же кругу опыта больше не даёт.', (s) => {
      s.set('w', { busy: false }); s.badge('n', 0, 5, 0, 'Вокруг пути всё нанесено', 'warn');
    }, 7),
  ]);

  // Steve holds the compass out in front; the dashed line runs from it to the place it points at.
  // Facing x+, his right fist is 0.56 block ahead of his cell centre, 0.34 to the y+ side, at 1.37.
  const COMPASS_ARMS = [1, 0];
  const pointer = (s, x, y, to) => s.beam('c', to && [x + 1.06, y + 0.84, 1.4], to, '#69d2d8');
  const scout = (s, x, y) => { s.put('pl', 'player', x, y, 0, { face: 'x+', arms: COMPASS_ARMS }); s.put('cp', 'scout_compass', x, y, 0, { face: 'x+', arms: COMPASS_ARMS }); };
  scene('scout_compass', [
    step('Держи компас в руке: стрелка показывает на ближайшую постройку со схемы станций, где ты ещё не был.', (s) => {
      mapWorld(s); scout(s, 1, 6); pointer(s, 1, 6, [9, 1, 1]); s.badge('h', 1, 6, 0.6, 'деревня · 140 блоков', 'info');
    }, 8),
    step('Правый клик переключает цель: постройки или биомы. Теперь стрелка ведёт к пустыне.', (s) => {
      pointer(s, 1, 6, [7, 6.5, 0]); s.badge('h', 1, 6, 0.6, 'пустыня · 90 блоков', 'info');
    }, 7),
    step('Ты дошёл до пустыни. Опыт исследователя за новое место получаешь ты сам, компас опыта не даёт.', (s) => {
      pointer(s, 0, 0, null); s.badges = [];
      for (const id of ['pl', 'cp']) s.move(id, [[1, 6, 0], [7, 6, 0]], 1.5, 'once');
      s.badge('x', 7, 6, 0.6, '+опыт исследователя', 'ok');
    }, 8),
    step('Когда ты дошёл до места, компас указывает на следующее: на деревню.', (s) => {
      for (const id of ['pl', 'cp']) s.set(id, { path: null, x: 7, y: 6 });
      pointer(s, 7, 6, [9, 1, 1]); s.badges = []; s.badge('h', 7, 6, 0.6, 'деревня · 50 блоков', 'info');
    }, 7),
    step('Компас знает только то, что картографы нанесли на схему. Все места пройдены, и новых нет.', (s) => {
      pointer(s, 0, 0, null); s.badges = [];
      for (const id of ['pl', 'cp']) s.move(id, [[7, 6, 0], [7, 3, 0], [9, 3, 0]], 1.5, 'once');
      s.badge('d', 9, 3, 0.6, 'Нет новых мест на схеме', 'warn');
    }, 8),
  ]);

  // ================================================================ butcher (spec 23)
  // The butcher stands in the pen in front of the line and touches its housing; the condenser is
  // beside it, the furnace and the chest on the line. The surplus cow stands next to the butcher.
  const BEEF = '#c0504a';
  plugScene('butcher', [
    step('Забойщик стоит в загоне рядом с кормушкой, в его гнезде меч. Вплотную к нему конденсатор опыта. В окне задано: оставить 2 взрослых.', (s) => {
      bus(s, [null, null, ['furnace'], ['chest']]); floor(s, -1, 2, 7, 3, 'grass');
      s.put('bt', 'butcher', 1, 2, 0, { tier: 2 }); s.put('xt', 'xp_tank', 0, 2, 0, { tier: 3, level: 0.2, ease: 2 });
      s.put('fd', 'feeder', 5, 3, 0, { tier: 2, on: true }); s.put('c1', 'cow', 2, 4, 0, { face: 'x+' }); s.put('c2', 'cow', 4, 4, 0, { face: 'x-' });
      s.badge('k', 1, 2, 0.6, 'оставить: 2 взрослых', 'info');
    }, 9),
    step('Кормушка разводит коров, телята подрастают. Взрослых коров стало три, одна лишняя. Телят забойщик не трогает.', (s) => {
      s.put('c3', 'cow', 2, 2, 0, { face: 'x+' }); s.put('cb', 'cow', 0, 4, 0, { baby: true, face: 'x+' });
      s.badge('k', 1, 2, 0.6, 'взрослых: 3 · оставить 2', 'warn');
    }, 8),
    step('Раз в 10 секунд забойщик бьёт одно лишнее взрослое животное. Говядина и кожа уходят в магистраль, опыт — в конденсатор вплотную.', (s) => {
      s.set('bt', { on: true }).set('c3', { gone: s.now + 1.6 }).set('xt', { on: true, level: 0.45 }).set('b2', { on: true }); power(s, 2);
      s.flow('m', [[2, 2, 0], [1, 2, 0], [1, 1, 0], [2, 1, 0], [2, 0, 0]], { color: BEEF, n: 2 }); s.flow('l', [[2, 2, 0], [1, 2, 0], [1, 1, 0], [3, 1, 0], [3, 0, 0]], { color: '#a0603a', n: 1 });
      s.flow('x', [[1, 2, 0.4], [0, 2, 0.4]], { color: XP_C, n: 2 }); s.badge('k', 1, 2, 0.6, 'взрослых: 2', 'info');
    }, 9),
    step('Электропечь жарит говядину, стейки уходят в сундук. Новых лишних нет, и забойщик ждёт.', (s) => {
      s.set('bt', { on: false }); s.flow('x', null); s.flow('o', r(2, 3), { color: '#8a4a2a', n: 2 });
      s.badge('k', 1, 2, 0.6, 'оставляет 2 взрослых', 'ok');
    }, 8),
  ]);

  // ================================================================ utilizer and disintegrator (spec 25)
  // The utilizer stands at the end of the line: a chest of cobblestone and seeds on the left, the
  // store (a terminal and three cells) in the middle, the utilizer on the right, one cell short of
  // the node, so the node does not hide it.
  const SEEDS = '#7fb84a', IRON = '#d8d8d8', STICK = '#a8793f', GEAR = '#c9ccd4', ENCHANTED = '#b08aff';
  const trashLine = (s) => {
    bus(s, [['chest'], ['terminal'], ['cell'], ['trash'], null]);
    s.put('c1', 'cell', 1, 0, 1); s.put('c2', 'cell', 2, 0, 1);
  };
  plugScene('trash', [
    step('Утилизатор стоит в конце линии. На складе лимит: 64 булыжника и 32 семени. На стыке утилизатора фильтр: булыжник и семена.', (s) => {
      trashLine(s); s.flow('c', r(0, 1), { color: COBBLE }); s.flow('sd', r(0, 2), { color: SEEDS, n: 2 });
      s.badge('l', 2, 0, 0, 'булыжник 40 / 64', 'info'); s.badge('f', 3, 0, -0.6, 'фильтр: булыжник, семена', 'info');
    }, 9),
    step('Склад дошёл до лимита. Теперь булыжник и семена не берёт никто, кроме утилизатора, и излишек сгорает. Каждый предмет стоит 1 Е.', (s) => {
      s.set('b3', { on: true }); s.flow('c', r(0, 3), { color: COBBLE }); s.flow('sd', r(0, 3), { color: SEEDS, n: 2 }); power(s, 3);
      s.badge('l', 2, 0, 0, 'булыжник 64 / 64 · лимит', 'warn'); s.badge('f', 3, 0, -0.6, 'излишек сгорает', 'ok');
    }, 9),
    step('Без фильтра утилизатор ничего не принимает, чтобы случайно не сжечь всю сеть. Излишек остаётся в сундуке.', (s) => {
      s.flows = []; s.set('b3', { on: false }); s.badge('f', 3, 0, -0.6, 'Фильтр не задан');
    }, 8),
    step('С жидкостями так же. Насос качает воду, бак уже полон, и лишняя вода сгорает в утилизаторе.', (s) => {
      s.objs.clear(); s.flows = []; s.badges = [];
      pumpPool(s, 4); pumpLine(s, 4); s.set('b0', { on: true });
      s.put('tw', 'tank', 2, 0, 0, { fluid: 'water', level: 1 }); s.put('b3', 'trash', 3, 0, 0, { on: true });
      s.flow('w', r(0, 3), { color: FL }); pumpPower(s, 0, 3);
      s.badge('t', 2, 0, 0, 'бак полон', 'info'); s.badge('f', 3, 0, -0.6, 'фильтр: вода · излишек сгорает', 'ok');
    }, 9),
  ]);

  // The disintegrator stands on the line next to the terminal of the store (a terminal and three
  // cells); in the last step a utilizer joins the line beside it.
  const disLine = (s) => {
    bus(s, [['cell'], ['terminal'], ['disintegrator', { tier: 3 }], null, null], { tier: 3 });
    s.put('c1', 'cell', 0, 0, 1); s.put('c2', 'cell', 1, 0, 1);
  };
  plugScene('disintegrator', [
    step('Склад отдаёт старые железные кирки в дезинтегратор. На кирку уходит 3 слитка, назад выходит половина. Лишняя половинка ждёт внутри, и две кирки дают 3 слитка.', (s) => {
      disLine(s); s.set('b2', { on: true }); power(s, 2);
      s.flow('in', r(1, 2), { color: GEAR, n: 2, speed: 1 }); s.flow('out', r(2, 1), { color: IRON, n: 3 });
      s.badge('y', 2, 0, 0, '2 кирки → 3 слитка + 2 палки', 'ok');
    }, 10),
    step('Изношенное даёт долю по прочности. Нагрудник с половиной прочности вернёт 2 слитка вместо 4. Чары при разборке теряются.', (s) => {
      s.flow('in', r(1, 2), { color: ENCHANTED, n: 1, speed: 1 }); s.flow('out', r(2, 1), { color: IRON, n: 2 });
      s.badge('y', 2, 0, 0, 'прочность 50% → 2 слитка из 4', 'warn'); s.badge('e', 2, 0, 0.5, 'чары теряются', 'warn');
    }, 9),
    step('Блок железа не разбирается: его и так раскладывает верстак. Доски тоже: из одного бревна их выходит сразу четыре.', (s) => {
      s.flows = []; s.badges = []; s.set('b2', { on: false }); s.badge('d', 2, 0, 0, 'Предмет не разбирается');
    }, 8),
    step('Вместе с утилизатором: слитки идут на склад, а палки сверх лимита склада сгорают в утилизаторе по его фильтру.', (s) => {
      s.badges = []; s.put('b3', 'trash', 3, 0, 0, { on: true }); s.set('b2', { on: true }); power(s, 2, 3);
      s.flow('in', r(1, 2), { color: GEAR, n: 2, speed: 1 }); s.flow('out', r(2, 1), { color: IRON, n: 3 }); s.flow('st', r(2, 3), { color: STICK, n: 2 });
      s.badge('f', 3, 0, -0.6, 'лишние палки сгорают', 'ok');
    }, 9),
  ]);
})();
