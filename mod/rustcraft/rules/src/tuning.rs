//! Every balance number in one table (spec section 7: numbers are data, not code). The bundle
//! loads it from a data file; `Tuning::default()` holds the numbers of the spec.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tuning {
    /// N3: how far a node of each tier (1–4) serves a line, in blocks of pipe.
    pub node_range: [u32; 4],
    /// M12: the tier from which a node joins all six faces instead of opposite pairs.
    pub hub_tier: u8,
    /// M10: what a channel of each tier carries a second (tier table of the guide).
    pub item_per_second: [f64; 4],
    /// Slice 17 (proposal): how fast an order's items ride an item channel of each tier, blocks a
    /// second (spec 19, O4: «ингредиенты приходят по магистрали»).
    pub item_ride_speed: [f64; 4],
    pub fluid_buckets_per_second: [f64; 4],
    /// M13: how many fluids a fluid channel of each tier carries at once.
    pub fluid_kinds: [usize; 4],
    /// E2: the E a second an energy channel of each tier lets through.
    pub energy_per_second: [f64; 4],
    /// Server ticks a second.
    pub ticks_per_second: f64,
    /// A single energy crystal's output, E a second, always and free (energy table, E6).
    pub crystal_output: f64,
    /// The electric furnace's draw in normal mode, E a second (guide `furnace`).
    pub furnace_energy: f64,
    /// How many times faster than the base-game furnace the electric one smelts (guide
    /// `furnace`: «вдвое быстрее»): a 200-tick recipe takes it 100.
    pub furnace_speedup: f64,
    /// The crusher's draw in normal mode, E a second (guide `crusher`). Its recipes' times are
    /// in its recipe files.
    pub crusher_energy: f64,
    /// The fuel generator's output while its fuel burns, E a second (energy table: 16 E/s, coal
    /// 1280 E over its 1600 ticks).
    pub generator_output: f64,
    /// The alloy smelter: ticks a recipe takes on fuel and on full energy, and its draw.
    pub alloy_ticks_on_fuel: f64,
    pub alloy_ticks_on_energy: f64,
    pub alloy_energy: f64,
    /// The autocrafter: ticks a craft takes in normal mode, and its draw (guide `crafter`).
    pub crafter_ticks: f64,
    pub crafter_energy: f64,
    /// The autocrafter's manual pages without modules (C2), and how many a capacity module adds.
    pub crafter_pages: usize,
    /// What one tank block holds, in buckets (spec 10, item 11: «бак 16 вёдер»).
    pub tank_buckets: u64,
    /// The pump: ticks it takes to pump a bucket on full energy (round 15: tier 1 takes a bucket
    /// every 10 seconds), and its draw (spec 10, item 11: «насос 4 Е/с»).
    pub pump_ticks: f64,
    pub pump_energy: f64,
    /// The monorail (spec 4.6, guide `monorail` and `station`): blocks a second on a rail of each
    /// tier (T4), blocks of path a trip pays 1 E for (T8), what a station's buffer holds (spec 10,
    /// item 11: «запас станции 2000 Е») and how fast it fills, E a second (a tier-1 energy
    /// channel's worth).
    pub rail_speed: [f64; 4],
    pub rail_blocks_per_energy: f64,
    pub station_buffer: f64,
    pub station_charge: f64,
    /// The worker (spec 4.7, slice 7; numbers are playtest hypotheses): how fast it rides, blocks
    /// a second; its battery, E, and how much each battery upgrade multiplies it (A9: in full);
    /// the share of the battery under which it goes to charge (W4.b); E it spends a block of way.
    pub worker_speed: f64,
    pub worker_battery: f64,
    pub worker_battery_upgrade: f64,
    pub worker_charge_below: f64,
    pub worker_energy_per_block: f64,
    /// Its hold: slots without modules, slots a storage module adds, and the most it can have
    /// (the chassis window shows five rows).
    pub worker_hold: usize,
    pub worker_hold_per_storage: usize,
    pub worker_hold_max: usize,
    /// Upgrade slots of a chassis of sparksteel (guide `chassis`: 3, 5, 7 by tier).
    pub worker_upgrade_slots: usize,
    /// The tool's zone (H4): cells aside from the way and blocks up from it, without and with a
    /// radius module.
    pub tool_side: [i32; 2],
    pub tool_height: [i32; 2],
    /// The saw: ticks it spends a log, E a log, the most logs one tree has, and how far from the
    /// logs its leaves reach.
    pub saw_ticks_per_log: f64,
    pub saw_energy_per_log: f64,
    pub saw_max_logs: usize,
    pub saw_leaf_reach: i32,
    /// The reaper: ticks and E a plant.
    pub reap_ticks: f64,
    pub reap_energy: f64,
    /// The unload dock: its buffer slots and the items a second it takes from a worker; the
    /// charge dock: its buffer, E; what it draws from energy, E/s; what it gives a worker, E/s.
    pub dock_slots: usize,
    pub dock_items_per_second: f64,
    pub dock_buffer: f64,
    pub dock_draw: f64,
    pub dock_charge_rate: f64,
    /// The turret (spec 10 item 11, 27): range, blocks; a shot's damage, E and ticks between
    /// shots; E each effect module adds to a shot (TR1); its buffer and how fast it fills, E/s.
    pub turret_range: f64,
    pub turret_damage: f64,
    pub turret_shot_energy: f64,
    pub turret_effect_energy: f64,
    pub turret_shot_ticks: f64,
    pub turret_buffer: f64,
    pub turret_charge: f64,
    /// The experience a kill gives the owner (D5): the base game's for a common monster.
    pub turret_kill_experience: u32,
    /// Effects of a shot, each module (spec 27 table): bleeding damage a second and seconds;
    /// poison's seconds; slowness in percent a module, its cap and seconds.
    pub bleed_damage: f64,
    pub bleed_seconds: f64,
    pub poison_seconds: f64,
    pub slow_percent: f64,
    pub slow_cap: f64,
    pub slow_seconds: f64,
    /// Support (spec 27): range, blocks; heal a module and ticks between heals; guard percent a
    /// module and its cap; power and haste percent a module; E/s a module while it acts; ticks
    /// between refreshes and how long a buff lasts after a player leaves.
    pub support_range: f64,
    pub heal_amount: f64,
    pub heal_ticks: f64,
    pub guard_percent: f64,
    pub guard_cap: f64,
    pub power_percent: f64,
    pub haste_percent: f64,
    pub support_energy: f64,
    pub support_refresh_ticks: u64,
    pub support_linger_ticks: u64,
    /// The common modules (A7): speed is work ×2 for energy ×2.5, economy energy ×0.6 for work
    /// ×0.85.
    pub speed_work: f64,
    pub speed_energy: f64,
    pub economy_energy: f64,
    pub economy_work: f64,
    /// The anchor (guide `anchor`, B5): E/s a held chunk, and the side of the square of chunks it
    /// holds with 0, 1 and 2 radius modules.
    pub anchor_energy_per_chunk: f64,
    pub anchor_sides: [i32; 3],
    /// Slice 8, the energy sources (spec 4.2, 18.2, 21; guide `solar`, `windmill`, `gen_lava`,
    /// `battery`, `structure`): a solar panel's output by day, E/s, and the shares it keeps in rain
    /// and at night (E13); the rain level from which it counts as rain.
    pub solar_day: f64,
    pub solar_rain_share: f64,
    pub solar_night_share: f64,
    pub solar_rain_from: f64,
    /// The windmill (E14): output at `windmill_low_y` and from `windmill_high_y` up, E/s, rising
    /// in a straight line between.
    pub windmill_low: f64,
    pub windmill_high: f64,
    pub windmill_low_y: i32,
    pub windmill_high_y: i32,
    /// The lava generator: E/s while it burns, E a bucket of lava gives, buckets it holds.
    pub lava_output: f64,
    pub lava_energy_per_bucket: f64,
    pub lava_buckets: u64,
    /// The battery (guide `battery`, spec 10 item 11): E a block holds, E/s a block takes and
    /// gives; what each capacity and output module adds to the whole structure (A7: +50%).
    pub battery_capacity: f64,
    pub battery_rate: f64,
    pub battery_capacity_module: f64,
    pub battery_output_module: f64,
    /// A crystal structure (E7, E8): 4 × N^1.5 E/s while fed, one crystal's otherwise; the
    /// solution it drinks, buckets a minute a crystal (spec 12.3), and what it keeps, buckets.
    pub structure_base: f64,
    pub structure_power: f64,
    pub solution_per_minute: f64,
    pub solution_buffer: f64,
    /// The miner (D1–D5, guide `miner`, `mod_area`, `mod_scout`, `mod_storage`): its draw in
    /// normal mode, E/s; the side of its square with 0, 1 and 2 area modules; cells it scans a tick
    /// at full energy; ticks it digs one ore; its slots without storage modules and the most it
    /// has; the tool its drops are rolled with.
    pub miner_energy: f64,
    pub miner_sides: [i32; 3],
    pub miner_scan_per_tick: f64,
    pub miner_dig_ticks: f64,
    pub miner_slots: usize,
    pub miner_slots_max: usize,
    pub miner_tool: String,
    /// The block placer and breaker (guide `placer`, `breaker`): their draw while they have work,
    /// E/s, ticks a block in normal mode, and the breaker's tool.
    pub placer_energy: f64,
    pub placer_ticks: f64,
    pub breaker_energy: f64,
    pub breaker_ticks: f64,
    pub breaker_tool: String,
    /// Where SoulsCraft's mobs.json may be (its README, «Где лежат числа»), from the server's
    /// folder, first found first: its `turret_effects` and `effects.bleeding` (spec 27.1).
    pub soulscraft_mobs: Vec<String>,
    /// Slice 10: the mixer (guide `mixer`, `fluids`): E/s in normal mode, ticks a bucket of
    /// product takes in normal mode, and the buckets its water tank and its product tank each hold.
    pub mixer_energy: f64,
    pub mixer_ticks: f64,
    pub mixer_buckets: u64,
    /// Slice 10: the cargo hold tool (guide `tool_hold`): a cargo worker's hold, slots; the load
    /// dock's slots and how fast it loads a worker, items a second.
    pub cargo_hold: usize,
    pub load_dock_slots: usize,
    pub load_items_per_second: f64,
    /// Slice 10: the grab (guide `tool_grab`): ticks and E an ore block takes; the scoop (guide
    /// `tool_scoop`): ticks and E a block takes (2 a second, 4 E) and how deep under the way it
    /// digs.
    pub grab_ticks: f64,
    pub grab_energy: f64,
    pub scoop_ticks: f64,
    pub scoop_energy: f64,
    pub scoop_depth: i32,
    /// Slice 10: the hydro turbine (round 15, E12): E/s in a current and in still water, the share
    /// it gains a block of depth (at most `turbine_depth_max` blocks) and a turbine upstream in its
    /// structure (at most `turbine_upstream_max`).
    pub turbine_flowing: f64,
    pub turbine_still: f64,
    pub turbine_depth_bonus: f64,
    pub turbine_depth_max: u32,
    pub turbine_upstream_bonus: f64,
    pub turbine_upstream_max: u32,
    /// Slice 10: the backpacks' slots by tier (G4: sparksteel 27, crystallite 54).
    pub backpack_slots: [usize; 2],
    /// Slice 11: the catalysts (K1, K2, spec 4.12): the buckets a machine's catalyst tank holds,
    /// the seconds of work one bucket lasts, and what each does: the accelerator's speed, the
    /// economizer's energy, the enricher's chance of an extra product. **Proposals.**
    pub catalyst_buckets: u64,
    pub catalyst_seconds_per_bucket: f64,
    pub accelerator_speed: f64,
    pub economizer_energy: f64,
    pub enricher_chance: f64,
    /// Slice 11: the guard (P2, the shooter tool of tier 3): its range in blocks, a shot's damage,
    /// its battery's E and the ticks it stands for a shot. **Proposals.**
    pub guard_range: f64,
    pub guard_damage: f64,
    pub guard_shot_energy: f64,
    pub guard_shot_ticks: f64,
    /// Slice 11: the butcher (round 26, K1–K8): its radius, with a radius module, ticks between
    /// strikes, and how many adults of a kind it leaves by default, at least and at most.
    pub butcher_radius: f64,
    pub butcher_radius_module: f64,
    pub butcher_ticks: u64,
    pub butcher_keep_default: u32,
    pub butcher_keep_min: u32,
    pub butcher_keep_max: u32,
    // ---- Slice 18, energy stream ----
    /// CR2 (guide `grow`): a seed on a tank of solution grows into a druse in this many minutes,
    /// drinking this many buckets over its growth; it keeps at most `seed_buffer` buckets.
    pub grow_minutes: f64,
    pub grow_buckets: f64,
    pub seed_buffer: f64,
}

impl Default for Tuning {
    fn default() -> Self {
        Tuning {
            node_range: [32, 64, 128, 256],
            hub_tier: 4,
            item_per_second: [4.0, 16.0, 64.0, 256.0],
            item_ride_speed: [4.0, 8.0, 16.0, 32.0],
            fluid_buckets_per_second: [1.0, 4.0, 16.0, 64.0],
            fluid_kinds: [1, 2, 4, 8],
            energy_per_second: [32.0, 128.0, 512.0, 2048.0],
            ticks_per_second: 20.0,
            crystal_output: 4.0,
            furnace_energy: 8.0,
            furnace_speedup: 2.0,
            crusher_energy: 8.0,
            generator_output: 16.0,
            alloy_ticks_on_fuel: 200.0,
            alloy_ticks_on_energy: 100.0,
            alloy_energy: 8.0,
            crafter_ticks: 20.0,
            crafter_energy: 8.0,
            crafter_pages: 1,
            tank_buckets: 16,
            pump_ticks: 200.0,
            pump_energy: 4.0,
            rail_speed: [8.0, 14.0, 22.0, 32.0],
            rail_blocks_per_energy: 4.0,
            station_buffer: 2000.0,
            station_charge: 32.0,
            worker_speed: 2.0,
            worker_battery: 2000.0,
            worker_battery_upgrade: 1.5,
            worker_charge_below: 0.2,
            worker_energy_per_block: 0.5,
            worker_hold: 27,
            worker_hold_per_storage: 9,
            worker_hold_max: 45,
            worker_upgrade_slots: 3,
            tool_side: [1, 2],
            tool_height: [3, 5],
            saw_ticks_per_log: 4.0,
            saw_energy_per_log: 4.0,
            saw_max_logs: 128,
            saw_leaf_reach: 6,
            reap_ticks: 4.0,
            reap_energy: 2.0,
            dock_slots: 9,
            dock_items_per_second: 32.0,
            dock_buffer: 1000.0,
            dock_draw: 16.0,
            dock_charge_rate: 64.0,
            turret_range: 12.0,
            turret_damage: 4.0,
            turret_shot_energy: 40.0,
            turret_effect_energy: 10.0,
            turret_shot_ticks: 20.0,
            turret_buffer: 800.0,
            turret_charge: 40.0,
            turret_kill_experience: 5,
            bleed_damage: 1.0,
            bleed_seconds: 4.0,
            poison_seconds: 4.0,
            slow_percent: 20.0,
            slow_cap: 60.0,
            slow_seconds: 3.0,
            support_range: 6.0,
            heal_amount: 1.0,
            heal_ticks: 60.0,
            guard_percent: 10.0,
            guard_cap: 30.0,
            power_percent: 10.0,
            haste_percent: 10.0,
            support_energy: 8.0,
            support_refresh_ticks: 20,
            support_linger_ticks: 40,
            speed_work: 2.0,
            speed_energy: 2.5,
            economy_energy: 0.6,
            economy_work: 0.85,
            anchor_energy_per_chunk: 2.0,
            anchor_sides: [1, 3, 5],
            solar_day: 2.0,
            solar_rain_share: 0.5,
            solar_night_share: 0.25,
            solar_rain_from: 0.2,
            windmill_low: 4.0,
            windmill_high: 10.0,
            windmill_low_y: 63,
            windmill_high_y: 160,
            lava_output: 40.0,
            lava_energy_per_bucket: 8000.0,
            lava_buckets: 16,
            battery_capacity: 40_000.0,
            battery_rate: 32.0,
            battery_capacity_module: 0.5,
            battery_output_module: 0.5,
            structure_base: 4.0,
            structure_power: 1.5,
            solution_per_minute: 0.1,
            solution_buffer: 1.0,
            miner_energy: 8.0,
            miner_sides: [11, 22, 33],
            miner_scan_per_tick: 64.0,
            miner_dig_ticks: 20.0,
            miner_slots: 9,
            miner_slots_max: 27,
            miner_tool: "minecraft:diamond_pickaxe".to_owned(),
            placer_energy: 2.0,
            placer_ticks: 20.0,
            breaker_energy: 2.0,
            breaker_ticks: 20.0,
            breaker_tool: "minecraft:diamond_pickaxe".to_owned(),
            soulscraft_mobs: vec![
                "horizon-mods/soulscraft/data/soulscraft/mobs.json".to_owned(),
                "config/soulscraft/mobs.json".to_owned(),
            ],
            mixer_energy: 8.0,
            mixer_ticks: 100.0,
            mixer_buckets: 4,
            cargo_hold: 45,
            load_dock_slots: 9,
            load_items_per_second: 32.0,
            grab_ticks: 10.0,
            grab_energy: 4.0,
            scoop_ticks: 10.0,
            scoop_energy: 4.0,
            scoop_depth: 3,
            turbine_flowing: 6.0,
            turbine_still: 1.0,
            turbine_depth_bonus: 0.1,
            turbine_depth_max: 8,
            turbine_upstream_bonus: 0.05,
            turbine_upstream_max: 10,
            backpack_slots: [27, 54],
            catalyst_buckets: 1,
            catalyst_seconds_per_bucket: 300.0,
            accelerator_speed: 1.5,
            economizer_energy: 0.7,
            enricher_chance: 0.15,
            guard_range: 10.0,
            guard_damage: 4.0,
            guard_shot_energy: 40.0,
            guard_shot_ticks: 20.0,
            butcher_radius: 4.0,
            butcher_radius_module: 8.0,
            butcher_ticks: 200,
            butcher_keep_default: 2,
            butcher_keep_min: 2,
            butcher_keep_max: 16,
            grow_minutes: 10.0,
            grow_buckets: 4.0,
            seed_buffer: 1.0,
        }
    }
}

impl Tuning {
    /// The range of a node of `tier`, clamped to the known tiers.
    pub fn range_of(&self, tier: u8) -> u32 {
        self.node_range[tier_index(tier)]
    }

    /// Items a second an item net with weakest channel `tier` carries.
    pub fn items_of(&self, tier: u8) -> f64 {
        self.item_per_second[tier_index(tier)]
    }

    /// Blocks a second an order's items ride an item net with weakest channel `tier` (slice 17).
    pub fn ride_speed_of(&self, tier: u8) -> f64 {
        self.item_ride_speed[tier_index(tier)]
    }

    /// Buckets a second a fluid net with weakest channel `tier` carries, each fluid in its lane.
    pub fn fluids_of(&self, tier: u8) -> f64 {
        self.fluid_buckets_per_second[tier_index(tier)]
    }

    /// How many fluids a fluid net with weakest channel `tier` carries at once (M13).
    pub fn kinds_of(&self, tier: u8) -> usize {
        self.fluid_kinds[tier_index(tier)]
    }

    /// E a second an energy net with weakest channel `tier` lets through (E2).
    pub fn energy_of(&self, tier: u8) -> f64 {
        self.energy_per_second[tier_index(tier)]
    }

    /// The monorail's numbers as its rules take them.
    pub fn rail_spec(&self) -> crate::monorail::RailSpec {
        crate::monorail::RailSpec {
            blocks_per_energy: self.rail_blocks_per_energy,
            speed: self.rail_speed,
        }
    }
}

/// Tiers run 1–4; anything outside is clamped.
fn tier_index(tier: u8) -> usize {
    usize::from(tier.clamp(1, 4)) - 1
}
