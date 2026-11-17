# Срез 18, волна 2: магистраль и энергия — план

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans. Steps use checkbox (`- [x]`) syntax.

**Goal:** закрыть «Ещё не сделано» второго аудита `docs/design/06-audit/after/pipe.md` и `after/energy.md` (кроме README — это интегратору) и падающий самотест `s18_placer_and_breaker_by_tier`.

**Architecture:** правила — в `mod/rustcraft/rules` с юнит-тестами (RED → GREEN), бандл связывает их с игрой. Каждая механика — самотест в мире (`rules/src/selftest/pipe18.rs`, `s18_energy.rs`, `sources.rs`, `s18_cross.rs`).

**Tech Stack:** Rust, Horizon 7e4e421 (SRC), rustcraft_rules.

**Spec:** `docs/superpowers/specs/2026-11-16-handbook-parity-design.md`, аудит `docs/design/06-audit/after/{pipe,energy}.md`, справочник `docs/showcase/js/{data,ui}.js`.

## Global Constraints

- Правила в rules-крейте с тестами; бандл только связывает.
- Общие файлы (server.rs, selftest, lang, hook-файлы, transport.rs): только добавлять свои строки.
- Тексты для игрока — ru_ru и en_us. Коммит — по задаче, `git commit -- <paths>`, почта noreply.
- Проверка сборки: `scratchpad/chk-w2-pipe-energy.sh <worktree>`; bench — `joint.ps1 try` со снимка.

---

### Task 1: F5 — панель возвращается, когда ключ убран
**Files:** `mod/rustcraft/pipe18.rs` (`draw_housing`).
- [x] Корпус с панелью (`builder::has_facade`) не сохраняет отрисовку (`draw.keep()`): без ключа `draw_facade` снова рисует панель в следующем кадре. Сохраняется только корпус без панели, без посылок и без потока (Task 8).
- [x] Только глазами: ключ в руке → панели прозрачны; убрать ключ → панели на месте.

### Task 2: снятая ключом машина 2–4-й ступени сохраняет ступень
**Files:** `pipe18.rs` (`lift_machine`, бенч `lift_and_place`), `rules/src/selftest/pipe18.rs`.
- [x] Предмет — `Family::item(tier)` (`tiers::family_at`), не id блока.
- [x] Бенч `lift_and_place` ставит блок семьи с `tier` предмета (`tiers::family_of_item`), как `state_for_placement`.
- [x] Самотест `w2_wrench_lifts_a_tier_3_machine_as_tier_3`: печь 3-й ступени → предмет `rustcraft:furnace_3` → поставлена с `tier=3`.

### Task 3: сломанный узел отдаёт панель, блок вида и модули ускорения линии
**Files:** `server.rs` (`on_destroy_block`, свой блок строк), `node_window.rs` (`node_returns`), `pipe18.rs` (бенч `node_returns`), самотест.
- [x] RED/GREEN `rules/src/net/node_window.rs`: `returned_modules(n) -> Vec<&str>` (n модулей → n предметов `module_line`, не больше `SOCKETS`).
- [x] Узел в выживании: `facade_items` + модули → в инвентарь, иначе на землю.
- [x] Самотест `w2_broken_node_gives_back_facade_look_and_modules` (бенч: что вернёт узел).

### Task 4: взорванный корпус роняет панель и блок вида
**Files:** `pipe18.rs` (`drop_lost`, запоминание панелей при пересканировании), `server.rs` (вызов в `rescan`, одна строка).
- [x] При каждом пересканировании запоминать панель и блок вида каждого корпуса (`facades: (dim,pos) → Vec<item>`); корпус, пропавший без игрока, роняет и их.
- [x] Самотест `w2_lost_housing_drops_its_facade_and_look` (корпус под панелью с камнем → воздух командой → лежат панель и камень).

### Task 5: строка «В запасе» в окне узла
**Files:** `node_window.rs` (`view_of`, `rows`), lang.
- [x] `view.stored`/`capacity` — заряд и ёмкость накопителей цепей линии (как у окна генератора, `power.rs` stores).
- [x] Строка «В запасе: X из Y Е» между «Потребление» и «Хватает» (ui.js:72). Без накопителей — «—».
- [x] Тест rules: `stored_text` форматирует «12 400 из 40 000 Е».

### Task 6: стык — три направления справочника
**Files:** `rules/src/net/items.rs` (`Mode::next/previous`), lang (`rustcraft.mode.both`).
- [x] Окно перебирает «в машину → из машины → выключен» (data.js:187, 396). `Both` остаётся только как «по умолчанию» блока, пока стык не настроен (data.js:329: «По умолчанию магистраль сама подаёт машине то, что она принимает, и забирает готовое»): так и подписан.
- [x] Тесты: цикл из трёх, `Both.next() == Into`.

### Task 7: энергоканал за тик — не больше предела своей ступени
**Files:** `rules/src/net/energy.rs` (`supply_with_storage`, `share_out`).
- [x] RED: цепь с пределом 32, источник 100, машина 100, накопитель с зарядом — за тик через канал проходит ≤ 32 на все три прохода.
- [x] GREEN: остаток предела каждой цепи переходит из прохода в проход.

### Task 8: обычный поток предметов виден в окошках корпуса (M5)
**Files:** `pipe18.rs` (поток: сбор переносов, `show_flow`, `draw_flow`), `transport.rs` (одна строка: переносы тика в pipe18), `server.rs` (вызов раз в секунду), `rules/src/net/flow.rs` (новый: какие переносы показать).
- [x] RED/GREEN `net/flow.rs`: из переносов секунды — не больше `MAX_SHOWN` пар (откуда, куда) на сеть, по предмету на пару.
- [x] Раз в секунду: путь (`Transport::path`), спаны `orders::ride::spans` с отдельным ключом `flow`, клиент рисует как посылки; корпус без потока очищается.
- [x] Самотест `w2_plain_flow_rides_the_housings`.

### Task 9: MN1 — при сундуке рядом всё идёт в сундук
**Files:** `mining.rs` (`run`, `push_out`).
- [x] Добытчик и разрушитель с сундуком вплотную отдают в него в тот же тик, когда добыли (и после подачи из скрытых ячеек): магистраль не застаёт руду в ячейках, пока в сундуке есть место.
- [x] Самотест `w2_miner_with_a_chest_beside_fills_only_the_chest`.

### Task 10: каждый добытчик / установщик / разрушитель выдаёт по своему счётчику
**Files:** `mining.rs` (`State`), `rules/src/mining.rs` (`push_due`).
- [x] Счётчик на (измерение, позицию) вместо общего.
- [x] Самотест `w2_two_miners_both_fill_their_load_docks` (выдача в блок погрузки идёт только по счётчику).

### Task 11: модуль стака добытчика и предел ячейки меню 99
**Files:** `rules/src/mining.rs` (`stacked_split`), `mining.rs` (`take`, `settle_cells`, `read`, окно).
- [x] Ячейки окна держат обычный стак предмета; ×4 модуля стака — в скрытых ячейках (на каждую ячейку окна ещё три стака, `stacked_split`).
- [x] Тест rules; самотест: с модулем стака ни одна ячейка окна не больше 64.

### Task 12: проверка лопастей ветряка на новой высоте ротора
**Files:** `rules/src/power.rs` (`windmill_room(pos, facing)`), `power.rs`, `rules/src/selftest/sources.rs`.
- [x] Клетки: мачта +1, +2 и по оси лопастей (x для north/south, z для east/west) ±1 на +1 и +2.
- [x] Тест rules и самотест: блок сбоку на +2 мешает, блок за ветряком на +1 — нет.

### Task 13: `s18_placer_and_breaker_by_tier`
**Files:** `rules/src/selftest/s18_cross.rs` (`pace_shown`).
- [x] Строка темпа в окне есть и верна. Самотест не находил «1», потому что движок передаёт целое число аргументом перевода как число (`"with":[1,2]`, SRC `crates/horizon-client-core/src/text.rs:342-355`). Дробное значение приходит текстовым компонентом (`{"text":"0,5"}`, try 1). Сравнение читает оба вида. Окно не меняется: строка темпа в нём была и верна.

## Пределы движка

Новых нет. Самотесты — в `rules/src/selftest/w2_pipe_energy.rs` (одна строка в `selftest/mod.rs`).

## Передать другим

- README «Что срезы пока не умеют» (pipe.md «Новое» 5, energy.md «Новое» 4) — интегратору.
- `rules/src/orders/ride.rs:14-15`: комментарий «Plain item-net moves … only orders' shipments are drawn» устарел (поток теперь рисует `net::flow`), и строка «Пределы движка» в `s18-last.md:25` про M5 тоже.
- `transport.rs:377`, `server.rs` (`on_level_tick`, `on_use_item_on`, `on_destroy_block`): по одной добавленной строке или блоку.

## Итог

Try 2 (снимок после bcadef4): RC 233/234, PCT 42/42, SC 105/105. Все 10 самотестов `w2_*` и `s18_placer_and_breaker_by_tier` проходят. Падает только `orders18_bread_through_the_kitchen` — он падал и в try интеграции до волны 2 (`integ-try-integration-4.log`), не моя область.
