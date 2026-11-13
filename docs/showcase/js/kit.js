'use strict';
// The guide kit: a command palette over the sidebar, toasts, bottom sheets for narrow screens,
// rolling numbers and a light that follows the pointer over cards. It only reads the page shell
// every mod's guide shares (.top, #search, #nav with h3 + ul of a[data-id]), so it works on any
// of them. Patterns after cmdk, Sonner, Vaul, NumberFlow and Magic UI; no library.
(function () {
  const Kit = (window.Kit = {});
  const $ = (sel, root) => (root || document).querySelector(sel);
  const el = (tag, cls, html) => {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (html != null) e.innerHTML = html;
    return e;
  };
  const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]);
  const GLASS = '<svg class="gd-glass" viewBox="0 0 16 16" aria-hidden="true"><circle cx="6.5" cy="6.5" r="4.5" fill="none" stroke="currentColor" stroke-width="2"/><path d="M10 10l4.5 4.5" stroke="currentColor" stroke-width="2"/></svg>';
  const NARROW = window.matchMedia('(max-width: 860px)');
  const IS_MAC = /Mac|iPhone|iPad/.test(navigator.platform || '');
  // How long a toast stays, and how many stay stacked.
  const TOAST_MS = 4000;
  const TOAST_MAX = 3;
  const TOAST_OUT_MS = 380;
  // A sheet dragged down further than this, or flicked faster, closes.
  const SHEET_CLOSE_PX = 90;
  const SHEET_FLICK = 0.6;

  // ---------------------------------------------------------------- toasts
  let toastBox = null;
  Kit.toast = (text, opts) => {
    const o = opts || {};
    if (!toastBox) {
      toastBox = el('ol', 'gd-toasts');
      toastBox.setAttribute('aria-live', 'polite');
      document.body.appendChild(toastBox);
    }
    const t = el('li', 'gd-toast' + (o.kind ? ' ' + o.kind : ''), '<i></i><div>' + esc(text) + (o.sub ? '<small>' + esc(o.sub) + '</small>' : '') + '</div>');
    toastBox.prepend(t);
    const items = [...toastBox.children];
    items.forEach((x, i) => x.style.setProperty('--i', i));
    items.slice(TOAST_MAX).forEach((x) => x.remove());
    // A pointer over the stack holds every toast in it, as in Sonner: the time left is kept.
    let left = TOAST_MS, since = 0, timer = 0;
    const run = () => { since = performance.now(); timer = setTimeout(gone, left); };
    const hold = () => { clearTimeout(timer); left -= performance.now() - since; };
    function gone() {
      toastBox.removeEventListener('pointerenter', hold);
      toastBox.removeEventListener('pointerleave', run);
      t.classList.add('gd-out');
      setTimeout(() => {
        t.remove();
        [...toastBox.children].forEach((x, i) => x.style.setProperty('--i', i));
      }, TOAST_OUT_MS);
    }
    toastBox.addEventListener('pointerenter', hold);
    toastBox.addEventListener('pointerleave', run);
    run();
  };

  // ---------------------------------------------------------------- rolling numbers
  // Kit.flow(el, '2,8') shows the text with every digit as a column that slides to its value.
  // A change in the shape of the text (more digits, a comma) builds the columns anew.
  Kit.flow = (target, value) => {
    const text = String(value);
    target.classList.add('gd-nf');
    target.setAttribute('aria-label', text);
    const shape = text.replace(/\d/g, '0');
    const fresh = target.dataset.shape !== shape;
    if (fresh) {
      target.dataset.shape = shape;
      target.innerHTML = [...text].map((c) => /\d/.test(c)
        ? '<span class="gd-nf-d" aria-hidden="true"><span>' + '0123456789'.split('').map((d) => '<span>' + d + '</span>').join('') + '</span></span>'
        : '<span aria-hidden="true">' + esc(c) + '</span>').join('');
    }
    const cols = target.querySelectorAll('.gd-nf-d > span');
    const digits = text.replace(/\D/g, '');
    const set = () => cols.forEach((c, i) => (c.style.transform = 'translateY(' + -1.2 * +digits[i] + 'em)'));
    // New columns start at zero and roll up to the value on the next frame.
    if (fresh) requestAnimationFrame(() => requestAnimationFrame(set));
    else set();
  };

  // ---------------------------------------------------------------- bottom sheets
  const scrim = el('div', 'gd-sheet-scrim');
  let openSheet = null;
  class Sheet {
    constructor(node, label) {
      this.node = node;
      this.label = label;
      this.grip = el('div', 'gd-grip');
      this.grip.setAttribute('aria-hidden', 'true');
      let start = null;
      this.grip.addEventListener('pointerdown', (e) => {
        start = { y: e.clientY, t: performance.now() };
        this.grip.setPointerCapture(e.pointerId);
        node.classList.add('gd-dragging');
      });
      this.grip.addEventListener('pointermove', (e) => {
        if (start) node.style.setProperty('--drag', Math.max(0, e.clientY - start.y) + 'px');
      });
      const end = (e) => {
        if (!start) return;
        const dy = Math.max(0, e.clientY - start.y), v = dy / Math.max(1, performance.now() - start.t);
        start = null;
        node.classList.remove('gd-dragging');
        if (dy > SHEET_CLOSE_PX || v > SHEET_FLICK) this.close();
        else node.style.setProperty('--drag', '0px');
      };
      this.grip.addEventListener('pointerup', end);
      this.grip.addEventListener('pointercancel', end);
    }
    attach() {
      this.node.classList.add('gd-sheet');
      this.node.setAttribute('role', 'dialog');
      this.node.setAttribute('aria-label', this.label);
      this.node.prepend(this.grip);
    }
    detach() {
      this.close(true);
      this.node.classList.remove('gd-sheet');
      this.node.removeAttribute('role');
      this.node.removeAttribute('aria-label');
      this.grip.remove();
    }
    open() {
      if (openSheet && openSheet !== this) openSheet.close(true);
      if (!scrim.isConnected) document.body.appendChild(scrim);
      this.back = document.activeElement;
      this.node.style.setProperty('--drag', '0px');
      this.node.classList.add('gd-open');
      scrim.classList.add('gd-open');
      openSheet = this;
      const first = this.node.querySelector('a.on, button, a, [tabindex]');
      if (first) setTimeout(() => first.focus({ preventScroll: false }), 50);
    }
    close(quiet) {
      if (openSheet !== this) return;
      this.node.classList.remove('gd-open');
      scrim.classList.remove('gd-open');
      openSheet = null;
      if (!quiet && this.back && this.back.isConnected) this.back.focus({ preventScroll: true });
    }
  }
  scrim.addEventListener('click', () => openSheet && openSheet.close());

  // Kit.drawer(html, label): details in a sheet from the bottom of the screen.
  let detail = null;
  Kit.drawer = (html, label) => {
    if (!detail) {
      const node = el('div', null, '<div class="gd-sheet-body"></div>');
      document.body.appendChild(node);
      detail = new Sheet(node, label || 'Подробности');
      detail.attach();
      node.addEventListener('click', (e) => { if (e.target.closest('.gd-x')) detail.close(); });
    }
    detail.node.setAttribute('aria-label', label || 'Подробности');
    $('.gd-sheet-body', detail.node).innerHTML = '<button class="gd-x" type="button">Закрыть</button>' + html;
    detail.open();
  };
  Kit.narrow = () => NARROW.matches;

  // ---------------------------------------------------------------- the sidebar as a sheet
  function navSheet() {
    const nav = $('#nav'), top = $('.top');
    if (!nav || !top) return;
    const sheet = new Sheet(nav, 'Разделы справочника');
    const btn = el('button', 'gd-navbtn', '<svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true"><path d="M1 3h12M1 7h12M1 11h12" stroke="currentColor" stroke-width="2"/></svg>Разделы');
    btn.type = 'button';
    btn.setAttribute('aria-haspopup', 'dialog');
    btn.addEventListener('click', () => {
      sheet.open();
      const on = $('a.on', nav);
      if (on) on.scrollIntoView({ block: 'center' });
    });
    top.appendChild(btn);
    const apply = () => (NARROW.matches ? sheet.attach() : sheet.detach());
    NARROW.addEventListener('change', apply);
    apply();
    window.addEventListener('hashchange', () => sheet.close(true));
  }

  // ---------------------------------------------------------------- command palette
  // Extra result groups: Kit.palette.add((query) => ({ group, items: [{ label, sub, run }] })).
  const sources = [];
  Kit.palette = { add: (fn) => sources.push(fn), open: (q) => openPalette(q) };
  let pal = null, results = [], active = 0, trigger = null;

  const norm = (s) => s.toLowerCase().replace(/ё/g, 'е');
  // Lower is better: the name starts with the query, a word of it does, the name has it, the summary has it.
  function score(q, name, find) {
    const n = norm(name), words = q.split(/\s+/).filter(Boolean);
    if (!words.every((w) => norm(find || name).includes(w) || n.includes(w))) return -1;
    if (n.startsWith(q)) return 0;
    if (n.split(/[\s-]+/).some((w) => w.startsWith(words[0]))) return 1;
    if (n.includes(q)) return 2;
    return 3;
  }
  const mark = (name, q) => {
    const i = q ? norm(name).indexOf(q) : -1;
    return i < 0 ? esc(name) : esc(name.slice(0, i)) + '<mark>' + esc(name.slice(i, i + q.length)) + '</mark>' + esc(name.slice(i + q.length));
  };
  function navEntries() {
    const out = [];
    let group = '';
    for (const node of $('#nav').children) {
      if (node.tagName === 'H3') group = node.textContent;
      else for (const a of node.querySelectorAll('a[data-id]'))
        out.push({ group, a, label: a.querySelector('.nm') ? a.querySelector('.nm').textContent : a.textContent, find: a.dataset.find || '', sub: a.dataset.sum || '' });
    }
    return out;
  }
  function iconOf(a) {
    const c = a && a.querySelector('.icon canvas');
    const wrap = el('span', 'icon');
    if (!c) return wrap;
    const copy = document.createElement('canvas');
    copy.width = c.width;
    copy.height = c.height;
    copy.getContext('2d').drawImage(c, 0, 0);
    wrap.appendChild(copy);
    return wrap;
  }

  function buildPalette() {
    pal = el('div', 'gd-cmdk');
    pal.hidden = true;
    pal.innerHTML =
      '<div class="gd-cmdk-box" role="dialog" aria-modal="true" aria-label="Поиск по справочнику">' +
      '<div class="gd-cmdk-bar">' + GLASS + '<input type="text" role="combobox" aria-expanded="true" aria-controls="gd-cmdk-list" aria-autocomplete="list" placeholder="Блок, машина, модуль или слово из словаря" autocomplete="off" spellcheck="false"><span class="gd-kbd">Esc</span></div>' +
      '<div class="gd-cmdk-list" id="gd-cmdk-list" role="listbox" aria-label="Результаты"></div>' +
      '<div class="gd-cmdk-foot"><span><span class="gd-kbd">↑</span><span class="gd-kbd">↓</span> выбрать</span><span><span class="gd-kbd">Enter</span> открыть</span><span><span class="gd-kbd">Esc</span> закрыть</span></div></div>';
    document.body.appendChild(pal);
    const input = $('input', pal), list = $('.gd-cmdk-list', pal);
    pal.addEventListener('mousedown', (e) => { if (e.target === pal) closePalette(); });
    input.addEventListener('input', () => render(input.value));
    input.addEventListener('keydown', (e) => {
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        e.preventDefault();
        if (results.length) select((active + (e.key === 'ArrowDown' ? 1 : -1) + results.length) % results.length);
      } else if (e.key === 'Home' && e.ctrlKey) select(0);
      else if (e.key === 'End' && e.ctrlKey) select(results.length - 1);
      else if (e.key === 'Enter') {
        e.preventDefault();
        if (results[active]) choose(results[active]);
      } else if (e.key === 'Escape') {
        e.preventDefault();
        closePalette();
      } else if (e.key === 'Tab') e.preventDefault();
    });
    list.addEventListener('pointermove', (e) => {
      const it = e.target.closest('.gd-cmdk-item');
      if (it && +it.dataset.i !== active) select(+it.dataset.i, true);
    });
    list.addEventListener('click', (e) => {
      const it = e.target.closest('.gd-cmdk-item');
      if (it) choose(results[+it.dataset.i]);
    });
  }
  function render(raw) {
    const q = norm(raw.trim());
    const list = $('.gd-cmdk-list', pal);
    const groups = new Map();
    const push = (g, r) => (groups.has(g) ? groups.get(g) : groups.set(g, []).get(g)).push(r);
    for (const e of navEntries()) {
      const s = q ? score(q, e.label, e.find) : 0;
      if (s >= 0) push(e.group, { s, label: e.label, sub: e.sub, a: e.a, run: () => (location.hash = e.a.dataset.id) });
    }
    for (const fn of sources) {
      const extra = fn(q);
      if (!extra) continue;
      for (const it of extra.items) {
        const s = q ? score(q, it.label, it.find || it.label) : 0;
        if (s >= 0) push(extra.group, Object.assign({ s: s + (extra.weight || 0) }, it));
      }
    }
    // Groups with the best match come first; inside a group, better matches first.
    const ordered = [...groups.entries()].map(([g, items]) => [g, q ? items.sort((x, y) => x.s - y.s) : items]);
    if (q) ordered.sort((x, y) => Math.min(...x[1].map((r) => r.s)) - Math.min(...y[1].map((r) => r.s)));
    results = [];
    list.innerHTML = '';
    for (const [g, items] of ordered) {
      list.appendChild(el('div', 'gd-cmdk-group', esc(g))).setAttribute('role', 'presentation');
      for (const r of items) {
        const it = el('div', 'gd-cmdk-item', '<span class="gd-t"><b>' + mark(r.label, q) + '</b>' + (r.sub ? '<small>' + esc(r.sub) + '</small>' : '') + '</span>');
        it.id = 'gd-opt-' + results.length;
        it.dataset.i = results.length;
        it.setAttribute('role', 'option');
        it.prepend(iconOf(r.a));
        const tier = r.a && r.a.querySelector('.tier');
        if (tier) it.appendChild(tier.cloneNode(true));
        list.appendChild(it);
        results.push(r);
      }
    }
    if (!results.length)
      list.innerHTML = '<div class="gd-cmdk-empty">Ничего не нашлось по запросу «' + esc(raw.trim()) + '». Попробуй часть слова: «дроб», «узел», «склад».</div>';
    select(0);
  }
  function select(i, byPointer) {
    active = i;
    const input = $('input', pal);
    pal.querySelectorAll('.gd-cmdk-item').forEach((it) => it.setAttribute('aria-selected', String(+it.dataset.i === i)));
    const cur = $('#gd-opt-' + i, pal);
    if (cur) {
      input.setAttribute('aria-activedescendant', cur.id);
      if (!byPointer) cur.scrollIntoView({ block: 'nearest' });
    } else input.removeAttribute('aria-activedescendant');
  }
  function choose(r) {
    closePalette(true);
    r.run();
  }
  function openPalette(q) {
    if (!$('#nav')) return;
    if (!pal) buildPalette();
    trigger = document.activeElement;
    pal.hidden = false;
    document.documentElement.style.overflow = 'hidden';
    const input = $('input', pal);
    input.value = q || '';
    render(input.value);
    input.focus();
  }
  function closePalette(keepFocus) {
    if (!pal || pal.hidden) return;
    pal.hidden = true;
    document.documentElement.style.overflow = '';
    if (!keepFocus && trigger && trigger.isConnected) trigger.focus({ preventScroll: true });
  }
  // The page's search field becomes the palette's button.
  function searchButton() {
    const old = $('#search');
    if (!old) return;
    const b = el('button', 'gd-find', GLASS + '<span>' + esc(old.placeholder || 'Поиск') + '</span><span class="gd-kbd">' + (IS_MAC ? '⌘' : 'Ctrl') + ' K</span>');
    b.type = 'button';
    b.id = 'search';
    b.setAttribute('aria-haspopup', 'dialog');
    b.setAttribute('aria-label', (old.getAttribute('aria-label') || 'Поиск') + ' (' + (IS_MAC ? '⌘' : 'Ctrl') + '+K)');
    b.addEventListener('click', () => openPalette(''));
    old.replaceWith(b);
  }
  sources.push((q) => {
    if (q && !'скопировать ссылку на страницу'.includes(q) && !q.startsWith('ссыл')) return null;
    return {
      group: 'Действия', weight: 1, items: [{
        label: 'Скопировать ссылку на страницу', sub: 'Адрес открытой страницы, чтобы отправить его',
        run: () => Kit.copy(location.href, 'Ссылка на страницу скопирована'),
      }],
    };
  });
  // Copies text and says so in a toast; when the browser refuses, the toast shows the text instead.
  Kit.copy = (text, done) => {
    const fail = () => Kit.toast('Не удалось скопировать', { kind: 'warn', sub: text });
    if (!navigator.clipboard) return fail();
    navigator.clipboard.writeText(text).then(() => Kit.toast(done, { sub: text }), fail);
  };

  // ---------------------------------------------------------------- keys
  document.addEventListener('keydown', (e) => {
    const typing = /^(INPUT|TEXTAREA|SELECT)$/.test(e.target.tagName) || e.target.isContentEditable;
    if ((e.ctrlKey || e.metaKey) && !e.altKey && (e.key === 'k' || e.key === 'K' || e.code === 'KeyK')) {
      e.preventDefault();
      if (pal && !pal.hidden) closePalette();
      else openPalette('');
    } else if (e.key === '/' && !typing && !(pal && !pal.hidden)) {
      e.preventDefault();
      openPalette('');
    } else if (e.key === 'Escape' && openSheet && !(pal && !pal.hidden)) openSheet.close();
  });

  // ---------------------------------------------------------------- light over cards
  document.addEventListener('pointermove', (e) => {
    const card = e.target.closest && e.target.closest('.gd-glow');
    if (!card) return;
    const r = card.getBoundingClientRect();
    card.style.setProperty('--mx', e.clientX - r.left + 'px');
    card.style.setProperty('--my', e.clientY - r.top + 'px');
  }, { passive: true });

  searchButton();
  navSheet();
})();
