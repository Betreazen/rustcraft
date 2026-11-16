# Срез 18, поток 1 «Магистраль» — план

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** every MISSING / DIFFERS line of `docs/design/06-audit/pipe.md` (sections start, pipe, mat) becomes DONE in the game or a recorded engine limit.

**Architecture:** logic in `mod/rustcraft/rules` (unit tests, `cargo test --release -q`), wiring in the bundle (`mod/rustcraft/*.rs`), data in the content mod (`mod/content/...`). Each mechanic gets a self-test scenario in `rules/src/selftest/pipe18.rs`, registered once in `selftest/mod.rs` (`all.extend(pipe18::ALL)`). Shared files are only extended with blocks marked `// s18/pipe`.

**Tech Stack:** Rust 2024 (Horizon native mod), JSON content mod, PowerShell (`joint.ps1 try`).

**Spec:** `docs/superpowers/specs/2026-11-16-handbook-parity-design.md`; master plan `docs/superpowers/plans/2026-11-16-handbook-parity.md`.

## Global Constraints

- Horizon `7e4e421`, Minecraft 26.3, Horizon not changed.
- The game follows `docs/showcase/js/data.js`, `ui.js`, `scenes.js`.
- Keys: Z magnet, B backpack, ; night vision; no new key without checking all pack mods.
- Texts ru_ru + en_us; commits in English, email `128125070+Betreazen@users.noreply.github.com`, last line `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Check: `pwsh -NoProfile -File "C:\Claude code projects\horizon-joint-dev\joint.ps1" try -Mod rustcraft -Path "C:\Claude code projects\rustcraft-s18\pipe" -Owner s18-pipe` every 3–5 tasks.
- Commit with `git commit -- <paths>`; never push, never merge.

## Cross-stream notes

- **Tiers (U2).** Stream 3 owns `rustcraft_rules::tiers`. Until it is merged, node tiers live in `rules/src/net/node_tier.rs` with the same shape: `tier_of_item(&str) -> Option<u8>`, `item_of_tier(u8) -> &'static str`, `swap(placed: u8, held: u8) -> Option<u8>` (the tier that comes back to the hand). The integrator points the node at `tiers` after the merge.
- **Line-boost module.** Stream 4 adds `Module::Line` / the item `rustcraft:module_line` and `Holder::Node`. Until then the node's sockets accept the item id `rustcraft:module_line` through `rules/src/net/line_boost.rs::LINE_MODULE`; the rule `boost(modules) = 1 + 0.5 × modules` lives there.
- **Crystal shard.** Recipes use `minecraft:amethyst_shard`; the integrator switches them to `rustcraft:crystal_shard` (stream 2).
- **Not mine:** crusher on ore blocks (`machine.md:50`, stream 3); energy channel limit per tick (`energy.md:169`, stream 2, `net/energy.rs`); druses, shard, seed growing (stream 2).

## Tasks

### Task 1: Recipes of the housing, the node and the wrench
Files: `mod/content/data/rustcraft/recipe/{housing,node,wrench}.json`; `rules/src/selftest/pipe18.rs` (new), `selftest/mod.rs` (+2 lines).
- [ ] housing `aga/g g/aga` (coppersteel, glass) → 8; node `aca/csc/aca` (coppersteel, copper ingot, amethyst shard) → 1; wrench `a a/ a / a ` (coppersteel) → 1.
- [ ] rules test: `pipe18::tests::recipes_match_the_handbook` reads the three JSON files (`include_str!`) and checks pattern, keys and count.
- [ ] self-test `pipe_recipes_craft`: `Bench::craft` of each grid gives the item and count.
- [ ] commit.

### Task 2: Node tiers 2–4 (items, recipes, loot, swap)
Files: `rules/src/net/node_tier.rs` (new), `rules/src/net/mod.rs`; `content.rs` (items `rustcraft:node_2..4` with `place_block: NODE`, `NODE_ITEMS`), `transport.rs` or new bundle part `pipe18.rs` (placement tier, swap), `hook_state_for_placement.rs` (+1 line); content: `recipe/node_{2,3,4}.json`, `loot_table/blocks/node.json` by tier, `items/node_{2,3,4}.json`, lang.
- [ ] RED/GREEN: `node_tier` tests — item ↔ tier, swap only to a higher tier, gives back the old tier's item.
- [ ] placement sets `tier` from the item (as `monorail::state_for_placement`).
- [ ] right click with a higher node item on a node: state tier changes in place (block entity and facade kept), old item to the hand / inventory, message.
- [ ] loot: `minecraft:alternatives` by `tier` like `loot_table/blocks/monorail.json`.
- [ ] self-test `pipe_node_tiers`: craft grids of tiers 2–4, a `node[tier=3]` serves 128 blocks; swap through testkit.
- [ ] commit.

### Task 3: Tier-4 channels
Files: `content.rs` (`CHANNEL_ITEMS` 9 → 12), `recipe/{item,fluid,energy}_channel_4.json` (starsteel), `items/*_channel_4.json`, lang.
- [ ] rules test (tuning already has tier 4): `channel_item(kind, 4)` names `_4`.
- [ ] self-test `pipe_channels_tier4`: crafts; a tier-4 item net carries 256/s (net min_tier 4 → `items_of(4)`).
- [ ] commit.

### Task 4: Chests work both ways by default (quickstart 5)
Files: `rules/src/net/items.rs` (`Joint.auto`), `joints.rs` (`item_joints(.., auto: &BTreeSet<Pos>)`), `fluids.rs`/`stock.rs` constructors, `transport.rs` (vanilla container with no slice-2 mode → `Mode::Both` + auto).
- [ ] RED: `an_auto_chest_feeds_a_machine_and_takes_its_products`, `auto_chests_never_trade_between_themselves`, `auto_chest_is_the_last_receiver`.
- [ ] GREEN in `items::tick`.
- [ ] self-test `pipe_chest_to_furnace`: chest → furnace → chest moves ore and ingots without any setting.
- [ ] commit.

### Task 5: Texts — glossary modes, energy diagnosis, «Под фальш-панелью»
Files: lang `ru_ru.json`/`en_us.json` (mode names «в машину / из машины / в обе стороны / выключен»; `rustcraft.machine.slowed` «Не хватает энергии: машины замедлены на %s»), `server.rs` facade diagnosis line.
- [ ] diagnosis: a housing or node under a facade shows «Под фальш-панелью · …» first.
- [ ] commit.

### Task 6: Facade — kept on break, F4 collision, sneaking wrench then takes the housing
Files: `content.rs` (housing `covered` state property + `collision_shapes` for `covered=true` = full block), `builder.rs` facade part (set/clear `covered`), `server.rs` (on break: give facade + look), `server.rs` take_channel order.
- [ ] rules: `rules/src/housing.rs::wrench_takes(covered, channels)` → Facade | Channel | Housing; tests.
- [ ] self-test `pipe_facade_keeps`: covered housing has `covered=true`; property clears on wrench.
- [ ] commit.

### Task 7: Node window (sheet) + line boost
Files: `rules/src/net/node_window.rs` (rows of the window from topology: axis lines, housings count, farthest N of M, throughput with boost, balance), `rules/src/net/line_boost.rs`, sheet kind wiring, `content.rs` (node → furnace template? see Пределы), transport.rs throughput × boost.
- [ ] RED/GREEN: `farthest_housing_of_a_node_line`, `boost_raises_the_net_throughput`, `a_net_takes_the_weakest_sections_boost`.
- [ ] window: line choice (3 toggles / one on a hub), balance, rows, 3 sockets, status.
- [ ] self-test `pipe_node_window`, `pipe_line_boost`.
- [ ] commit.

### Task 8: Wrench on machines — rotate; sneaking lifts with contents
Files: `rules/src/net/wrench.rs` (`rotate(facing) -> facing`, what a click does), `transport.rs` right-click path.
- [ ] RED/GREEN: rotation order N→E→S→W; up/down stay.
- [ ] rotate: `facing` property set in place; lift: block entity saved into the item's custom data, restored on placing.
- [ ] self-test `pipe_wrench_rotates`.
- [ ] commit.

### Task 9: Wrench in hand — bottlenecks and see-through facades (F5)
Files: `rules/src/net/bottleneck.rs` (housings whose channel tier is the net's min and lower than another's on the net), client drawing in the facade draw path (skip facade while the local player holds a wrench; outline bottleneck housings).
- [ ] RED/GREEN `bottlenecks_are_the_weakest_channels_of_a_mixed_net`.
- [ ] commit.

### Task 10: Coloured channel strips on the housing
Files: housing update tag with `slots`, `native_block_entity_submit` draws tinted standalone strip models per slot (anim.rs pattern), models `housing_strip_{0..3}.json`.
- [ ] rules: `strip_colour(kind) -> u32` (item #ffa53c, fluid #4c9bf5, energy #ffe14a, from data.js).
- [ ] commit.

### Task 11: Joint window as the handbook panel
Files: `window.rs` (rows «Гнездо / Канал / Направление / Фильтр / Приоритет»), fluid channel filter and priority, closes when the player walks away (> 8 blocks).
- [ ] RED/GREEN `joint_window` tests for distance and fluid filter rows.
- [ ] commit.

### Task 12: Housing channels drop on explosion; XP reports; catalysts
- [ ] channels of a housing broken by an explosion drop (rescan finds it gone).
- [ ] furnace/alloy/crafter `Report::Craft`/`smelt` to Path-of-craft-tree (stream 3 files — additive block only).
- [ ] catalysts in alloy smelter / mixer (stream 3 files — additive).
- [ ] commit.

### Task 13: README «Что проверить в игре: срез 18» (поток 1), last try, report.

## Пределы движка

(filled in as found, with SRC evidence)

## Status log

(each finished task: commit hash)
