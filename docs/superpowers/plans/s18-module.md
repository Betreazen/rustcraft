# Срез 18, поток 4 «Модули» — план

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** every MISSING / DIFFERS line of `docs/design/06-audit/module.md` becomes DONE, or a recorded engine limit, or is handed to the stream that owns the block (with the rule ready here).

**Architecture:** `rules/src/modules.rs` is the one source of truth: which modules exist, which block takes which, how many sockets a tier has, and the numbers of each module's effect (pure functions with unit tests). Streams 3 (machines), 5 (craft and storage), 1 (node), 6 (workers), 7 (backpack) call these functions in their blocks. This stream also owns the module items, their recipes, and the turret's module effects.

**Tech Stack:** Rust (rules crate, `cargo test --release -q` in `mod/rustcraft/rules`), JSON content mod, `assets/models/icons_mc.py` + `export_mc.py` for item textures.

**Spec:** [docs/superpowers/specs/2026-11-16-handbook-parity-design.md](../specs/2026-11-16-handbook-parity-design.md), master plan [2026-11-16-handbook-parity.md](2026-11-16-handbook-parity.md).

## Global Constraints

- Horizon pinned to `7e4e421224743469035ba0c34fc21e569e64a17d`, Minecraft 26.3. Horizon is not changed.
- The game follows the handbook (`docs/showcase/js/data.js`, `ui.js`, `scenes.js`).
- Shared files (`content.rs`, `server.rs`, `selftest.rs`, `rules/src/selftest/mod.rs`, `rules/src/tuning.rs`, lang json): only add own blocks of lines.
- Text in Russian per `CONTRIBUTING.md`, en_us too; commits in English, email `128125070+Betreazen@users.noreply.github.com`, ending `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Recipes use `minecraft:amethyst_shard` for the shard until stream 2 is merged (the integrator switches them).
- Check: `pwsh -NoProfile -File "C:\Claude code projects\horizon-joint-dev\joint.ps1" try -Mod rustcraft -Path "C:\Claude code projects\rustcraft-s18\module" -Owner s18-module`.

---

## Контракт для потоков 3 и 5

Всё в `rustcraft_rules::modules` (файл `mod/rustcraft/rules/src/modules.rs`).

**Модули (`Module`).** К прежним 25 добавлены: `Capacity` (`rustcraft:module_capacity`, теперь в `ITEMS`, вставляется в гнёзда окон), `Stack` / `Stack2` / `Stack3` (`rustcraft:module_stack`, `module_stack_2`, `module_stack_3`: искросталь, кристаллит, звёздная сталь), `Yield` (`rustcraft:module_yield`), `Line` (`rustcraft:module_line`).

**Кто что берёт (`Holder` + `takes`).** Наборы точно по `data.js` (`MODS(...)` у каждой записи):

| Holder | Блоки | Модули |
|---|---|---|
| `Processor` | печь, дробилка, плавильня | скорость, экономия, ёмкость, стак, выход |
| `Pump` | насос | скорость, экономия |
| `Crafter` | автокрафтер | скорость, экономия, ёмкость, стак |
| `Mixer` | смеситель | скорость, экономия, ёмкость, стак |
| `Placer` | установщик, разрушитель | скорость, экономия |
| `Battery` | накопитель | ёмкость накопителя, отдача |
| `Node` | узел | ускорение линии |
| `Miner` | добытчик | скорость, экономия, разведка, площадь, хранилище, стак |
| `Terminal` | терминал склада | стак, книга рецептов |
| `Backpack` | рюкзак | стак |
| `Anchor` | якорь базы | радиус, экономия |
| `Butcher` | забойщик | экономия, радиус |
| `Turret` | турель | 3 эффекта, 4 поддержки, скорость, экономия, радиус |
| `Worker` | шасси (любой инструмент) | улучшения батареи и скорости, хранилище, радиус, коридор, высота, лава, застройка |
| `Util(u)` | машины среза 12 | см. `Util` ниже |

`Util`: утилизатор, зарядник, зачаровательный автомат, археологический стол, строительный автомат — скорость, экономия; дезинтегратор — скорость, экономия, выход; ремонтный стенд — скорость, экономия, ёмкость; кухонный комбайн, варочный куб — скорость, экономия, ёмкость, стак; рыболовная станция — экономия, хранилище; кормушка, стригаль, пасечный сборщик — экономия, радиус; живая жила — скорость, экономия, редкость, выбор руды, пласт. Зарядная плита, конденсатор опыта, голопроектор, портальный док — **модулей нет** (`Holder::has_sockets()` = false: окно гнёзд не рисует).

Что из модулей шасси работает с каким инструментом: `tool_uses(tool: worker::Tool, m: Module) -> bool` (пила, жатка, захват, ковш — радиус, хранилище; бур — коридор, высота, лава, застройка, хранилище; картограф — радиус; улучшения — всем).

**Гнёзда.** `sockets(tier: u8) -> usize` — 3 / 3 / 4 / 4 для ступеней 1–4 (A5); `MAX_SOCKETS = 4` (раскладка окна); `chassis_sockets(tier) -> usize` — шасси ступеней 2 / 3 / 4: 3 / 5 / 7; `MAX_CHASSIS_SOCKETS = 7`. `Holder::has_sockets(self) -> bool`.

**Действие (числа справочника, A9 — одинаковые складываются полностью):**

- `factors(slots, t) -> (work, energy)` — скорость ×2 / ×2,5, экономия ×0,85 / ×0,6 за модуль (без режима; режим умножается отдельно).
- `paced_ticks(step_ticks, slots, t) -> u64` — тики шага машины после модулей: `round(step / work)`, не меньше 1.
- `capacity_slots(slots) -> usize` — +2 ячейки очереди ингредиентов за модуль ёмкости; `capacity_pages(slots) -> usize` — +1 страница рецептов (автокрафтер, комбайн, варочный куб).
- `stack_tier(slots) -> u8` — старшая ступень модуля стака в гнёздах (0 — нет); `machine_stack(slots, base_max) -> u32` — стак ячейки машины/добытчика: 64 → 256 при любом модуле стака (16 → 64, 1 → 1); `backpack_stack(slots, base_max) -> u32` — как склад, но не больше 1024; склад — `storage::STACK_BY_MODULE[stack_tier]` (поле `Store::stack_module`).
- `yield_chance(slots) -> f64` — 0,15 за модуль выхода, не больше 1 (печь, дробилка, плавильня); `yield_lucky(slots, roll: f64) -> bool` — `roll < chance`. Дезинтегратор: `utility::disassembly::share_percent(count(slots, Yield))` — 50 / 60 / 70, не выше 75.
- `storage_factor(slots) -> usize` — ×2 за модуль хранилища (добытчик, рыболовная станция, трюм работника).
- `line_factor(slots) -> f64` — пропускная способность участка узла: 1 + 0,5 за модуль.
- `height_step(module: bool, target: Option<i32>, y: i32) -> i32` — модуль высоты бура: на сколько (−1/0/+1) сменить уровень следующего рельса; без модуля или без заданной высоты 0.

## Задачи

Статус на 2026-11-16 (коммиты ветки `s18/module`).

- [x] **Task 1. Наборы, гнёзда, новые предметы** — `modules.rs` по таблице контракта, тесты `each_holder_takes_exactly_the_handbook_set`, `economy_fits_every_machine_and_speed_all_but_animals_and_fishing`, `sockets_by_tier`, `tools_use_their_own_chassis_modules`, `speed_and_economy_stack_in_full_and_economy_slows_work`, `capacity_adds_two_slots_and_a_page_a_module`, `stack_tiers_and_slot_sizes`, `yield_is_15_percent_a_module`, `storage_doubles_line_adds_half_height_steps_toward_its_target`. Предметы стака (3), выхода, линии: иконки (`icons_mc.py` → `export_mc.py`), модели, имена ru/en, рецепты. Модуль ёмкости — в `ITEMS` (`content.rs` больше не регистрирует его отдельно). Коммит 47dc675.
- [x] **Task 2. Рецепты пласта, высоты, лавы, застройки, книги** по `data.js`; тест `every_module_has_a_recipe_and_the_slice_18_ones_are_the_handbooks` читает JSON рецептов. Коммит 47dc675.
- [x] **Task 3. Дезинтегратор — модуль выхода** (`utility.rs`), самотест `module_disintegrator_yield`. Коммит cd32cf1.
- [x] **Task 4. Экономия ×0,85 работы в машинах среза 12** (`utility.rs::paced` через `modules::paced_ticks`/`factors`), самотест `module_economy_slows_a_step`. Коммит cd32cf1.
- [x] **Task 5. Хранилище ×2** — правило `storage_factor`; `mining::slots_of` и `Worker::hold_size` считают ×2 за модуль. Пределы окон остаются (добытчик 27 = 3 ряда, трюм 45 = 5 рядов): **передано** — добытчик 36 и больше — поток 2 (окно и `MINER_SLOTS`), трюм — поток 6, буфер рыболовной станции 9 → 18 — поток 5. Коммит 7015498.
- [x] **Task 6. Модуль высоты — правило** `height_step` (Task 1). Бур и поле высоты в окне шасси — **поток 6**.
- [x] **Task 7. Жила**: редстоун — частая руда, выбор руды не дороже по энергии (тест `redstone_is_not_rare_and_the_pick_costs_no_more`), коммит 02a19ac; выбор руд в окне жилы виден только с модулем выбора руды, самотест `module_vein_choice_with_the_module`, коммит 9074709.
- [x] **Task 8. Замедление турели 20/40/60%** — модификатор атрибута `movement_speed` моба (`horizon_client::mobs::finalize`, то же, что правит `/attribute`), `turret::slow_fraction`; самотесты турели читают модификатор (`MobView::slow`). Коммит bb20543.
- [x] **Task 9. Луч лечения** — `turret::beam_points` (тест), розовая пыль `/particle minecraft:dust` от турели к лечимому игроку раз в 10 тиков (`execute in <измерение>`). Самотеста нет (частицы сервер не хранит) — пункт 4 в README «Что проверить в игре: срез 18, модули». Коммит b5a6fe0.
- [x] **README** — раздел «Что проверить в игре: срез 18, модули». Коммит 464ebc8.
- [x] **Task 11. Пласт: выбор песок или гравий в окне жилы** — `vein::STRATUM_ROCKS`, `stratum_rock` (тест `the_stratum_takes_the_rock_its_window_chose`); окно жилы с модулем пласта показывает «Песок», «Гравий», жила берёт выбранный камень; самотест `module_stratum_grows_the_chosen_rock`. Коммит b77073d.
- [x] **Скорость в утилизаторе** (замечание к `mod_speed`): шаг уже делится на 2 за модуль (`paced`), порция больше не удваивается — работа ×2, а не ×4. Коммит 261342b.

### Task 10: строки таблицы, переданные владельцам блоков (правило готово здесь)

| Строка таблицы | Что готово здесь | Кто подключает |
|---|---|---|
| 3 гнезда на ступенях 1–2, 4 на 3–4 (A5) | `sockets(tier)`, `MAX_SOCKETS` | поток 3 (ступени, `sheet::SOCKETS`, раскладка окон, `selftest.rs::set_modules` держит 3) |
| Шасси 3/5/7 гнёзд, 4 коридора 9×9 | `chassis_sockets(tier)` | поток 6 |
| Печь, дробилка, плавильня: ёмкость, стак, выход; скорость и экономия | `Holder::Processor`, `capacity_slots`, `machine_stack`, `yield_lucky`, `factors` | поток 3 |
| Смеситель, автокрафтер: ёмкость, стак | `Holder::Mixer`/`Crafter`, `capacity_slots`, `capacity_pages`, `machine_stack` | поток 3 |
| Насос: скорость, экономия | `Holder::Pump` | поток 3 |
| Комбайн, варочный куб: ёмкость (+страница), стак; ремонтный стенд: ёмкость | `Holder::Util(..)`, те же функции | поток 5 |
| Рыболовная станция: хранилище ×2 | `Holder::Util(Fishery)`, `storage_factor` | поток 5 |
| Забойщик: экономия | `Holder::Butcher` берёт экономию; забойщик сейчас энергии не тратит | поток 5 |
| Терминал: стак 256/1024/4096 | `Holder::Terminal`, `stack_tier` → `Store::stack_module` | поток 5 |
| Рюкзак: стак до 1024 | `Holder::Backpack`, `backpack_stack` | поток 7 |
| Добытчик: стак 256 | `Holder::Miner`, `machine_stack` | поток 2 |
| Узел: ускорение линии +50% | предмет, `Holder::Node`, `line_factor` | поток 1 (окно узла, участок) |
| Бур: высота, без радиуса | `height_step`, `tool_uses(Bore, Radius)` = false | поток 6 |
| Пласт: таблица находок в окне жилы (выбор песок/гравий сделан, Task 11) | — | поток 5: таблица должна доехать с блоком «жила → захват (трюм, `grown::hold_name`) → стол» (`found::brush_table`); у стола справочник выбора таблицы не даёт |
| Окна без модулей (зарядная плита, конденсатор опыта, голопроектор, портальный док) рисуют пустые гнёзда | `Holder::has_sockets()` = false | поток 3 (`Kind::shape`: `sockets = holder().is_some_and(Holder::has_sockets)`) |
| Страницы справочника в игре `mod_stack`, `mod_yield`, `mod_line`: `item: None` | предметы `module_stack`, `module_yield`, `module_line` есть | поток 8 (`handbook/pages.rs`) |
| Осколок аметиста в рецептах модулей | — | интегратор после потока 2 |

## Пределы движка

Нет. Всё, что нужно было потоку, движок даёт: модификаторы атрибутов моба (`horizon_client::mobs::finalize::add_modifier`, SRC `crates/horizon-client/src/mobs/finalize.rs:652`), `/particle` с опциями пыли (SRC `game/commands/world_cmds.rs:749`).
