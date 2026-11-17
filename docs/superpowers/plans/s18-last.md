# Срез 18, последние остатки — план

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. A resumed agent continues from the first unticked task; the ticks are the truth.

**Goal:** последние строки среза 18 становятся DONE: хранилище добытчика больше 27 и трюм больше 45 (×2 за модуль хранилища), рюкзак с гнездом модуля стака и стаками до 1024 в своём экране, модуль стака варочного куба, заказы O11 (продукт шага сразу в машину следующего шага) и предметы заказа, видимые в корпусах.

**Architecture:** правила — в `mod/rustcraft/rules` с юнит-тестами (RED → GREEN), бандл (`mod/rustcraft/*.rs`) только связывает их с игрой. Ячейка меню держит не больше 99 и не больше стака предмета (`SRC/docs/native-mods.md:4617-4627`), поэтому всё, что больше, лежит в скрытых ячейках NBT (как очередь машин потока 3, `rules/src/machines/queue.rs`, и запас кухни `utility::kitchen_stash`) и показывается числом в окне; рюкзак получает свой экран API 111 (как планшет потока 5, `tablet.rs`). Самотесты — `rules/src/selftest/s18_last.rs` (новый файл, одна строка в `selftest/mod.rs`).

**Tech Stack:** Rust 2024, Horizon native mod API на `7e4e421`, JSON контент-мода.

**Spec:** `docs/superpowers/specs/2026-11-16-handbook-parity-design.md`; мастер-план `docs/superpowers/plans/2026-11-16-handbook-parity.md`; передачи — `s18-cross.md` (Task 10a, 14, 15), `s18-craft-storage.md` (Task 26, 30), `s18-base-gear.md` (рюкзак, «модуль стака»); аудит — `docs/design/06-audit/module.md:77-79,130-131`, `base-gear.md:149-150`, `craft-storage.md` (O11).

## Global Constraints

- Horizon pinned `7e4e421224743469035ba0c34fc21e569e64a17d`, Minecraft 26.3; Horizon не меняется.
- Игра следует справочнику (`docs/showcase/js/data.js`, `ui.js`, `scenes.js`).
- Тексты по-русски (`CONTRIBUTING.md`), ru_ru и en_us; коммиты по-английски, почта `128125070+Betreazen@users.noreply.github.com`, последняя строка `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Общие файлы (`content.rs`, `server.rs`, `selftest.rs`, `rules/src/selftest/mod.rs`, `rules/src/tuning.rs`, `rules/src/modules.rs`, lang json, `horizon-mixin.toml`, файлы окон): только добавлять свой блок строк.
- Коммиты только `git commit -- <paths>` на своей ветке; тесты правил `cargo test --release -q` в `mod/rustcraft/rules`.
- Стенд: `joint.ps1 try -Mod rustcraft -Path <снимок> -Owner s18-last` — только когда интегратор скажет «bench free»; журнал `scratchpad\try-last-N.log`.

## Пределы движка

- **Стак больше 99 в ячейке меню.** `native_menu_slot_rules` держит `max_stack` в 1–99 и не выше стака предмета (`SRC/docs/native-mods.md:4617-4627`). Поэтому: добытчик и работник показывают в окне 27 и 45 ячеек, остальное — скрытые ячейки, числом в окне; рюкзак — свой экран, где число в ячейке рисует мод.
- **Корпус рисует предметы заказа, а не предметы сетей.** Обычные перемещения предметной сети (`rules/src/net/items.rs`) — мгновенные переносы между стыками раз в тик, «едущего» предмета у них нет; едут только отправки заказов (`rules/src/orders/ship.rs`). Справочник показывает в корпусах предметы заказа — их и рисуем.

---

## Задачи

### Task 1: хранилище добытчика ×2 за модуль (больше 27) и трюм работника ×2 (больше 45)

**Files:** `rules/src/mining.rs` (`slots_of` без среза окном, `shown_slots`, `settle`), `rules/src/tuning.rs` (`miner_slots_max = 9 << MAX_SOCKETS`, `worker_hold_max = 27 << 7`), `rules/src/worker/hold.rs` (`settle`, `beyond`, хранение без хвоста пустых ячеек), `mod/rustcraft/mining.rs` (скрытые ячейки `miner_cells`, приём добычи, подача в видимые, строка окна), `mod/rustcraft/workers.rs` (строка окна, `settle` после взятия), `mod/rustcraft/machines.rs::on_destroyed` (свой блок: ячейки выпадают), lang, самотест `s18_miner_storage_beyond_the_window`.

- [x] RED: `mining` тест `storage_doubles_past_the_window` (0..4 модуля → 9, 18, 36, 72, 144; окно 27); `settle_moves_hidden_cells_into_free_slots`; `hold` тест `a_hold_bigger_than_its_window_moves_hidden_stacks_up`, `a_hold_saves_without_its_empty_tail`; `worker` тест `three_storage_modules_make_the_hold_216`.
- [x] GREEN, бандл (`mining.rs`: `CELLS_KEY` = `miner_cells`, добыча сверх слотов окна — в скрытые ячейки, раз в 8 тиков после выдачи ячейки подают в слоты окна `settle_cells`, строка окна `rustcraft.miner.hidden`; `workers.rs`: `Hold::settle(45)` после взятия, строка `rustcraft.worker.hold_beyond`; трюм пишется без хвоста пустых ячеек, старые записи читаются), самотест `s18_miner_storage_beyond_the_window` (27 слотов окна полны, два модуля хранилища: руда выкопана, железа в слотах окна нет, «Некуда складывать» не горит). Коммит.

### Task 2: рюкзак — гнездо модуля стака и стаки до 1024 в своём экране (API 111)

**Files:** `rules/src/backpack.rs` (раскладка экрана: сетка 9×N до 81, гнездо, инвентарь; `deposit`/`take` по стаку `modules::backpack_stack`; кодирование нажатий и содержимого), `mod/rustcraft/backpack.rs` (окно — свой экран `rustcraft:backpack` вместо окна сундука; сервер хранит ячейки с числом > 99 (число отдельно от предмета в NBT), шлёт содержимое клиенту, принимает нажатия; гнездо модуля), хуки экрана (как у планшета: `hook_screen_render_post.rs`, `hook_screen_mouse.rs`, `hook_net_from_server.rs`, `hook_net_from_client.rs`, `hook_tick.rs` — по строке), lang, `mod/README.md` (раздел «Что проверить в игре: срез 18»), самотест, если путь без игрока есть (иначе — проверка ботом).

- [x] RED/GREEN правил, бандл, коммит.

### Task 3: модуль стака варочного куба

**Files:** `rules/src/machines/queue.rs` (`stash`: общий для кухни и куба), `rules/src/utility/kitchen.rs` (`stash` через него), `rules/src/utility/brew.rs` (`STASHED` — ячейки ингредиентов и топлива), `mod/rustcraft/utility.rs` (`brew_stash`, `brew_cells`, варка берёт из ячеек), `machines.rs::on_destroyed` (свой блок), самотест `s18_brewer_stack_module`.

- [x] RED: `a_stack_module_lets_the_brewers_reagents_and_powder_hold_256` (две группы ячеек `brew::STASH_GROUPS`: ингредиенты и порошок отдельно, ингредиент не попадает в слот порошка); `queue::stash` — общий для кухни и куба; GREEN; бандл (`utility::brew_stash`, ключи `brew_cells`, `brew_powder_cells`, варка берёт сначала из ячеек; при сломе ячейки выпадают — свой блок в `machines::on_destroyed`); самотест `s18_brewer_stack_module`; коммит.

### Task 4: O11 — продукт шага сразу в машину следующего шага

**Files:** `rules/src/orders/held.rs` (новый: продукты заказа, ждущие у машины), `rules/src/orders/queue.rs` (раунд видит их как запас заказа), `rules/src/orders/saved.rs` (запись), `mod/rustcraft/orders.rs` (`pull` с `own` кладёт в «ждёт у машины», `deliver` берёт сначала оттуда и везёт из машины A в машину B), самотест в `selftest/s18_last.rs` (печь → автокрафтер: слитки не появляются на складе).

- [x] RED/GREEN правил, бандл, самотест, коммит. Сделано: `held.rs` (запись «ждёт у машины A для заказа X», в записи склада, версия 3), `Queue::round_with` (ждущее — запас только своего заказа, `held_need` — сколько нужно плану), `started_with`; бандл: `hold` вместо поездки на склад, `deliver` везёт ждущее из A в B по сети между машинами (без сети — прямо), остальное со склада; лишнее и ждущее отменённого/готового заказа едет на склад (`release_held`, заказ 0 — поездка никого не держит); второй приезд не сбрасывает временную страницу автокрафтера; сеть не забирает такой продукт из выхода машины (`kept_outputs` в `transport.rs`). Самотест `s18_o11_product_goes_straight_to_the_next_machine` (`selftest/s18_last_orders.rs`).

### Task 5: предметы заказа в корпусах

**Files:** `rules/src/net/topology.rs` (`pipe_path`: корпуса пути с расстояниями), `rules/src/orders/ship.rs` (место отправки на пути по времени), `mod/rustcraft/orders.rs` (раз в секунду — едущие предметы в NBT корпусов пути, тег обновления), `mod/rustcraft/pipe18.rs` (`update_tag` и `draw_housing`: `native_draw_item` в месте по `draw.game_time`), самотест (NBT корпуса на пути несёт едущий предмет).

- [x] RED/GREEN правил, бандл, самотест, коммит. Сделано: `topology::pipe_path` и `link_face`, `Shipment::progress`, `rules/src/orders/ride.rs` (участки «грань входа, грань выхода, тик входа и выхода» по корпусам пути; пока грузится — стоит у первого корпуса столько, сколько сеть грузила бы её одну; `place` — место предмета в корпусе по времени кадра; кодирование в NBT `ride`/`ride_items`); бандл: `Transport::path`, `orders.rs::show_rides` раз в секунду пишет корпуса пути, чистит оставленные и шлёт клиентам; `pipe18.rs` — ключи в теге обновления и `draw_riding` (`native_draw_item`, `Ground`) без `draw.keep()`, пока предмет едет. До трёх видов предметов отправки, каждый на 4 тика позади. Самотест `s18_order_items_ride_the_housings`.

### Task 6: проверка

- [ ] `cargo test --release -q` зелёный; после «bench free» — `joint.ps1 try` со снимка; README «Что проверить в игре».
