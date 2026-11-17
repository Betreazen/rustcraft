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

- **Поток 1 был не влит** (до d7913ac; теперь влит). `rules/src/net/node_tier.rs`, `rules/src/net/node_window.rs` (`net_boost`, `LINE_MODULE`), рецепты узлов `recipe/node*.json` и окно узла есть только на ветке `s18/pipe`. Поэтому узловая часть задачи 3 и задача 5 (ускорение линии на жидкостных и энергосетях — нужно число модулей линии в узлах, его хранит окно узла потока 1) делаются после слияния `s18/pipe`. Что сделать тогда — в задачах 3b и 5.
- **Стак больше 99 в ячейке меню.** `native_menu_slot_rules` держит `max_stack` в 1–99 (`SRC/docs/native-mods.md:4617-4627`, запись потока 3). Хранилище добытчика больше 27, трюм больше 45 и стаки рюкзака до 1024 требуют своих экранов с запасом в NBT (как очередь машин потока 3, `rules/src/machines/queue.rs`).

---

## Задачи

Статусы: `[ ]` — не начато, `[x]` — сделано и закоммичено.

### Task 1a: заметка рецепта узла (решение владельца)

**Files:** Modify `docs/showcase/js/data.js:347` (заметка `node`), затем `py -3 assets/models/handbook_mc.py` (перепишет справочник в игре и lang).

- [x] Заметка узла: вместо `tiered('узла')` — «Это рецепт узла первой ступени. Узел ступеней 2–4 делается так же, но вместо медностали берётся сплав этой ступени (искросталь, кристаллит или звёздная сталь), а в центр сетки вместо осколка кладут узел предыдущей ступени.»
- [x] `py -3 assets/models/handbook_mc.py` (заметки рецептов в игровой справочник не выводятся — файлы игры не изменились); `cargo test --release -q` (тесты справочника); коммит `docs: the node recipe note says a higher node is made from the node below`.

### Task 1b: режимы работы насоса (решение владельца)

**Files:** Modify `mod/rustcraft/rules/src/machines/pump.rs` (тест режима), `rules/src/machines/fluid_windows.rs` (`pump::MODE_AT`, `MODE_WELL`, `MODE_BUTTONS`, `LABEL_MODE`, `ALL_DATA_SLOTS += 1`), `mod/rustcraft/fluids.rs` (`pump_numbers` берёт `WorkMode::factors`, режим в данные окна), `mod/rustcraft/machines.rs` (`on_menu_button`: `Kind::Pump` пишет режим), `mod/rustcraft/screens.rs` (`on_click` насоса: кнопка режима), `mod/rustcraft/fluid_panels.rs` (рисует переключатель, подсказка), меню насоса `data_slots`, `docs/showcase/js/ui.js:147-150` (`mode(1)` в окне насоса), самотест `pump_boost_mode`.

**Interfaces:** Produces — `fluid_windows::pump::{MODE_AT, MODE_BUTTONS, mode_button_at(at) -> Option<WorkMode>}`.

- [x] RED: `pump.rs` тест `boost_pumps_a_bucket_in_five_seconds_for_three_times_the_energy` (режим «Форсаж»: тики/2, энергия ×3 через `WorkMode::Boost.factors()`); `fluid_windows` тест `a_pump_mode_button_is_inside_the_window_and_apart_from_the_sockets`.
- [x] GREEN; бандл; ui.js; самотест `s18_pump_boost_mode` (`selftest/s18_cross.rs`): насос над лавой, кнопка режима 2 → в NBT `work`=2, окно говорит «Форсаж», тики ведра 100, просит 12 Е/с. Ячейка режима в данных окна — код+1 (0 = ещё не прислано = обычный). Скамье добавлен `menu_data` (свой блок в `selftest/mod.rs`), `long` читает и байт.
- [x] Коммит `feat: the pump's work modes, boost pumps twice as fast for three times the energy`.

### Task 2: осколок кристалла во всех рецептах

**Files:** `mod/content/data/rustcraft/recipe/*.json` (все 70 с `minecraft:amethyst_shard`: в `data.js` предмета аметиста нет, ключ `shard` везде — «Осколок кристалла»), `rules/src/modules.rs:578-580` (тест рецептов модулей), самотест `recipes_take_the_crystal_shard` (сетка зарядника и модуля скорости с `rustcraft:crystal_shard` даёт предмет, с аметистом — нет).

- [x] RED: тест в `modules.rs` ждёт `rustcraft:crystal_shard`; новый тест `no_recipe_takes_the_amethyst_shard` обходит все рецепты контента (самотест не нужен: рецепты читает тест правил).
- [x] Замена во всех рецептах, GREEN, коммит `fix: every recipe takes the crystal shard, as the handbook says`.
- Узел (`recipe/node.json`) живёт на `s18/pipe`: при слиянии потока 1 заменить там тоже.

### Task 3a: шасси на `rustcraft_rules::tiers`

**Files:** Modify `rules/src/tiers.rs` (константа `CHASSIS: Family` — шасси не блок, в `FAMILIES` его нет), `rules/src/worker/chassis.rs` (`item`, `tier_of`, `swap` через `tiers::Family::item/tier_of` и `tiers::upgrade`).

- [x] Тесты шасси остаются зелёными; новые тесты `chassis_items_come_from_the_tier_family`, `the_chassis_family_stops_at_its_third_tier`. У `Family` поле `last` и `Family::up_to(block, first, last)` (`new` — до `MAX_TIER`, `tools/tiers.py` читает только `Family::new`). Коммит `refactor: the chassis tiers on rustcraft_rules::tiers`.

### Task 3b: узлы на `rustcraft_rules::tiers`

- [x] Поток 1 влит (d7913ac). `tiers::NODE = Family::new("rustcraft:node", 1)` — константа, как `CHASSIS`, **не** строка `FAMILIES`: у узла своё размещение и замена (`pipe18.rs`), а рецепт ступеней 2–4 — узел предыдущей ступени в центре (решение владельца), не замена сплава `tools/tiers.py`; в `FAMILIES` его взяли бы `rustcraft_tiers` и `tiers.py` второй раз. `node_tier::{tier_of_item, item_of_tier, swap}` — через `NODE` и `tiers::upgrade`; тест `node_items_come_from_the_tier_family`. Коммит `refactor: the node tiers on rustcraft_rules::tiers`.

### Task 4: установщик и разрушитель быстрее по ступени

**Files:** Modify `mod/rustcraft/mining.rs` (`read`: `work *= tiers::speed(family, tier, step)`, энергия ×ту же скорость, как у насоса; строка темпа окна), `rules/src/tiers.rs` (тест `placer_and_breaker_speed_up_from_tier_two`), самотест `placer_tier_three_is_faster`.

- [x] RED/GREEN (`placer_and_breaker_speed_up_from_tier_two`), бандл (`mining::tier_speed`: работа и расход ×скорость ступени; строка темпа окна считает ступень, режим и модули), самотест `s18_placer_and_breaker_by_tier` (темп в окне на ступенях 2/4 и 2/3), коммит.

### Task 5: ускорение линии на жидкостных и энергосетях

- [x] `fluids.rs::rebuild`: `buckets_per_second ×= node_window::net_boost(housings, &topology.reach, &boosts)`, модули узлов — `rustcraft_node_window::node_modules`, как `transport.rs`; `machines::circuits` берёт `boosts` (новый параметр, вызов в `server.rs` — своя строка), `cap ×= net_boost`. Нажатие гнезда узла уже помечает уровень устаревшим (`mark_level_stale`), сети перестраиваются. Самотест `s18_line_boost_on_a_fluid_net` (поток бак→бак за 100 тиков до и после модуля, ×1,5 ± 0,25). Энергия — тем же множителем, отдельного самотеста нет (правило `net_boost` проверено потоком 1).

### Task 6: опыт PCT от электропечи, плавильни и автокрафтера

**Files:** Modify `rules/src/reports.rs` (`Report::Smelt { owner, item, count }` → `pct-machine smelt <владелец> <предмет> <количество>`, вид из контракта), `mod/rustcraft/machines.rs` (печь: выданный результат), `mod/rustcraft/alloy.rs` (плавильня), `mod/rustcraft/crafter.rs` (`Report::Craft`, рецепт — id рецепта страницы), самотест `machines_report_smelt_and_craft`.

- [x] RED: тесты `a_smelt_report_is_the_contracts`, `gained_counts_only_what_came_out` (`reports::gained` — прирост ячеек готового по предметам); GREEN; бандл: печь (`machines::report_smelt`, дробилка не шлёт — контракт), плавильня (`alloy.rs`, вид `smelt` есть в контракте, PCT за сплавы опыта не даёт, но отчёт принимает), автокрафтер (`crafter::report_craft`: id рецепта игры по образцу страницы, результат, количество, 1 крафт); самотест `s18_machines_report_smelt_and_craft`. Подходящие виды нашлись в контракте, PCT не менялся.

### Task 7: катализаторы в других машинах

- [x] Правило `catalyst::TAKERS` (печь, дробилка, плавильня, автокрафтер, насос) и `Catalyst::acts_in(block)`: обогатитель — только печи, дробилке и плавильне (насос не делает предмета; у автокрафтера рецепты обратимы — блок железа и девять слитков, лишний продукт делал бы предметы из ничего). Смеситель катализаторов не берёт: он их делает и держит свои баки воды и продукта. Тест `every_machine_with_an_effect_takes_a_catalyst_but_the_enricher_skips_the_pump`.
- [x] Бандл: `machines::{takes_catalyst, catalyst_factors, catalyst_lucky}`; плавильня (`alloy::spec` и шанс, тратит и на топливе), автокрафтер (`crafter::spec`), насос (`pump_numbers`); каждый тратит катализатор, пока работает, и пишет бак в NBT. `fluids.rs`: катализаторы ищутся у всех `takes_catalyst`, а в бак катализатора идёт только катализатор (свой отказанный ковш насоса возвращается в насос). Самотест `s18_catalysts_in_more_machines`.

### Task 8: ключ поднимает работника целиком

- [x] Проверено: `workers.rs:417-421` — ключ с Shift по работнику → `pick_up` → `worker_stack` (шасси с упакованным работником, W3); самотест `worker_as_item` потока 6. DONE без кода.

### Task 9: таблицы добычи в форме 26.3

**Files:** Modify `tools/tiers.py` (`loot`: у записи `"condition": {"type": "minecraft:match_block", "blocks": <блок>, "state": {"tier": "N"}}`, форма из `SRC/crates/horizon-port-pack/src/carry.rs:1270-1276`), перегенерировать; вручную `loot_table/blocks/monorail.json`, `node.json` (не из `tiers.py`); самотест `tiered_blocks_drop_their_tier` (`break_block` каждого семейства на ступени 3 даёт `<блок>_3`).

- [x] `tools/tiers.py` пишет `match_block`, 11 таблиц семейств перегенерированы; `monorail.json` и `node.json` (поток 1) переведены скриптом; тест правил `no_loot_table_keeps_the_old_condition_list`, тест узла в `pipe18.rs` читает новую форму; самотест `s18_tiered_blocks_drop_their_tier` (каждое семейство, узел и монорельс на ступени 4 → предмет `_4`). Коммит `fix: loot tables in 26.3's form, a broken machine drops its own tier`.

### Task 10: передачи потока 4

- [ ] 10a (шасси 5/7 гнёзд — уже есть, `chassis::SOCKETS`) стак/хранилище добытчика больше 27, трюм больше 45, рюкзак 1024 — свои экраны с запасом в NBT (предел меню 99, см. «Пределы»).
- [x] 10b модуль высоты в буре — уже сделан потоком 6 (`bore::next_cell(.., height_module)`, `workers.rs:911`); бур без радиуса — `bore::size(tunnel)` больше не берёт модуль радиуса (`tool_uses(Bore, Radius)` = нет). Окно шасси: гнёзда по ступени 3/5/7 уже есть (`chassis::SOCKETS`).
- [ ] 10c страницы справочника в игре `mod_stack`, `mod_yield`, `mod_line` с предметами.

### Task 11: разряженный щит не блокирует

- [x] Справочник: «Когда заряд кончается, поле гаснет и щит не блокирует, пока его не зарядят». Правило `gear::shield::holds_up(charge)` (тест `a_drained_shield_goes_down`); новый хук `living_entity_use_item_tick` (API 131, `hook_use_item_tick.rs`, своя запись в `horizon-mixin.toml`): у поднятого щита без заряда на блок отмена — сервер заканчивает использование, ванильная блокировка кончается (SRC `native_hooks.rs:8140-8170`). Только там, где заряд известен (хранилище сервера). Самотеста нет: нужен игрок, держащий щит, — проверить ботом (поднять щит, разрядить ударами — щит опускается).

### Task 12: рецепт книги рецептов `aba|tst|aba`

- [x] Уже так: `recipe/module_recipes.json` — `aba|tst|aba` (кристаллит, книга, верстак, осколок), тест `every_module_has_a_recipe_and_the_slice_18_ones_are_the_handbooks` это держит. Кода нет.

### Task 13: четвёртое гнездо машин крафта среза 12 на ступенях 3–4

- [x] `machines::sockets::open`: машина среза 12 без своих ступеней — по `utility::sockets_for_tier(u.opening_tier())` (все пути гнёзд — нажатие, Shift-клик, окно — идут через `open`). Тест `a_slice_12_machine_of_tier_3_or_4_has_four_sockets`, самотест `s18_craft_machines_sockets_by_tier`.

### Task 14: модуль стака в машинах (256 в ячейке, запас в NBT как у печи)

- [ ] 

### Task 15: заказы O11 и предметы заказа в корпусах

- [ ] Планы `s18-craft-storage.md`, `s18-pipe.md`; видимость в корпусах — `transport.rs` потока 1, после его слияния.

### Task 16: добыча от стража работника как от владельца

- [x] `base::guard_shot`: запоминает удар (`remember_hit`), затем `owner_shot` — убивающий выстрел наносит владелец, если он в этом уровне (как у турели потока 7, `turret_owner::owner_blow`), иначе удар без атакующего, как раньше. Игра сама даёт добычу `killed_by_player` и опыт; отчёт `kill` и книга SoulsCraft для такого убийства не шлются (как у турели: PCT считает выстрел владельца сам). Самотест: на скамье нет игрока, путь «владельца нет» держит `worker_guard_kill_drops_where_the_mob_fell` (комментарий обновлён); путь владельца — проверка ботом (паук у стража при владельце рядом → глаз паука).

## Швы слияния (интегратор, приоритет)

- [x] `module_economy_slows_a_step`: окно мусорки с потока 3 показывает пропускную способность, а не шаг; тест читает шаг на ремонтном станке (`tiered::step_ticks` первой ступени, экономия ×0,85). try 2 — прошёл.
- [x] `orders18_bread_through_the_kitchen`: в try 2 заказ остался `done: 0, wait: Energy("kitchen")` с работающим заданием, хлеб уже на складе — свои стыки линии приносят хлеб на склад раньше, чем раунд заказов (раз в секунду) видит задание кухни законченным. Тест ждёт завершения заказа до 60 тиков вместо проверки в тот же тик.
