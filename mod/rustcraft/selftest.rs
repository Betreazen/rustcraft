// The in-world self-test of the joint bench (horizon-joint-dev, «Быстрые проверки»). The console
// command `rustcraft_selftest run [filter]` (registered with `native_register_commands`, heard from
// the console through `native_perform_console_command`, API 99) starts the scenarios of
// rustcraft_rules::selftest and answers at once; they then advance from the Overworld's
// `native_level_tick_post` (rustcraft_server::on_level_tick), so they run as fast as
// `/tick sprint` ticks. When the last one ends the result goes to selftest/rustcraft.json in the
// server folder (the world folder's parent):
//   {"mod": "rustcraft", "done": true, "passed": [...], "failed": [{"name", "why"}],
//    "timings": {"name": {"ticks", "ms"}}, "setup": {"ticks", "ms"}}
//
// This file is the `Bench` the scenarios see: real blocks set through `Level.setBlock`
// (`native_set_block`), the mod's own block entity formats, the hopper's way into a chest
// (`native_insert_item`), the mod's level state for what it worked out, and its own window, joint
// and trip code for what a player would press. The plots' chunks are held with a mod ticket
// (`native_force_chunk`, API 412) and cleared with one bulk edit (`native_edit_blocks`, API 126).
use crate::game::{
    Game,
    native_mobs::NativeSpawn,
    native_world_edit::{NativeBlockEdit, NativeBlockRun},
    rustcraft_base as base, rustcraft_butcher as butcher,
    rustcraft_content::Machine,
    rustcraft_fluids as fluids,
    rustcraft_machines::{self as machines, SMELTING, slot_of},
    rustcraft_mining as mining, rustcraft_mixer as mixer, rustcraft_monorail as monorail,
    rustcraft_power as power,
    rustcraft_server::{self as server, read_slots, write_slots},
    rustcraft_storage as storage, rustcraft_window as window, rustcraft_workers as workers,
};
use crate::mod_api::NativeCommand;
use horizon_client::{items::ItemStack, text::Text};
use horizon_core::geometry::Direction;
use horizon_world::nbt::{Compound, Limits, Tag, read_network, write_network};
use rustcraft_rules::{
    geom::{Face, Pos},
    housing::{self, Diagnosis},
    machines::{alloy::AlloyRecipe, processor::Shown},
    modules::Module,
    monorail::{Network, Trip, TripError},
    net::{
        settings::JointSetting,
        tanks::{TankBlock, TankGroup},
        topology::{Channel, Housing},
    },
    selftest::{self, Area, Bench, MobView, Slot, StorageView, run::Run},
    storage::{keys, terminal::LimitChange},
    tuning::Tuning,
    worker::{Tool, Worker},
};
use std::{
    path::PathBuf,
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
    time::Instant,
};

/// The console command the joint bench runs (`selftest` in its mods.json).
pub const COMMAND: &str = "rustcraft_selftest";
const MOD_ID: &str = "rustcraft";
const RESULT_DIR: &str = "selftest";
/// The level the plots are in.
const DIMENSION: &str = "minecraft:overworld";
/// The owner of the plots' chunk tickets (a mod ticket's owner is namespaced).
const TICKET: &str = "rustcraft:selftest";
const AIR: &str = "minecraft:air";
/// `Level.setBlock`'s UPDATE_ALL: neighbours hear it and players see it.
const UPDATE_ALL: i32 = 3;

/// A run under way: its scenarios, its own tick count and where its result goes.
struct Pending {
    run: Run,
    ticks: u64,
    out: PathBuf,
}

static PENDING: LazyLock<Mutex<Option<Pending>>> = LazyLock::new(Mutex::default);
static EPOCH: LazyLock<Instant> = LazyLock::new(Instant::now);

fn pending() -> MutexGuard<'static, Option<Pending>> {
    PENDING.lock().unwrap_or_else(PoisonError::into_inner)
}

/// `native_register_commands`: the console command, for operators of level 4 only.
pub(super) fn on_register_commands(commands: &mut Vec<NativeCommand>) {
    commands.push(NativeCommand::op(&format!("{COMMAND} <args:text>"), 4));
    // Other mods' self-tests: `rustcraft_testkit turret …` (rustcraft_base::on_testkit_command).
    commands.push(NativeCommand::op("rustcraft_testkit <args:text>", 4));
}

/// `native_perform_console_command`: `rustcraft_selftest run [filter]` starts the scenarios whose
/// name contains the filter and answers at once.
pub(super) fn on_console_command(game: &mut Game<'_>, command: &str, ran: &mut bool) {
    let Some(args) = command
        .strip_prefix(COMMAND)
        .filter(|rest| rest.is_empty() || rest.starts_with(' '))
    else {
        return;
    };
    *ran = true;
    let mut words = args.split_whitespace();
    let answer = match words.next() {
        Some("run") => start(game, words.next().unwrap_or("")),
        _ => format!("usage: {COMMAND} run [filter]"),
    };
    log::info!("rustcraft: {answer}");
    game.native_feedback(Text::literal(answer));
}

fn start(game: &Game<'_>, filter: &str) -> String {
    let Some(out) = result_path(game) else {
        return format!("{COMMAND}: no world folder on this side");
    };
    let mut slot = pending();
    if slot.is_some() {
        return format!("{COMMAND}: a run is under way already");
    }
    let scenarios = selftest::scenarios(filter, server::tuning());
    let count = scenarios.len();
    if let Err(e) = write(&out, "{\"mod\": \"rustcraft\", \"done\": false}\n") {
        return format!("{COMMAND}: {}: {e}", out.display());
    }
    LazyLock::force(&EPOCH);
    *slot = Some(Pending {
        run: Run::new(scenarios),
        ticks: 0,
        out,
    });
    format!("{COMMAND}: started {count} scenarios")
}

fn result_path(game: &Game<'_>) -> Option<PathBuf> {
    let world = game.native_world_dir()?;
    Some(
        world
            .parent()?
            .join(RESULT_DIR)
            .join(format!("{MOD_ID}.json")),
    )
}

/// Written whole under another name first, so the bench never reads half a file.
fn write(path: &std::path::Path, text: &str) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let part = path.with_extension("json.part");
    std::fs::write(&part, text).map_err(|e| e.to_string())?;
    std::fs::rename(&part, path).map_err(|e| e.to_string())
}

/// `native_level_tick_post` of the Overworld, after the mod's own work: one tick of the run.
pub(super) fn tick(game: &mut Game<'_>, dimension: &str) {
    if dimension != DIMENSION {
        return;
    }
    let Some(mut p) = pending().take() else {
        return;
    };
    p.ticks += 1;
    let (report, refresh) = {
        let mut bench = GameBench {
            game: &mut *game,
            refresh: false,
        };
        let report = p.run.tick(&mut bench, p.ticks);
        (report, bench.refresh)
    };
    if refresh {
        server::rescan_now(game, DIMENSION);
    }
    let Some(report) = report else {
        *pending() = Some(p);
        return;
    };
    let line = format!("{COMMAND}: {}", report.summary());
    for (name, why) in &report.failed {
        log::warn!("rustcraft: self-test {name} failed: {why}");
    }
    match write(&p.out, &report.to_json(MOD_ID)) {
        Ok(()) => log::info!("rustcraft: {line}; {}", p.out.display()),
        Err(e) => log::warn!("rustcraft: {line}; result not written: {e}"),
    }
}

/// The world as the scenarios see it: the Overworld while it ticks.
struct GameBench<'g, 'a> {
    game: &'g mut Game<'a>,
    refresh: bool,
}

/// `c` through the binary NBT a save writes, and read back.
fn through_nbt(c: &Compound) -> Result<Compound, String> {
    let bytes =
        write_network(&Tag::Compound(c.clone()), Limits::saved()).map_err(|e| format!("{e:?}"))?;
    match read_network(&bytes, Limits::saved())
        .map_err(|e| format!("{e:?}"))?
        .0
    {
        Tag::Compound(back) => Ok(back),
        other => Err(format!("a compound came back as {other:?}")),
    }
}

impl GameBench<'_, '_> {
    fn with_level<R>(&self, f: impl FnOnce(&server::LevelNet) -> R) -> Option<R> {
        server::with_level(DIMENSION, |level| f(level))
    }

    fn change_entity<R>(
        &mut self,
        pos: Pos,
        f: impl FnOnce(&mut horizon_client::containers::ModBlockEntity) -> R,
    ) -> Result<R, String> {
        self.game
            .native_with_mod_block_entity(pos, f)
            .ok_or_else(|| format!("no mod block entity at {pos:?}"))
    }
}

impl Bench for GameBench<'_, '_> {
    fn hold(&mut self, area: &Area, hold: bool) {
        for chunk in area.chunks() {
            if !self.game.native_force_chunk(TICKET, chunk, hold) {
                log::warn!("rustcraft: self-test chunk {chunk:?} not held");
            }
        }
    }

    fn loaded(&self, area: &Area) -> bool {
        area.chunks()
            .iter()
            .all(|c| self.game.world.chunks.contains_key(&(c[0], c[1])))
    }

    fn clear(&mut self, area: &Area) -> Result<(), String> {
        self.clear_slice7(area);
        let height = u32::try_from(area.max[1] - area.min[1] + 1).map_err(|e| e.to_string())?;
        let mut runs = Vec::new();
        for x in area.min[0]..=area.max[0] {
            for z in area.min[2]..=area.max[2] {
                runs.push(NativeBlockRun {
                    x,
                    z,
                    y: area.min[1],
                    height,
                    surface: None,
                    block: 0,
                    only: None,
                });
            }
        }
        let edit = NativeBlockEdit {
            palette: vec![AIR.to_owned()],
            filters: Vec::new(),
            runs,
        };
        self.game.native_edit_blocks(DIMENSION, &edit).map(|_| ())
    }

    fn refresh(&mut self) {
        self.refresh = true;
    }

    fn now_ms(&self) -> u128 {
        EPOCH.elapsed().as_millis()
    }

    fn place(&mut self, pos: Pos, block: &str, props: &[(&str, &str)]) -> Result<(), String> {
        let state = self
            .game
            .native_default_state(block)
            .ok_or_else(|| format!("no block {block}"))?;
        // False also when the block was there already.
        self.game
            .native_set_block(DIMENSION, pos, state, UPDATE_ALL);
        if self.game.native_block(pos) != block {
            return Err(format!("{block} not set at {pos:?}"));
        }
        for (key, value) in props {
            self.game.native_set_block_property(pos, key, value);
            let now = self.game.native_block_property(pos, key);
            if now.as_deref() != Some(*value) {
                return Err(format!("{block} at {pos:?}: {key} is {now:?}, not {value}"));
            }
        }
        Ok(())
    }

    fn put(&mut self, pos: Pos, slot: usize, item: &str, count: u32) -> Result<(), String> {
        self.change_entity(pos, |e| {
            if e.items.len() <= slot {
                e.items.resize(slot + 1, None);
            }
            e.items[slot] = Some(ItemStack::new(item, count));
        })
    }

    fn insert(&mut self, pos: Pos, item: &str, count: u32) -> Result<u32, String> {
        let stack = machines::max_stack(self.game, item).max(1);
        let mut went = 0;
        while went < count {
            let n = stack.min(count - went);
            let left = self
                .game
                .native_insert_item(pos, Direction::Up, ItemStack::new(item, n))
                .map_or(0, |s| s.count);
            went += n - left.min(n);
            if left > 0 {
                break;
            }
        }
        Ok(went)
    }

    fn set_channels(&mut self, pos: Pos, slots: [Option<Channel>; 4]) -> Result<(), String> {
        self.change_entity(pos, |e| write_slots(&mut e.nbt, &Housing { slots }))
    }

    fn set_joint(
        &mut self,
        pos: Pos,
        face: Face,
        slot: usize,
        setting: JointSetting,
    ) -> Result<(), String> {
        self.change_entity(pos, |e| {
            let joints = window::read_joints(&e.nbt).with(face, slot, setting);
            window::write_joints(&mut e.nbt, &joints);
        })
    }

    fn press(&mut self, pos: Pos, button: i32, carried: Option<(&str, u32)>) -> Result<(), String> {
        self.press_cursor(pos, button, carried).map(|_| ())
    }

    fn press_cursor(
        &mut self,
        pos: Pos,
        button: i32,
        carried: Option<(&str, u32)>,
    ) -> Result<Option<(String, u32)>, String> {
        let mut entity = self
            .game
            .native_mod_block_entity(pos)
            .ok_or_else(|| format!("no mod block entity at {pos:?}"))?;
        let mut cursor = carried.map(|(item, n)| ItemStack::new(item, n));
        // The order `mod_menu_button` calls them in.
        server::on_menu_button(&mut entity, button);
        server::on_menu_button_v2(self.game, pos, &mut entity, button, &mut cursor);
        self.change_entity(pos, move |e| *e = entity)?;
        Ok(cursor.map(|c| (c.item.to_string(), c.count)))
    }

    fn sheet(&self, pos: Pos) -> Option<rustcraft_rules::machines::sheet::Sheet> {
        let block = self.game.native_block(pos);
        let t = server::tuning();
        self.with_level(|l| base::sheet(self.game, DIMENSION, l, pos, &block, t))
            .flatten()
    }

    fn set_tank(&mut self, pos: Pos, fluid: &str, amount: u64) -> Result<(), String> {
        let block = TankBlock {
            fluid: Some(fluid.to_owned()),
            amount,
        };
        self.change_entity(pos, |e| fluids::write_tank(&mut e.nbt, &block))
    }

    fn set_station_energy(&mut self, pos: Pos, energy: f64) -> Result<(), String> {
        if monorail::set_energy(self.game, pos, energy) {
            Ok(())
        } else {
            Err(format!("no station at {pos:?}"))
        }
    }

    fn change_limit(&mut self, pos: Pos, item: &str, change: LimitChange) -> Result<(), String> {
        if storage::change_limit_at(DIMENSION, pos, &keys::key(item, 0), change) {
            Ok(())
        } else {
            Err(format!("no storage at {pos:?}"))
        }
    }

    fn tuning(&self) -> &Tuning {
        server::tuning()
    }

    fn block(&self, pos: Pos) -> String {
        self.game.native_block(pos)
    }

    fn property(&self, pos: Pos, key: &str) -> Option<String> {
        self.game.native_block_property(pos, key)
    }

    fn items(&self, pos: Pos) -> Vec<Slot> {
        if let Some(entity) = self.game.native_mod_block_entity(pos) {
            return (0..entity.items.len())
                .map(|i| slot_of(&entity, i))
                .collect();
        }
        self.game
            .native_container_slots(DIMENSION, pos, Direction::Up)
            .unwrap_or_default()
            .into_iter()
            .map(|s| {
                s.stack
                    .filter(|st| st.count > 0)
                    .map(|st| (st.item.to_string(), st.count))
            })
            .collect()
    }

    fn inputs(&self, pos: Pos) -> Vec<usize> {
        Machine::of(&self.game.native_block(pos)).map_or_else(Vec::new, Machine::inputs)
    }

    fn housing(&self, pos: Pos) -> Option<Diagnosis> {
        self.with_level(|l| housing::diagnose(&l.topology, &l.layout, pos))
            .flatten()
    }

    fn shown(&self, pos: Pos) -> Option<Shown> {
        self.with_level(|l| l.power.status.get(&pos).map(|s| s.shown))
            .flatten()
    }

    fn receiver_full(&self, pos: Pos) -> bool {
        self.with_level(|l| l.transport.is_full(pos, l.ticks))
            .unwrap_or(false)
    }

    fn storage(&self, pos: Pos) -> Option<StorageView> {
        let (cells, shape, store) = storage::view(DIMENSION, pos)?;
        Some(StorageView {
            cells,
            shape,
            store,
        })
    }

    fn tank(&self, pos: Pos) -> Option<TankGroup> {
        let game = &*self.game;
        server::with_level(DIMENSION, |l| {
            fluids::group_at(game, l, pos, server::tuning())
        })
        .flatten()
    }

    fn no_lane(&self, pos: Pos) -> Option<String> {
        self.with_level(|l| l.fluids.no_lane(pos, l.ticks).map(str::to_owned))
            .flatten()
    }

    fn station_energy(&self, pos: Pos) -> Option<f64> {
        monorail::energy_at(self.game, pos)
    }

    fn network(&self) -> Network {
        monorail::network(DIMENSION)
    }

    fn plan_trip(&mut self, from: Pos, to: Pos, pay: bool) -> Result<Trip, TripError> {
        monorail::plan_for_selftest(self.game, DIMENSION, from, to, pay, server::tuning())
    }

    fn cook_ticks(&self, item: &str) -> Option<f64> {
        self.game
            .recipes()
            .cook(SMELTING, &ItemStack::new(item, 1))
            .map(|c| f64::from(c.time))
    }

    fn crush(&self, item: &str) -> Option<(String, u32, f64)> {
        self.with_level(|l| {
            l.recipes
                .crushing
                .iter()
                .find(|r| r.inputs.contains(item))
                .map(|r| (r.result.item.to_string(), r.result.count, r.ticks))
        })
        .flatten()
    }

    fn alloy_recipes(&self) -> Vec<AlloyRecipe> {
        self.with_level(|l| l.recipes.alloying.clone())
            .unwrap_or_default()
    }

    fn craft(&self, grid: &[Option<&str>; 9]) -> Option<(String, u32)> {
        let grid: Vec<Option<ItemStack>> = grid
            .iter()
            .map(|c| c.map(|item| ItemStack::new(item, 1)))
            .collect();
        let (_, result) = self.game.recipes().find(&grid, 3)?;
        (result.count > 0).then(|| (result.item.to_string(), result.count))
    }

    fn round_trip(&self, pos: Pos) -> Result<(), String> {
        let block = self.game.native_block(pos);
        if storage::is_storage_block(&block) {
            return storage::round_trip(DIMENSION, through_nbt);
        }
        if Machine::of(&block) == Some(Machine::Station) {
            return monorail::round_trip(DIMENSION, through_nbt);
        }
        let entity = self
            .game
            .native_mod_block_entity(pos)
            .ok_or_else(|| format!("no mod block entity at {pos:?}"))?;
        let back = through_nbt(&entity.nbt)?;
        if read_slots(&back) != read_slots(&entity.nbt) {
            return Err(format!(
                "slots {:?} came back as {:?}",
                read_slots(&entity.nbt),
                read_slots(&back)
            ));
        }
        let (kept, read) = (window::read_joints(&entity.nbt), window::read_joints(&back));
        if read != kept || kept.settings.is_empty() {
            return Err(format!("joints {kept:?} came back as {read:?}"));
        }
        Ok(())
    }

    fn spawn_mob(&mut self, kind: &str, at: [f64; 3], health: Option<f32>) -> Result<u32, String> {
        slice7_spawn_mob(self.game, kind, at, health)
    }

    fn mob(&self, id: u32) -> Option<MobView> {
        slice7_mob(self.game, id)
    }

    fn set_modules(&mut self, pos: Pos, modules: &[&str]) -> Result<(), String> {
        let slots: Vec<Option<Module>> = (0..3)
            .map(|i| modules.get(i).and_then(|m| Module::of(m)))
            .collect();
        if base::set_modules(self.game, pos, &slots) {
            Ok(())
        } else {
            Err(format!("no block entity at {pos:?}"))
        }
    }

    fn set_owner(&mut self, pos: Pos, owner: u128, placed: u64) -> Result<(), String> {
        if base::set_owner(self.game, pos, owner, placed) {
            Ok(())
        } else {
            Err(format!("no block entity at {pos:?}"))
        }
    }

    fn set_buffer(&mut self, pos: Pos, energy: f64) -> Result<(), String> {
        if machines::set_buffer_energy(self.game, pos, energy) {
            Ok(())
        } else {
            Err(format!("no block entity at {pos:?}"))
        }
    }

    fn buffer(&self, pos: Pos) -> f64 {
        machines::buffer_energy(self.game, pos)
    }

    fn turret(&self, pos: Pos) -> Option<rustcraft_rules::turret::Shown> {
        base::turret_shown(DIMENSION, pos)
    }

    fn bleeding(&self, id: u32) -> bool {
        base::bleeding(self.game, id)
    }

    fn souls_found(&self) -> bool {
        base::souls_found(self.game, server::tuning())
    }

    fn spawn_worker(
        &mut self,
        cell: Pos,
        owner: u128,
        tool: Option<Tool>,
        hold: &[(&str, u32)],
        charge: Option<f64>,
    ) -> Result<u64, String> {
        Ok(workers::spawn_for_selftest(
            self.game,
            DIMENSION,
            cell,
            owner,
            tool,
            hold,
            charge,
            server::tuning(),
        ))
    }

    fn worker(&self, id: u64) -> Option<Worker> {
        workers::worker_for_selftest(DIMENSION, id)
    }

    fn reports(&self) -> Vec<String> {
        base::sent_reports()
    }

    fn pretend_online(&mut self, owner: u128, online: bool) {
        base::pretend_online(owner, online);
    }

    fn anchor(&self, pos: Pos) -> Option<rustcraft_rules::anchor::State> {
        base::anchor_state(DIMENSION, pos)
    }

    fn held_chunks(&self) -> Vec<[i32; 2]> {
        base::held_chunks(DIMENSION).into_iter().collect()
    }

    fn workers_round_trip(&self) -> Result<(), String> {
        workers::round_trip(DIMENSION)
    }

    fn power(&self, pos: Pos) -> Option<rustcraft_rules::power::PowerView> {
        self.with_level(|l| power::view(l, pos)).flatten()
    }

    fn sky(&self) -> (i64, f32) {
        (self.game.native_day_time(), self.game.native_weather().0)
    }

    fn light(&self, pos: Pos) -> (u8, u8) {
        self.game.native_light(pos)
    }

    fn min_y(&self) -> i32 {
        self.game.native_world_height().0
    }

    fn mining(&self, pos: Pos) -> Option<rustcraft_rules::mining::Shown> {
        mining::shown_at(DIMENSION, pos)
    }

    fn role(&self, pos: Pos) -> Option<String> {
        base::role_at(self.game, pos)
    }

    fn testkit(&mut self, args: &str) -> String {
        let mut ran = false;
        base::on_testkit_command(self.game, &format!("rustcraft_testkit {args}"), &mut ran);
        format!("ran {ran}")
    }

    fn set_ores_off(&mut self, pos: Pos, off: u16) -> Result<(), String> {
        if mining::set_ores_off(self.game, pos, off) {
            Ok(())
        } else {
            Err(format!("no block entity at {pos:?}"))
        }
    }

    fn mixer(&self, pos: Pos) -> Option<rustcraft_rules::machines::mixer::MixerView> {
        mixer::view(self.game, DIMENSION, pos)
    }

    fn catalyst(&self, pos: Pos) -> Option<rustcraft_rules::machines::catalyst::Tank> {
        machines::catalyst_at(self.game, pos)
    }

    fn butcher(&self, pos: Pos) -> Option<rustcraft_rules::butcher::Shown> {
        butcher::shown(DIMENSION, pos)
    }

    fn put_gear(&mut self, pos: Pos, slot: usize, item: &str, charge: f64) -> Result<(), String> {
        let mut stack = ItemStack::new(item, 1);
        crate::game::rustcraft_gear::number(&mut stack);
        crate::game::rustcraft_gear::set_held(
            &stack,
            rustcraft_rules::gear::Held { charge, ..Default::default() },
        );
        self.change_entity(pos, move |e| {
            if e.items.len() <= slot {
                e.items.resize(slot + 1, None);
            }
            e.items[slot] = Some(stack);
        })
    }

    fn gear_charge(&self, pos: Pos, slot: usize) -> Option<f64> {
        let stack = self.game.native_mod_block_entity(pos)?.items.get(slot).cloned().flatten()?;
        crate::game::rustcraft_gear::held(&stack).map(|h| h.charge)
    }

    fn craft_gear(&mut self, grid: &[Option<&str>; 9], charge: f64) -> Option<(String, Option<f64>)> {
        let grid: Vec<Option<ItemStack>> = grid
            .iter()
            .map(|c| {
                c.map(|item| {
                    let mut stack = ItemStack::new(item, 1);
                    if rustcraft_rules::gear::Gear::of(item).is_some() {
                        crate::game::rustcraft_gear::number(&mut stack);
                        crate::game::rustcraft_gear::set_held(
                            &stack,
                            rustcraft_rules::gear::Held { charge, ..Default::default() },
                        );
                    }
                    stack
                })
            })
            .collect();
        let (_, result) = self.game.recipes().find(&grid, 3)?;
        let kept = crate::game::rustcraft_gear::held(&result).map(|h| h.charge);
        Some((result.item.to_string(), kept))
    }

    fn order(&mut self, pos: Pos, item: &str, count: u32) -> Result<(), String> {
        let id = storage::storage_at(DIMENSION, pos).ok_or_else(|| format!("no storage at {pos:?}"))?;
        crate::game::rustcraft_orders::order(DIMENSION, id, item, count)
            .map_err(|(_, why)| why.to_owned())
    }

    fn orders(&self, pos: Pos) -> Vec<rustcraft_rules::orders::queue::Order> {
        storage::storage_at(DIMENSION, pos)
            .map(|id| crate::game::rustcraft_orders::queue_of(DIMENSION, id))
            .unwrap_or_default()
    }

    fn set_keep(&mut self, pos: Pos, item: &str, n: u64) -> Result<(), String> {
        if storage::set_keep_at(DIMENSION, pos, &keys::key(item, 0), n) {
            Ok(())
        } else {
            Err(format!("no storage at {pos:?}"))
        }
    }

    fn cancel_order(&mut self, pos: Pos, order: u16) -> Result<(), String> {
        let id = storage::storage_at(DIMENSION, pos).ok_or_else(|| format!("no storage at {pos:?}"))?;
        crate::game::rustcraft_orders::cancel(DIMENSION, id, order);
        Ok(())
    }

    fn put_grown(&mut self, pos: Pos, slot: usize, item: &str, count: u32) -> Result<(), String> {
        let name = rustcraft_rules::utility::grown::hold_name(item, true);
        let stack = crate::game::rustcraft_utility::hold_stack(&name, count);
        self.change_entity(pos, |e| {
            if e.items.len() <= slot {
                e.items.resize(slot + 1, None);
            }
            e.items[slot] = Some(stack);
        })
    }

    fn restart_orders(&mut self, pos: Pos) -> Result<(), String> {
        storage::storage_at(DIMENSION, pos).ok_or_else(|| format!("no storage at {pos:?}"))?;
        // A restart starts with no temporary pages: the record gives them back.
        server::with_level(DIMENSION, |level| level.temp_pages.clear());
        storage::restart_orders(DIMENSION, through_nbt)
    }

    fn tie_rope(&mut self, owner: u128, a: Pos, b: Pos) -> Result<u64, String> {
        crate::game::rustcraft_zipline::tie_for_selftest(self.game, owner, DIMENSION, a, b)
    }

    fn ropes(&self) -> Vec<rustcraft_rules::traversal::Rope> {
        crate::game::rustcraft_zipline::ropes_for_selftest(self.game)
    }

    fn harpoon_mob(&mut self, from: [f64; 3], id: u32) -> Result<(), String> {
        crate::game::rustcraft_zipline::pull_for_selftest(self.game, DIMENSION, from, id)
    }

    fn handbook_offer(&mut self, player: u128, carried: &[&str]) -> bool {
        crate::game::rustcraft_handbook::offer(self.game, player, carried)
    }

    fn handbook_forget(&mut self, player: u128) {
        crate::game::rustcraft_handbook::forget(self.game, player);
    }

    fn item_known(&self, item: &str) -> bool {
        self.game.items().get(item).is_some()
    }

    fn recipe_grid(&self, item: &str) -> Option<[Option<String>; 9]> {
        crate::game::rustcraft_handbook::recipe_of(self.game, item).map(|(grid, _)| grid)
    }

    fn set_brush_loot(&mut self, pos: Pos, table: &str) -> Result<(), String> {
        crate::game::rustcraft_utility::set_brush_loot(self.game, pos, table)
            .then_some(())
            .ok_or_else(|| format!("no brushable block entity at {pos:?}"))
    }

    fn wrench_lift(
        &mut self,
        pos: Pos,
        into: Pos,
        slot: usize,
    ) -> Result<rustcraft_rules::utility::found::Lift, String> {
        let (stack, _) = crate::game::rustcraft_utility::lift(self.game, DIMENSION, pos)
            .ok_or_else(|| format!("nothing lifted at {pos:?}"))?;
        // What the item says, read back from its custom data.
        let said = crate::game::rustcraft_utility::marks_of(&stack)
            .ok_or_else(|| format!("the lifted {} has no mark", stack.item))?;
        self.change_entity(into, |e| {
            if e.items.len() <= slot {
                e.items.resize(slot + 1, None);
            }
            e.items[slot] = Some(stack);
        })?;
        Ok(said)
    }

    // ---- Slice 18, energy stream ----

    fn break_block(&mut self, pos: Pos, tool: Option<&str>) -> Result<Vec<(String, u32)>, String> {
        let tool = tool.map(|t| ItemStack::new(t, 1));
        let drops = self
            .game
            .native_machine_destroy_block(pos, tool.as_ref(), true)
            .ok_or_else(|| format!("nothing broken at {pos:?}"))?;
        Ok(drops.into_iter().map(|d| (d.item.to_string(), d.count)).collect())
    }

    fn seed(&self, pos: Pos) -> Option<rustcraft_rules::grow::Seed> {
        power::seed_at(self.game, pos)
    }

    fn set_seed(&mut self, pos: Pos, progress: f64) -> Result<(), String> {
        self.change_entity(pos, |e| machines::write_f64(&mut e.nbt, "grow", progress))
    }

    fn crystal_drawn(&self, pos: Pos) -> Option<(Option<[i32; 3]>, bool, bool)> {
        self.game
            .native_mod_block_entity(pos)
            .map(|e| power::drawn_of(&e.nbt))
    }

    // ---- Slice 18, stream 6 (rail and workers) ----
    fn worker_as_item(&mut self, id: u64, cell: Pos) -> Result<u64, String> {
        workers::as_item_for_selftest(self.game, DIMENSION, id, cell, server::tuning())
    }

    fn swap_chassis(&mut self, id: u64, item: &str) -> Result<String, String> {
        workers::swap_for_selftest(DIMENSION, id, item)
    }

    fn worker_room(&self, owner: u128, limit: u32) -> bool {
        workers::may_place_for_selftest(owner, limit)
    }

    fn set_worker_filter(&mut self, id: u64, items: &[&str]) -> Result<(), String> {
        workers::filter_for_selftest(DIMENSION, id, items)
    }




    fn anim(&self, pos: Pos) -> Option<rustcraft_rules::anim::Shown> {
        self.game
            .native_mod_block_entity(pos)
            .map(|e| crate::game::rustcraft_anim::read(&e.nbt))
    }

    fn drop_item(&mut self, at: [f64; 3], item: &str, count: u32) -> Result<(), String> {
        let stack = Tag::compound([
            (
                "id",
                Tag::String(horizon_world::nbt::JavaString::from(item)),
            ),
            ("count", Tag::Int(count.min(99) as i32)),
        ]);
        // No pickup delay and no motion: it lies where it is put.
        let nbt = Compound::from([
            (horizon_world::nbt::JavaString::from("Item"), stack),
            (
                horizon_world::nbt::JavaString::from("Motion"),
                Tag::List(vec![Tag::Double(0f64.to_bits()); 3]),
            ),
        ]);
        self.game
            .native_summon(DIMENSION, "minecraft:item", at, nbt)
            .map(|_| ())
            .ok_or_else(|| format!("no {item} dropped at {at:?}"))
    }

    fn lying(&self, min: [f64; 3], max: [f64; 3]) -> Vec<(String, u32)> {
        self.game
            .native_item_entities(DIMENSION, min, max)
            .into_iter()
            .map(|e| (e.stack.item.to_string(), e.stack.count))
            .collect()
    }

    // ---- Slice 18, base and gear (stream 7) ----

    fn put_gear_held(&mut self, pos: Pos, slot: usize, item: &str, held: rustcraft_rules::gear::Held) -> Result<(), String> {
        let mut stack = ItemStack::new(item, 1);
        crate::game::rustcraft_gear::number(&mut stack);
        crate::game::rustcraft_gear::set_held(&stack, held);
        self.change_entity(pos, move |e| {
            if e.items.len() <= slot {
                e.items.resize(slot + 1, None);
            }
            e.items[slot] = Some(stack);
        })
    }

    fn gear_held(&self, pos: Pos, slot: usize) -> Option<rustcraft_rules::gear::Held> {
        let stack = self.game.native_mod_block_entity(pos)?.items.get(slot).cloned().flatten()?;
        crate::game::rustcraft_gear::held(&stack)
    }

    fn anvil_capacity(&mut self, piece: &str, level: u8, books: usize) -> Vec<Option<u8>> {
        crate::game::rustcraft_gear::anvil_for_selftest(piece, level, books)
    }

    fn age_rope(&mut self, id: u64, ms: u64) -> Result<(), String> {
        crate::game::rustcraft_zipline::age_rope_for_selftest(self.game, id, ms)
    }

    fn turret_aim(&self, pos: Pos) -> Option<(f32, f32)> {
        let e = self.game.native_mod_block_entity(pos)?;
        e.nbt
            .contains_key(&horizon_world::nbt::JavaString::from(base::AIM_KEYS[0]))
            .then(|| base::aim_of(&e.nbt))
    }

    // Slice 18, stream 3 (machines).
    fn swap_tier(&mut self, pos: Pos, item: &str) -> Result<String, String> {
        crate::game::rustcraft_tiers::swap_in_place(self.game, pos, item)
            .map(|(old, _, _)| old)
            .map_err(str::to_owned)
    }

    fn use_bucket(&mut self, pos: Pos, held: &str) -> Result<String, String> {
        fluids::bucket_on_tank(self.game, DIMENSION, pos, held)
            .map(str::to_owned)
            .map_err(|(_, why)| why.to_owned())
    }
}

/// The slice-7 half of the bench.
impl GameBench<'_, '_> {
    /// Mobs and workers in `area` go with its blocks; the self-test's anchors there are forgotten.
    fn clear_slice7(&mut self, area: &Area) {
        let inside = |p: [f64; 3]| {
            (0..3).all(|k| {
                p[k] >= f64::from(area.min[k]) - 1.0 && p[k] <= f64::from(area.max[k]) + 2.0
            })
        };
        let mobs: Vec<u32> = self
            .game
            .mobs
            .iter()
            .filter(|m| !m.removed && inside([m.position.x, m.position.y, m.position.z]))
            .map(|m| m.id)
            .collect();
        for id in mobs {
            self.game.native_remove_mob(DIMENSION, id);
        }
        workers::clear_area(self.game, DIMENSION, area.min, area.max);
        base::forget_area(self.game, DIMENSION, area.min, area.max);
    }
}

pub(super) fn slice7_spawn_mob(
    game: &mut Game<'_>,
    kind: &str,
    at: [f64; 3],
    health: Option<f32>,
) -> Result<u32, String> {
    let options = NativeSpawn {
        no_ai: true,
        silent: true,
        health,
        ..NativeSpawn::default()
    };
    game.native_spawn_mob(DIMENSION, kind, at, &options)
        .ok_or_else(|| format!("no {kind} spawned at {at:?}"))
}

pub(super) fn slice7_mob(game: &Game<'_>, id: u32) -> Option<MobView> {
    let m = game
        .mobs
        .iter()
        .find(|m| m.id == id && !m.removed && m.alive())?;
    Some(MobView {
        kind: m.kind.id.to_string(),
        pos: [m.position.x, m.position.y, m.position.z],
        health: m.health,
        effects: m
            .extra
            .effects
            .iter()
            .map(|(e, ticks, amp)| {
                (
                    (*e).to_owned(),
                    (*ticks).max(0) as u32,
                    (*amp).clamp(0, 255) as u8,
                )
            })
            .collect(),
    })
}
