# Срез 18, поток 3 «Машины» — план

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** каждая строка MISSING и DIFFERS таблицы `docs/design/06-audit/machine.md` становится DONE или записанным пределом движка; ступени машин и замена правым кликом (U2) — общий механизм для потоков 1 и 6.

**Architecture:** правила — в крейте `mod/rustcraft/rules` (новый модуль `tiers`, новые модули в `rules/src/machines/`), с юнит-тестами; бандл (`mod/rustcraft/*.rs`) только связывает их с игрой. Ступень машины — свойство состояния блока `tier` (как у узла и монорельса): замена U2 меняет только это свойство (`native_set_block_property`, блок-энтити остаётся, SRC `native_hooks.rs:10620-10627`), поэтому предметы, жидкости, модули и настройки сохраняются, а магистраль не перестраивается. Модули машин без листа (печь, дробилка, плавильня, насос) хранятся в NBT `modules`, как у листов (`base.rs:201`), гнёзда нажимаются кнопкой окна с курсором.

**Tech Stack:** Rust (Horizon native mod), JSON контент-мода, Python (генератор `tools/tiers.py`), PowerShell (`joint.ps1 try`).

**Spec:** `docs/superpowers/specs/2026-11-16-handbook-parity-design.md`, мастер-план `docs/superpowers/plans/2026-11-16-handbook-parity.md`.

## Global Constraints

- Horizon pinned `7e4e421224743469035ba0c34fc21e569e64a17d`, Minecraft 26.3; Horizon не меняется.
- Игра следует справочнику (`docs/showcase/js/data.js`, `ui.js`, `scenes.js`).
- Тексты по-русски (`CONTRIBUTING.md`), ru_ru и en_us; коммиты по-английски, почта `128125070+Betreazen@users.noreply.github.com`, строка `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Общие файлы (`content.rs`, `server.rs`, `selftest.rs`, `rules/src/selftest/mod.rs`, `rules/src/tuning.rs`, `rules/src/modules.rs`, lang, `horizon-mixin.toml`, файлы окон) — только своими блоками строк.
- Проверка ветки: `pwsh -NoProfile -File "C:\Claude code projects\horizon-joint-dev\joint.ps1" try -Mod rustcraft -Path "C:\Claude code projects\rustcraft-s18\machine" -Owner s18-machine`; тесты правил: `cargo test --release -q` в `mod/rustcraft/rules`.
- Коммиты только `git commit -- <paths>` на ветке `s18/machine`.

---

## Контракт для потоков 1 и 6

Модуль правил `rustcraft_rules::tiers` (`mod/rustcraft/rules/src/tiers.rs`):

| Имя | Что делает |
|---|---|
| `tiers::PROPERTY = "tier"` | свойство состояния блока со ступенью, значения `"1"`…`"4"` |
| `tiers::MAX_TIER = 4`, `tiers::ALLOYS: [&str; 4]` | сплав ступени: медносталь, искросталь, кристаллит, звёздная сталь |
| `struct Family { block: &'static str, first: u8 }`, `Family::new(block, first)` | семейство ступеней: блок и ступень, с которой он открывается |
| `Family::item(self, tier) -> Option<String>` | предмет ступени: на первой ступени — сам блок (`rustcraft:furnace`), выше — `<блок>_<n>` (`rustcraft:furnace_2`) |
| `Family::tier_of(self, item) -> Option<u8>` | ступень, которую ставит предмет |
| `Family::tier_from(self, value: Option<&str>) -> u8` | ступень из значения свойства, вне диапазона — первая |
| `tiers::FAMILIES: &[Family]` | все семейства-блоки; **поток 1 добавляет сюда строку узла** (`Family::new("rustcraft:node", 1)`) |
| `tiers::family_of_block(block) -> Option<Family>`, `tiers::family_of_item(item) -> Option<(Family, u8)>` | поиск по `FAMILIES` |
| `tiers::upgrade(placed: u8, held: u8) -> bool` | U2: замена только младшей ступени старшей (для шасси потока 6 — без блока) |
| `tiers::swap(held_item, block, placed_tier) -> Swap` | U2 для блока: `Swap::Up { to, give_back }` или отказ (`Swap::NotHigher`, `Swap::Other`) |
| `tiers::sockets(tier) -> usize`, `tiers::MAX_SOCKETS = 4` | гнёзда модулей по ступеням 3 / 3 / 4 / 4 (A5, `SLOTS_BY_TIER`) |
| `tiers::speed(family, tier, step) -> f64` | множитель скорости машины выше её первой ступени: `1 + step·(tier − first)`; `step` — `Tuning::tier_speed_step` |

Сторона бандла (`mod/rustcraft/tiers.rs`, модуль `rustcraft_tiers`):

| Имя | Что делает |
|---|---|
| `state_for_placement(level, state, context, answer)` | любой блок из `FAMILIES` ставится со ступенью предмета, которым его ставят |
| `on_right_click_block(game, pos, canceled)` | U2: предмет старшей ступени в руке по блоку младшей → замена; старая машина в руку |
| `swap_in_place(game, dimension, pos, item) -> Result<String, &'static str>` | сама замена (и для самотеста): ступень в состоянии блока, ответ — предмет старой ступени |
| `tier_at(game, pos) -> u8` | ступень блока на сервере и на клиенте (окно берёт заголовок отсюда) |

Узлу потока 1 достаточно добавить свою строку в `FAMILIES` (предметы ступеней объявляет `content.rs::declare_tier_items` для всех семейств — не объявлять их второй раз), рецепты, лут и модели — `py tools/tiers.py`: размещение и замена заработают сами. Шасси потока 6 — сущность, оно пользуется только `upgrade` и `Family::item/tier_of`.

## Пределы движка

- **Стак 256 в ячейке меню.** `native_menu_slot_rules` держит `max_stack` в 1–99, и размер стака предмета его тоже ограничивает (`SRC/docs/native-mods.md:4617-4627`). Ближайший вариант: ячейка машины — видимая ячейка меню плюс запас в NBT (`rules/src/machines/queue.rs`), машина считает их вместе до 256, окно рисует общее число.
- **Ячейки очереди модуля ёмкости как настоящие ячейки меню.** Правила ячеек меню спрашиваются только по стаку, без блок-энтити (`native-mods.md:4622-4624`), а число ячеек контейнера задано блоком. Поэтому ячейки очереди — те же запасы в NBT, нарисованные в окне; приток идёт через ячейку ввода.

---

## Задачи

Статусы: `[ ]` — не начато, `[x]` — сделано и закоммичено.

### Task 1: правила ступеней `rustcraft_rules::tiers`

**Files:** Create `mod/rustcraft/rules/src/tiers.rs`; Modify `mod/rustcraft/rules/src/lib.rs` (строка `pub mod tiers;`), `mod/rustcraft/rules/src/tuning.rs` (свой блок: `tier_speed_step: 0.5`).

**Interfaces:** Produces — всё из таблицы контракта выше.

- [x] Тесты (RED): `family_items_by_tier` (furnace: 1→`rustcraft:furnace`, 2→`rustcraft:furnace_2`, 5→None; crafter first 2: 1→None, 2→`rustcraft:crafter`, 3→`rustcraft:crafter_3`), `tier_of_round_trips`, `tier_from_clamps`, `swap_only_upward` (furnace_3 по furnace tier 1 → Up{to:3, give_back:"rustcraft:furnace"}; furnace_2 по tier 2 → NotHigher; crusher_2 по furnace → Other), `sockets_three_three_four_four`, `speed_grows_from_first_tier`, `every_family_block_is_unique`.
- [x] Реализация, GREEN, коммит `feat(rules): machine tiers and the U2 swap rule`.

### Task 2: ступени в игре — блоки, предметы, рецепты, лут, размещение, заголовок окна

**Files:** Create `mod/rustcraft/tiers.rs`, `mod/rustcraft/hook_*` не нужно (хуки уже есть); `tools/tiers.py` (генератор); Modify `mod/rustcraft/content.rs` (свойство `tier` у блоков семейств, предметы ступеней), `mod/rustcraft/horizon-mixin.toml` (модуль `tiers`), `mod/rustcraft/hook_state_for_placement.rs` (+1 строка), `mod/rustcraft/screens.rs` (`title` берёт `tier_at`), `mod/rustcraft/sheets.rs` (заголовок листа «· ступень N» для семейств); контент: `data/rustcraft/recipe/<блок>_<n>.json`, `loot_table/blocks/<блок>.json` (по `block_state_property`), `assets/rustcraft/items/<блок>_<n>.json`, lang (свой блок: «Электропечь · ступень 2»).

- [x] Генератор `tools/tiers.py`: для каждого семейства берёт рецепт первой ступени, заменяет сплав первой ступени на сплав ступени n, пишет рецепт, предмет-модель (модель блока), лут-таблицу с альтернативами по `tier`, строки lang.
- [x] `content.rs`: у блоков семейств `state_properties` `tier` (значения 1..4, по умолчанию первая ступень); предметы ступеней `place_block` = блок.
- [x] `tiers.rs::state_for_placement`, `tier_at`; заголовок окна печи/дробилки/плавильни/автокрафтера/насоса/листов — из блока.
- [x] Тест правил генератора не нужен (данные), но `cargo test` зелёный; коммит `feat: machine tiers 2-4 - items, recipes, loot, tier in the window title`.

### Task 3: замена правым кликом (U2) и самотест

**Files:** Modify `mod/rustcraft/tiers.rs` (`on_right_click_block`, `swap_in_place`), `mod/rustcraft/server.rs` (+1 строка в `on_right_click_block`), `mod/rustcraft/selftest.rs` (метод скамьи `swap_tier`), `mod/rustcraft/rules/src/selftest/mod.rs` (метод трейта с реализацией по умолчанию, `all.extend(slice18_machine::ALL)`), Create `mod/rustcraft/rules/src/selftest/slice18_machine.rs`; lang (`rustcraft.tier.swapped`).

- [x] Самотест `tier_swap_keeps_contents`: печь ступени 1 с 5 сырого железа во вводе и режимом «форсаж» → `swap_tier(pos, "rustcraft:furnace_3")` → свойство `tier` = 3, во вводе те же 5, режим тот же, ответ — `rustcraft:furnace`; `swap_tier(pos, "rustcraft:furnace_2")` → отказ.
- [x] Коммит `feat: replace a machine by a higher tier with a right click (U2)`.

### Task 4: скорость и гнёзда по ступеням в машинах потока

**Files:** `machines.rs` (процессор), `alloy.rs`, `crafter.rs`, `mixer.rs`, `fluids.rs` (насос), `utility.rs` (утилизатор 8/16/32/64, дезинтегратор 5/3 с, зарядник); `base.rs` — только чтение числа гнёзд через `tiers::sockets` (одна строка, отмечено в отчёте); Create `rules/src/machines/tiered.rs` (пропускная способность утилизатора, время дезинтегратора по ступени) с тестами.

- [ ] Тесты: `trash_rate_by_tier` (8,16,32,64), `disintegrator_ticks_by_tier` (3→100, 4→60), `speed_module_doubles_trash_once` (1 модуль → ×2, не ×4).
- [ ] Коммит.

### Task 5: модули в печи, дробилке, плавильне, насосе (гнёзда в окне, эффекты)

**Files:** Create `rules/src/machines/sockets.rs` (сделано: 464e7f8; `queue.rs`, `extras.rs`, раскладка окна процессора — b95cef6; модули потока 4 взяты 1:1 — c9e2ce1) (общая логика гнёзд машин без листа: чтение, нажатие, число по ступени), `rules/src/machines/queue.rs` (очередь ёмкости и запас стака); Modify `rules/src/modules.rs` (свой блок: `Holder::Processor`, `Holder::Alloy`, `Holder::Pump`, `Holder::Crafter`; модули `Capacity`, `Stack`, `Yield`, если поток 4 их ещё не завёл), `machines.rs`, `alloy.rs`, `fluids.rs`, `screens.rs`, `panels.rs`, `fluid_panels.rs`, lang.

- [ ] Тесты: гнёзда принимают только свои модули и не больше `sockets(tier)`; скорость/экономия меняют факторы процессора; выход — шанс 15% лишнего продукта; ёмкость — +2 ячейки очереди на модуль; стак — ячейка 256.
- [ ] Самотест: печь с модулем скорости плавит вдвое быстрее; дробилка с выходом даёт иногда третью пыль (правило), ёмкость — очередь принимает больше 64.
- [ ] Коммит(ы).

### Task 6: автокрафтер — ёмкость +2 ячейки, скорость, экономия, стак

**Files:** `crafter.rs`, `rules/src/machines/crafter.rs`, `machines.rs` (правила ячеек модулей), `panels.rs`.

- [ ] Тесты: ёмкость → страница и +2 ячейки буфера; скорость/экономия → факторы.
- [ ] Коммит.

### Task 7: смеситель — ёмкость и стак

**Files:** `mixer.rs`, `rules/src/machines/mixer.rs`, `rules/src/modules.rs` (строка смесителя), `tuning.rs` (баки смесителя 16 вёдер по справочнику `ui.js:164-170`).

- [ ] Тесты, самотест, коммит.

### Task 8: ведро в бак и из бака

**Files:** `fluids.rs` (обработчик `on_use_item_on` для бака), `server.rs` (+1 строка), Create `rules/src/machines/bucket.rs` (правило: вода/лава в бак, набрать воду/лаву, раствор и катализаторы ведром не набрать).

- [ ] Тесты правил, самотест `bucket_fills_and_empties_a_tank` (метод скамьи `use_bucket`), коммит.

### Task 9: утилизатор — приоритет, жидкости, фильтр стыка, режим

**Files:** `utility.rs`, `rules/src/net/settings.rs` — нет, это поток 1: приоритет по умолчанию задаётся при первой записи настроек стыка утилизатора из `utility.rs` (своя функция); жидкости — `fluids.rs` (утилизатор как приёмник жидкостной сети); фильтр — фильтр стыка (ключ) вместо 9 своих ячеек; режим — `rules/src/machines/sheet.rs` (`mode: true` для машин среза 12).

- [ ] Тесты, самотест, коммит.

### Task 10: зарядная плита и зарядник

**Files:** `utility.rs`, `content.rs` (форма плиты, `occludes: false`), Create `rules/src/machines/charging.rs` (128 Е/с на игрока, диагноз пустого буфера, скорость/экономия зарядника).

- [ ] Тесты: `plate_shares_per_player`, `empty_plate_says_no_energy`, `charger_speed_module_charges_faster`; самотест; коммит.

### Task 11: плавильня — пустое ведро от лавы; дробилка — блоки руды

**Files:** `alloy.rs` (остаток топлива в ячейку готового или из машины в магистраль), рецепты `crushing_iron_ore.json` и т. д. (`iron_ore`, `deepslate_iron_ore`, медь, золото; `nether_gold_ore` — нет, не по тексту).

- [ ] Тест, самотест, коммит.

### Task 12: окна по `ui.js`

**Files:** `rules/src/machines/panel.rs`, `panels.rs`, `screens.rs`, `fluid_panels.rs`, `utility.rs` (листы утилизатора, дезинтегратора, зарядника), lang.

- [ ] Печь/дробилка (подпись «Ход работы», гнёзда под энергией), плавильня, автокрафтер (подпись «Режим работы», вкладки «1 · Бумага»), смеситель (стрелка хода), утилизатор (фильтр, приоритет, сгорело за минуту, пропускная способность), дезинтегратор (превью, прочность, доля, накопленное), зарядник (4 слота брони, полоса заряда).
- [ ] Коммиты по окнам.

### Task 13: полоса ступени на модели, свечение зарядника и плиты, README

**Files:** blockstates семейств (multipart: полоса цвета сплава у ступеней 2–4), `rules/src/anim/parts.rs` — только если движок даёт свечение; `mod/README.md` (раздел «Что проверить в игре: срез 18 — машины»).

- [ ] Коммит.

### Проверки

- [ ] `try` после задач 3, 5, 8, 11, 13 — все самотесты RC, PCT, SC зелёные.

## Ход работ (обновляется после каждого коммита)

- [x] T1 правила ступеней — 057edf9
- [x] T2 ступени в игре — d25cd8f
- [x] T3 U2 и самотест — b374e26
- [x] T5a печь и дробилка: гнёзда по ступени, скорость/экономия, выход, очередь ёмкости, запас стака, окно по `ui.js` в 240 px — e5aa287, 5e62954
- [x] T5b плавильня: то же; T11a пустое ведро от лавы уходит в ячейки готового — e032f86, d452b3b
- [x] T5c насос: гнёзда, скорость/экономия, ступень — 09f851d
- [x] T6 автокрафтер: гнёзда, скорость/экономия/ёмкость/стак, очередь, ступень — e19fa3f
- [x] T7 смеситель: ёмкость, стак, ступень, баки по 16 вёдер — 0f70c6c
- [ ] T4 скорость по ступеням: утилизатор 8/16/32/64, дезинтегратор 5/3 с, зарядник
- [ ] сборка `try` после T5 (первая сборка ждала обслуживание стенда)
