// What RustCraft adds to the registries, declared in code (`native_blocks`, `native_items`,
// SRC/crates/horizon-client/src/game/native_hooks.rs:3220-3248) rather than in the content mod's
// data/rustcraft/horizon/ files: the blocks then exist whenever the code that runs them is built
// in, and their ids live next to that code. They join the content mod with the same id
// (mod/content) when it is on, which gives them models, textures, loot, tags and names; without
// it they look like their templates (SRC/docs/native-mods.md:1223).
use crate::game::Game;
use horizon_client::containers::ModContainer;
use horizon_mods::content::{Block, BlockProperties, BlockShapes, Item, StateProperty};
use horizon_world::nbt::Compound;
use rustcraft_rules::net::topology::ChannelKind;
use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, OnceLock},
};

pub const HOUSING: &str = "rustcraft:housing";
pub const NODE: &str = "rustcraft:node";
/// The energy crystal: 4 E/s, always and free (E6), to the machines it touches (E10) and to an
/// energy channel of a housing it touches.
pub const CRYSTAL: &str = "rustcraft:crystal";
/// The electric furnace: smelts what the base game's furnace smelts, on energy, twice as fast.
pub const FURNACE: &str = "rustcraft:furnace";

/// The crusher: raw iron, copper and gold into two dusts each, on energy (guide `crusher`).
pub const CRUSHER: &str = "rustcraft:crusher";
pub const DUST_IRON: &str = "rustcraft:dust_iron";
pub const DUST_COPPER: &str = "rustcraft:dust_copper";
pub const DUST_GOLD: &str = "rustcraft:dust_gold";

/// A one-input machine's slots: input, a slot nothing reaches, result. Slice 2's furnace opened
/// vanilla's furnace menu, whose fuel slot is 1, and its saved items keep those places; the
/// crusher has the same three so both machines share one layout. Slot 1 is in no face's list and
/// the machine's window hides it, so its menu has no slot 1 either (native_screen,
/// SRC/crates/horizon-client/src/game/native_hooks.rs:3742-3748).
pub const MACHINE_INPUT: usize = 0;
pub const MACHINE_OUTPUT: usize = 2;
const MACHINE_SLOTS: usize = 3;

/// The fuel generator: 16 E/s while a furnace fuel burns (guide `gen_fuel`).
pub const GENERATOR: &str = "rustcraft:gen_fuel";
/// Its one slot: fuel.
pub const GEN_FUEL: usize = 0;
/// The alloy smelter (guide `alloy`): four inputs, fuel, three outputs.
pub const ALLOY: &str = "rustcraft:alloy";
pub const ALLOY_INPUTS: std::ops::Range<usize> = 0..4;
pub const ALLOY_FUEL: usize = 4;
pub const ALLOY_OUTPUTS: std::ops::Range<usize> = 5..8;
/// The autocrafter (guide `crafter`): a buffer the net fills, outputs, module slots.
pub const CRAFTER: &str = "rustcraft:crafter";
pub const CRAFTER_BUFFER: std::ops::Range<usize> = 0..9;
pub const CRAFTER_OUTPUTS: std::ops::Range<usize> = 9..12;
pub const CRAFTER_MODULES: std::ops::Range<usize> = 12..15;

/// The alloys, one a tier (guide `alloys`): copper-steel from the crafting table, the others
/// from the alloy smelter.
pub const COPPERSTEEL: &str = "rustcraft:coppersteel";
pub const SPARKSTEEL: &str = "rustcraft:sparksteel";
pub const CRYSTALLITE: &str = "rustcraft:crystallite";
pub const STARSTEEL: &str = "rustcraft:starsteel";
/// The capacity module (guide `mod_cap`): in an autocrafter's module slot, one more page (C2).
pub const MODULE_CAPACITY: &str = "rustcraft:module_capacity";

/// The storage (guide `cell`, `terminal`, spec 4.5): cells in a solid box hold counts of items,
/// a terminal that touches them shows the whole storage (rustcraft_storage).
pub const CELL: &str = "rustcraft:cell";
pub const TERMINAL: &str = "rustcraft:terminal";
/// The tank (guide `tank`): one fluid, 16 buckets a block, touching tanks are one
/// (rustcraft_fluids).
pub const TANK: &str = "rustcraft:tank";
/// The pump (guide `pump`): a machine on energy that takes the source block under it.
pub const PUMP: &str = "rustcraft:pump";
/// The monorail (spec 4.6): one block for every tier and mount (`tier`, `mount` and `shape` in its
/// state, rustcraft_rules::monorail::shape), placed by one item a tier.
pub const MONORAIL: &str = "rustcraft:monorail";
pub const MONORAIL_ITEMS: [&str; 4] = [
    "rustcraft:monorail",
    "rustcraft:monorail_2",
    "rustcraft:monorail_3",
    "rustcraft:monorail_4",
];
/// The station (guide `station`): a machine that fills its buffer on energy and sends the
/// capsule (rustcraft_monorail).
pub const STATION: &str = "rustcraft:station";
/// Slice 7: the worker's docks (guide `dock_unload`, `dock_charge`), the turret and the base anchor
/// (rustcraft_workers, rustcraft_base). They copy `minecraft:iron_block` with a `facing` of their
/// own: no template menu, their windows are server menus.
pub const DOCK_UNLOAD: &str = "rustcraft:dock_unload";
pub const DOCK_CHARGE: &str = "rustcraft:dock_charge";
pub const TURRET: &str = "rustcraft:turret";
pub const ANCHOR: &str = "rustcraft:anchor";
/// Slice 8: the energy sources and the battery (rustcraft_power), the miner, the block placer and
/// the block breaker (rustcraft_mining). They copy `minecraft:iron_block` with a `facing`, as the
/// slice-7 blocks do; their windows are server menus.
pub const SOLAR: &str = "rustcraft:solar";
pub const WINDMILL: &str = "rustcraft:windmill";
pub const LAVA_GENERATOR: &str = "rustcraft:gen_lava";
pub const BATTERY: &str = "rustcraft:battery";
pub const MINER: &str = "rustcraft:miner";
pub const PLACER: &str = "rustcraft:placer";
pub const BREAKER: &str = "rustcraft:breaker";
/// The miner's slots (the most `Tuning::miner_slots_max` lets it use) and the placer's and the
/// breaker's.
pub const MINER_SLOTS: usize = 27;
pub const BLOCK_SLOTS: usize = 9;
/// The worker's chassis and tools (rustcraft_workers), placed on a rail as a worker.
pub const CHASSIS: &str = "rustcraft:chassis";
/// The `facing` of the slice-7 blocks: the side their front looks to.
pub const FACING: &str = "facing";
pub const FACINGS: [&str; 4] = ["north", "east", "south", "west"];
/// The tier a monorail item places.
pub fn monorail_tier(item: &str) -> Option<u8> {
    MONORAIL_ITEMS
        .iter()
        .position(|i| *i == item)
        .map(|i| i as u8 + 1)
}
/// The one slot of the terminal's, the tank's and the pump's menus: their windows have no slot
/// of their own, but a menu type has at least one (SRC/docs/mods.md:371-393). No face reaches it,
/// the window hides it and the slot rules refuse everything there.
const HIDDEN_SLOTS: usize = 1;
/// The unload dock's buffer: slots the net and hoppers take from (Tuning::dock_slots).
pub const DOCK_SLOTS: usize = 9;

/// RustCraft's machines: the blocks with slots and a window of their own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Machine {
    Furnace,
    Crusher,
    Generator,
    Alloy,
    Crafter,
    Pump,
    Station,
    DockUnload,
    DockCharge,
    Turret,
    Anchor,
    Solar,
    Windmill,
    LavaGenerator,
    Battery,
    Miner,
    Placer,
    Breaker,
}

/// How a machine works: the rules type that runs it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// One input, one result (rustcraft_rules::machines::processor).
    Processor,
    Generator,
    Alloy,
    Crafter,
    /// rustcraft_rules::machines::pump.
    Pump,
    /// rustcraft_monorail: its buffer fills on energy.
    Station,
    /// rustcraft_workers: the unload dock, slots the net takes from.
    Dock,
    /// rustcraft_workers: the charge dock, a buffer on energy for workers.
    Charger,
    /// rustcraft_base: a buffer on energy for its shots and buffs.
    Turret,
    /// rustcraft_base: draws for the chunks it holds.
    Anchor,
    /// rustcraft_power: a solar panel, a windmill, a lava generator.
    Power,
    /// rustcraft_power: a battery block of a battery structure (E11).
    Battery,
    /// rustcraft_mining: the miner, the placer and the breaker.
    Mining,
}

impl Machine {
    pub const ALL: [Machine; 18] = [
        Machine::Furnace,
        Machine::Crusher,
        Machine::Generator,
        Machine::Alloy,
        Machine::Crafter,
        Machine::Pump,
        Machine::Station,
        Machine::DockUnload,
        Machine::DockCharge,
        Machine::Turret,
        Machine::Anchor,
        Machine::Solar,
        Machine::Windmill,
        Machine::LavaGenerator,
        Machine::Battery,
        Machine::Miner,
        Machine::Placer,
        Machine::Breaker,
    ];

    pub fn of(block: &str) -> Option<Machine> {
        Machine::ALL.into_iter().find(|m| m.block() == block)
    }

    pub fn block(self) -> &'static str {
        match self {
            Machine::Furnace => FURNACE,
            Machine::Crusher => CRUSHER,
            Machine::Generator => GENERATOR,
            Machine::Alloy => ALLOY,
            Machine::Crafter => CRAFTER,
            Machine::Pump => PUMP,
            Machine::Station => STATION,
            Machine::DockUnload => DOCK_UNLOAD,
            Machine::DockCharge => DOCK_CHARGE,
            Machine::Turret => TURRET,
            Machine::Anchor => ANCHOR,
            Machine::Solar => SOLAR,
            Machine::Windmill => WINDMILL,
            Machine::LavaGenerator => LAVA_GENERATOR,
            Machine::Battery => BATTERY,
            Machine::Miner => MINER,
            Machine::Placer => PLACER,
            Machine::Breaker => BREAKER,
        }
    }

    pub fn kind(self) -> Kind {
        match self {
            Machine::Furnace | Machine::Crusher => Kind::Processor,
            Machine::Generator => Kind::Generator,
            Machine::Alloy => Kind::Alloy,
            Machine::Crafter => Kind::Crafter,
            Machine::Pump => Kind::Pump,
            Machine::Station => Kind::Station,
            Machine::DockUnload => Kind::Dock,
            Machine::DockCharge => Kind::Charger,
            Machine::Turret => Kind::Turret,
            Machine::Anchor => Kind::Anchor,
            Machine::Solar | Machine::Windmill | Machine::LavaGenerator => Kind::Power,
            Machine::Battery => Kind::Battery,
            Machine::Miner | Machine::Placer | Machine::Breaker => Kind::Mining,
        }
    }

    /// The machine's own menu type, the content mod's data/rustcraft/horizon/menu/<name>.json
    /// (its slots, data slots and result slots): API 85, SRC/docs/mods.md:371-393. It opens when
    /// the block is used because the machines' template, vanilla's furnace, opens a menu on use
    /// (SRC/crates/horizon-world/src/level/interact.rs:430-454 through game.rs:6059-6165, where
    /// `open_mod_menu` takes a mod container's menu); a Horizon client shows it through
    /// rustcraft_screens. Each menu type is named after its block.
    pub fn menu(self) -> &'static str {
        self.block()
    }

    /// How many slots its block entity holds.
    pub fn slots(self) -> usize {
        match self.kind() {
            Kind::Processor => MACHINE_SLOTS,
            Kind::Generator => 1,
            Kind::Alloy => ALLOY_OUTPUTS.end,
            Kind::Crafter => CRAFTER_MODULES.end,
            Kind::Pump | Kind::Station => HIDDEN_SLOTS,
            Kind::Dock => DOCK_SLOTS,
            Kind::Mining if self == Machine::Miner => MINER_SLOTS,
            Kind::Mining => BLOCK_SLOTS,
            Kind::Charger | Kind::Turret | Kind::Anchor | Kind::Power | Kind::Battery => 0,
        }
    }

    /// The slots the net and hoppers put into.
    pub fn inputs(self) -> Vec<usize> {
        match self.kind() {
            Kind::Processor => vec![MACHINE_INPUT],
            Kind::Generator => vec![GEN_FUEL],
            Kind::Alloy => ALLOY_INPUTS.chain([ALLOY_FUEL]).collect(),
            Kind::Crafter => CRAFTER_BUFFER.collect(),
            Kind::Mining if self == Machine::Placer => (0..BLOCK_SLOTS).collect(),
            _ => Vec::new(),
        }
    }

    /// The slots the net and hoppers take from: the products.
    pub fn outputs(self) -> Vec<usize> {
        match self.kind() {
            Kind::Processor => vec![MACHINE_OUTPUT],
            Kind::Generator => Vec::new(),
            Kind::Alloy => ALLOY_OUTPUTS.collect(),
            Kind::Crafter => CRAFTER_OUTPUTS.collect(),
            Kind::Dock => (0..DOCK_SLOTS).collect(),
            Kind::Mining if self != Machine::Placer => (0..self.slots()).collect(),
            _ => Vec::new(),
        }
    }

    /// Whether the net or a hopper reaches any of its slots: not the pump's.
    pub fn has_item_slots(self) -> bool {
        !self.inputs().is_empty() || !self.outputs().is_empty()
    }

    /// A source of energy (the generators, the panel, the windmill) rather than a consumer.
    pub fn is_source(self) -> bool {
        matches!(self.kind(), Kind::Generator | Kind::Power)
    }

    /// Whether its block copies the furnace and opens its own menu type; the slice-7 blocks copy
    /// the iron block and open server menus.
    pub fn has_menu(self) -> bool {
        !matches!(
            self.kind(),
            Kind::Dock
                | Kind::Charger
                | Kind::Turret
                | Kind::Anchor
                | Kind::Power
                | Kind::Battery
                | Kind::Mining
        )
    }
}

/// The solar panel's outline and collision: the guide's panel on its post, 6.5 pixels high.
const SOLAR_BOX: [f64; 6] = [0.0, 0.0, 0.0, 1.0, 0.40625, 1.0];
/// The crystal's outline and collision: the staged model's cluster, 2..14 across, 15 high.
const CRYSTAL_BOX: [f64; 6] = [0.125, 0.0, 0.125, 0.875, 0.9375, 0.875];
pub const ITEM_CHANNEL: &str = "rustcraft:item_channel";
pub const FLUID_CHANNEL: &str = "rustcraft:fluid_channel";
pub const ENERGY_CHANNEL: &str = "rustcraft:energy_channel";
pub const WRENCH: &str = "rustcraft:wrench";

/// The node's state property: its tier, 1 to 4 (N3).
pub const TIER: &str = "tier";
const TIERS: [&str; 4] = ["1", "2", "3", "4"];

/// The block tag a housing joins besides other housings: the node. The tag file is the content
/// mod's data/rustcraft/tags/block/magistral.json; code names a tag without `#`
/// (SRC/crates/horizon-mods/src/content.rs:1043-1044, test at 1460-1465).
const JOINS: &str = "rustcraft:magistral";
/// The housing's width in pixels: a core and an arm to each joined side, as `PipeBlock` builds
/// them (SRC/docs/mods.md:281-294). Must match the content mod's housing models (4 to 12): the
/// exported cage is 6 pixels across (5..11) and the collars at the arms' ends 6.5 (4.75..11.25),
/// so the outline and collision wrap the collars.
const HOUSING_THICKNESS: f32 = 6.5;

/// The item that carries a channel of `kind`.
pub fn channel_item(kind: ChannelKind) -> &'static str {
    match kind {
        ChannelKind::Item => ITEM_CHANNEL,
        ChannelKind::Fluid => FLUID_CHANNEL,
        ChannelKind::Energy => ENERGY_CHANNEL,
    }
}

/// The kind of channel item `item` carries, if it is one.
pub fn channel_kind(item: &str) -> Option<ChannelKind> {
    [ChannelKind::Item, ChannelKind::Fluid, ChannelKind::Energy]
        .into_iter()
        .find(|&kind| channel_item(kind) == item)
}

/// The housing, the node, the crystal and the machines, each with its block item.
///
/// States: the housing copies `minecraft:chorus_plant`, whose six side booleans give it 64
/// states; the node copies `minecraft:iron_block` (one state) times its tier (4). Every state of
/// every block must fit in 2^20 (SRC/docs/mods.md:160-166; vanilla takes 35,723), and these 68
/// with the crystal's and the cell's 1 each, 8 for each of the seven machines (the station is
/// one), the terminal and the tank (72) and the monorail's 480 keep the registry under 2^16, so
/// chunks keep vanilla's 16-bit palette
/// (SRC/docs/modpacks.md:281-284). Which channel sits in which slot is block entity data, not
/// state: four slots of four kinds would multiply the housing's states by 256.
pub fn declare_blocks(blocks: &mut Vec<Block>) {
    blocks.push(Block {
        id: HOUSING.into(),
        template: "minecraft:chorus_plant".into(),
        properties: BlockProperties {
            hardness: Some(1.0),
            explosion_resistance: Some(3.0),
            sounds: Some("minecraft:copper_block".into()),
            ..BlockProperties::default()
        },
        connects_to: Some(JOINS.into()),
        thickness: Some(HOUSING_THICKNESS),
        item: true,
        ..Block::default()
    });
    blocks.push(Block {
        id: NODE.into(),
        template: "minecraft:iron_block".into(),
        properties: BlockProperties {
            hardness: Some(3.0),
            explosion_resistance: Some(6.0),
            sounds: Some("minecraft:copper_block".into()),
            ..BlockProperties::default()
        },
        state_properties: BTreeMap::from([(
            TIER.to_owned(),
            StateProperty::Enum {
                values: TIERS.map(str::to_owned).to_vec(),
            },
        )]),
        default_state: BTreeMap::from([(TIER.to_owned(), TIERS[0].to_owned())]),
        item: true,
        ..Block::default()
    });
    declare_machines(blocks);
    declare_slice7(blocks);
    declare_storage_and_tanks(blocks);
    declare_monorail(blocks);
}

/// The monorail: it copies `minecraft:iron_block` (one state) and declares its tier, its mount
/// (the side of its cell the block it hangs on is behind, chosen from the clicked face as it is
/// placed, rustcraft_monorail::state_for_placement) and its shape (the halves, corner or climb
/// it shows, set by the server from its neighbours): 4 × 6 × 20 = 480 states. It lets players and
/// the capsule through, as vanilla's rails do, and its outline is a thin plate on its mount's
/// side. Its block entity only lists it for the server's scan.
fn declare_monorail(blocks: &mut Vec<Block>) {
    use rustcraft_rules::monorail::shape::{self, MOUNT_VALUES, Shape};
    let plate = |mount: &str| -> [f64; 6] {
        const T: f64 = 2.0 / 16.0;
        match mount {
            "down" => [0.0, 0.0, 0.0, 1.0, T, 1.0],
            "up" => [0.0, 1.0 - T, 0.0, 1.0, 1.0, 1.0],
            "north" => [0.0, 0.0, 0.0, 1.0, 1.0, T],
            "south" => [0.0, 0.0, 1.0 - T, 1.0, 1.0, 1.0],
            "west" => [0.0, 0.0, 0.0, T, 1.0, 1.0],
            _ => [1.0 - T, 0.0, 0.0, 1.0, 1.0, 1.0],
        }
    };
    let outline = BlockShapes {
        property: shape::MOUNT.to_owned(),
        properties: Vec::new(),
        values: MOUNT_VALUES
            .iter()
            .map(|m| ((*m).to_owned(), vec![plate(m)]))
            .collect(),
        default: None,
    };
    let through = BlockShapes {
        property: String::new(),
        properties: Vec::new(),
        values: BTreeMap::new(),
        default: Some(Vec::new()),
    };
    blocks.push(Block {
        id: MONORAIL.into(),
        template: "minecraft:iron_block".into(),
        properties: BlockProperties {
            hardness: Some(1.0),
            explosion_resistance: Some(3.0),
            sounds: Some("minecraft:copper_block".into()),
            occludes: Some(false),
            ..BlockProperties::default()
        },
        state_properties: BTreeMap::from([
            (
                TIER.to_owned(),
                StateProperty::Enum {
                    values: TIERS.map(str::to_owned).to_vec(),
                },
            ),
            (
                shape::MOUNT.to_owned(),
                StateProperty::Enum {
                    values: MOUNT_VALUES.map(str::to_owned).to_vec(),
                },
            ),
            (
                shape::PROPERTY.to_owned(),
                StateProperty::Enum {
                    values: Shape::values(),
                },
            ),
        ]),
        default_state: BTreeMap::from([
            (TIER.to_owned(), TIERS[0].to_owned()),
            (shape::MOUNT.to_owned(), "down".to_owned()),
            (shape::PROPERTY.to_owned(), Shape::Halves(0).value()),
        ]),
        shapes: Some(outline),
        collision_shapes: Some(through),
        item: true,
        ..Block::default()
    });
}

/// The cell, the terminal and the tank. The cell copies `minecraft:iron_block` (one state) and has
/// a block entity only for the storage id it keeps; the terminal and the tank copy
/// `minecraft:furnace`, as the machines do, for a menu on use and `facing`.
fn declare_storage_and_tanks(blocks: &mut Vec<Block>) {
    blocks.push(Block {
        id: CELL.into(),
        template: "minecraft:iron_block".into(),
        properties: BlockProperties {
            hardness: Some(3.0),
            explosion_resistance: Some(6.0),
            sounds: Some("minecraft:copper_block".into()),
            ..BlockProperties::default()
        },
        item: true,
        ..Block::default()
    });
    for id in [TERMINAL, TANK] {
        blocks.push(Block {
            id: id.into(),
            template: "minecraft:furnace".into(),
            properties: BlockProperties {
                hardness: Some(3.0),
                explosion_resistance: Some(6.0),
                sounds: Some("minecraft:copper_block".into()),
                ..BlockProperties::default()
            },
            item: true,
            ..Block::default()
        });
    }
}

/// The crystal and the machines. The crystal copies `minecraft:amethyst_block` (one state) with
/// the cluster's outline and no occlusion; the furnace and the crusher copy `minecraft:furnace`
/// for a menu on use and `facing` on placement (times `lit`: 8 states each, `lit` never set, so
/// they never glow).
/// The slice-7 blocks: the iron block's one state times their own `facing` (4), set as a
/// furnace sets it, to face the player placing them (rustcraft_base::state_for_placement). The
/// turret's head reaches past its cell, so it hides nothing behind it.
fn declare_slice7(blocks: &mut Vec<Block>) {
    for machine in Machine::ALL.into_iter().filter(|m| !m.has_menu()) {
        // E19: the solar panel is a thin plate that lets the light through, so there is no
        // shadow under it; the turret's head and the windmill's mast reach past their cells.
        let open = matches!(
            machine,
            Machine::Turret | Machine::Windmill | Machine::Solar
        );
        let plate = (machine == Machine::Solar).then(|| BlockShapes {
            property: String::new(),
            properties: Vec::new(),
            values: BTreeMap::new(),
            default: Some(vec![SOLAR_BOX]),
        });
        blocks.push(Block {
            id: machine.block().into(),
            // E19: glass passes the sky's light down whole (the self-test found the iron block's
            // non-occluding copy still dims it by one level).
            template: if machine == Machine::Solar {
                "minecraft:glass"
            } else {
                "minecraft:iron_block"
            }
            .into(),
            properties: BlockProperties {
                hardness: Some(3.5),
                explosion_resistance: Some(6.0),
                sounds: Some("minecraft:copper_block".into()),
                occludes: open.then_some(false),
                ..BlockProperties::default()
            },
            shapes: plate.clone(),
            collision_shapes: plate,
            state_properties: BTreeMap::from([(
                FACING.to_owned(),
                StateProperty::Enum {
                    values: FACINGS.map(str::to_owned).to_vec(),
                },
            )]),
            default_state: BTreeMap::from([(FACING.to_owned(), FACINGS[0].to_owned())]),
            item: true,
            ..Block::default()
        });
    }
}

fn declare_machines(blocks: &mut Vec<Block>) {
    let crystal_shape = || BlockShapes {
        property: String::new(),
        properties: Vec::new(),
        values: BTreeMap::new(),
        default: Some(vec![CRYSTAL_BOX]),
    };
    blocks.push(Block {
        id: CRYSTAL.into(),
        template: "minecraft:amethyst_block".into(),
        properties: BlockProperties {
            hardness: Some(1.5),
            explosion_resistance: Some(3.0),
            light: Some(7),
            sounds: Some("minecraft:amethyst_block".into()),
            occludes: Some(false),
            ..BlockProperties::default()
        },
        shapes: Some(crystal_shape()),
        collision_shapes: Some(crystal_shape()),
        item: true,
        ..Block::default()
    });
    for machine in Machine::ALL.into_iter().filter(|m| m.has_menu()) {
        blocks.push(Block {
            id: machine.block().into(),
            template: "minecraft:furnace".into(),
            properties: BlockProperties {
                hardness: Some(3.5),
                explosion_resistance: Some(6.0),
                sounds: Some("minecraft:copper_block".into()),
                ..BlockProperties::default()
            },
            item: true,
            ..Block::default()
        });
    }
}

/// The channels, the wrench and the dusts. The channels' and the wrench's templates have no use of their own: what a click with them
/// does is all `native_use_item_on`'s (rustcraft_server::on_use_item_on).
pub fn declare_items(items: &mut Vec<Item>) {
    for id in [ITEM_CHANNEL, FLUID_CHANNEL, ENERGY_CHANNEL] {
        items.push(Item {
            id: id.into(),
            template: "minecraft:iron_ingot".into(),
            ..Item::default()
        });
    }
    items.push(Item {
        id: WRENCH.into(),
        template: "minecraft:stick".into(),
        max_stack_size: Some(1),
        ..Item::default()
    });
    // The crusher's dusts: plain items (sugar has no use of its own); the content mod's
    // smelting recipes turn each into an ingot.
    for id in [DUST_IRON, DUST_COPPER, DUST_GOLD] {
        items.push(Item {
            id: id.into(),
            template: "minecraft:sugar".into(),
            ..Item::default()
        });
    }
    // The monorail's higher tiers place the same block (the tier is set as it is placed).
    for id in &MONORAIL_ITEMS[1..] {
        items.push(Item {
            id: (*id).into(),
            template: "minecraft:iron_ingot".into(),
            place_block: Some(MONORAIL.into()),
            ..Item::default()
        });
    }
    // Slice 7: the chassis and the tools, one to a stack; the modules and upgrades, plain items.
    for id in std::iter::once(CHASSIS).chain(rustcraft_rules::worker::Tool::ALL.map(|t| t.item())) {
        items.push(Item {
            id: id.into(),
            template: "minecraft:iron_ingot".into(),
            max_stack_size: Some(1),
            ..Item::default()
        });
    }
    for (_, id) in rustcraft_rules::modules::ITEMS {
        items.push(Item {
            id: id.into(),
            template: "minecraft:iron_ingot".into(),
            ..Item::default()
        });
    }
    // The alloys and the capacity module: plain items, as an iron ingot is.
    for id in [
        COPPERSTEEL,
        SPARKSTEEL,
        CRYSTALLITE,
        STARSTEEL,
        MODULE_CAPACITY,
    ] {
        items.push(Item {
            id: id.into(),
            template: "minecraft:iron_ingot".into(),
            ..Item::default()
        });
    }
}

/// `native_block_entity`: the housing keeps its slots and its joints' settings in a block entity
/// and each machine its items, progress and work mode; the node and the crystal have an empty one only so that
/// `native_level_block_entities` lists them when the server rebuilds the magistral
/// (rustcraft_server::scan). Any template may have one since API 129
/// (SRC/crates/horizon-client/src/game/native_hooks.rs:3292-3306); a mod block's entity is made
/// by this hook, not by its template's (`new_block_entity`, native_hooks.rs:11641-11665).
pub fn block_entity(block: &str, entity: &mut Option<(String, Compound)>) {
    let ours = [HOUSING, NODE, CRYSTAL, CELL, TERMINAL, TANK, MONORAIL].contains(&block)
        || Machine::of(block).is_some();
    if entity.is_none() && ours {
        *entity = Some((block.to_owned(), Compound::new()));
    }
}

/// `native_block_entity_container` (native_hooks.rs:3349-3364): each machine's slots and its own
/// menu type. Every face reaches its inputs and its products; the net and hoppers put only into
/// the inputs and take only from the products (`inputs`, `outputs`; the item helpers follow
/// them, SRC/crates/horizon-client/src/game/machines/native_items.rs:16-19 and 204), and
/// `native_container_accepts` (rustcraft_machines::accepts) says which input takes what. The
/// engine saves the slots with the block entity and drops them when the block breaks
/// (docs/native-mods.md:224).
pub fn block_entity_container(block: &str, container: &mut Option<ModContainer>) {
    if container.is_none() && [TERMINAL, TANK, STATION].contains(&block) {
        *container = Some(hidden_container(block));
        return;
    }
    let Some(machine) = Machine::of(block) else {
        return;
    };
    if machine.slots() == 0 {
        return;
    }
    if container.is_none() {
        let reached: Vec<usize> = machine
            .inputs()
            .into_iter()
            .chain(machine.outputs())
            .collect();
        *container = Some(ModContainer {
            slots: machine.slots(),
            faces: std::array::from_fn(|_| reached.clone()),
            inputs: machine.inputs(),
            outputs: machine.outputs(),
            menu: machine.has_menu().then(|| machine.menu().into()),
        });
    }
}

/// The container of a block whose window has no slot: one slot no face reaches, and its menu type,
/// named after the block.
fn hidden_container(block: &str) -> ModContainer {
    ModContainer {
        slots: HIDDEN_SLOTS,
        faces: std::array::from_fn(|_| Vec::new()),
        inputs: Vec::new(),
        outputs: Vec::new(),
        menu: Some(block.into()),
    }
}

// ---- Items as numbers, for the windows' data slots ----

/// Item ids by name, both sides alike: the client and the server hold the same items (a client
/// joins only with the server's mods), and the base resources' table is the one both start with
/// (`Resources::items`, SRC/crates/horizon-client/src/game/resource_tables.rs:14-16).
struct Numbers {
    names: Vec<Arc<str>>,
    numbers: HashMap<Arc<str>, u32>,
}

static NUMBERS: OnceLock<Numbers> = OnceLock::new();

fn numbers(game: &Game<'_>) -> &'static Numbers {
    NUMBERS.get_or_init(|| {
        let mut names: Vec<Arc<str>> = game.res.items().items.keys().cloned().collect();
        names.sort();
        let numbers = names
            .iter()
            .enumerate()
            .map(|(i, n)| (n.clone(), i as u32))
            .collect();
        Numbers { names, numbers }
    })
}

/// The number a window's data slot carries for `item`.
pub(super) fn item_number(game: &Game<'_>, item: &str) -> Option<u32> {
    numbers(game).numbers.get(item).copied()
}

/// The item a window's data slot names.
pub(super) fn item_of_number(game: &Game<'_>, number: u32) -> Option<&'static str> {
    numbers(game)
        .names
        .get(usize::try_from(number).ok()?)
        .map(|n| &**n)
}
