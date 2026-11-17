# Slice 18, stream 6 «Монорельс и работники» Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** every MISSING / DIFFERS line of `docs/design/06-audit/rail-worker.md` becomes DONE in the game, or an engine limit recorded with evidence under «Пределы движка».

**Architecture:** rules in `mod/rustcraft/rules/src/worker/*` and `rules/src/monorail/*` with unit tests (TDD); the bundle (`workers.rs`, `monorail.rs`, `station.rs`, `explore.rs`) wires them to the game; every mechanic gets an in-world self-test in the new file `rules/src/selftest/s18_rail.rs` (one line added to `selftest/mod.rs`, at the end of `catalog()`, so no plot of another stream moves).

**Tech Stack:** Rust 2024 (Horizon native mod, horizon-mixins), the content mod (JSON recipes, models, lang), Python `assets/models/export_mc.py` for the worker's part models.

**Spec:** [docs/superpowers/specs/2026-11-16-handbook-parity-design.md](../specs/2026-11-16-handbook-parity-design.md), master plan [2026-11-16-handbook-parity.md](2026-11-16-handbook-parity.md), work list [docs/design/06-audit/rail-worker.md](../../design/06-audit/rail-worker.md).

## Global Constraints

- Horizon is pinned to `7e4e421224743469035ba0c34fc21e569e64a17d`, Minecraft 26.3. Horizon itself is not changed.
- The game follows the handbook (`docs/showcase/js/data.js`, `ui.js`, `scenes.js`). The handbook changes only for the spec's contradictions and for an engine limit that has been checked in `SRC`.
- Text and docs are in Russian per `CONTRIBUTING.md` (lang ru_ru and en_us both). Commit messages are in English. Commit email: `128125070+Betreazen@users.noreply.github.com`. Every commit ends with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- No Mojang assets in git. Shared files (`content.rs`, `server.rs`, `selftest.rs`, `rules/src/selftest/mod.rs`, `rules/src/tuning.rs`, `rules/src/modules.rs`, lang json, `horizon-mixin.toml`) are only appended to, in this stream's own blocks.
- Checks: `cargo test --release -q` in `mod/rustcraft/rules`; every 3–5 tasks `joint.ps1 try -Mod rustcraft -Path <worktree> -Owner s18-rail-worker` (all RC, PCT, SC self-tests green). Never publish, never stage.
- Stream 3 owns the general tier mechanism `rustcraft_rules::tiers`. Until it is merged the chassis uses `rustcraft_rules::worker::chassis` with the same shape (tier items, a swap that keeps contents); the integrator switches it after the merge.
- Files of other streams this stream must touch (kept to small, marked blocks): `builder.rs` and `rules/src/utility/mod.rs` (portal dock, stream 5), `base.rs` dock windows (stream 7). Each such edit is listed in the final report.

## Files

- `rules/src/worker/mod.rs` — riding on every mount, zone by mount, speed by tier, filter, limit, item form.
- `rules/src/worker/chassis.rs` (new) — chassis tiers 1–3 (3 / 5 / 7 sockets), the U4 swap.
- `rules/src/worker/config.rs` (new) — the server settings file `config/rustcraft-server.toml` (`workers_per_player`).
- `rules/src/worker/plants.rs` — columns planned from their top (any zone), filter by plant.
- `rules/src/worker/bore.rs` — height module, tier 2–4 rails from the hold.
- `rules/src/worker/wire.rs` (new) — the `rustcraft:workers` note: seat, yaw, tool, mount, busy, aim.
- `rules/src/monorail/shape.rs`, `capsule.rs`, `window.rs`, `wire.rs` — climb fork, floor→wall half, cabin room, scheme labels and lift floors, atlas on the scheme.
- `rules/src/selftest/s18_rail.rs` (new) — this stream's scenarios.
- `workers.rs`, `station.rs`, `monorail.rs`, `explore.rs` — wiring, drawing, window.
- `assets/models/export_mc.py` — worker part models for every tool part and every mount.

---

### Task 1: The worker rides every rail (T3, W2, H-wall, lift)

Audit lines: chassis «Ставится на монорельс», «Ездит по любому монорельсу»; monorail T3 for the worker; lift «Лифт для работников»; tool_reaper «Какао по стволу, ламинария, бамбук», «Этажи полей»; tool_hold / dock_load «Между базами»; dock_unload «Ставится вплотную» (any rail).

**Files:** `rules/src/worker/mod.rs`, `rules/src/worker/plants.rs`, `workers.rs`, `rules/src/selftest/s18_rail.rs`, `rules/src/selftest/mod.rs` (one line), lang (`rustcraft.worker.place_on_rail`).

**Interfaces (produces):**
- `worker::ways(net, p) -> Vec<Pos>`: every joined rail (any mount).
- `worker::mount(net, p) -> Face` (YNeg when unknown).
- `Worker::zone_cells(&self, cell: Pos, mount: Face, t) -> Vec<Pos>`: depth along `mount.opposite()` 0..height, ±side across; a reaper adds the wall layer (depth −1) without the mount block.
- `Worker::zone_span(&self, cell, mount, t) -> (i32, i32)`: lowest and highest y of the zone.
- `plants::plan(view, pos, bottom, top)`: a column is planned from its top cell (or from the zone's top when it goes higher), cut down to `max(base + 1, bottom)`.

- [x] Step 1: failing tests in `worker/mod.rs`: `a_worker_rides_up_a_wall_rail_and_onto_the_ceiling`, `the_zone_grows_away_from_the_mount`, `docks_beside_a_wall_rail_serve`; in `plants.rs`: `a_column_is_cut_from_its_top_inside_the_zone`, `kelp_under_a_ceiling_rail_is_reached`.
- [x] Step 2: `cargo test` RED.
- [x] Step 3: implement (drop `floor()`, 3-D straight-on, zone by mount, column plan by top).
- [x] Step 4: GREEN.
- [x] Step 5: wiring: chassis placed on any monorail; `reap` and `saw`/`grab` take the zone by mount; self-tests `worker_reaper_climbs_a_trunk` (cocoa, vertical wall rail), `worker_reaper_hangs_over_kelp` (ceiling rail), `worker_reaper_rides_up_bamboo`, `worker_floors_share_a_dock_by_a_lift`.
- [x] Step 6: commit.

### Task 2: Worker models on every mount (T3 for the worker)

**Files:** `assets/models/export_mc.py` (`WORKER_PARTS` + `above`, `side`), models `worker_above.json`, `worker_side.json`, `rules/src/worker/wire.rs`, `workers.rs::notes`, `station.rs::draw_worker`.

- [x] Step 1: test `wire.rs::a_note_reads_back` (seat, yaw, tool, mount, busy, aim).
- [x] Step 2–4: RED → GREEN.
- [x] Step 5: the bracket drawn by mount (side bracket turned to the wall, as the capsule's); commit.

### Task 3: Worker speed by rail tier (T4)

- [x] Test `a_worker_rides_as_fast_as_its_rail_tier` (8 / 14 / 22 / 32 bl/s, every cell still worked: overflow carried, the bundle rides again in the same tick while `carry > 0`).
- [x] Implement `Worker::speed(net, to, t)` from `RailSpec::speed_of`; `#[serde(default)] carry`.
- [x] Commit.

### Task 4: The worker as one item (W3)

- [x] Tests: `a_worker_packs_into_its_chassis_and_back` (tool, upgrades, hold, charge, filter, tier survive the JSON in `custom_data` `rustcraft:worker`).
- [x] Wiring: the wrench with sneaking gives one chassis stack (its tier) carrying the worker; placing it restores; the hand item shows a tooltip line count. Self-test `worker_picked_up_as_one_item` (Bench `worker_as_item(id, cell)`).
- [x] Commit.

### Task 5: Chassis tiers 3 / 5 / 7 and the swap (U4)

- [x] Tests in `worker/chassis.rs`: `each_tier_has_its_sockets`, `a_higher_chassis_swaps_keeping_tool_and_hold`, `a_lower_or_same_chassis_does_not_swap`.
- [x] Items `rustcraft:chassis_2` (crystallite), `rustcraft:chassis_3` (starsteel) in `content.rs` (own block), recipes `b b/bsb/bbb`, lang, item models; window cells for up to 7 sockets; right click on a worker with a higher chassis swaps, the old chassis back to the player. Self-test `worker_chassis_swaps_up`.
- [x] Commit.

### Task 6: Worker limit per player and the server settings file (W8)

- [x] Tests: `config::reads_the_limit`, `config::a_bad_file_keeps_the_default`, `Fleet::owned_by`, `the_limit_refuses_one_more`.
- [x] Hooks `register_configs`, `config_loading`, `config_reloading` (API 397) in `horizon-mixin.toml` (own block) → `rustcraft-server.toml` `workers_per_player = 16`; refusal «Достигнут предел работников: %s» . Self-test `worker_limit_per_player`.
- [x] Commit.

### Task 7: Harvest filter H6 and the scoop's filter

- [x] Tests: `a_filtered_reaper_takes_only_pumpkins`, `a_filtered_scoop_digs_only_sand`.
- [x] `Worker.filter: Vec<String>`; in the window: shift-click (or a click on the filter row) puts an item into the filter, a click on a filter cell clears it. Self-tests `worker_reaper_filter_pumpkins`, `worker_scoop_filter_sand`.
- [x] Commit.

### Task 8: The chassis window as the handbook draws it

- [x] Engine check: API 78 (`native_screen_render_post`, drawing over a server menu by title) in SRC.
- [x] Panel drawn over the menu: «Инструмент», «Улучшения и модули» (3/5/7), bars «Заряд» and «Трюм», «Груз», «Фильтр», status line; title by chassis tier; tool hint lists all 8 tools.
- [x] Commit.

### Task 9: Tool models and animations

- [x] Export every tool part (`worker_<tool>` static, moving parts `worker_<part>` with pivots) from `transport.py`.
- [x] Note carries busy + aim; `draw_worker` turns `aim_yaw_*` to the target, slides `reach_*`, spins `saw_disc`, `reaper_reel`, `bore_bit`, `map_drum`, opens the grab, curls the bucket, recoils the gun — `poses.js:150-175`. Body colour by tool.
- [x] Rules test for the pose math (`wire::pose_of`).
- [x] Commit.

### Task 10: The cartographer on the station scheme, «Вокруг пути всё нанесено»

- [x] Test `Note::AllMapped` after `idle` cells with nothing new; `window::atlas_marks` projection test.
- [x] Listing carries the atlas sites (biomes, structures, chunk count) of the network's span; the scheme draws them. Self-test `worker_map_says_all_mapped`.
- [x] Commit.

### Task 11: Portal dock as a worker's dock (PD2), fluids, «Буфер дока полон», chest beside, buffer 27

- [x] The worker's unload targets include portal docks (into its send slots 0..18); the unload docks' push hands the portal dock's received slots to a container beside it; «Буфер дока полон» when its pair's store has no room.
- [ ] Fluids, 16 buckets, and the window's «Тот мир · выгружен» (stream 5's builder.rs and utility rules: left to stream 5, see the report).
- [x] Self-test `worker_unloads_into_a_portal_dock`.
- [ ] Commit.

### Task 12: Dock windows' lines, dock_load takes from the chest beside it

- [x] «Отдаёт груз · в магистраль/в сундук», «Принято за час», «Берёт груз · из сундука рядом», «Грузит · всё подряд», «Заряжает · Лесоруб · 46%».
- [x] Load dock pulls from a container beside it. Self-test `worker_load_dock_takes_from_a_chest`.
- [x] Commit.

### Task 13: The bore: height module, tier 2–4 rails; the guard shoots while it stands; shooter recipe

- [x] Tests `the_bore_lays_the_best_rail_it_carries`, `the_height_module_keeps_the_level`, `a_guard_aims_while_standing`.
- [x] Recipe `tool_shooter`: ` x /xbx/ r `, x = bow, b = crystallite.
- [x] Commit.

### Task 14: Monorail shape, cabin room, scheme labels and lift floors

- [x] Scheme labels set apart, lift floors told apart: `window::place_labels`, test `lift_floors_and_near_names_do_not_overlap`.
- [x] Climb fork and the floor rail's half up a wall: `Shape::ClimbFork(k, fork)` (`c<k>f<fork>`: climb + hub + half to its foot + halves to the rails beside its lower cell) and `Shape::Rise(k, mask)` (`r<k>s<mask>`: a floor rail under a wall rail, a ceiling rail over one: its halves with the wall's side and the piece up/down that wall). 64 shape values, the first 20 unchanged. Rise models `monorail_t<n>_rise<k>` from `export_mc.py` (`rail_rise`, written there, not in transport.py, so the guide's rail.glb is unchanged); blockstate regenerated. Tests `a_climb_cell_forks_to_a_rail_beside_it`, `a_ceiling_climb_forks_too`, `a_floor_rail_rises_under_a_wall_rail`, `a_ceiling_rail_drops_over_a_wall_rail`, `the_shape_values_are_sixty_four_and_distinct`; self-test `monorail_climb_forks_and_floor_rises_to_a_wall`. Commit e2380aa.
- [x] Capsule: `MODEL_SCALE` 2.5 → 2.0, so the cabin's 8 pixels are one block and it keeps inside a one-block passage (the guide draws it inside one cell; the guide's 1× would leave a seated player 1.4 blocks tall in a 0.6-block cabin, which T8/T9 «сажает игрока» rule out, so the scale is the largest that keeps it in a one-block passage). The rider is centred (`FEET_OVER_FLOOR` derived, now −0.2): shoes and hair show 0.1 block past floor and roof. `WALL_SHIFT` 0. `room_cells(cell, mount, ways)`: every block the cabin and its rider reach into deeper than 1/16 across its width and floor-to-roof, for each way it faces there (`ways_at`); its ends along the way stay over the next rail of the way (decision: at a stop and on a corner's arc the cabin turns about its middle, the ends' third of a block is not checked; README gap line). Going straight up or down a wall the cabin faces along the wall (`along_wall`, station.rs `heading`). Route tests take `FEET_OVER_FLOOR` instead of 0.05. Self-test `monorail_lift_needs_room_for_the_cabin`. Commit ff9437b.
- [x] Commit.

### Task 14b: Tests for what was left unchecked

- [x] `worker_grown_block_reaches_the_archeology_table`: a grab's hold of grown suspicious sand → unload dock → magistral housing → archaeology table (not beside the dock); the table brushes sand out of it and sends no PCT `brush` (ST4 → ST5).
- [x] `worker_guard_kill_drops_where_the_mob_fell`: a weak zoglin (rotten flesh 1–3, no condition) shot dead by a guard: its loot lies on the plot. Finding (DIFFERS, not an engine limit): loot with `killed_by_player` (spider eye, blaze rod, zombie rares) never drops, because the guard's and turret's shot has no attacker (`base.rs` «Shots», so PCT does not count the kill as the player's own) and `horizon-world/src/loot.rs:1264` needs a player in the context. Fix path for base.rs's owner: a mod loot condition `rustcraft:killed_by_guard` (`native_loot_condition`, API 133, matched on `ctx.place`, API 156, against deaths `on_living_death` records) OR-ed into every `killed_by_player` of the mob tables in `native_loot_table_load` (API 75); the hooks run in `horizon-world`, so the shot state must reach that crate. Not done here: base.rs's shots are not this stream's code.
- [x] W5 board over the mod model: cannot be self-tested (the server's `CustomName` is checked; whether the client draws it over the model needs eyes). For the integrator's bots: a screenshot of a worker from 3–5 blocks, README item 12.
- [x] Commit 486206f.

### Task 15: Hand-off

- [x] `mod/README.md` «Что проверить в игре: срез 18» — this stream's part.
- [x] Last `try` green: 35ceb15, RustCraft 156 passed 0 failed, PCT 42/0, SoulsCraft 87/0 (exit 0).

## Commits

- Tasks 1–4 and Task 9's models and poses: db932ff. Task 5: 3259831. Task 6: 2528e24.

## Left after this run (for a resumed run or the owning stream)

- Portal dock: fluids with a 16-bucket buffer, the window's «Тот мир · выгружен», a 27-slot buffer laid out as the handbook's window (now 18 to send + 9 received). The code is stream 5's (`builder.rs`, `rules/src/utility`).
- The miner putting into a load dock (`mining.rs`, stream 2).
- The guard's (and turret's) kill drops no `killed_by_player` loot: fix path in Task 14b, base.rs's owner.
- The cartographer's structure detection limits (`/place structure`, deep structures): unchanged.
- W5 board over the mod's model: needs a look in the game (README item 12).

## Пределы движка

None found: every line above is doable with the Horizon API at 7e4e421 (mod configs API 397, drawing over a server menu API 78, standalone models API 151, mob drawing API 110 were all used).
