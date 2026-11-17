# Срез 18, волна 2, «Машины и модули» — план

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** каждая строка «Ещё не сделано» в `docs/design/06-audit/after/machine.md` и `after/module.md` (кроме строк, переданных другим агентам ниже) становится DONE; плюс новые ошибки второго аудита.

**Architecture:** правила и раскладка окон — в крейте `mod/rustcraft/rules` с юнит-тестами (`rules/src/machines/sheet.rs` получает новые части окна: ряд «ввод → стрелка хода → превью», скрытые ячейки меню, подпись ряда); бандл (`utility.rs`, `sheets.rs`, `fluids.rs`, `crafter.rs`, `mixer.rs`, `panels.rs`, `farm.rs`) только связывает их с игрой. Каждая механика получает самотест в `rules/src/selftest/w2_machine.rs`.

**Tech Stack:** Rust (Horizon native mod, API 46/71/94), JSON контент-мода.

**Spec:** `docs/superpowers/specs/2026-11-16-handbook-parity-design.md`; мастер-план `docs/superpowers/plans/2026-11-16-handbook-parity.md`; аудит `docs/design/06-audit/after/machine.md`, `after/module.md`; цель — `docs/showcase/js/ui.js`, `data.js`.

## Global Constraints

- Horizon pinned `7e4e421224743469035ba0c34fc21e569e64a17d`, Minecraft 26.3; Horizon не меняется.
- Окно помещается в 240 GUI px (`sheet::MAX_HEIGHT`, экран 1280×720 при масштабе 3).
- Тексты по-русски (`CONTRIBUTING.md`), ru_ru и en_us; коммиты по-английски, почта `128125070+Betreazen@users.noreply.github.com`, строка `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Общие файлы (`content.rs`, `server.rs`, `selftest.rs`, `rules/src/selftest/mod.rs`, `rules/src/tuning.rs`, `rules/src/modules.rs`, lang, `horizon-mixin.toml`, хуки) — только своими блоками строк.
- Быстрая проверка: `bash scratchpad/chk-w2-machine-module.sh <worktree>`; тесты правил `cargo test --release -q` в `mod/rustcraft/rules`; стенд `joint.ps1 try -Mod rustcraft -Path scratchpad/snap-w2-machine-module -Owner s18-w2-machine-module`.
- Коммиты только `git commit -- <paths>` на ветке `s18/w2-machine-module`.

## Передачи другим агентам волны 2

| Кому | Что | Готовое правило |
|---|---|---|
| w2-craft | Комбайн и варочный куб: +2 скрытые ячейки очереди за модуль ёмкости (`kitchen.rs`/`brew` в бандле — их файлы) | `rustcraft_rules::modules::capacity_slots(&slots)` (2 за модуль) и `machines::queue::{open_cells, intake, spill, front}`; задача 9 добавляет `utility::queue_cells(u, &slots) -> usize` |
| w2-craft | Забойщик: экономия замедляет удар ×0,85 (`butcher.rs` бандла) | задача 12: `rustcraft_rules::butcher::strike_ticks(base, &slots, t) -> u64` |
| w2-craft | Терминал: id модулей стака 2/3 (`rules/src/storage/modules.rs:13-17`) и предмет `module_filter` (строки `after/craft-storage.md`) | `Module::of(id).map(Module::stack_tier)` |
| w2-rail-gear | Поле высоты в окне шасси и подъём/спуск бура к ней | `modules::height_step(module, target, y)` (уже есть) |
| w2-rail-gear | Замена монорельса правым кликом (`swap`) | `rustcraft_rules::tiers` |

---

### Task 1: окна без модулей не рисуют гнёзд

**Files:** Modify `rules/src/machines/sheet.rs:143` (`Kind::shape`).
**Interfaces:** Produces: `Shape.sockets == Kind::holder().is_some_and(Holder::has_sockets)`.

- [x] Тест `blocks_without_modules_draw_no_sockets`: `Kind::Util(Util::ChargePlate|XpTank|Projector|PortalDock).shape().sockets == false`, `Kind::Util(Util::Trash).shape().sockets == true`. RED.
- [x] `let sockets = self.holder().is_some_and(|h| h.has_sockets());` GREEN; `cargo test`.
- [x] Коммит.

### Task 2: утилизатор принимает и сжигает только то, что пропускает фильтр стыка

**Files:** Modify `rules/src/machines/tiered.rs` (правило), `rules/src/utility/mod.rs` (`Util::takes` для утилизатора, слоты), `machines.rs:1112-1131` (`slot_takes`), `utility.rs` (`trash`, `joint_filter_set`), lang `rustcraft.util.trash_no_filter`. Самотест `w2_trash_takes_by_joint_filter`.
**Interfaces:**
- Produces: `tiered::trash_passes(filters: &[Filter], item: &str) -> bool` — хоть один заданный фильтр стыка (`trash_filter_set`) пропускает предмет; пустой список — ничего.
- Produces: NBT утилизатора `trash_jf` — JSON `Vec<Filter>` фильтров его стыков, пишется на каждом шаге (`slot_takes` спрашивается без мира, поэтому читает копию).
- Свои 9 ячеек фильтра больше не фильтр: окно их прячет (задача 5), предметы из них уходят в ячейки сжигания.

- [x] Тесты `trash_passes_only_what_a_set_joint_filter_lets_through` (пустой белый список не пропускает; белый список с булыжником пропускает булыжник, не землю; чёрный список с землёй пропускает булыжник) RED → GREEN.
- [x] Бандл: `trash()` собирает фильтры стыков (`joint_filters`), пишет `trash_jf`, сжигает только `trash_passes`; без фильтра `Blocked("trash_no_filter")` «Фильтр не задан»; `slot_takes` для утилизатора читает `trash_jf`.
- [x] Самотест: корпус + утилизатор, стык с белым списком «булыжник»; воронка/`native_container_accepts` берёт булыжник и отказывает земле; после шага сгорел только булыжник.
- [x] Коммит.

### Task 3: жидкость в утилизаторе — скорость, экономия, режим

**Files:** `rules/src/machines/tiered.rs` (`trash_fluid_room`), `fluids.rs:127-152` (`trash_room`), `utility.rs` (учёт сгоревших вёдер для окна).
**Interfaces:** Produces `tiered::trash_rate(tier, &slots, mode, t) -> (f64 per_second, f64 energy_per_unit)`: 8/16/32/64 × 2^скорость × режим; энергия 1 Е × 0,6^экономия × режим. Одна функция для предметов и жидкости.

- [x] Тест `trash_rate_follows_tier_modules_and_mode` RED → GREEN.
- [x] `trash_room` и `trash()` берут её; `note_fluid_burnt(dim,pos,fluid,mb)` для окна.
- [x] Коммит.

### Task 4: новые части окна-листа

**Files:** `rules/src/machines/sheet.rs` (Shape, Layout, Sheet), `sheets.rs` (рисует).
**Interfaces (Produces):**
- `Shape.lead: usize` — первые `lead` показанных слотов идут рядом от `LEFT`; `Shape.work: bool` — стрелка хода после них; `Shape.preview: usize` — столько призрачных ячеек превью после стрелки; `Shape.label: bool` — строка подписи над рядом.
- `Shape.shown: Option<(usize, usize)>` — диапазон слотов меню, которые видны (остальные скрыты: `Layout.slot_at: Vec<Option<[f32;2]>>` длиной `Shape.slots`).
- `Sheet.work: Option<f64>`, `Sheet.preview: Vec<(String, String)>` (предмет, число), `Sheet.label: Option<Value>`.
- Все окна по-прежнему ≤ 240 px, части не перекрываются (тест `every_window_fits…` расширен на Util-окна).

- [x] Тесты раскладки: стрелка после ряда, превью после стрелки, скрытые слоты `None`, высота ≤ 240 у всех `Kind::Util`.
- [x] `sheets.rs`: `layout()` отдаёт `None` для скрытых, рисует стрелку `kit::arrow`, превью `native_item` + число, подпись.
- [x] Коммит.

### Task 5: окно утилизатора как `ui.js:314-325`

**Files:** `rules/src/machines/sheet.rs` (форма), `utility.rs` (`sheet_extras` Trash), lang.
- Строки: «Фильтр стыка: белый список: булыжник, семена» (имена предметов на языке игрока — `Text::translate_with_fallback`), «Приоритет: самый низкий» (−9 = `-PRIORITY_LIMIT` → «самый низкий»), «Сгорело за минуту: N предметов · M вёдер», «Пропускная способность: X из Y в секунду» (по ступени и модулям, задача 3).
- Полосы: «Жидкость: вода · 2 ведра в секунду», энергия. Ряд «Сгорает сейчас»: 9 ячеек сжигания + стрелка хода порции. Ячейки фильтра скрыты.
- Предел 240 px: пропускная способность — строка, а подпись ряда «Сгорает сейчас» — в подсказке ряда (записать в README «что проверить»).
- [x] Тест формы утилизатора; самотест окна (`sheet` содержит строки «самый низкий» и имя предмета). Коммит.

### Task 6: окно дезинтегратора как `ui.js:326-338`

**Files:** `rules/src/utility/disassembly.rs` (`preview`, `amount_text`), `utility.rs` (`sheet_extras` Disintegrator), lang.
**Interfaces:** `disassembly::preview(recipe, percent, durability) -> Vec<(String, u32 /*тысячные*/)>`; `disassembly::amount_text(thousandths) -> String` («1½», «½», «⅓», «2»).
- Ряд: ввод → стрелка хода → превью «Разборка» (что выйдет, дроби). Полоса «Прочность: железная кирка · целая / 62%». Строки: «Доля возврата 60% · чары теряются» (подсказка: модули выхода), «Накоплено: железный слиток ½». «Готовое некуда отдать» вместо «Слоты готового заполнены».
- [x] Тесты `preview_is_half_of_each_ingredient`, `amount_text_whole_and_fractions`. Коммит.

### Task 7: окно зарядника как `ui.js:171-175`

**Files:** `rules/src/machines/sheet.rs` (форма: без режима, 4 видимых слота «Броня», 4..8 скрыты), `rules/src/utility/mod.rs`, `utility.rs` (`charge_slots`).
- Заряженное уходит в скрытые ячейки выдачи только когда рядом корпус магистрали (её забирают); без корпуса — остаётся в «Броне», а скрытое возвращается в свободные ячейки.
- Имя предмета на языке игрока («Заряд: Нагрудник · ступень 3»).
- [x] Тест формы; правило `charging::to_output(full, housing_beside) -> bool`. Коммит.

### Task 8: пульсирующее свечение зарядника и плиты

**Files:** `rules/src/machines/charging.rs` (`GLOW_*` коробки катушек из моделей, `pulse(t) -> f32`), новый `glow` в `utility.rs` (клиент), хук `hook_block_entity_submit.rs` (одна строка).
- Пока `lit=true`, каждый кадр поверх катушек рисуются кубы чуть больше, полной яркости (`draw.light = 0xF0F0`), цвет пульсирует (период 1,5 с); без `keep()`.
- Пределы: `native_emissive` статичен (пара спрайтов на кадр, без цвета по блоку) — поэтому рисунок блок-энтити (API 71).
- [x] Тесты `pulse_stays_in_range_and_repeats`, коробки внутри блока. Коммит.

### Task 9: смеситель — стрелка хода; ёмкость и стак правилами

**Files:** `rules/src/machines/sheet.rs` (Mixer: `lead 3`, `work`), `mixer.rs` (строка хода → `Sheet.work`; стак 256 на каждый ввод через `queue::stash`), `crafter.rs` (стак 256 на каждый из 9 вводов через `queue::stash`), `rules/src/utility/mod.rs` (`queue_cells` для w2-craft).
- [x] Тест формы смесителя; тест `stash_gives_each_input_its_cell` (есть в queue.rs — проверить для 9 вводов). Самотест: автокрафтер со стаком держит больше 64 железа на ввод. Коммит.

### Task 10: вкладки автокрафтера «1 · Бумага»

**Files:** `rules/src/machines/windows.rs` (`crafter::tab(i, pages)` делит 121 px поровну), `panels.rs` (`tab`: «N · имя» через `kit::fit`, при узкой вкладке — номер и значок), `crafter.rs` (клик).
- [x] Тест `tabs_share_the_row_and_do_not_overlap_the_sockets`. Коммит.

### Task 11: тексты состояния и подсказка закрытого гнезда

**Files:** lang (`rustcraft.status.output_full` → «Готовое некуда отдать», `trash_no_filter` → «Фильтр не задан»), `screens.rs:893-899`, `panels.rs:461` (закрытое гнездо: «Гнездо откроется на ступени 3»).
- [x] Тест правила `sockets::locked_until(tier) -> Option<u8>`. Коммит.

### Task 12: модули — рыболовная станция, забойщик, турель

**Files:** `rules/src/utility/farm.rs` (`fish_buffer(storage) = 9 << storage`, до 72), `farm.rs` (модуль считается один раз; сверх 18 видимых — скрытый запас как у добытчика), `rules/src/butcher.rs` (`strike_ticks`), `rules/src/turret.rs` (`support_draw_for(t, healing)` — энергия лечения только когда есть раненый), `base.rs` (одна строка вызова).
- [x] Тесты `fish_buffer_doubles_each_module`, `economy_slows_the_strike`, `heal_draws_only_while_someone_is_hurt`. Коммит.

### Task 13: пласт — таблица находок в окне жилы

**Files:** `rules/src/utility/vein.rs` (`STRATUM_TABLES`, выбор), `rules/src/utility/grown.rs` (имя в трюме несёт таблицу), `utility.rs` (окно жилы, рост ставит `LootTable` блок-энтити, поднятый блок хранит таблицу, стол чистки берёт её).
- [x] Тесты `hold_name_keeps_the_table`, `the_chosen_table_is_the_grown_blocks`. Коммит.

### Task 14: установщик и разрушитель — справочник по окну

**Files:** `docs/showcase/js/ui.js` (`simple()`: две строки «что делает», «темп» и `mode(1)`), `data.js` (`MODES` в stats установщика и разрушителя).
- [x] Коммит.

### Task 15: стенд и аудит

- [x] `joint.ps1 try` после задач 1–5, 6–9, 10–15.
- [x] Таблицы `after/machine.md`, `after/module.md`: отметить сделанное.

## Пределы движка

- Ячейка меню держит 1–99 (`SRC/docs/native-mods.md:4617-4627`): стак 256 и буфер станции сверх окна — скрытые ячейки в NBT (как у печи и добытчика).
- 240 px высоты окна: у утилизатора пропускная способность — строка, подпись ряда «Сгорает сейчас» — в подсказке.
- Пульс свечения: `native_emissive` (`SRC/docs/native-mods.md:411-416`) даёт только постоянное полнояркое наложение спрайта для всех блоков с ним, без пульса и без выбора блока; пульс нарисован блок-энтити (API 71, `native_block_entity_draw.rs:48-56`: `light` задаётся на часть).

## Итог волны 2

Коммиты `2135414`…(последний) на `s18/w2-machine-module`. Тесты правил: 722 (было 705). Самотесты: новые `w2_trash_takes_by_joint_filter`, `w2_disintegrator_preview`, `w2_charger_keeps_charged_gear`; переписан `util_trash_burns_its_filter` (фильтр теперь на стыке), расширен `module_stratum_grows_the_chosen_rock` (выбор таблицы), исправлена проверка `s18_placer_and_breaker_by_tier` (целые секунды приходят числом JSON).

Ещё передачи (сверх таблицы выше):

| Кому | Что |
|---|---|
| w2-rail-gear | `workers.rs:833` (`take_grown`): `grown::hold_name(&block, true)` → `util::grown_hold_name(game, p, &block)`, чтобы хватка везла таблицу находок пласта; `drop_stack` там же может взять `grown::from_hold_table` и записать `grown::TABLE_KEY`. Без этого таблица едет с блоком только через ключ (подъём) и в мире |
| w2-craft | `butcher.rs:143` бьёт по общему `t.butcher_ticks`; с экономией шаг каждого забойщика — `rules::strike_ticks(t.butcher_ticks, economy, t.economy_work)`, цена удара прежняя `strike_energy(t.butcher_ticks, economy)` |
| w2-craft | комбайн и куб: число скрытых ячеек очереди — `rustcraft_rules::utility::queue_cells(u, &modules)` |

Что смотреть ботам: окно утилизатора (4 строки, полоса жидкости, 9 ячеек «Сгорает сейчас» и стрелка; подсказка ряда), воронка с землёй над утилизатором с фильтром «булыжник» — земля не входит; окно дезинтегратора с железной киркой («1½» слитка, палка, полоса прочности); окно зарядника (4 ячейки, без режима, заряженное остаётся без корпуса); пульс катушек зарядника и плиты при зарядке; стрелка смесителя; вкладки автокрафтера «1 · Бумага» с 1–2 страницами; подсказка закрытого 4-го гнезда плавильни; окно жилы с пластом (2 + 5 переключателей); рыболовная станция с 2 модулями хранилища (строка «ещё N сверх окна»).
