//! Slice 13: the player's gear (spec 4.11 G1–G7, 4.12 K3, 20 W1–W8, R1–R3, 26 G6–G7). Each piece
//! has a battery instead of durability. What a piece holds — its charge, its player modules, its
//! technical enchantments — is kept by the server under the piece's number, as the backpacks'
//! contents are; the piece carries only the number.
//!
//! Numbers not in the spec are proposals for the playtest.

use serde::{Deserialize, Serialize};

pub mod shot;

/// A piece of technical armour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Piece {
    Helmet,
    Chestplate,
    Leggings,
    Boots,
}

impl Piece {
    pub const ALL: [Piece; 4] = [Piece::Helmet, Piece::Chestplate, Piece::Leggings, Piece::Boots];

    pub fn name(self) -> &'static str {
        match self {
            Piece::Helmet => "helmet",
            Piece::Chestplate => "chestplate",
            Piece::Leggings => "leggings",
            Piece::Boots => "boots",
        }
    }
}

/// What a gear item is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Armor(Piece),
    /// W: the energy blade.
    Blade,
    /// W: the energy shield.
    Shield,
    /// R: the energy rifle, tier 4 only.
    Rifle,
}

/// A gear item: its kind and tier (2 sparksteel, 3 crystallite, 4 starsteel; G7).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Gear {
    pub kind: Kind,
    pub tier: u8,
}

pub const TIERS: [u8; 3] = [2, 3, 4];

impl Gear {
    /// Every gear item of the mod.
    pub fn all() -> Vec<Gear> {
        let mut out = Vec::new();
        for tier in TIERS {
            for p in Piece::ALL {
                out.push(Gear { kind: Kind::Armor(p), tier });
            }
            out.push(Gear { kind: Kind::Blade, tier });
            out.push(Gear { kind: Kind::Shield, tier });
        }
        out.push(Gear { kind: Kind::Rifle, tier: 4 });
        out
    }

    pub fn item(self) -> String {
        match self.kind {
            Kind::Armor(p) => format!("rustcraft:tech_{}_{}", p.name(), self.tier),
            Kind::Blade => format!("rustcraft:blade_{}", self.tier),
            Kind::Shield => format!("rustcraft:eshield_{}", self.tier),
            Kind::Rifle => "rustcraft:rifle".to_owned(),
        }
    }

    pub fn of(item: &str) -> Option<Gear> {
        Gear::all().into_iter().find(|g| g.item() == item)
    }

    /// Its battery before enchantments and the suit capacity module, E (G7, W, R3).
    pub fn base_capacity(self) -> f64 {
        let i = usize::from(self.tier.clamp(2, 4) - 2);
        match self.kind {
            Kind::Armor(_) => [8_000.0, 20_000.0, 50_000.0][i],
            Kind::Blade => [10_000.0, 20_000.0, 40_000.0][i],
            Kind::Shield => [6_000.0, 12_000.0, 24_000.0][i],
            Kind::Rifle => 40_000.0,
        }
    }

    /// Player-module sockets (G7: 1 on sparksteel armour, 2 above; weapons none, W4).
    pub fn sockets(self) -> usize {
        match self.kind {
            Kind::Armor(_) if self.tier <= 2 => 1,
            Kind::Armor(_) => 2,
            _ => 0,
        }
    }

    /// The vanilla item it copies for its look in hand and its protection or damage: iron,
    /// diamond and netherite armour by tier; diamond and netherite swords (the fourth tier's ninth
    /// point of damage is the blade's own, `blade_bonus`); the shield.
    pub fn template(self) -> String {
        let metal = ["iron", "diamond", "netherite"][usize::from(self.tier.clamp(2, 4) - 2)];
        match self.kind {
            Kind::Armor(p) => format!("minecraft:{metal}_{}", p.name()),
            Kind::Blade if self.tier <= 2 => "minecraft:diamond_sword".to_owned(),
            Kind::Blade => "minecraft:netherite_sword".to_owned(),
            Kind::Shield => "minecraft:shield".to_owned(),
            Kind::Rifle => "minecraft:stick".to_owned(),
        }
    }

    /// The equipment slot armour goes to.
    pub fn slot_name(self) -> Option<&'static str> {
        match self.kind {
            Kind::Armor(Piece::Helmet) => Some("head"),
            Kind::Armor(Piece::Chestplate) => Some("chest"),
            Kind::Armor(Piece::Leggings) => Some("legs"),
            Kind::Armor(Piece::Boots) => Some("feet"),
            _ => None,
        }
    }
}

/// A player module (spec 4.11, guide `pmodules`, `jetpack`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Module {
    Dash,
    Run,
    Jump,
    Absorber,
    Capacity,
    Night,
    Magnet,
    Damper,
    Jetpack,
}

impl Module {
    pub const ALL: [Module; 9] = [
        Module::Dash,
        Module::Run,
        Module::Jump,
        Module::Absorber,
        Module::Capacity,
        Module::Night,
        Module::Magnet,
        Module::Damper,
        Module::Jetpack,
    ];

    pub fn item(self) -> &'static str {
        match self {
            Module::Dash => "rustcraft:pm_dash",
            Module::Run => "rustcraft:pm_run",
            Module::Jump => "rustcraft:pm_jump",
            Module::Absorber => "rustcraft:pm_absorber",
            Module::Capacity => "rustcraft:pm_capacity",
            Module::Night => "rustcraft:pm_night",
            Module::Magnet => "rustcraft:pm_magnet",
            Module::Damper => "rustcraft:pm_damper",
            Module::Jetpack => "rustcraft:jetpack",
        }
    }

    pub fn of(item: &str) -> Option<Module> {
        Module::ALL.into_iter().find(|m| m.item() == item)
    }

    /// The piece it goes into (guide `armor`).
    pub fn piece(self) -> Piece {
        match self {
            Module::Night => Piece::Helmet,
            Module::Capacity | Module::Magnet | Module::Damper | Module::Jetpack => Piece::Chestplate,
            Module::Dash | Module::Run => Piece::Leggings,
            Module::Jump | Module::Absorber => Piece::Boots,
        }
    }

    /// E a second while it works (night vision on, magnet on, running boosted, jumping boosted,
    /// flying); the event modules pay per event instead.
    pub fn drain(self) -> f64 {
        match self {
            Module::Night => 2.0,
            Module::Magnet => 4.0,
            Module::Run => 6.0,
            Module::Jump => 1.0,
            Module::Jetpack => 30.0,
            Module::Dash | Module::Absorber | Module::Capacity | Module::Damper => 0.0,
        }
    }
}

/// A dash costs this, E.
pub const DASH_COST: f64 = 200.0;
/// The shock absorber pays this per point of fall damage it takes, E.
pub const ABSORB_PER_DAMAGE: f64 = 25.0;
/// The damper: 5 s after damage, −20 % damage, this much a hit it softens.
pub const DAMPER_TICKS: u64 = 100;
pub const DAMPER_SHARE: f32 = 0.8;
pub const DAMPER_COST: f64 = 40.0;
/// The magnet's reach, blocks.
pub const MAGNET_REACH: f64 = 6.0;
/// W7: a hit with the blade, E; W2: the blade without charge strikes as a hand.
pub const BLADE_HIT: f64 = 25.0;
pub const HAND_DAMAGE: f32 = 1.0;
/// W8: the shield pays 10 E a point of damage it blocks, at least 10 E a block.
pub const SHIELD_PER_DAMAGE: f64 = 10.0;
/// R1–R3: the rifle.
pub const RIFLE_DAMAGE: f32 = 16.0;
pub const RIFLE_RANGE: f64 = 64.0;
pub const RIFLE_COOLDOWN_TICKS: u64 = 25;
pub const RIFLE_SHOT: f64 = 400.0;

/// The fourth tier's blade strikes one point harder than the netherite sword it copies.
pub fn blade_bonus(tier: u8) -> f32 {
    if tier >= 4 { 1.0 } else { 0.0 }
}

/// The technical enchantments' books (4.12 K3, round 14: made on the crafting table, levels by
/// how many go into a piece's window, up to III).
pub const BOOK_CAPACITY: &str = "rustcraft:book_capacity";
pub const BOOK_THRIFT: &str = "rustcraft:book_thrift";
pub const MAX_LEVEL: u8 = 3;

/// What the server keeps for one piece.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Held {
    pub charge: f64,
    /// Its modules' item ids, one a socket.
    pub modules: Vec<String>,
    pub capacity: u8,
    pub thrift: u8,
}

impl Held {
    pub fn has(&self, m: Module) -> bool {
        self.modules.iter().any(|i| i == m.item())
    }
}

/// The battery of `g` holding `h`: +25 % a level of Capacity, +50 % when the suit's chestplate
/// holds the suit capacity module (G6, applied to every armour piece).
pub fn capacity(g: Gear, h: &Held, suit_capacity: bool) -> f64 {
    let mut c = g.base_capacity() * (1.0 + 0.25 * f64::from(h.capacity.min(MAX_LEVEL)));
    if suit_capacity && matches!(g.kind, Kind::Armor(_)) {
        c *= 1.5;
    }
    c
}

/// A cost after Thrift: −10 % a level.
pub fn cost(base: f64, h: &Held) -> f64 {
    base * (1.0 - 0.1 * f64::from(h.thrift.min(MAX_LEVEL)))
}

/// Spends `amount` if the piece holds it; false and nothing spent otherwise.
pub fn spend(h: &mut Held, amount: f64) -> bool {
    if h.charge + 1e-9 < amount {
        return false;
    }
    h.charge = (h.charge - amount).max(0.0);
    true
}

/// The window of a piece: its module sockets, then the two book slots.
pub fn window_slots(g: Gear) -> usize {
    g.sockets() + 2
}

/// Whether `item` may stay in slot `slot` of `g`'s window: a module of its piece in a socket
/// (no two alike), a book of its kind in the book slots (up to III).
pub fn window_takes(g: Gear, slot: usize, item: &str, count: u32) -> bool {
    let sockets = g.sockets();
    if slot < sockets {
        let Kind::Armor(piece) = g.kind else {
            return false;
        };
        return Module::of(item).is_some_and(|m| m.piece() == piece) && count == 1;
    }
    match slot - sockets {
        0 => item == BOOK_CAPACITY && count <= u32::from(MAX_LEVEL),
        1 => item == BOOK_THRIFT && count <= u32::from(MAX_LEVEL),
        _ => false,
    }
}

/// The window's slots as `Held` keeps them: (modules, capacity level, thrift level).
pub fn from_window(g: Gear, slots: &[Option<(String, u32)>]) -> (Vec<String>, u8, u8) {
    let sockets = g.sockets();
    let mut modules: Vec<String> = Vec::new();
    for (i, s) in slots.iter().enumerate().take(sockets) {
        if let Some((item, n)) = s
            && window_takes(g, i, item, *n)
            && !modules.contains(item)
        {
            modules.push(item.clone());
        }
    }
    let level = |i: usize, book: &str| {
        slots
            .get(sockets + i)
            .cloned()
            .flatten()
            .filter(|(item, _)| item == book)
            .map_or(0, |(_, n)| n.min(u32::from(MAX_LEVEL)) as u8)
    };
    (modules, level(0, BOOK_CAPACITY), level(1, BOOK_THRIFT))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_item_names_itself_back() {
        for g in Gear::all() {
            assert_eq!(Gear::of(&g.item()), Some(g));
        }
        assert_eq!(Gear::all().len(), 19);
        assert_eq!(Gear::of("rustcraft:tech_boots_3").map(|g| g.sockets()), Some(2));
        assert_eq!(Gear::of("rustcraft:tech_boots_2").map(|g| g.sockets()), Some(1));
    }

    #[test]
    fn enchantments_and_the_suit_module_grow_the_battery() {
        let chest = Gear { kind: Kind::Armor(Piece::Chestplate), tier: 3 };
        let mut h = Held::default();
        assert_eq!(capacity(chest, &h, false), 20_000.0);
        h.capacity = 2;
        assert_eq!(capacity(chest, &h, true), 20_000.0 * 1.5 * 1.5);
        let blade = Gear { kind: Kind::Blade, tier: 4 };
        assert_eq!(capacity(blade, &h, true), 40_000.0 * 1.5, "the suit module is armour's only");
        h.thrift = 3;
        assert!((cost(BLADE_HIT, &h) - 17.5).abs() < 1e-9);
    }

    #[test]
    fn a_window_keeps_modules_of_its_piece_and_books() {
        let boots = Gear { kind: Kind::Armor(Piece::Boots), tier: 3 };
        assert_eq!(window_slots(boots), 4);
        let slots = vec![
            Some(("rustcraft:pm_jump".to_owned(), 1)),
            Some(("rustcraft:pm_night".to_owned(), 1)),
            Some((BOOK_CAPACITY.to_owned(), 2)),
            Some((BOOK_THRIFT.to_owned(), 5)),
        ];
        let (modules, cap, thrift) = from_window(boots, &slots);
        assert_eq!(modules, vec!["rustcraft:pm_jump".to_owned()], "night vision is the helmet's");
        assert_eq!((cap, thrift), (2, 3), "more than three books give level III");
        let blade = Gear { kind: Kind::Blade, tier: 2 };
        assert_eq!(window_slots(blade), 2);
        assert!(!window_takes(blade, 0, "rustcraft:pm_run", 1));
    }

    #[test]
    fn spending_needs_the_charge() {
        let mut h = Held { charge: 30.0, ..Held::default() };
        assert!(spend(&mut h, 25.0));
        assert!(!spend(&mut h, 25.0));
        assert_eq!(h.charge, 5.0);
        assert_eq!(blade_bonus(4), 1.0);
        assert_eq!(Module::Jetpack.piece(), Piece::Chestplate);
    }
}
