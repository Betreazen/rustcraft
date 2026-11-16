# Срез 18, поток 8 «Справочник в игре и документы» — план

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Справочник инженера в игре показывает то же, что HTML-справочник: рецепты всех видов, таблицы, «см. также», подписи и таблички сцен, окно блока картинкой; противоречия справочника из спеки сняты.

**Architecture:** HTML-справочник (`docs/showcase/js/*`) — источник. `assets/models/handbook_mc.py` и `scenes_mc.py` переносят его в таблицы крейта правил (`rules/src/handbook/pages.rs`, `scenes.rs`) и ключи `rustcraft.handbook.*`. Раскладка, разбор рецептов и расчёт таблиц — чистые функции в `rules/src/handbook/` с юнит-тестами; `mod/rustcraft/handbook.rs` только читает игру (рецепты, язык, ширину текста) и рисует.

**Tech Stack:** Rust (rules crate, bundle), Python 3 + Node (генераторы), JS (HTML-справочник).

**Spec:** [docs/superpowers/specs/2026-11-16-handbook-parity-design.md](../specs/2026-11-16-handbook-parity-design.md), работа — раздел `guide` в [docs/design/06-audit/base-gear.md](../../design/06-audit/base-gear.md) и строки «Сцена … (см. раздел guide)» других таблиц.

## Global Constraints

- Horizon `7e4e421224743469035ba0c34fc21e569e64a17d`, Minecraft 26.3; Horizon не меняется.
- Игра следует справочнику. Справочник меняется только по противоречиям спеки и по проверенному в `SRC` пределу движка. Тексты механик в справочнике под игру не подгоняются.
- Клавиши: **Z** магнит, **B** рюкзак, **;** ночное зрение. Гнёзд брони 1 / 2 / 2. Ёмкость костюма: +50% батареи. Чары накладываются на наковальне.
- Тексты на русском по `CONTRIBUTING.md`, коммиты на английском, почта `128125070+Betreazen@users.noreply.github.com`, строка `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Все ключи `rustcraft.handbook.` пишет только `py -3 assets/models/handbook_mc.py`; языковые файлы руками не правятся.
- Общие файлы (`selftest.rs`, `rules/src/selftest/mod.rs`, lang) — только своим блоком строк.
- Проверка ветки: `joint.ps1 try -Mod rustcraft -Path <worktree> -Owner s18-handbook`.

## Строки работы (из сверки)

| # | Строка сверки | Статус до | Задача |
|---|---|---|---|
| A | Противоречия: гнёзда `pmodules`/`armor` 1/2/2, окно брони «+50%», клавиши Z/B/;, чары на наковальне, «Гарпун пока проверяется» | DIFFERS | 1 |
| B | Рецепты плавильни, дробилки и смесителя; рецепты старших ступеней (превращения) | MISSING | 2 |
| C | Таблицы (модули турели, сопротивления) и ссылки «см. также» | MISSING | 3 |
| D | Сцена: таблички с числами, названия блоков, выпавшие предметы, сущности яйцами | DIFFERS | 4 |
| E | Окно блока картинкой с номерами частей | DIFFERS | 5 |
| F | Сцена: модели и позы (поворот, работает ли, уровень бака) | DIFFERS | 6 |
| G | Английский — только название и описание | DIFFERS | 7 (решение) |
| H | README: раздел «Что проверить в игре: срез 18», черновик «Что срезы пока не умеют» | — | 8 |

---

### Task 1: Противоречия справочника и перегенерация

**Files:** `docs/showcase/js/data.js` (pmodules :1235–1236, backpack :1305), `ui.js` (:291, :293), `scenes.js` (:1346, :1358, :1389, :1397, :1491), `info.js` (:96–99, наковальня), `models.js` (модель `anvil`), `assets/models/scenes_mc.py` (`anvil` → `minecraft:anvil`), перегенерация `pages.rs`, `scenes.rs`, lang.

- [x] Тексты: «1 / 2 / 2» вместо «по два гнезда» (pmodules, сцены armor/pmodules, info.js, заметка окна брони); окно брони — `['Ёмкость костюма', 'батарея костюма +50%']`, `['Магнит', 'включён · клавиша Z']`; ночное зрение «;», магнит Z, рюкзак B там, где справочник говорит «клавишей»; подпись гарпуна без «пока проверяется»; сцена чар — наковальня (`s.put('an', 'anvil', …)`), модель `M.anvil` из коробок, карточка `anvil` в info.js.
- [x] `py -3 assets/models/handbook_mc.py`; `cargo test --release -q` в `mod/rustcraft/rules` — зелёный.
- [x] Коммит `docs: settle the handbook's contradictions (sockets 1/2/2, suit capacity, keys, anvil, harpoon)`.

### Task 2: Все рецепты страницы — верстак, превращения, машины

**Files:** create `rules/src/handbook/recipes.rs` (+ тесты в `rules/src/handbook/tests_recipes.rs`), modify `rules/src/handbook/mod.rs` (`Page.makes`, `Piece::Recipes`, `Row::Recipes`), `assets/models/handbook_mc.py` (`MAKES`), `handbook.rs` (чтение рецептов игры, рисование), `rules/src/selftest/handbook.rs` + блок в `selftest.rs` (метод доски).

**Interfaces (produces):**
- `recipes::Shown { made: Made, out: Out }`; `Made::Grid([Option<String>; 9])` | `Made::Machine { machine: &'static str, inputs: Vec<(String, u32)> }`; `Out::Item(String, u32)` | `Out::Fluid(String, u32 /* мВ */)`.
- `recipes::machine_of(kind: &str) -> Option<&'static str>`: `rustcraft:crushing` → `rustcraft:crusher`, `rustcraft:alloying` → `rustcraft:alloy`, `rustcraft:mixing` → `rustcraft:mixer`, `minecraft:smelting` → `rustcraft:furnace`.
- `recipes::custom(kind, &serde_json::Value, resolve: &dyn Fn(&str) -> Option<String>) -> Option<Shown>` — файлы `crushing` (`ingredient`), `alloying`/`mixing` (`ingredients` с `count`, `water` в мВ → ведро воды), `result` с `id`/`count` или `fluid`/`amount`.
- `recipes::transmute(input, material, count, out, n) -> Shown` — вход в центре сетки, материалы вокруг.
- `Page::makes(&self, id) -> bool` — предмет страницы, его ступени `<основа>_<n>` и `Page.makes` (сплавы, пыль, жидкости, части брони, модули игрока, улучшения, книги чар). Правило по имени ловит и предметы, которые добавят другие потоки (ступени машин, узлов, модулей).
- `recipes::for_page(page, all: Vec<Shown>) -> Vec<Shown>` — те, что делают предметы страницы, без повторов, по ступени.
- Раскладка: по два рецепта в ряд (`RECIPE_CELL` 150 точек), высота ряда — сетка 58, машина 40.

- [x] RED: тесты `custom_crushing_reads_ingredient_and_result`, `custom_mixing_reads_water_and_fluid`, `custom_alloying_keeps_counts`, `transmute_puts_input_in_the_middle`, `page_makes_its_tiers_and_extras`, `for_page_orders_by_tier_and_drops_repeats`, `recipes_lay_out_two_to_a_row`.
- [x] GREEN; бандл: `recipes_of(game, page)` собирает `crafting` (форма, без формы, `Special::Transmute`), `cooking` (`smelting`) и `native_recipes_of_type` трёх типов мода; рисует сетку или входы, машину над стрелкой, жидкость — цветным квадратом; подсказка по наведению.
- [x] Самотест `handbook_pages_show_machine_recipes`: страница `alloys` находит 1 верстачный + 3 рецепта плавильни, `dusts` — дробилку, `fluids` — смеситель, `blade` — превращения ступеней 3 и 4.
- [x] Коммит.

### Task 3: Таблицы и «см. также»

**Files:** `rules/src/handbook/mod.rs` (`Page.tables: &[[usize; 2]]`, `Page.see: &[&str]`, `Piece::Table`, `Piece::See`, `Row::Cells`, `Row::Links`, `columns`, `link_at`), `handbook_mc.py` (ключи `page.<id>.table.<t>.title|note|head.<c>|row.<r>.<c>`, `see`), `handbook.rs` (рисование, клик и наведение по ссылке).

- [x] RED: `columns_fit_natural_widths`, `columns_share_what_is_left`, `table_cells_wrap_inside_their_column`, `see_links_two_to_a_row`, `link_at_finds_the_page_under_the_mouse`, `see_skips_unknown_pages`.
- [x] GREEN, генератор, рисование: заголовок таблицы, заметка, шапка цветом подписи, ячейки по колонкам; ссылки — иконка страницы и имя, клик открывает страницу.
- [x] Коммит.

### Task 4: Таблички, названия, выпавшие предметы и сущности в сценах

**Files:** `scenes_mc.py` (снимок `badges` и `tags`, `drop` → тонированный малый куб, сущности → головы, где у игры есть голова), `rules/src/handbook/scene.rs` (`Label`, `labels_at`), `handbook.rs` (рисование табличек в панели), lang `rustcraft.handbook.scene.<id>.<n>.label.<k>`.

- [x] RED: `labels_follow_their_mover`, `labels_stand_above_their_point`, `label_colours_by_kind`.
- [x] GREEN; таблички рисуются уменьшенным текстом (`HudBatch::scale_since`) на тёмной плашке, цвет по виду (`bad`, `warn`, `ok`, `info`, `tag`); названия блоков (`tags`) — как в HTML, включены.
- [x] Коммит.

### Task 5: Окно блока картинкой

**Files:** `handbook_mc.py` (вид части из `ui.js`: slots, work, energy, gives, mode, modules, tank, gauge, status, rows, прочее), `rules/src/handbook/window.rs` (раскладка частей столбиком с номерами), `handbook.rs` (рисование частей стилем окон).

- [x] RED: `window_parts_stack_with_numbers`, `window_sheet_fits_the_body`.
- [x] GREEN; лист «Окно блока» перед списком частей: схема окна с номерами частей, под ней тот же нумерованный список.
- [x] Коммит.

### Task 6: Позы моделей в сценах

Сделано: блок с движущимися частями (anim_mc.py: огонь печи, валки дробилки, ротор ветряка, жидкость бака…) рисуется в сцене снимком модели с частями в состоянии шага (`on` из scenes.js: работает или стоит; «always»-блоки всегда работают; уровни — по poses.js). `assets/models/poses_mc.py` пишет модели `models/item/hb/<блок>_<on|still>.json` и определения предметов `items/hb/…`; окно рисует их `native_item_model` (API 111, native_hooks.rs:10992-11016); вложенные папки определений движок читает (horizon-client item_render.rs:1429 `item_definitions_load_from_nested_folders`), составные модели и постоянный оттенок — тоже (horizon-assets item_model.rs:1-5, :635).

Не сделано (не предел движка): поворот блока (`face`) и прицел турели в сценах — снимок смотрит, как иконка блока; движение частей (снимок неподвижен). Поворот возможен дочерними моделями с другим `display.gui`, но без проверки на экране направление осей сцены и игры не подтверждено; оставлено интегратору на скриншоты.

- [x] Снимки моделей с частями, состояние «работает / стоит», уровни.
- [ ] Поворот (`face`) — после скриншотов интегратора.

### Task 7: Английский

Решение: HTML-справочник только на русском, поэтому короткий английский — не отличие от справочника. Остаётся как есть; в README записано.

- [x] Записано.

### Task 8: README

- [x] Раздел «Что проверить в игре: срез 18» с подразделами потоков (каждый поток дописывает свой) и пункты справочника.
- [x] Абзац «Справочник в игре» в README — по новому устройству.
- [x] Черновик «Что срезы пока не умеют» — ниже, в этом файле.

---

## Пределы движка

1. **Сущности в окне не рисуются.** Мод рисует в своём экране только прямоугольники (`native_fill`), четырёхугольники (`native_quad`, API 111), иконки предметов (`native_item`, API 20; `native_item_model`, API 111; `native_item_stack`, API 313) и текст; хука, рисующего моба в GUI (как кукла игрока в инвентаре), в `SRC/docs/native-mods.md` нет (поиск «entity», «mob», «gui» по разделам API 20, 78, 111, 139, 313, 410). Вместо мобов сцены рисуют предметы: зомби и скелет — головами (`zombie_head`, `skeleton_skull`), игрок — головой игрока, корова, овца и пчела — яйцами призыва.
2. **Модель сущности моделью предмета не собрать.** Текстуры мобов лежат вне атласа блоков и предметов, а ассеты Minecraft в git не коммитятся (CONTRIBUTING).

## Черновик «Что срезы пока не умеют» (для интегратора, по второй сверке)

Структура: по области, только настоящие отличия от справочника; строка = что не так · почему (предел движка со ссылкой или «не сделано») · где видно.

- **Магистраль** — (из `after/pipe.md`)
- **Энергия и добыча** — (из `after/energy.md`)
- **Машины** — (из `after/machine.md`)
- **Модули** — (из `after/module.md`)
- **Крафт и склад** — (из `after/craft-storage.md`)
- **Монорельс и работники** — (из `after/rail-worker.md`)
- **База и снаряжение** — (из `after/base-gear.md`)
- **Справочник в игре** — мобы в сценах головами и яйцами (предел движка 1–2); блоки сцены не поворачиваются и стоят снимком, турель не наводится (не сделано, задача 6); схемы окон станции и картографа — списком названий; английский короче (HTML-справочник только на русском); 103 подписи с числами (H4). Готовая строка — в `mod/README.md`, «Справочник в игре: …».

Как переписывать раздел: убрать строки, которые вторая сверка отметила DONE или STALE (строки по срезам 8–17 почти все такие после среза 18); каждую оставшуюся строку начинать с области, а не со среза; предел движка помечать «предел движка» со ссылкой на план потока; общие оговорки (выделенный сервер, `Tuning::default()`, повтор LetsCam) — одной строкой в конце.
- **Общее** — выделенный сервер, `Tuning::default()`, повтор LetsCam.

## Журнал

- 2026-11-16: план; задачи 1–5, 7, 8 сделаны, задача 6 — без поворота.
