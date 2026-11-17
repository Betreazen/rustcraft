# Срез 18, поток 5 «Крафт и склад» — план

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. A resumed agent continues from the first unticked task; the ticks below are the truth.

**Goal:** every MISSING / DIFFERS line of `docs/design/06-audit/craft-storage.md` becomes DONE in the game, or a recorded engine limit.

**Architecture:** logic in `mod/rustcraft/rules` (storage, orders, utility, butcher) with unit tests; the bundle (`storage.rs`, `terminal.rs`, `tablet.rs`, `orders.rs`, `farm.rs`, `butcher.rs`, `builder.rs`, the craft functions of `utility.rs`) only wires it to the game. Every mechanic gets a self-test scenario in `rules/src/selftest/craft18.rs` (new file, registered in `selftest/mod.rs` with one added line).

**Tech Stack:** Rust 2024, Horizon native mod API at `7e4e421` (SRC = the launcher's source tree), JSON content mod.

**Spec:** `docs/superpowers/specs/2026-11-16-handbook-parity-design.md`; master plan `docs/superpowers/plans/2026-11-16-handbook-parity.md`.

## Global Constraints

- Horizon is pinned to `7e4e421224743469035ba0c34fc21e569e64a17d`, Minecraft 26.3. Horizon itself is not changed.
- The game follows the handbook (`docs/showcase/js/data.js`, `ui.js`, `scenes.js`). The handbook changes only for the spec's contradictions and for an engine limit that has been checked in `SRC`.
- Text and docs are in Russian per `CONTRIBUTING.md` (ru_ru and en_us). Commit messages in English, email `128125070+Betreazen@users.noreply.github.com`, every commit ends with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Commit with `git commit -- <paths>` on `s18/craft-storage` only.
- Shared files (`content.rs`, `server.rs`, `selftest.rs`, `rules/src/selftest/mod.rs`, `rules/src/tuning.rs`, `rules/src/modules.rs`, lang json, `horizon-mixin.toml`, window files): only add own blocks of lines, never rewrite others'.
- Every 3–5 tasks: `pwsh -NoProfile -File "C:\Claude code projects\horizon-joint-dev\joint.ps1" try -Mod rustcraft -Path "C:\Claude code projects\rustcraft-s18\craft-storage" -Owner s18-craft-storage` — RC, PCT, SC self-tests all green. Rules tests any time: `cargo test --release -q` in `mod/rustcraft/rules`.
- Numbers: where the handbook names a number, take it.

## Try status

- 2026-11-17, snapshot 1e5ccb6: the game built with this branch; self-tests PCT 42/42, SoulsCraft 87/87, RustCraft 157/158 — the one failure `storage18_keeps_component_stacks` checked before the net had brought all five stones (fixed in db6fee7, the scenario waits for them). Every other stream-5 scenario passed (`storage18_*`, `orders18_*`, `util18_*`, `kitchen18_*`, `repair18_*`, `fishery18_*`, `butcher18_*`, `brewer18_*`, `builder18_*`) and the updated slice-11/12/15 scenarios.

- 2026-11-17, snapshot db6fee7: green — RustCraft 158/158, PCT 42/42, SoulsCraft 87/87 (not published, -Path).

## Cross-stream contracts this plan relies on

- **Module items (stream 4).** The stack and filter modules are stream 4's items. This stream recognises them by item id in ONE place, `rules/src/storage/modules.rs`: `STACK_MODULES = [("rustcraft:module_stack", 1), ("rustcraft:module_stack_1024", 2), ("rustcraft:module_stack_4096", 3)]`, `FILTER_MODULE = "rustcraft:module_filter"`, `CAPACITY_MODULE = "rustcraft:module_capacity"` (exists today). If stream 4 names them otherwise, the integrator edits that one file.
- **Module sets (stream 4).** Which machine takes which module is `modules.rs` `Holder::takes` (stream 4). This stream implements the *effects* (capacity pages and slots, stack, storage buffer) by counting the item ids in the block's module slots, and does not edit `Holder::takes`.
- **Tiers (stream 3).** The 3/3/4/4 socket rule needs machine tiers (stream 3's `rustcraft_rules::tiers`). This stream adds `utility::sockets(u, tier)` and reads the tier from the block entity key `tier` (default: the machine's opening tier from the handbook), so stream 3's swap writes it and it works.
- **Crystal shard (stream 2).** Recipes stay on the amethyst shard; the integrator switches them.

## Пределы движка

(Filled in as found, each with SRC evidence.)

- **Ghost translucency.** `native_draw_block(draw, block, properties)` and `native_draw_model(draw, model, tint)` take no alpha (SRC/docs/native-mods.md:920-930, 1247; native_block_entity_draw.rs:663-668): a block entity draws block models as they are. The ghost is each block's own model at 0.6 scale in its cell; a cell with another block gets red stained glass (translucent by its texture).
- **Order kinds.** Only the kitchen joins the order kinds: the brewer and the enchanting machine make potions and enchanted items from settings their windows choose, which a recipe book entry does not name; the disintegrator un-makes. Not an engine limit: recorded as the closest honest reading of «через машины магистрали».
- **The 4th socket (tier 3–4).** Not an engine limit: waits for stream 3's per-tier sockets in sheet.rs (rules ready: `Util::opening_tier`, `sockets_for_tier`).
- **Tanks by channel.** The kitchen and the shearer use the tank they touch (as the mixer's water and the catalysts do, fluids.rs:371-430), not a fluid-channel joint: joints for them would be stream 3's fluids.rs net code.
- **Stack module in machines (256 a slot).** Not made in this stream: per-slot max stacks of the craft machines' menus are content menu types (stream 3/4).

---

## Part A — the storage

### Task 1: Items with components in the storage (cell «хранит любые предметы»; brewer/enchanter «готовое на склад»; README 614, 643)

**Files:** `rules/src/storage/keys.rs`, `rules/src/storage/wire.rs`, `rules/src/storage/terminal.rs` (ItemRef variant bit), `rules/src/net/stock.rs` (only the `storable` callers keep working), `storage.rs` (`key_of`, `stack_of`, `stored_stack`, listing variants, `key_of_ref`), `terminal.rs` (draw the variant's stack), `transport.rs:378` (one line: the net key carries the payload — stream 1's file, flagged to the integrator).

- Key of a stack with components: `item#<hex of the network NBT of the stack's components>` (`keys::component_key(item, payload_hex)`); `keys::storable` is true unless the payload is empty (`item#` alone stays the unstorable mark for over-long payloads > `MAX_PAYLOAD` = 2048 hex chars).
- `keys::parse` returns (item, damage); new `keys::payload(key) -> Option<&str>`.
- Wire: `Line.damage` top bit `VARIANT = 0x8000` marks a component line whose low 11 bits are its index among the store's component keys of that item (sorted); `Listing.variants: Vec<(u16 item, u16 index, String payload)>` so the client builds the stack (`save::item_from_nbt`) to draw and name it.
- Press: `ItemRef = (number, damage_or_variant)`; encode: a variant press sets bit 31 (negative i32) with the 11-bit index in the damage field.
- [x] rules tests: key round trip; two enchanted books with different payloads are two keys; wire round trip with variants; press round trip with a variant.
- [x] wiring + self-test `storage18_keeps_component_stacks` (a hopper-fed enchanted book and a named stone go into the storage and come back out whole).
- [x] commit 3424fe0 (in-world run: pending the first try)

### Task 2: Stack module S3 and the terminal's three sockets (stack, recipe book, filter)

**Files:** `rules/src/storage/modules.rs` (new: ids above, `stack_tier(items) -> u8`), `rules/src/storage/terminal.rs` (sockets rects `SOCKETS: [[f32;2];3]`, `Press::Socket(k)` replacing `Press::Book`), `storage.rs` (sockets saved in the terminal's compound `sockets`; `Store::stack_module` set from the storage's terminals on rescan/press), `terminal.rs` (draw three sockets).
- rules: `stack_tier(&[Some("rustcraft:module_stack_1024")]) == 2`; the best module of all the storage's terminals counts; `Store::stack_of(64)` = 1024 with tier 2.
- self-test `storage_stack_module_raises_the_stack`: a 1-cell storage takes 27×256 stone with the module.
- [x] done 7827b42 (with Task 3; self-tests `storage18_stack_module_raises_the_stack`, `storage18_filter_socket`)

### Task 3: Filter module and the «своя» tab (TM1)

**Files:** `rules/src/storage/view.rs` (`Tab::Own`, `Facts.own`), `rules/src/storage/filter.rs` (new: the filter's list in custom data `rustcraft:filter`: item ids, at most 27), `storage.rs` (Listing `own: Vec<u16>` from the filter module in a socket), `terminal.rs` (fifth tab), filter window: right click with the filter module opens a three-row server menu (`native_open_menu`, the tablet's way) — a click with an item on the cursor marks it, a click on a marked item unmarks it (`filter_menu.rs`, new, own file).
- rules tests: mark / unmark / limit 27; `Tab::Own.shows` only marked ids.
- self-test `storage_filter_tab_lists_marked`: (rules-level view with a filter list; in world: filter module in a socket makes `Listing.own` non-empty).
- [x] done 7827b42 (filter window = bundle `filter.rs`: a click on an inventory item marks it, since a server menu has no cursor)

### Task 4: Extra terminals anywhere on the magistral (S6)

**Files:** `rules/src/storage/cells.rs` (`terminal_storage_on_net(terminal, cell_of, topology)`: a terminal touching no cell takes the storage that is a joint of an item net its touched housing is on), `storage.rs` (rescan uses it; needs the level topology → `rescan` gets `&Topology`; the caller in `server.rs` passes it — one added argument).
- rules test: terminal two housings away on the same item net finds the storage; on another net — none.
- self-test `storage_far_terminal_sees_the_storage`.
- [x] done d6fb79c (`cells::terminal_on_net`, `storage::link_far_terminals` called after the topology in server.rs; self-test `storage18_far_terminal_on_the_net`)

### Task 5: Terminal window: any count, limits list, order chain, «Склад полон», «Нет свободного автокрафтера»

**Files:** `rules/src/storage/terminal.rs`, `rules/src/storage/wire.rs` (order steps), `terminal.rs`, `storage.rs`, lang.
- Order count: a count row «− N +» (±1 / ±8 with shift / ×64 with right button), «Заказать» orders N (≤ 2047).
- «Лимиты» mode: the grid shows only items with a limit, each cell's tooltip / overlay its limit; the row edits the chosen one (list of all limits).
- Order queue: each order's steps (`OrderLine.steps: Vec<(kind, done, of)>`, current highlighted) drawn as a chain under the status.
- State `Full` («Склад полон») when used == total; order wait `NoFreeCrafter` («Нет свободного автокрафтера») when a crafter exists but all are busy.
- [x] rules: count row, limits rows, OrderStep chain in the wire tail (ecc342b)
- [x] orders: `Order.chain` filled by `Queue::round`, `orders.rs` sends it
- [x] client: count row, limits rows, chain line, «Склад полон», busy texts per kind; lang
- [x] self-test `storage18_full_says_so`, commits ce3946d f5613fd 828ef3a

### Task 6: Tablet: drawn window (API 111), deposit, search, tabs, limits

**Files:** `tablet.rs` (server: the list goes by mod message `rustcraft:tablet`, the presses come back by `native_net_send_to_server` → `native_net_from_client`), new `tablet_screen.rs` (client: `native_mod_screen_open`, draws with the terminal's drawing helpers, the player's inventory drawn by the screen, click on an inventory item = deposit), `rules/src/storage/tablet.rs` (press codec: take / deposit slot / limit), `hook_net_from_client.rs` (one added dispatch line).
- rules tests: press codec round trip; deposit of a stack the storage has no room for leaves the rest.
- self-test `tablet_deposit_puts_into_storage` (server function `tablet::deposit_from` called by the bench).
- [x] done 4e8101f abc1059: screen id `rustcraft:tablet`, terminal drawing in a tablet context (`terminal::in_tablet`), presses on `rustcraft:tablet_press` (single player: straight into the server queue). No in-world self-test: deposit and take need a real player (`native_player_stacks`); bots must check it. Try pending.

### Task 7: Cell holes highlighted in the world (S8)

**Files:** `storage.rs` (holes written into each terminal's block entity `holes` int array on rescan), `terminal.rs` client block-entity draw: red stained glass at each hole relative to the terminal (`native_draw_block`, as the projector's ghost).
- rules test: none new (shape exists); self-test `storage_holes_written_to_terminal`.
- [x] done (self-test `storage18_terminal_keeps_its_holes`; red glass at 0.9 scale; a far terminal (S6) does not draw them)

### Task 8: Orders through the kitchen (and the brewer)

**Files:** `rules/src/orders/mod.rs` (`KINDS` + kitchen kind, steps for food recipes), `orders.rs` (dispatch to the kitchen: put ingredients into its slots 0..9, take the dish from 9..13), `terminal.rs` `KIND_BLOCKS`.
- rules test: an order for bread plans a kitchen step.
- self-test `orders_bread_through_kitchen`.
- [x] done (KINDS + kitchen, `orders::kitchen_copy`, kitchen cooks the job's dish first; self-test `orders18_bread_through_the_kitchen`; brewer and other slice-12 machines not order kinds: their outputs are potions/enchanted items from player-chosen settings, recorded as a remaining gap)

## Part B — the craft machines, common

### Task 9: Work modes for repair, kitchen, brewer, enchanter, archeo, builder

**Files:** `rules/src/utility/mod.rs` (`has_modes(u)`, `step_ticks/step_energy` with a `WorkMode`), `rules/src/machines/sheet.rs` (`Kind::Util` shape `mode: u.has_modes()` — one changed line, stream 3's file, flagged), `utility.rs` (`paced` reads the mode `util_mode`; `on_button` handles `Press::Mode`; `sheet` sets `s.mode`).
- rules test: boost halves ticks ×3 energy (via `WorkMode::factors`), eco ×2 ticks ×0.35.
- self-test `util_modes_switch` (press Boost on a repair stand → `util_mode` = 2, step 2.5 s in its row).
- [x] done 81b0781 (`utility::paced` in rules; sheet.rs one line, flagged; self-test `util18_work_mode_switch`)

### Task 10: Sockets 3/3/4/4 by tier; module effects capacity / stack / storage

**Files:** `rules/src/utility/mod.rs` (`opening_tier(u)`, `sockets(tier)`), `rules/src/storage/modules.rs` ids, `utility.rs`, `farm.rs`.
- Capacity: kitchen and brewer +1 page each; repair +2 material slots.
- Storage module: fishery buffer 9 → 18.
- Sockets: brewer, archeo, projector, builder (tier 3) and enchanter (tier 4) get 4 sockets — needs `sheet.rs` `SOCKETS` per shape (stream 3's file; done as `Shape.socket_count`, flagged).
- [x] rules 19f7219: `Util::opening_tier`, `sockets_for_tier`. Capacity effect for repair (cabeef1), kitchen/brewer pages in tasks 13/19, storage module in task 14.
- [ ] the 4th socket in the window: waits for stream 3's per-tier sockets in sheet.rs (same SOCKETS const for all sheets); after the merge `sheet.rs` takes `sockets_for_tier`

### Task 11: Levels on models (feeder food, shearer milk, apiary honey, brewer brew)

**Files:** `utility.rs`, `farm.rs` — `anim::show_level` each step.
- [x] done in tasks 15, 17, 18, 19 (`show_level_cached`)

## Part C — the craft machines, one by one (windows per ui.js through the sheet: rows, bars, toggles)

### Task 12: Repair — string for rod, bow, crossbow; slots 3 / 2 / 3; durability bar «алмазная кирка · 970 из 1561»
- `rules/src/utility/repair.rs` (`material_of`), `rules/src/utility/mod.rs` slots `Repair => s(8, 0..5, 5..8)` (0..3 to repair, 3..5 material, 5..8 out), `utility.rs`, update `selftest/slice12.rs` output slot.
- [x] done cabeef1 (capacity read from raw module items via `utility::module_items`; machines.rs Util slot rule passes them, flagged; `Util::bars` + sheet.rs line, flagged; self-test `repair18_mends_a_rod_with_string`)

### Task 13: Kitchen — pages, milk from a tank, empty buckets back/none, 4 s, «Нет молока в баке», handbook dish list
- Pages: page = chosen dish (toggle row picks the dish of the page in view; pages 1 + capacity modules); `KITCHEN_RECIPES` without the golden carrot / apple (not in the handbook); milk: a recipe with `milk_bucket` takes 1000 mB of `rustcraft:milk` from a tank it touches instead (helper `fluids::take_touching` / `put_touching`, added block in `fluids.rs`, flagged), no bucket item; step 80 ticks; window rows: page tabs, the dish, its ingredients, milk bar.
- [x] done (rules `utility::kitchen`; fluids.rs block `touching_amount/take_touching/put_touching`, flagged; tank = the one it touches, as the mixer's water — a fluid-channel joint for the kitchen would need fluids.rs net code of stream 3; milk buckets as items still work and leave their buckets; self-test `kitchen18_cake_with_milk_from_a_tank`)

### Task 14: Fishery — sky over the water, Lure, rod kept at its last point («Удочка изношена»), storage module, no speed, window rows
- [x] done 8bc4712 (`Util::paced_by_nature`: speed modules do nothing in the four farm machines; storage module read both as Module::Storage and as the raw item; rows: water, rod·wait (240 px); test `every_machine_window_fits_the_screen`; self-test `fishery18_keeps_the_rods_last_point`)

### Task 15: Feeder — babies fed (age +10%), head counts per kind in the window, «Предел животных», a pair every 20 s, 3 food slots
- [x] done 01aaddd (rules `baby_fed`, `baby_to_feed`, `herd_full`, `heads`; food level on the model; no in-world self-test for babies: the bench spawns no baby mob — bots check; slice12 feeder test waits 2×LIMIT for the 20 s pace)

### Task 16: Butcher — recipe `bsb|bhb|brb`, energy blade accepted, 4 E/s and «Нет энергии», looting / fire aspect, kill XP into a condenser beside it, economy module effect
- [x] done 8e38c52 (machines.rs: Butcher gets a buffer, flagged; looting as the tables add it (0..level a drop), fire aspect sets the animal alight and the bonus cooked; blade spends 25 E a strike; recipe key s lists the iron sword and the blades; slice11 butcher scenarios get a full buffer; self-test `butcher18_energy_and_kill_xp`)

### Task 17: Shearer — built-in shears, milking into a tank (cows, mooshrooms, goats), «Некуда сливать молоко», window rows and milk bar
- [x] done (slots: 9 wool; milk 1000 mB a milking into the touching tank, fluid `rustcraft:milk`; slice12 `util_shearer_shears`/`util_shearer_milks` updated)

### Task 18: Apiary — choice honey / combs in the window, «Нет пустых бутылок», hive rows
- [x] done (choice toggles, `farm::harvest_of`; levels on the model for the feeder, shearer and apiary (task 11 except the brewer: task 19))

### Task 19: Brewer — pages: potion + kind; automatic steps (nether wart → effect ingredient → redstone / glowstone / gunpowder / dragon breath); capacity pages; steps and fuel bar in the window
- [x] done 27ab28c (rules `utility::brew`: pages, chain, `next_step`; slots 3 bottles / 4 ingredients / powder / 3 out; window: 4 page buttons + 5 kind buttons, rows page and fuel (no room for a bar in 240 px); bottles on the model; self-test `brewer18_brews_through_the_chain`; stack module in machines: not made (a slot of 256 needs per-slot max stack in content menus, stream 4/3))

### Task 20: Experience condenser — 30 970 points, give 1 / 10 / all, take 1 / 10 / all, 2 E/s while moving, «Конденсатор полон», bar «Хранит», condenser keeps its points when broken
- [x] done df07e2f (2 E a press from its buffer; points kept on break is not in the handbook: left out; slice12 window test: six buttons and the bar)

### Task 21: Enchanter — level 1–30 chosen (− / + buttons), cost 1/2/3 by level, condenser must hold ≥ level levels, 10 s; archeo — the table of where the block lay (the toggle row becomes a line «Расчищает: …»), 10 s
- [x] done (level buttons −10/−1/+1/+10, `apply_enchantment` at the chosen level and its row; a block that remembers no place brushes by `found::default_table`)

## Part D — the projector and the builder

### Task 22: Projector — area by two wrench corners up to 32³, plan kept in the item on break, rotate / mirror, block states, compact plan encoding
- [x] done 47782db 4c05f7d (rules `plan`: `Area::of`, states with 18 kept properties, `turn_state`, `transformed`, `pack/unpack` runs; bundle `projector.rs` (new module); wrench on the projector arms it, two wrench clicks set corners; sneaking wrench lifts it into an item with custom data `rustcraft:plan`, placing restores it)

### Task 23: Projector — missing-block list with storage counts, «В руку», red cell for a foreign block, 4 E/s while the ghost is shown, «Нет сохранённой постройки», «Нет склада на магистрали»
- [x] done 4c05f7d (rows: remembers/turn, missing with storage counts; «В руку» from `storage::storage_by`; red stained glass in a foreign cell; `ghost_lit` off without energy)

### Task 24: Builder — blocks from the storage on the same magistral, 2 blocks/s, places block states, waits naming what is missing, «Клетка занята чужим блоком», window rows (layer N of M, progress, missing, skipped)
- [x] done 4c05f7d 914c6ac (from the storage on its magistral first, then own slots; self-test `builder18_turned_plan_from_storage`)

## Part E — hand-off

### Task 25: `mod/README.md` «Что проверить в игре: срез 18» (stream 5 section), last try green, report
- [x] done 47f1831 (README section), try green 2e5b7a1

## Part F — orders handed over from stream 1 (orders.rs is this stream's)

### Task 26: O11 — an order's product goes straight to the next machine
- Today an intermediate product rides to the storage and from there to the next step's machine (`orders.rs` `pull` with `own`, `queue.rs` `keep_for`). The product of a step whose next step runs on another machine goes from the first machine to the second.
- [x] done in s18-last Task 4: the product waits at its machine for the order (`orders::held`, the order's own stock in `Queue::round_with`) and rides from that machine straight to the next step's machine (`orders.rs` `deliver`); self-test `s18_o11_product_goes_straight_to_the_next_machine`.

### Task 27: O12 — the order brings its water
- A mixer step's water (and any fluid input) comes with the order from the storage's tanks / the line's fluid net, not only from the mixer's own tank (`orders::reachable`).
- [x] done (`fluids::bring_water`, added block in fluids.rs, flagged: when a mixer job starts, the water of its runs moves from the tanks `reachable` counts into the mixer's tank; no in-world self-test yet)

### Task 28: the autocrafter asks the storage for a missing ingredient
- An autocrafter on the storage's magistral, short of an ingredient of its page, gets it from the storage (an order-like pull of one craft's worth).
- [x] done 0eb4b03 (`orders::page_shortfall`, `orders.rs feed_crafters` each orders round, `crafter::pages_and_buffer` added to crafter.rs, flagged; straight into the buffer (no ride); self-test `orders18_crafter_asks_the_storage`)

### Task 29: all machines take part in orders
- Beyond crafter, furnace, crusher, alloy, mixer and (task 8) the kitchen: every machine whose recipes the book can name (the disintegrator, the brewer for its pages' potions…), or a recorded reason why not.
- [x] done as far as recipes exist: crafter, furnace, crusher, alloy, mixer and the kitchen (task 8). The rest make nothing a book recipe names by item: the brewer's potions are component stacks chosen by its pages, the enchanting machine and the archaeology table roll at random, the disintegrator un-makes, the repair stand mends, the farm machines gather; recorded under «Пределы движка» (not an engine limit: no recipe to order by).

### Task 30: order items visible in the housings
- The items of an order's shipment drawn moving through the housings along their route (`rules/src/orders/ship.rs` gives the route and the time), as the client can draw them (block-entity drawing of the housings).
- [x] done in s18-last Task 5: `rustcraft_rules::orders::ride` lays each shipment on the housings of its way (`net::topology::pipe_path`) as spans (faces in and out, game ticks in and out); `orders.rs::show_rides` writes them into the housings once a second and syncs them; `pipe18.rs` sends them in the update tag and draws them with `native_draw_item` (`orders.rs::draw_riding`); self-test `s18_order_items_ride_the_housings`. Plain item-net moves have no riding item and are not drawn (s18-last «Пределы движка»).
