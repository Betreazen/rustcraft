# Срез 18, межпоточный агент — план

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** дела интеграции после слияния потоков (список `docs/superpowers/plans/s18-integration.md`, разделы «Межпоточные дела», «Решения владельца», передачи потока 4, и добавки интегратора 12–16) становятся DONE или записанным пределом движка.

**Architecture:** правила — в крейте `mod/rustcraft/rules` с юнит-тестами (RED → GREEN), бандл (`mod/rustcraft/*.rs`) только связывает их с игрой; у каждой механики — самотест в `rules/src/selftest/s18_cross.rs` (новый файл этого потока, регистрируется одной строкой в `selftest/mod.rs`).

**Tech Stack:** Rust (Horizon native mod), JSON контент-мода, Python (`tools/tiers.py`, `assets/models/handbook_mc.py`), PowerShell (`joint.ps1 try`).

**Spec:** `docs/superpowers/specs/2026-11-16-handbook-parity-design.md`, мастер-план `docs/superpowers/plans/2026-11-16-handbook-parity.md`, контракт ступеней — шапка `docs/superpowers/plans/s18-machine.md`.

## Global Constraints

- Horizon pinned `7e4e421224743469035ba0c34fc21e569e64a17d`, Minecraft 26.3; Horizon не меняется.
- Игра следует справочнику (`docs/showcase/js/data.js`, `ui.js`, `scenes.js`).
- Тексты по-русски (`CONTRIBUTING.md`), ru_ru и en_us; коммиты по-английски, почта `128125070+Betreazen@users.noreply.github.com`, строка `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Не трогать: `transport.rs`, `window.rs`, `node_window*` (поток 1 «трубы» ещё работает), `monorail.rs`, `station.rs` и модели рельса (агент остатков потока 6). Файлы потока 5 (`orders.rs`, `storage.rs`, `terminal.rs`, `tablet.rs`, `farm.rs`, `butcher.rs`, `builder.rs`) свободны после слияния f0028a0.
- Проверка ветки: `pwsh -NoProfile -File "C:\Claude code projects\horizon-joint-dev\joint.ps1" try -Mod rustcraft -Path "C:\Claude code projects\rustcraft-s18\cross" -Owner s18-cross`, журнал в `scratchpad\try-cross-N.log`; тесты правил: `cargo test --release -q` в `mod/rustcraft/rules`.
- Коммиты только `git commit -- <paths>` на ветке `s18/cross`.

## Пределы и зависимости

- **Поток 1 не влит.** `rules/src/net/node_tier.rs`, `rules/src/net/node_window.rs` (`net_boost`, `LINE_MODULE`), рецепты узлов `recipe/node*.json` и окно узла есть только на ветке `s18/pipe`. Поэтому узловая часть задачи 3 и задача 5 (ускорение линии на жидкостных и энергосетях — нужно число модулей линии в узлах, его хранит окно узла потока 1) делаются после слияния `s18/pipe`. Что сделать тогда — в задачах 3b и 5.
- **Стак больше 99 в ячейке меню.** `native_menu_slot_rules` держит `max_stack` в 1–99 (`SRC/docs/native-mods.md:4617-4627`, запись потока 3). Хранилище добытчика больше 27, трюм больше 45 и стаки рюкзака до 1024 требуют своих экранов с запасом в NBT (как очередь машин потока 3, `rules/src/machines/queue.rs`).

---

## Задачи

Статусы: `[ ]` — не начато, `[x]` — сделано и закоммичено, `[~]` — ждёт слияния потока 1.

### Task 1a: заметка рецепта узла (решение владельца)

**Files:** Modify `docs/showcase/js/data.js:347` (заметка `node`), затем `py -3 assets/models/handbook_mc.py` (перепишет справочник в игре и lang).

- [x] Заметка узла: вместо `tiered('узла')` — «Это рецепт узла первой ступени. Узел ступеней 2–4 делается так же, но вместо медностали берётся сплав этой ступени (искросталь, кристаллит или звёздная сталь), а в центр сетки вместо осколка кладут узел предыдущей ступени.»
- [x] `py -3 assets/models/handbook_mc.py` (заметки рецептов в игровой справочник не выводятся — файлы игры не изменились); `cargo test --release -q` (тесты справочника); коммит `docs: the node recipe note says a higher node is made from the node below`.

### Task 1b: режимы работы насоса (решение владельца)

**Files:** Modify `mod/rustcraft/rules/src/machines/pump.rs` (тест режима), `rules/src/machines/fluid_windows.rs` (`pump::MODE_AT`, `MODE_WELL`, `MODE_BUTTONS`, `LABEL_MODE`, `ALL_DATA_SLOTS += 1`), `mod/rustcraft/fluids.rs` (`pump_numbers` берёт `WorkMode::factors`, режим в данные окна), `mod/rustcraft/machines.rs` (`on_menu_button`: `Kind::Pump` пишет режим), `mod/rustcraft/screens.rs` (`on_click` насоса: кнопка режима), `mod/rustcraft/fluid_panels.rs` (рисует переключатель, подсказка), меню насоса `data_slots`, `docs/showcase/js/ui.js:147-150` (`mode(1)` в окне насоса), самотест `pump_boost_mode`.

**Interfaces:** Produces — `fluid_windows::pump::{MODE_AT, MODE_BUTTONS, mode_button_at(at) -> Option<WorkMode>}`.

- [ ] RED: `pump.rs` тест `boost_pumps_a_bucket_in_five_seconds_for_three_times_the_energy` (режим «Форсаж»: тики/2, энергия ×3 через `WorkMode::Boost.factors()`); `fluid_windows` тест `a_pump_mode_button_is_inside_the_window_and_apart_from_the_sockets`.
- [ ] GREEN; бандл; ui.js; самотест: насос над водой, кнопка режима 2 → данные окна говорят «Форсаж» и тики ведра 100.
- [ ] Коммит `feat: the pump's work modes, boost pumps twice as fast for three times the energy`.

### Task 2: осколок кристалла во всех рецептах

**Files:** `mod/content/data/rustcraft/recipe/*.json` (все 70 с `minecraft:amethyst_shard`: в `data.js` предмета аметиста нет, ключ `shard` везде — «Осколок кристалла»), `rules/src/modules.rs:578-580` (тест рецептов модулей), самотест `recipes_take_the_crystal_shard` (сетка зарядника и модуля скорости с `rustcraft:crystal_shard` даёт предмет, с аметистом — нет).

- [ ] RED: тест в `modules.rs` ждёт `rustcraft:crystal_shard`.
- [ ] Замена во всех рецептах, GREEN, коммит `fix: every recipe takes the crystal shard, as the handbook says`.
- Узел (`recipe/node.json`) живёт на `s18/pipe`: при слиянии потока 1 заменить там тоже.

### Task 3a: шасси на `rustcraft_rules::tiers`

**Files:** Modify `rules/src/tiers.rs` (константа `CHASSIS: Family` — шасси не блок, в `FAMILIES` его нет), `rules/src/worker/chassis.rs` (`item`, `tier_of`, `swap` через `tiers::Family::item/tier_of` и `tiers::upgrade`).

- [ ] Тесты шасси остаются зелёными; новый тест `chassis_items_come_from_the_tier_family`. Коммит `refactor: the chassis tiers on rustcraft_rules::tiers`.

### Task 3b: узлы на `rustcraft_rules::tiers` — [~] после слияния `s18/pipe`

- [~] В `tiers::FAMILIES` строка `Family::new("rustcraft:node", 1)`; `node_tier.rs` — обёртки над `tiers`; `py tools/tiers.py` (не объявлять предметы узла второй раз в `content.rs`; рецепт узла выше первой ступени — узел предыдущей ступени в центре, `tools/tiers.py` должен это знать).

### Task 4: установщик и разрушитель быстрее по ступени

**Files:** Modify `mod/rustcraft/mining.rs` (`read`: `work *= tiers::speed(family, tier, step)`, энергия ×ту же скорость, как у насоса; строка темпа окна), `rules/src/tiers.rs` (тест `placer_and_breaker_speed_up_from_tier_two`), самотест `placer_tier_three_is_faster`.

- [ ] RED/GREEN, бандл, самотест, коммит `feat: the placer and the breaker work faster on a higher tier`.

### Task 5: ускорение линии на жидкостных и энергосетях — [~] после слияния `s18/pipe`

- [~] В `fluids.rs` (поток ведра по каналу) и `power.rs` (пропуск энергии канала) множитель `node_window::net_boost(housings, &topo.reach, &modules)`, модули узлов — как их читает `transport.rs:253`.

### Task 6: опыт PCT от электропечи, плавильни и автокрафтера

**Files:** Modify `rules/src/reports.rs` (`Report::Smelt { owner, item, count }` → `pct-machine smelt <владелец> <предмет> <количество>`, вид из контракта), `mod/rustcraft/machines.rs` (печь: выданный результат), `mod/rustcraft/alloy.rs` (плавильня), `mod/rustcraft/crafter.rs` (`Report::Craft`, рецепт — id рецепта страницы), самотест `machines_report_smelt_and_craft`.

- [ ] RED: тест команды `smelt`; GREEN; бандл; самотест: печь с сырым железом и энергией → в `reports()` строка `smelt … minecraft:iron_ingot 1`; автокрафтер → `craft …`. Коммит `feat: the furnace, the alloy smelter and the autocrafter report their work to Path-of-craft-tree`.

### Task 7: катализаторы в других машинах

**Files:** Modify `mod/rustcraft/machines.rs` (`takes_catalyst`: + плавильня, смеситель, насос, автокрафтер), `alloy.rs`, `crafter.rs`, `mixer.rs`, `fluids.rs` (их множители берут катализатор: `catalyst::Tank::factors`), самотест `alloy_takes_accelerator`.

- [ ] Правило: `catalyst.rs` — эффект «имеет смысл»: ускоритель и экономайзер — всем, обогатитель — тем, у кого есть продукт (не насосу). Тест `the_enricher_is_not_for_the_pump`. Коммит `feat: the alloy smelter, mixer, pump and autocrafter take catalysts`.

### Task 8: ключ поднимает работника целиком

- [x] Проверено: `workers.rs:417-421` — ключ с Shift по работнику → `pick_up` → `worker_stack` (шасси с упакованным работником, W3); самотест `worker_as_item` потока 6. DONE без кода.

### Task 9: таблицы добычи в форме 26.3

**Files:** Modify `tools/tiers.py` (`loot`: у записи `"condition": {"type": "minecraft:match_block", "blocks": <блок>, "state": {"tier": "N"}}`, форма из `SRC/crates/horizon-port-pack/src/carry.rs:1270-1276`), перегенерировать; вручную `loot_table/blocks/monorail.json`, `disintegrator.json`, `trash.json` (то, что не из `tiers.py`); самотест `tiered_blocks_drop_their_tier` (`break_block` каждого семейства на ступени 3 даёт `<блок>_3`).

- [ ] Коммит `fix: loot tables in 26.3's form, a broken machine drops its own tier`.

### Task 10: передачи потока 4

- [ ] 10a стак/хранилище добытчика больше 27, трюм больше 45, рюкзак 1024 — свои экраны с запасом в NBT (предел меню 99, см. «Пределы»).
- [ ] 10b модуль высоты в буре и окне шасси; бур без радиуса.
- [ ] 10c страницы справочника в игре `mod_stack`, `mod_yield`, `mod_line` с предметами.

### Task 11: разряженный щит не блокирует

- [ ] Сверить со справочником (`data.js` `eshield`), правило в `rules/src/gear`, бандл `gear.rs`, самотест.

### Task 12: рецепт книги рецептов `aba|tst|aba`

- [ ] `recipe/module_recipes.json` по `data.js:976`.

### Task 13: четвёртое гнездо машин крафта среза 12 на ступенях 3–4

- [ ] `Util::opening_tier`, `sockets_for_tier`, гнёзда листа `sheet.rs`.

### Task 14: модуль стака в машинах (256 в ячейке, запас в NBT как у печи)

- [ ] 

### Task 15: заказы O11 и предметы заказа в корпусах

- [ ] Планы `s18-craft-storage.md`, `s18-pipe.md`; видимость в корпусах — `transport.rs` потока 1, после его слияния.

### Task 16: добыча от стража работника как от владельца

- [ ] Удар владельца (`base.rs` `owner_shot`) для убийства стража; самотест на основе `worker_guard_kill_drops_where_the_mob_fell`.
