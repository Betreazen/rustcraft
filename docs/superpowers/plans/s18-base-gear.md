# Срез 18, поток 7 «База и снаряжение» — план

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** every MISSING / DIFFERS line of `docs/design/06-audit/base-gear.md` (categories anchor, turret, armor, pmodules, jetpack, blade, eshield, rifle, scout_compass, backpack, harpoon, zipline, enchants) is DONE in the game or a recorded engine limit.

**Architecture:** logic in `mod/rustcraft/rules` (unit tests first), the bundle (`gear.rs`, `base.rs`, `backpack.rs`, `zipline.rs` and new hook files of this stream) only wires it. Each mechanic gets a self-test scenario in `rules/src/selftest/gear18.rs` (new, this stream's) with the bench methods it needs added at the end of the `Bench` trait and in `selftest.rs`.

**Tech Stack:** Rust (Horizon 7e4e421 native mod, horizon-mixins), content mod JSON, Python + Pillow for generated textures.

**Spec:** [docs/superpowers/specs/2026-11-16-handbook-parity-design.md](../specs/2026-11-16-handbook-parity-design.md); master plan [2026-11-16-handbook-parity.md](2026-11-16-handbook-parity.md).

## Global Constraints

- Horizon is pinned to `7e4e421224743469035ba0c34fc21e569e64a17d`, Minecraft 26.3. Horizon itself is not changed.
- The game follows the handbook (`docs/showcase/js/data.js`, `ui.js`, `scenes.js`). The handbook changes only for the spec's contradictions and for an engine limit that has been checked in `SRC`.
- Keys: **Z** magnet, **B** backpack, **;** night vision. Every new key is first checked against all pack mods. Taken: PCT V K Y U J I H C; LetsCam P N K F9 F10; JEI R U; worldmap M H E; betterf3 X V M F; the game O G and its defaults.
- Armour sockets: 1 / 2 / 2. Suit capacity: +50% battery. Enchantments are applied at the anvil.
- Text and docs are in Russian per `CONTRIBUTING.md`. Commit messages are in English. Commit email: `128125070+Betreazen@users.noreply.github.com`. Every commit ends with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- No Mojang assets in git. Shared files (`content.rs`, `server.rs`, `selftest.rs`, `rules/src/selftest/mod.rs`, `rules/src/tuning.rs`, `rules/src/modules.rs`, lang json, `horizon-mixin.toml`) get only appended blocks of this stream.
- Stream 4 owns turret module effects (slowness step, support drain): not touched here.
- Checks: `cargo test --release -q` in `mod/rustcraft/rules`; every 3–5 tasks `pwsh -NoProfile -File "C:\Claude code projects\horizon-joint-dev\joint.ps1" try -Mod rustcraft -Path "<worktree>" -Owner s18-base-gear`.

## Engine API checked in SRC (before design)

| Need | API | Evidence |
|---|---|---|
| Rebindable keys | `native_mods(mods)` lists `NativeMod { keys: [name, label, default] }`, Options › Mods shows them as Key Binds; `native_mod_key(mod_id, name, default)` reads the choice; `native_key(key, pressed)` hears presses | `crates/horizon-client/src/mod_api.rs:414-425`, `game/native_hooks.rs:3274`, `:11252`, `:2832`; `horizon-client-render/src/options_ui.rs:2624-2646` |
| Charge bar under the icon | `native_item_bar(stack, bar)` (API 60), client | `docs/native-mods.md:877` |
| Models on players | `native_mob_layer(mob, who, draw)` + `native_player_render_listened` (API 149) | `docs/native-mods.md:1243`, `:3834-3860` |
| Rifle zoom | `native_compute_fov_modifier(player, base, modifier)` (API 362) | `docs/native-mods.md:5` |
| Shield reflection | `native_projectile_impact` + `native_hears_projectile_impacts` (API 136) | `docs/native-mods.md:1197` |
| Enchantments at the anvil | `native_anvil_update(...)` (by target, both sides), `native_anvil_craft_post` | `docs/native-mods.md:2490-2491`, `game/native_hooks.rs:6103` |
| Item look by state | vanilla item model `minecraft:condition` on `minecraft:custom_model_data`, `range_dispatch` `minecraft:compass` with `lodestone_tracker` | `crates/horizon-assets/src/item_model.rs:63,636` |
| Beams | `GameLevel::native_send_particles` | `game/native_hooks.rs:9936` |

## Пределы движка

1. **Слот рюкзака на спине.** В Horizon нет API, которое добавляет игроку слот снаряжения: `EquipmentSlot` — восемь слотов игры (`SRC/crates/horizon-world/src/player_inventory.rs:8-17`), в `docs/native-mods.md` нет хука регистрации слота (поиск «equipment slot», «accessory», «curio», «trinket»). Сделано вместо этого: свой слот мода — сервер хранит, какой рюкзак надет на игрока (`data/rustcraft_backpacks.dat`, `worn`), рюкзак рисуется на спине слоем игрока (API 149), открывается клавишей B, снимается B с приседанием. В окне инвентаря игры этого слота не видно.
2. **Окно на 81 слот.** Окно-сундук игры — не больше 6 рядов (54 слота). Рюкзак из звёздной стали открывается страницами по 45 слотов со стрелками в шестом ряду.

---

### Task 1: Suit capacity charges; chargers leave the zipline alone

Audit: pmodules «Ёмкость костюма» DIFFERS (bug), zipline «16 зарядов» DONE-with-bug.

**Files:** Modify `mod/rustcraft/rules/src/gear/mod.rs`, `mod/rustcraft/gear.rs` (`battery`, `charge`, `player_tick`); Create `mod/rustcraft/rules/src/selftest/gear18.rs`; append to `rules/src/selftest/mod.rs` (module + catalog line), `selftest.rs` (bench method).

**Interfaces — Produces:**
- `Held.suit: bool` (`#[serde(default)]`): whether the piece was last worn in a suit whose chestplate holds the capacity module (the chestplate with the module counts itself).
- `pub fn charger_capacity(g: Gear, h: &Held) -> Option<f64>`: None for the zipline (its charges come from string only), else `capacity(g, h, h.suit || self_suit)`.
- `pub fn add_charge(h: &mut Held, cap: f64, to: f64)`: a charger never lowers the charge.

- [x] Test (RED): `the_suit_module_lets_chargers_fill_half_again` — chestplate t3 with `pm_capacity` → `charger_capacity == 30 000`; leggings with `suit: true` → 30 000; zipline → None; `add_charge` of 10 over a charge of 31 000 above 30 000 keeps 31 000.
- [x] GREEN in rules; `battery()` uses `charger_capacity`; `charge()` never lowers; `player_tick` writes `suit` on worn armour, stops trimming the charge to the capacity.
- [x] Self-test `gear_charger_fills_the_suit_capacity`: charger with a t3 chestplate holding the capacity module at 19 990 E; until its charge > 20 100. `gear_charger_skips_the_zipline`: a zipline with 3 charges stays at 3 after 60 ticks on a charger.
- [x] Commit.

### Task 2: The turret turns to its target and shows its shot

Audit: turret «Сцена: поворачивает голову и дуло» MISSING, «луч выстрела и луч лечения» MISSING.

**Files:** `rules/src/aim.rs` (new, the aim math), `base.rs` (attack/support: write aim, send particles), `anim.rs` (one call for the turret's two aim parts).

**Interfaces — Produces:** `pub fn aim(from: [f64;3], to: [f64;3]) -> (f32, f32)` (yaw about +y from +x counter-clockwise seen from above, pitch up, degrees); `pub fn aim_changed(old: (f32,f32), new: (f32,f32)) -> bool` (≥ 4°); `pub fn beam(from, to, step) -> Vec<[f64;3]>`. Block entity keys `aim_y`, `aim_p` (Short, degrees) in the update tag.

- [x] Tests: east target → yaw 0; north (−z) → 90; above → pitch > 0; `aim_changed` thresholds; a beam of 6 blocks at step 0.5 has 12 points ending at the target.
- [x] GREEN; base.rs writes the aim when changed (`native_block_entity_sync`), particles `minecraft:electric_spark` along the shot, `minecraft:happy_villager` along the heal; anim.rs turns `turret__aim_yaw` (and its carried pitch part) by `aim_y − facing`, the pitch part about z by `aim_p`.
- [x] Self-test `turret_aims_at_its_target`: attack turret, zombie east of it; until the block entity's `aim_y` is within 10° of the expected angle.
- [x] Commit.

### Task 3: A turret kill is the owner's kill

Audit: turret «Убитый турелью моб даёт владельцу добычу и опыт» DIFFERS.

**Files:** `rules/src/turret_owner.rs` (new), `base.rs` (`attack`, `deaths`).

**Interfaces:** `pub fn owner_blow(health: f32, damage: f32, owner_here: bool) -> bool` — the shot that kills is struck by the owner (attacker = owner's entity, damage type `minecraft:arrow`) when the owner is in the level, so the game drops the mob's own experience and player-only loot and Path-of-craft-tree counts the owner's ranged kill itself; then no fixed experience and no `kill` report. Otherwise as before.

- [x] Tests: lethal shot with owner here → true; non-lethal → false; owner away → false.
- [x] GREEN, wiring; the `Hit` remembers `by_owner`; `deaths` skips experience and report for it.
- [x] Self-test: covered by the existing `turret_shoots_a_hostile_mob_only` (owner away → old path); new `turret_owner_blow_rule` none (a real owner player is the bots' check).
- [x] Commit.

### Task 4: The charge bar under every gear icon

Audit: armor «Заряд части виден игроку» MISSING.

**Files:** `rules/src/gear/mod.rs`, `gear.rs` (`on_inventory_tick` writes `rustcraft:bar` into custom data when it changes), new `hook_item_bar.rs` + injection (client).

**Interfaces:** `pub fn bar(charge: f64, cap: f64) -> u8` (0..=13); `pub fn bar_rgb(width: u8) -> u32` (red → yellow → cyan); custom-data key `rustcraft:bar` (Byte).

- [x] Tests: 0 → 0, full → 13, half → 7 (rounded); a zipline's 8 of 16 → 7; colours differ at 2 and 13.
- [x] GREEN, wiring; the item bar hook shows `bar` for a stack with the key.
- [x] Self-test: none — the bar is written for stacks a player carries (`native_inventory_tick`), and the bench has no player; unit tests cover the width and colour, the bots check the bar.
- [x] Commit.

### Task 5: Armour of a higher tier keeps its modules and enchantments

Audit: armor «Модули и чары при замене остаются» MISSING.

**Files:** new `mod/content/data/rustcraft/recipe/tech_{helmet,chestplate,leggings,boots}_{3,4}_upgrade.json`, `minecraft:crafting_transmute`: the piece one tier below and as many of the new alloy as its shaped recipe takes (5 / 7 / 7 / 4). The shaped recipes stay for new armour.

- [x] Rules test: none (data); self-test `gear_armour_upgrade_keeps_the_piece`: `craft_gear` with each t2/t3 piece + its alloys → `tech_helmet_3` with the same charge.
- [x] Commit.

### Task 6: Keys Z, B, ; as rebindable mod keys; night vision and magnet switched by key

Audit: pmodules «Ночное зрение … клавишей» DIFFERS, «Магнит … клавишей» DIFFERS; backpack «Открывается клавишей» DIFFERS.

**Files:** new `gear_keys.rs` module (`rustcraft_gear_keys`) (client side: `native_mods`, `native_key`; net channel `rustcraft:gear_key`), hook files `hook_mods.rs`, `hook_key.rs`; `gear.rs` (server: toggles); `backpack.rs` (B opens the first backpack carried); rules `gear/keys.rs` (new).

**Interfaces:** `Module::toggled(self) -> bool` (Night, Magnet); `Held.on: Vec<String>` (switched-on toggled modules); `pub fn works(h: &Held, m: Module) -> bool`; `pub fn toggle(h: &mut Held, m: Module) -> bool` (new state); `pub enum GearKey { Magnet, Backpack, Night }` with `name()`, `default()` (`key.keyboard.z`, `key.keyboard.b`, `key.keyboard.semicolon`), `from_byte`.

- [x] Tests: night vision does not work until toggled; toggle twice is off; non-toggled modules always work; key defaults and byte round trip.
- [x] GREEN, wiring; the action bar says «Магнит: включён» / «выключен».
- [x] Self-test `gear_night_vision_waits_for_its_key`: rules-level scenario on a bench player is not possible (no real player) → covered by unit tests; bots press `;`.
- [x] Commit.

### Task 7: The magnet pulls, running +50 % for 5 s

Audit: pmodules «Магнит … притягивает» DIFFERS, «Бег +50% на 5 с» DIFFERS.

**Files:** rules `gear/moves.rs`, `gear.rs`.

**Interfaces:** `rules/src/gear/moves.rs` (new): `pub fn pull_command(dimension, to, reach) -> String`; `pub const RUN_TICKS: u64 = 100`, `pub const RUN_BOOST: f64 = 0.5`; `pub fn run_boost(sprint_since: Option<u64>, now: u64) -> bool`.

- [x] Tests: a step never overshoots; boost true at 0..100 ticks of sprint, false after and when not sprinting.
- [x] GREEN; wiring: items within 6 blocks move toward the player each 2 ticks (`execute … as @e[type=item,distance=..6] at @s facing … run tp @s ^ ^ ^0.4`), the game's pickup takes them; the run boost is a movement-speed attribute modifier `rustcraft:pm_run` +0.5 `add_multiplied_total` for 5 s from sprint start, charged 6 E/s while on.
- [x] Self-test `gear_magnet_rule` none (needs a player); unit tests cover the rule.
- [x] Commit.

### Task 8: The armour window shows its charge and its modules

Audit: armor «Окно части: гнёзда, полоса заряда, строки модулей» DIFFERS.

**Files:** rules `gear/window.rs` (new), `gear.rs` (server sends the view on `rustcraft:gear_window` when the window opens; client draws a panel beside the chest menu in `native_screen_render_post`, API 78, found by the title key), `hook_screen_render_post.rs` (one line).

- [x] Checked: API 78 `native_screen_render_post` draws over any open screen, server menus included (`docs/native-mods.md:965`), title in `NativeScreenView.title`.
- [x] Rules: `window::view(g, h) -> View { charge, capacity, modules: [(item, State::{Auto,On,Off})], levels }`, `grouped` («11 000»), `key_label`, tests.
- [x] Wiring. Self-test: none (a drawn client panel); unit tests cover the view, the bots' screenshot checks the panel.
- [x] Commit.

### Task 9: The energy shield: axe by tier, tier 4 reflects

Audit: eshield «Топор выключает на 5 / 3 / 1,5 с» DIFFERS, «Ступень 4 отражает снаряды» MISSING.

**Files:** rules `gear/shield.rs` (new: `axe_seconds`, `axe_disable_scale` 1.0 / 0.6 / 0.3, `blocks_attacks(tier)` JSON, `back_motion`), `content.rs` (the shields' `blocks_attacks.disable_cooldown_scale` — gear's own lines), new `hook_projectile_impact.rs`, `hook_hears_projectiles.rs`, `gear.rs::on_projectile_impact`.

**Interfaces:** `pub fn reflects(tier: u8, facing_yaw: f32, from: [f64;3], to_player: [f64;3]) -> bool` (tier 4 and the projectile comes from the front half).

- [x] Tests for both functions.
- [x] GREEN, wiring: a projectile hitting a blocking tier-4 shield bearer from the front is cancelled and a new one of its kind flies back to its shooter.
- [x] Commit.

### Task 10: The rifle's beam and zoom

Audit: rifle «Сцена: луч выстрела» MISSING, «приближение при прицеливании» MISSING.

**Files:** `gear.rs::shoot` (particles from the muzzle to the hit or the wall), new `hook_fov.rs` (client: rifle in the main hand (`native_held`) and `key.use` held (`native_mapping`, read in the client tick) → modifier × 0.5), rules `RIFLE_ZOOM`, `zoom`, `BEAM_STEP`. Right click still shoots at once; holding it aims.

- [x] Test: `beam` reused from turret; `zoom(base) == base * 0.5`.
- [x] Commit.

### Task 11: The scout compass points and switches

Audit: scout_compass «Стрелка показывает на цель» MISSING, «Правый клик переключает цель» MISSING.

**Files:** rules `gear/compass.rs` (new: `Mode { Structures, Biomes }`, `toggled`, `takes`, `target(atlas, mode, …)`); `gear.rs` (right click toggles, stored per player); `explore.rs` (stream 6's file, smallest change: `player` takes the compass `Option<Mode>` instead of a bool and returns the needle's target); item model `items/scout_compass.json` → `range_dispatch` on `minecraft:compass` target `lodestone`, 32 generated textures `scout_compass_00..31.png` (needle drawn over the compass by `tools/gen_scout_compass.py`); `native_inventory_tick` writes `minecraft:lodestone_tracker` (target, tracked false) on a carried compass when its target changes.

- [x] Tests: toggle, filter of places by mode.
- [x] Commit.

### Task 12: Zipline ropes come down after 5 minutes

Audit: zipline «Трос держится 5 минут» DIFFERS.

**Files:** `rules/src/traversal.rs` (`Rope.tied_ms: u64` wall clock, `#[serde(default)]`; `ROPE_LIFE_MS` = 5 min; `Ropes::stamp(now)`, `Ropes::expired(now) -> Vec<u64>`), `zipline.rs` (sweep unties expired ropes, a rope being ridden waits for the ride to end).

- [x] Test `a_rope_comes_down_five_minutes_after_it_was_tied`: expired exactly at 5 min, older stores read with 0 and are stamped on load.
- [x] Self-test `zipline_rope_comes_down_in_time` with the rope aged through the bench (`age_rope`).
- [x] Commit.

### Task 13: Gear seen on the player; blade and shield go dark without charge

Audit: jetpack «джетпак на спине, пламя» MISSING; zipline «ручка с роликами» MISSING; backpack (star) on the back; blade «гаснет» MISSING; eshield «поле светится» MISSING.

**Files:** rules `gear/looks.rs` (new); `gear.rs` (server: a player's look flags — jetpack in the chestplate, flying, riding a zipline rope (noted by `rustcraft_zipline::ride`), starsteel backpack worn — sent to the level's players on `rustcraft:looks` when they change; client: `native_mob_layer` draws standalone models from `tools/gen_worn_models.py` on the back and a handle with two rollers over the head), hook files `hook_mob_layer.rs`, `hook_player_render_listened.rs`; `gear.rs` writes `minecraft:custom_model_data` flags `[charged]` on blades and shields; item models with `minecraft:condition` and dim textures generated by `tools/gen_dim.py`.

- [x] Rules: `pub struct Look { jetpack, flying, handle, backpack }` with `to_byte` / `from_byte`, `encode` / `decode` tests.
- [x] Blade and shield dark without charge (commit 126df44).
- [x] Looks wired; the backpack flag waits for task 14 (`rustcraft_backpack::wears_star`).
- [x] Commit.

### Task 14: The star-steel backpack: 81 slots on the back

Audit: backpack «Рюкзак из звёздной стали (81 слот) в своём слоте на спине» MISSING.

**Files:** content: item `backpack_3` (`content.rs` block), icon/model/item/recipe `lbl|lcl|lll` with starsteel from `tools/gen_backpack_star.py`; `backpack.rs`: a sneaking right click puts it on the mod's own back slot (taken from the inventory in `native_inventory_tick`), the B key opens the worn one, a sneaking B takes it off (`native_give_stack`); worn backpacks in the store's `worn`; the window shows pages of 45 slots (45 + 36) with arrows in the sixth row (`server.rs` one line in `on_menu_clicked`); the looks flag draws it on the back.
- [x] Checked SRC: no API adds a player equipment slot — recorded under «Пределы движка».
- [x] Rules: `backpack::STAR`, `STAR_SLOTS` 81, `pages`, `page_range`, `turn` with tests. Self-test: none (wearing needs a player; the bots check).
- [x] Commit.

### Task 15: Technical enchantments at the anvil

Audit: enchants «Книга накладывается на наковальне, уровни складываются» DIFFERS.

**Files:** rules `gear/enchant.rs` (new: `Book { kind, level }` from custom data, `combine(a, b)` like vanilla books, `apply(piece_level, book_level)`, level cost), hook file `hook_anvil_update.rs` (target), `gear.rs` (levels mirrored on the stack: custom data `rustcraft:cap` / `rustcraft:thrift`, glint, lore; the window loses its book slots, books left in old pieces stay counted).
- [x] Tests: I+I = II, II+I = II, III+III = III; piece 1 + book 1 = 2; piece 2 + book 1 = 2; cost.
- [x] Commit.

### Task 16: The anchor everywhere and its window

Audit: anchor «в любом мире» DIFFERS, «Окно: … полоса энергии, на сервере» DIFFERS.

**Files:** rules `anchor.rs` (`Index` of every level's anchors, `to_wake`), `base.rs` (the index in the world's `data/rustcraft_anchor_levels.json`, written when a level's anchors are read or change; each second a level not read yet with an anchor of an online owner is opened at that chunk by `native_force_chunk_in` (API 413), the ticket put into the level's held chunks so its own tick keeps or releases it; the window's energy bar «получает X из Y Е/с» and the owner line «Steve · на сервере»).
- [x] Rules: `Index::set`, `Index::to_wake` with a test. Self-test: none (a restart with a level nobody is in is the bots' check); the bar is drawn from the share the anchors' tick keeps.
- [x] Commit.

### Task 17: Turret windows as the handbook draws them

Audit: turret «Цели чипами», «Окно поддержки: Игроков в радиусе, Лечит: Alex · 6 из 20», «Окно с ошибкой роли: Работают / Не работает».

**Files:** `base.rs::turret_sheet` (targets as the sheet's chips, fixed; support rows «Игроков в радиусе», «Лечит: Alex · 6 из 20»; one row «Работают … · не работает …» for effect modules in a support turret; the «N E/s» row gave way, the energy bar stays), `rules/src/machines/sheet.rs` (stream 3's file: the turret's and the anchor's `Shape` only — turret rows 5 + 4 toggles, anchor rows 2 + a bar).
- [x] Test: the layout test `every_window_fits_the_screen_and_its_parts_do_not_overlap` holds the new shapes; the rows are the window self-tests' and the bots' screenshots.
- [x] Commit.

### Task 18: Zipline anchor ghost (check) and the README checks

Audit: zipline «Перед установкой виден контур якоря» DIFFERS.

- [ ] Check SRC for a client hook drawing a model in the world for the own view (beyond `native_visuals` outlines); record the result under «Пределы движка».
- [ ] `mod/README.md` «Что проверить в игре: срез 18 — база и снаряжение».
- [ ] Final `try`; report.

## Left to other streams

- Turret slowness step and support drain (stream 4, module effects).
- Handbook text contradictions (`pmodules` two sockets, armour scene, key M, anvil scene, harpoon caption) and the guide's scenes (stream 8, `docs/showcase/js/*`).
- The stack module item `mod_stack` for the backpack socket (stream 4's module list).
