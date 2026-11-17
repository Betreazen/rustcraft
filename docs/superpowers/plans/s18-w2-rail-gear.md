# Slice 18, wave 2: rail, workers, base and gear leftovers — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans. Steps use checkbox (`- [x]`) syntax.

**Goal:** every line of «Ещё не сделано» in `docs/design/06-audit/after/rail-worker.md` (except the portal dock lines, w2-craft's) and `after/base-gear.md`, plus the wave-2 bugs the integrator listed, DONE or a proven engine limit.

**Architecture:** logic and numbers in `mod/rustcraft/rules` with unit tests (`cargo test --release -q`); the bundle (`mod/rustcraft/*.rs`) wires them; a self-test scenario per mechanic in `rules/src/selftest/s18_w2_rail_gear.rs` + `selftest.rs`.

**Tech Stack:** Rust, Horizon 7e4e421 native mod API, horizon-mixins.

**Spec:** `docs/superpowers/specs/2026-11-16-handbook-parity-design.md`; master plan `docs/superpowers/plans/2026-11-16-handbook-parity.md` (Global Constraints apply verbatim).

## Global Constraints

- Shared files (content.rs, server.rs, selftest.rs, rules/src/selftest/mod.rs, rules/src/tuning.rs, rules/src/modules.rs, lang json, horizon-mixin.toml, hook_*.rs, window files): only add lines.
- User text Russian first (ru_ru), en_us too. Commits `git commit -- <paths>`, English, with the Co-Authored-By line.
- Build check: `scratchpad/chk-w2-rail-gear.sh <worktree>`; bench try only every few tasks, from a snapshot `scratchpad/snap-w2-rail-gear`.

## Tasks

### Task 1: Turret windows (base-gear #10, #12, #6)
Files: `base.rs` (`support_rows`, `turret_sheet`, `support`), `rules/src/turret.rs`, lang.
- [x] RED: `turret::support_draw_for(l, t, someone_hurt)` counts the heal module only while someone is hurt; `turret::shot_breakdown(l, t)` gives (base, effects, each, total) = (40, 3, 10, 70); `turret::slow_share(l,t)` → 0.2.
- [x] GREEN and wire: «Лечит: Alex · 6 из 20 здоровья» on its own key `rustcraft.turret.healing_row` (two `%s`); «Выстрел: 40 Е + 3 × 10 Е = 70 Е»; «−20% скорости» for slowness; the support window's bar «Энергия · получает N из M Е/с».
- [x] Commit.

### Task 2: Turret and guard kills are the owner's (base-gear #3, rail-worker «Страж: добыча killed_by_player»)
Files: `base.rs`, new helpers `hook_loot_table_load.rs`, `hook_loot_condition.rs`, module `loot_marks.rs` (pub module in `horizon-world/src/loot.rs`), `horizon-mixin.toml` (add), `rules/src/turret_owner.rs`.
- [x] RED: `turret_owner::owned_tables(table)` rewrites every `minecraft:killed_by_player` condition into `any_of[killed_by_player, rustcraft:killed_by_owner_device]`; `turret_owner::kill_experience(kind)` gives vanilla's mob experience (5 most, blaze 10, …); the mark matcher (`marks::matches`: same dimension, within 2 blocks, within 40 ticks).
- [x] GREEN and wire: bleeding hits as the owner when he is in the level (`owner_shot` in `bleed`); a lethal guard/turret/bleed hit without the owner marks the place (dimension, position, game time) in the pub module; the loot condition holds on a marked place; the owner gets the table's experience instead of a flat 5.
- [x] Self-test: a spider killed by a guard with the owner away drops spider eyes over several kills (looting-free chance 1/3 for 0..2: kill 6, at least one eye).
- [x] Commit.

### Task 3: Armour window module lines (base-gear #15)
Files: `gear.rs` (`draw_window` rows), `rules/src/gear/window.rs`, lang `rustcraft.gear.window.does.<module>`.
- [x] RED: `window::does(module)` gives the key and Russian fallback of what each player module gives («батарея костюма +50%», …).
- [x] GREEN, wire, commit.

### Task 4: Charge dock line (rail-worker)
Files: `base.rs` dock rows, `workers.rs` `dock_rows`.
- [x] «Энергия · получает N из 16 Е/с», N = what it took in the last second. Rules: `worker::dock_intake`. Commit.

### Task 5: Packed worker that does not read keeps its contents (rail-worker W3)
Files: `workers.rs:279-284, 615-623`.
- [x] A packed stack that `put_back` cannot read is not taken from the player and no worker is placed: the player is told «Работник не читается». Rules test `worker::unpack` on old JSON. Commit.

### Task 6: Worker glow while working (rail-worker)
Files: `station.rs` `draw_worker`, `rules/src/worker/pose.rs`.
- [x] RED: `pose::glow(tool, busy, t)` → light/brightness for the tool part (poses.js `glow`). GREEN: the tool part drawn full bright while `busy`. Commit.

### Task 7: Monorail right-click tier swap
Files: `monorail.rs`, `server.rs` on_right_click_block (one added line).
- [x] A right click on a rail with another tier's rail item swaps it in place (shape kept), the old rail to the player. Rules: `monorail::swap(rail, item)`. Self-test. Commit.

### Task 8: Rifle zoom does not fire
Files: `gear.rs` (rifle use).
- [x] Holding use zooms; the shot is the attack key (or the release after a charge) — whichever keeps «зум при удержании правой кнопки» without a shot. Rules test. Commit.

### Task 9: Zipline: rope stays while ridden, rollers turn, the rider hangs (base-gear #40)
Files: `zipline.rs`, `gear.rs` `draw_looks`, `rules/src/gear/looks.rs`.
- [x] RED: `zipline::expired(rope, now, ridden)` false while ridden. Rollers' angle from the ride's age. Rider pose: check SRC for a player pose API; else record the limit. Commit.

### Task 10: Compass counts a place, not a kind (base-gear #33)
Files: `explore.rs`, `rules/src/explore.rs`, `rules/src/gear/compass.rs`.
- [x] RED: atlas `enter(uuid, id, at)` keeps the place (structure start chunk); compass target skips only places visited. Old saves read. Commit.

### Task 11: Cartographer sees deep structures and `/place structure` (rail-worker)
Files: `explore.rs`.
- [x] Check SRC for the structure starts of a chunk; widen the test from «box touches the grid cell» to «start in a chunk of the strip». Commit.

### Task 12: Chassis window as the mod's own screen (rail-worker, ui.js `chassis`) with the height field
Files: `rules/src/worker/screen.rs` (new: layout, parts, view, presses), `rules/src/worker/mod.rs` (`height`), `bore.rs` (height step), `workers.rs` (sessions over the net), `station.rs` (client: draw, mouse, open/close), `monorail.rs` `on_client_message` (route presses), `hook_screen_mouse.rs` (one added line), lang.
- [x] RED: layout hit test, press codec, `bore::next_cell` with a target height (climb/drop one a step).
- [x] GREEN and wire. Self-test: open → press tool/module/hold/inventory → worker changed; bore climbs to the set height.
- [x] Commit.

### Task 13: In-game handbook scenes: facing and moving parts (base-gear #43)
Files: `handbook.rs`, `rules/src/handbook/scene.rs`, `scenes.rs`.
- [x] Scene blocks turned by the scene's `face`; moving parts (turret head, rotors, drums) turn by the guide's poses over time. Axes checked against `docs/showcase/js/scenes.js`. Screenshots list for the integrator. Commit.

### Task 14: README and decisions
- [x] `mod/README.md` stale lines of «Что срезы пока не умеют» for rail/workers/turret. Capsule size: record the decision. Commit.

## Hand-offs

- From w2-machine-module (done here): the chassis window's height field uses `modules::height_step` (`bore::toward_height`); the monorail right-click tier swap; the grab keeps the stratum find table (`util::grown_hold_name`).
- To the integrator: `assets/models/handbook_mc.py` rewrites ru_ru.json and en_us.json in its own order (the keys of the other streams move above the handbook's block, same keys and values). This branch kept the lang files as they were and committed only `scenes.rs`; a later run of the script reorders them.
- Portal dock lines of rail-worker.md: w2-craft's.

## What bots and screenshots must check

1. Handbook scenes (Task 13): blocks face their way (furnace and alloy fronts, rails on walls, cocoa), machines' parts move (crusher rolls, windmill rotor, mixer), the turret's head looks at the zombie; icons, dots and labels lie on the blocks. Screenshots: pages `turret`, `windmill`, `crusher`, `tool_reaper` (cocoa on the trunk), `monorail`.
2. Chassis window (Task 12): opens as the mod's screen on a right click of a worker; tool/module/hold/inventory clicks; right click on an inventory item adds it to the filter; the bore's height field with the height module; 5 and 7 sockets.
3. Guard/turret kill with the owner in another world: spider eyes drop, the owner gets the spider's experience.
4. Rifle: holding use zooms without a shot, attack while aiming fires, a tap fires.
5. Zipline: rollers turn while riding; a rope ridden at its five minutes stays until the rider is off.
6. Worker glow while working; the charge dock's «получает N из 16 Е/с»; turret windows (shot parts, −20% скорости, the support bar «получает N из M Е/с»); the armour window's module lines.
7. Monorail right click with another tier's rail; the compass points at a second village after the first was visited.

## Пределы движка

1. **Поза седока зиплайна.** Мод не задаёт позу игрока: `native_player_submit` (API 149) только заменяет модель целиком (`drawn`), в `NativeMobRender` нет углов рук (SRC crates/horizon-client/src/game/native_mob_draw.rs:24-53), а нарисовать игрока своей моделью нельзя: `native_draw_model_part` берёт текстуру из атласа интерфейса (native_block_entity_draw.rs:727-735), скина игрока там нет. Седок сидит на невидимом сиденье; ролики ручки над головой крутятся.
2. **`/place structure`.** Команда ставит блоки постройки и не записывает старт в чанк (SRC game/commands/place.rs:203-216), как и ванильный `PlaceCommand`: такой постройки нет ни для игры, ни для `/locate`, ни для картографа. Глубокие постройки картограф теперь видит (столбец клетки на любой высоте).
3. **Капсула в 2 раза больше клетки справочника** — решение потока rail-worker (сидящий игрок должен поместиться), не предел движка; записано в README.
