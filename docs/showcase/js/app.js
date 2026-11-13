'use strict';
// Page shell of the guide: sidebar, search, entry pages, recipes.
(function () {
  const $ = (sel, el) => (el || document).querySelector(sel);
  const el = (tag, cls, html) => {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (html != null) e.innerHTML = html;
    return e;
  };
  const byId = Object.fromEntries(RC.ENTRIES.map((e) => [e.id, e]));
  const ITEM_ENTRY = { a1: 'alloys', a2: 'alloys', a3: 'alloys', a4: 'alloys', dust: 'dusts', f_sol: 'fluids', f_fast: 'fluids', f_eco: 'fluids', f_rich: 'fluids' };
  const MACHINE_NAME = { alloy: 'Плавильня сплавов', crusher: 'Дробилка', mixer: 'Смеситель' };

  window.onerror = (msg, src, line) => {
    $('#err').textContent = 'Ошибка страницы: ' + msg + ' (' + String(src).split('/').pop() + ':' + line + ')';
  };

  function icon(itemId, size) {
    const it = RC.ITEMS[itemId];
    const wrap = el('span', 'icon');
    if (!it) return wrap;
    wrap.title = it.name;
    let c;
    if (it.icon[0] === 'block') c = RC.thumb(it.icon[1], it.icon[2], size || 32);
    else {
      c = document.createElement('canvas');
      c.width = c.height = 16;
      c.getContext('2d').drawImage(RC.icon(it.icon[0], it.icon[1]), 0, 0);
    }
    wrap.appendChild(c);
    return wrap;
  }
  const itemLink = (id) => {
    const target = byId[id] ? id : ITEM_ENTRY[id];
    const name = RC.ITEMS[id] ? RC.ITEMS[id].name : id;
    return target ? '<a href="#' + target + '">' + name + '</a>' : name;
  };

  // ---------------------------------------------------------------- sidebar
  function buildNav() {
    const nav = $('#nav');
    for (const [cat, title] of RC.CATS) {
      nav.appendChild(el('h3', null, title));
      const ul = el('ul');
      for (const e of RC.ENTRIES.filter((x) => x.cat === cat)) {
        const li = el('li');
        const a = el('a');
        a.href = '#' + e.id;
        a.dataset.id = e.id;
        a.dataset.find = (e.name + ' ' + e.sum).toLowerCase();
        a.dataset.sum = e.sum;
        a.appendChild(icon(e.id, 28));
        a.appendChild(el('span', 'nm', e.name));
        if (e.tier) a.appendChild(el('span', 'tier t' + e.tier, String(e.tier)));
        li.appendChild(a);
        ul.appendChild(li);
      }
      nav.appendChild(ul);
    }
  }
  // The palette (js/kit.js) also finds words of the glossary and opens the glossary on them.
  Kit.palette.add((q) => q && ({
    group: 'Словарь', weight: 0.5,
    items: RC.GLOSSARY.map(([term, text], i) => ({ label: term, sub: text, find: term + ' ' + text, run: () => openTerm(i) })),
  }));
  function openTerm(i) {
    const go = () => {
      const dt = document.getElementById('term-' + i);
      if (!dt) return;
      dt.scrollIntoView({ block: 'start' });
      dt.animate([{ color: 'var(--accent-hi)', textShadow: '0 0 12px var(--hot)' }, {}], { duration: 1600, easing: 'ease-out' });
    };
    if (location.hash === '#glossary') go();
    else {
      // The glossary page is built on hashchange; the term is looked up after that.
      window.addEventListener('hashchange', () => requestAnimationFrame(go), { once: true });
      location.hash = 'glossary';
    }
  }

  // ---------------------------------------------------------------- recipes
  function recipeEl(rec, entry) {
    const box = el('div', 'recipe gd-glow');
    if (rec.name) box.appendChild(el('div', 'rname', rec.name));
    const row = el('div', 'rrow');
    const used = new Set();
    if (rec.g) {
      const grid = el('div', 'grid');
      for (const ch of rec.g.replace(/\|/g, '')) {
        const cell = el('div', 'cell');
        const id = rec.k[ch];
        if (id) {
          cell.appendChild(icon(id, 32));
          used.add(id);
        }
        grid.appendChild(cell);
      }
      row.appendChild(grid);
      row.appendChild(el('div', 'arrow', 'верстак<br>→'));
    } else {
      const list = el('div', 'inlist');
      for (const [id, n] of rec.from) {
        const cell = el('div', 'cell');
        cell.appendChild(icon(id, 32));
        if (n > 1) cell.appendChild(el('b', null, '×' + n));
        list.appendChild(cell);
        used.add(id);
      }
      row.appendChild(list);
      row.appendChild(el('div', 'arrow', MACHINE_NAME[rec.in] + '<br>→'));
    }
    const out = el('div', 'cell out');
    out.appendChild(icon(rec.out || entry.id, 32));
    if (rec.n > 1) out.appendChild(el('b', null, '×' + rec.n));
    row.appendChild(out);
    box.appendChild(row);
    box.appendChild(el('div', 'legend', [...used].map(itemLink).join(' · ')));
    if (rec.note) box.appendChild(el('div', 'note', rec.note));
    return box;
  }

  // A few sparks rise behind the page head, as over a forge (ui.css; none with reduced motion).
  const SPARKS = 9;
  function sparks() {
    const box = el('span', 'sparks');
    box.setAttribute('aria-hidden', 'true');
    for (let i = 0; i < SPARKS; i++) {
      const s = el('i');
      s.style.cssText = '--x:' + (4 + Math.random() * 60).toFixed(1) + '%;--dx:' + (Math.random() * 60 - 30).toFixed(0) + 'px;--d:' + (5 + Math.random() * 5).toFixed(1) + 's;--w:' + (Math.random() * -9).toFixed(1) + 's';
      box.appendChild(s);
    }
    return box;
  }

  // ---------------------------------------------------------------- entry page
  let player = null;
  function tiersTable() {
    const t = RC.TIERS;
    let h = '<table class="tiers"><tr><th></th>' + [1, 2, 3, 4].map((i) => '<th class="t' + i + '">Ступень ' + i + '<br>' + t[i].name + '</th>').join('') + '</tr>';
    h += '<tr><td>Когда доступна</td>' + [1, 2, 3, 4].map((i) => '<td>' + t[i].when + '</td>').join('') + '</tr>';
    for (const row of RC.TIER_TABLE) h += '<tr><td>' + row[0] + '</td>' + row.slice(1).map((v) => '<td>' + v + '</td>').join('') + '</tr>';
    const open = (n) => RC.ENTRIES.filter((e) => e.tier === n).map((e) => '<a href="#' + e.id + '">' + e.name + '</a>').join(', ');
    h += '<tr><td>Что открывает</td>' + [1, 2, 3, 4].map((i) => '<td class="small">' + open(i) + '</td>').join('') + '</tr>';
    return h + '</table>';
  }

  // The address may name a step: #crusher/3 opens the crusher page on the third step of its scene.
  function show(address) {
    const [id, stepNo] = address.split('/');
    const e = byId[id] || byId.overview;
    if (player) player.stop();
    const main = $('#main');
    main.innerHTML = '';
    main.scrollTop = 0;
    window.scrollTo(0, 0);
    document.title = e.name + ' · RustCraft';
    for (const a of document.querySelectorAll('#nav a')) {
      a.classList.toggle('on', a.dataset.id === e.id);
      if (a.dataset.id === e.id) a.scrollIntoView({ block: 'nearest' });
    }

    const head = el('header', 'head');
    head.appendChild(icon(e.id, 48));
    const ht = el('div');
    const cat = RC.CATS.find(([c]) => c === e.cat);
    ht.appendChild(el('p', 'eyebrow', (cat ? cat[1] : '') + (e.tier ? ' <span class="tier wide t' + e.tier + '">Ступень ' + e.tier + ' · ' + RC.TIERS[e.tier].name + '</span>' : '')));
    ht.appendChild(el('h1', null, e.name));
    head.appendChild(ht);
    head.appendChild(sparks());
    main.appendChild(head);
    main.appendChild(el('p', 'sum', e.sum));

    const sceneDef = e.page === 'glossary' ? null : RC.SCENES[e.id];
    if (sceneDef) {
      const sc = el('section', 'scene');
      main.appendChild(sc);
      player = new RC.Player(sc, sceneDef);
      if (stepNo > 1) player.go(Math.min(sceneDef.steps.length, +stepNo) - 1);
    }

    const cols = el('div', 'cols');
    const text = el('div', 'text');
    for (const p of e.text || []) text.appendChild(el('p', null, p));
    cols.appendChild(text);
    if (e.stats) {
      const side = el('aside', 'gd-glow');
      side.appendChild(el('h2', null, 'Характеристики'));
      side.appendChild(el('table', 'stats', e.stats.map(([k, v]) => '<tr><td>' + k + '</td><td>' + v + '</td></tr>').join('')));
      cols.appendChild(side);
    }
    main.appendChild(cols);
    if (e.page === 'tiers') main.appendChild(el('div', 'wide', tiersTable()));
    if (e.page === 'glossary') main.appendChild(el('dl', 'glossary', RC.GLOSSARY.map(([term, text], i) => '<dt id="term-' + i + '">' + term + '</dt><dd>' + text + '</dd>').join('')));
    // Tables of an entry: { title, note, head: [...], rows: [[...], ...] }.
    for (const t of e.tables || []) {
      main.appendChild(el('h2', null, t.title));
      if (t.note) main.appendChild(el('p', 'tnote', t.note));
      const row = (cells, tag) => '<tr>' + cells.map((c) => '<' + tag + '>' + c + '</' + tag + '>').join('') + '</tr>';
      main.appendChild(el('div', 'wide', '<table class="ntable">' + row(t.head, 'th') + t.rows.map((r) => row(r, 'td')).join('') + '</table>'));
    }

    const windowMock = RC.renderUI(e.id);
    if (windowMock) {
      main.appendChild(el('h2', null, 'Окно блока: что ты видишь, когда открываешь его'));
      main.appendChild(windowMock);
    }
    if (e.recipes) {
      main.appendChild(el('h2', null, e.recipes.length > 1 ? 'Рецепты' : 'Рецепт'));
      const list = el('div', 'recipes');
      for (const rec of e.recipes) list.appendChild(recipeEl(rec, e));
      main.appendChild(list);
    }
    if (e.diag) {
      main.appendChild(el('h2', null, 'Что блок говорит, когда не работает, и что делать'));
      main.appendChild(
        el('table', 'diags', e.diag.map((d) => '<tr><td><span class="badge bad static">' + d + '</span></td><td>' + (RC.DIAG_FIX[d] || '') + '</td></tr>').join(''))
      );
    }
    if (e.see) {
      main.appendChild(el('h2', null, 'См. также'));
      const see = el('div', 'see');
      for (const sid of e.see.filter((s) => byId[s])) {
        const a = el('a', 'gd-glow');
        a.href = '#' + sid;
        a.appendChild(icon(sid, 28));
        a.appendChild(el('span', null, byId[sid].name));
        see.appendChild(a);
      }
      main.appendChild(see);
    }
  }

  // Icons and scenes are drawn from the Blender models, so the page waits for them; they are
  // parsed by the time this file runs, so the wait does not hold the first paint.
  RC.modelsReady()
    .then(() => {
      buildNav();
      const route = () => show(decodeURIComponent(location.hash.slice(1)) || 'overview');
      window.addEventListener('hashchange', route);
      route();
    })
    // Rethrown outside the promise, so the page's error line shows it.
    .catch((e) => setTimeout(() => { throw e; }));
})();
