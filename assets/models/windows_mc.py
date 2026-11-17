"""The block windows of the handbook in the game (guide «Окно блока»: ui.js `RC.UI`), as pictures.

The guide draws every block's window as a mock-up whose parts carry numbers, with a legend of
the parts beside it. handbook_mc.py hands this script each window's parts as ui.js builds them
(their HTML); `widget` reads which of ui.js's part builders made each one (slots, the work
arrow, an energy or other bar, the mode switch, module sockets, a tank, the status line, rows of
key and value, chips, a craft, a map, a text field) and the numbers and colours it shows. The
words a part shows become lang keys `rustcraft.handbook.page.<id>.part.<i>.w.<k>`; the table is
`mod/rustcraft/rules/src/handbook/windows.rs` (rustcraft_rules::handbook::windows), whose types
and drawing rules are in `rules/src/handbook/window.rs`.

Run through handbook_mc.py, which owns every `rustcraft.handbook.` key.
"""
import html
import json
import re
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
TABLE = ROOT / 'mod' / 'rustcraft' / 'rules' / 'src' / 'handbook' / 'windows.rs'
ENERGY = 0xFFE14A


def plain(s):
    s = re.sub(r'<[^>]+>', ' ', str(s))
    return re.sub(r'\s+', ' ', html.unescape(s)).strip()


def colour(s, default=0x8A8F99):
    m = re.search(r'--c:\s*(#[0-9a-fA-F]{6})', s)
    return int(m.group(1)[1:], 16) if m else default


def pct(s):
    m = re.search(r'--p:\s*([0-9.]+)', s)
    return max(0, min(100, round(float(m.group(1))))) if m else 0


def without_label(h):
    return re.sub(r'<label>.*?</label>', '', h, count=1, flags=re.S)


def cells(h):
    out = []
    for m in re.finditer(r'<i class="w-s( w-ghost)?">(.*?)</i>', h, re.S):
        inner = m.group(2)
        rgb = colour(inner, 0) if '<b' in inner else 0
        em = re.search(r'<em>(.*?)</em>', inner, re.S)
        out.append((rgb, plain(em.group(1)) if em else '', bool(m.group(1))))
    return out


def cell_rust(c):
    rgb, count, ghost = c
    return 'Cell { rgb: 0x%06x, count: %s, ghost: %s }' % (rgb, json.dumps(count, ensure_ascii=False),
                                                          'true' if ghost else 'false')


def widget(h):
    """(Rust expression, [words]) for one part's HTML."""
    body = without_label(h)
    if 'w-craft' in body:
        left, _, right = body.partition('<span>→</span>')
        return ('Widget::Craft { from: &[%s], to: &[%s] }' % (
            ', '.join(cell_rust(c) for c in cells(left)), ', '.join(cell_rust(c) for c in cells(right))), [])
    if 'w-grid' in body:
        m = re.search(r'--cols:\s*(\d+)', body)
        cs = cells(body)
        cols = int(m.group(1)) if m else max(1, len(cs))
        return ('Widget::Slots { cols: %d, cells: &[%s] }' % (cols, ', '.join(cell_rust(c) for c in cs)), [])
    if 'w-arrow' in body:
        return ('Widget::Work { pct: %d }' % pct(body), [])
    if 'w-seg' in body:
        buttons = re.findall(r'<button[^>]*aria-checked="(true|false)"[^>]*>(.*?)</button>', body, re.S)
        active = next((i for i, (on, _) in enumerate(buttons) if on == 'true'), 0)
        return ('Widget::Mode { active: %d }' % active, [plain(t) for _, t in buttons])
    if 'w-sock' in body:
        socks = re.findall(r'<span class="w-sock( w-empty)?"[^>]*><b([^>]*)>', body)
        filled = ['0x%06x' % (0 if empty else colour(b, 0x8A8F99)) for empty, b in socks]
        return ('Widget::Sockets { filled: &[%s] }' % ', '.join(filled), [])
    if 'w-tank"' in body:
        small = re.search(r'<small>(.*?)</small>', body, re.S)
        return ('Widget::Tank { pct: %d, rgb: 0x%06x }' % (pct(body), colour(body)),
                [plain(small.group(1)) if small else ''])
    if 'w-status' in body:
        tone = re.search(r'w-status (ok|warn|bad)', body)
        text = re.search(r'<span>(.*?)</span>', body, re.S)
        return ('Widget::Status { tone: %d }' % ['ok', 'warn', 'bad'].index(tone.group(1) if tone else 'ok'),
                [plain(text.group(1)) if text else ''])
    if 'w-kv' in body:
        pairs = re.findall(r'<div class="w-kv"><span>(.*?)</span><b>(.*?)</b></div>', body, re.S)
        words = []
        for k, v in pairs:
            words += [plain(k), plain(v)]
        return ('Widget::Rows { n: %d }' % len(pairs), words)
    if 'w-bar' in body:
        small = re.search(r'<small>(.*?)</small>', body, re.S)
        rgb = ENERGY if 'w-en' in body else colour(body)
        return ('Widget::Bar { pct: %d, rgb: 0x%06x }' % (pct(body), rgb), [plain(small.group(1)) if small else ''])
    chips = re.findall(r'<span class="w-(?:tog|btn)( on)?">(.*?)</span>', body, re.S)
    if 'w-tabs' in body:
        chips = re.findall(r'<span( class="on")?>(.*?)</span>', body.split('w-tabs', 1)[1], re.S)
    if chips:
        small = re.search(r'<small>(.*?)</small>', body, re.S)
        words = [plain(t) for _, t in chips] + ([plain(small.group(1))] if small else [])
        return ('Widget::Chips { on: &[%s], note: %s }' % (
            ', '.join('true' if on else 'false' for on, _ in chips), 'true' if small else 'false'), words)
    if 'w-map' in body:
        texts = [plain(t) for t in re.findall(r'<text[^>]*>(.*?)</text>', body, re.S)]
        texts = [t for t in texts if t]
        return ('Widget::Map { n: %d }' % len(texts), texts)
    text = plain(body)
    return ('Widget::Text', [text] if text else [])


def build(ui):
    """The Rust table and the Russian words of every window: `ui` maps a page id to its window
    as ui.js has it ({title, parts: [{html, …}]})."""
    rows, ru = [], {}
    for pid, w in ui.items():
        parts = []
        for i, part in enumerate(w['parts']):
            rust, words = widget(part['html'])
            parts.append(rust)
            for k, word in enumerate(words):
                ru['rustcraft.handbook.page.%s.part.%d.w.%d' % (pid, i, k)] = word
        ru['rustcraft.handbook.page.%s.window' % pid] = plain(w['title'])
        rows.append('    (%s, &[\n        %s,\n    ]),' % (json.dumps(pid), ',\n        '.join(parts)))
    text = ('//! The block windows of the guide as pictures, written by assets/models/windows_mc.py\n'
            '//! (through handbook_mc.py) from docs/showcase/js/ui.js: do not edit by hand, run the script.\n\n'
            'use super::window::{Cell, Widget};\n\n'
            '/// Each page\'s window: its parts\' widgets in the order of their numbers.\n'
            '#[rustfmt::skip]\npub const WINDOWS: &[(&str, &[Widget])] = &[\n%s\n];\n' % '\n'.join(rows))
    return text, ru


def write(text):
    data = text.encode('utf-8')
    if not TABLE.exists() or TABLE.read_bytes() != data:
        TABLE.write_bytes(data)
