# Срез 18, волна 2, «w2-craft» — крафт и склад после второго аудита

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. A resumed agent continues from the first unticked task; the ticks below are the truth.

**Goal:** every line of «Ещё не сделано» of `docs/design/06-audit/after/craft-storage.md` and the portal dock lines of `after/rail-worker.md` becomes DONE (or a limit proven from SRC), plus the failing self-test `orders18_bread_through_the_kitchen`.

**Architecture:** rules in `mod/rustcraft/rules` (`utility`, `storage`, `orders`, `tiers` rows) with unit tests; the bundle (`utility.rs`, `farm.rs`, `butcher.rs`, `builder.rs`, `projector.rs`, `storage.rs`, `terminal.rs`, the stream-5 block of `fluids.rs`) only wires it. Each mechanic gets a scenario in `rules/src/selftest/craft18.rs`.

**Tech Stack:** Rust 2024, Horizon native mod API at `7e4e421` (SRC), JSON content mod, `py tools/tiers.py`.

**Spec:** `docs/superpowers/specs/2026-11-16-handbook-parity-design.md`; master plan `docs/superpowers/plans/2026-11-16-handbook-parity.md`; wave brief (integrator's scratchpad).

## Global Constraints

- Horizon is pinned to `7e4e421224743469035ba0c34fc21e569e64a17d`, Minecraft 26.3. Horizon itself is not changed.
- The game follows the handbook (`docs/showcase/js/data.js`, `ui.js`, `scenes.js`).
- Text in Russian per `CONTRIBUTING.md` (ru_ru and en_us). Commits in English, email `128125070+Betreazen@users.noreply.github.com`, ending `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`, with `git commit -- <paths>` on `s18/w2-craft` only.
- Shared files (`content.rs`, `server.rs`, `selftest.rs`, `rules/src/selftest/mod.rs`, `rules/src/tuning.rs`, `rules/src/modules.rs`, `rules/src/tiers.rs`, `fluids.rs` outside its stream-5 block, lang json): only add own lines.
- Fast check: `scratchpad/chk-w2-craft.sh <worktree>` (cargo check of the bundle on clean Horizon). Bench try every few tasks on a snapshot copy `scratchpad/snap-w2-craft`, owner `s18-w2-craft`, log `scratchpad/try-w2-craft-N.log`.
- Rules tests: `cargo test --release -q` in `mod/rustcraft/rules`.

## Hand-offs accepted (from w2-machine-module, audit `after/module.md`)

- Kitchen and brewer: +2 hidden input cells a capacity module (Task 6).
- Butcher: the economy module slows the strike ×0.85 (Task 5).

## Пределы движка / решения

- Ghost translucency: unchanged limit of the first plan (`native_draw_block` / `native_draw_model` have no alpha, SRC docs/native-mods.md:920-930, 1247).
- Orders through the brewer, the enchanting machine, the table, the disintegrator, the stand and the farms: not an engine limit; their products are chosen by window settings or chance, not by a recipe the book can plan. Kept as the decision of the first plan (Task 14 says so in the README).

---

### Task 1: the kitchen spends what it cooks (fixes `orders18_bread_through_the_kitchen`)

Root cause: `utility.rs::cook` took `slots::take_from(&mut cells, item, n)` as «how many came from the cells», but it returns how many were still to take. With empty cells `n - got = 0`: the dish was made and nothing was taken from the slots, so the order job's inputs never emptied (`orders.rs::inputs_spent`), the order stayed with `done: 0`, and the kitchen cooked again for free. Flaky in the bench only because the net sometimes pulled things around.

**Files:** `rules/src/utility/kitchen.rs` (`take_ingredients`), `utility.rs::cook`, `rules/src/selftest/craft18.rs` (cake scenario checks the dry parts are gone).

- [ ] RED: `kitchen::tests::ingredients_come_from_the_cells_first_then_the_slots`: cells hold 1 wheat, need 3 → takes 1 from cells, returns 2 for the slots; empty cells → 3 for the slots.
- [ ] GREEN: `pub fn take_ingredients(cells: &mut Vec<Slot>, need: &[(String, u32)]) -> Vec<u32>` (for each, what the slots give); `cook` uses it.
- [ ] Scenario `kitchen18_cake_with_milk_from_a_tank`: after the cake, sugar, egg and wheat are 0 in the inputs.
- [ ] Commit `fix: the kitchen takes the ingredients it cooks from its slots`.

### Task 2: the projector takes the wrench for its corners

**Files:** `rules/src/net/wrench.rs` (`KEPT` += `rustcraft:projector`, test), `projector.rs` (doc of `lift`: now the sneaking click's), scenario `builder18_projector_corners` (wrench press through the bench: `on_wrench` arm → two corners → `area` int array of the block entity).

- [ ] RED: `the_projector_is_kept_so_its_wrench_sets_corners` (`!handles("rustcraft:projector")`).
- [ ] GREEN; the plain click no longer turns it (`pipe18::rotate_machine` checks `handles`), `projector::on_wrench` arms it; the sneaking click lifts it with `projector::lift` (the plan rides in its item, `projector::on_place` puts it back).
- [ ] Self-test of corners through the bench method `wrench_click` if one exists, else `projector_area` rule test only (the hook path is the integrator's bot check).
- [ ] Commit `fix: the wrench sets a projector's corners instead of turning it`.

### Task 3: the terminal takes the crystallite and starsteel stack modules

**Files:** `rules/src/storage/modules.rs` (`STACK_MODULES` ids → `rustcraft:module_stack_2`, `_3`), test `stack_modules_are_the_items_of_the_mod` (each id is in `crate::modules::ITEMS`), scenario `storage18_stack_module_tier_two` (crystallite module → stack 1024).

- [ ] RED test, GREEN, scenario, commit `fix: the terminal takes the stack modules of crystallite and starsteel`.

### Task 4: the filter module item

**Files:** `content.rs` (own block: item `rustcraft:module_filter`), `mod/content/assets/rustcraft/items/module_filter.json`, `models/item/module_filter.json`, `textures/item/module_filter.png` (generated from the module chip: tinted), recipe `data/rustcraft/recipe/module_filter.json` `b b| h |b b` (b sparksteel, h hopper — the handbook names no recipe; the module chip pattern of `mod_cap` with a hopper as the filter), lang ru «Модуль фильтра» / en «Filter Module», rules test `the_filter_module_is_an_item` (content list test in rules: `storage::modules::FILTER` in `EXTRA_ITEMS`).

- [ ] Item, model, texture, recipe, lang; commit `feat: the filter module item, so the «своя» tab can be filled`.

### Task 5: tiers 3–4 for repair, kitchen, fishery, feeder, butcher, shearer, apiary

**Files:** `rules/src/tiers.rs` (own block of 7 `Family::new(.., 2)` rows), `py tools/tiers.py` output (recipes `<b>_3/_4`, item models, loot, blockstates with stripe, lang), `rules/src/machines/tiered.rs` (`tier_ticks(u, tier, step)`: a family machine not paced by nature works `tiers::speed` faster), `utility.rs::paced` and `butcher.rs` (TIER_KEY copy into the block entity so socket presses see the tier; butcher's strike faster by tier and ×0.85 slower with economy), scenario `util18_kitchen_tier_four_has_four_sockets`.

- [ ] RED: `craft_machines_have_tiers_three_and_four` (family_of_block for the 7, sockets(4)=4, item `rustcraft:kitchen_4`), `tier_speeds_a_craft_machine_but_not_a_farm`, `butcher_strike_ticks` (economy ×0.85 slower, tier faster).
- [ ] GREEN, tiers.py run, scenario, commit.

### Task 6: the kitchen's and brewer's capacity cells (+2 hidden input cells a capacity module)

**Files:** `rules/src/utility/kitchen.rs` (`stash` gets `open = INPUTS·stacked + 2·capacity`), `rules/src/utility/brew.rs` (same for reagents), `utility.rs` (`kitchen_stash`, `brew_stash` pass capacity).

- [ ] RED: `a_capacity_module_gives_two_hidden_cells` (no stack module, 1 capacity: two full visible stacks go into two cells; module out → they spill back).
- [ ] GREEN, commit.

### Task 7: energy as the handbook says (E/s × step seconds)

**Files:** `rules/src/utility/mod.rs::step_energy`, test `energy_a_second_is_the_handbooks`: repair 8, kitchen 8, brewer 8, enchanter 32, archeo 8, fishery 4, feeder 2, shearer 4, apiary 2, builder 16, projector 4 (E/s = step_energy / (step_ticks/20)).

- [ ] RED, GREEN (repair 40, kitchen 32, brewer 160, enchanter 320, archeo 80, fishery 4 a 1 s step, feeder 40, shearer 20), commit.

### Task 8: the fishery's own 5×5 water

**Files:** `rules/src/utility/farm.rs` (`water_taken(me, others) -> bool`: another station within 4 blocks on x and z at the same height, and lower in block order, keeps the patch), `farm.rs` fishery step (`Shown::Blocked("fishery_shared")`), lang «Участок воды занят другой станцией», scenario `fishery18_two_on_one_patch`.

- [ ] RED/GREEN, scenario, commit.

### Task 9: projector status «Нет склада на магистрали»

**Files:** `projector.rs` (status: ghost shown, plan has missing blocks and no storage on the magistral → `Shown::Blocked("projector_no_storage")`), lang key exists (`rustcraft.util.projector.no_storage`).

- [ ] Commit.

### Task 10: kitchen and shearer milk through a fluid channel

**Files:** `fluids.rs` stream-5 block (`touching_group` → the tanks touching it or behind a fluid joint of a net it is a joint of: takes from joints not `Into` for the kitchen, gives into joints not `From` for the shearer), one line in `fluids::default_mode` (kitchen: takes, shearer: gives; their joints carry no flow of the net: `Inventories` gives them no room and offers nothing), scenario `kitchen18_milk_through_a_channel`.

- [ ] Implement, scenario, commit.

### Task 11: storage protrusion highlighted

**Files:** `rules/src/storage/mod.rs` (`Shape::Extra(cells)`: when the biggest full box inside the cells leaves fewer cells outside it than the bounding box has holes), `storage.rs` (writes `extras` like `holes`, draws red glass over each extra cell at 1.04), tests `a_cell_sticking_out_is_the_extra_one`, `a_hole_is_still_a_hole`, scenario `storage18_terminal_marks_the_extra_cell`.

- [ ] RED/GREEN, scenario, commit.

### Task 12: the order queue shows the list of chains

**Files:** `terminal.rs` (the queue part lists every order: its line, its chain of steps with the current one marked, its wait; scrolled when it does not fit), test in `rules/src/storage/wire.rs` or terminal rules for the rows.

- [ ] Implement, commit.

### Task 13: bars for the kitchen's milk and the brewer's fuel

**Files:** `rules/src/utility/mod.rs::bars` (kitchen 2, brewer 2), `utility.rs::craft_sheet` (milk bar «N из M вёдер», fuel bar), window rows fitted to 240 px.

- [ ] Commit.

### Task 14: portal dock — fluids (16 buckets), window rows, one 27-slot buffer

**Files:** `rules/src/utility/transit.rs` (fluids in the store, 16-bucket room a dock), `builder.rs` (dock takes fluid from the tanks touching it / its fluid channel into its 16-bucket buffer, sends it to the pair, gives what came into a tank; rows «Пара · док в <мир> · связан», «Тот мир · загружен/выгружен», tank bar, status «Копит груз: другой мир выгружен» when the pair's level is not loaded and the store holds its load), `utility.rs` sheet for the dock, lang, tests `fluid_goes_to_the_pair_up_to_sixteen_buckets`.

- [ ] RED/GREEN, scenario `dock18_fluid_to_the_pair`, commit.

### Task 15: texts

**Files:** `mod/README.md` (stale lines of «Что срезы пока не умеют»), `butcher.rs` header, `projector.rs::lift` doc.

- [ ] Commit.

### Task 16: bench try, plan ticks, report

- [ ] try on a snapshot; all RC / PCT / SC green; plan «Try status»; report.

## Try status

(filled in)
