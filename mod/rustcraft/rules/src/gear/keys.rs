//! Slice 18: the gear's keys (owner's decision 2026-11-16: Z magnet, ; night vision;
//! M is the world map's and betterf3's; 2026-11-17: ] the backpack, since B is Horizon's emote
//! wheel, `key.emote`, SRC crates/horizon-client-input/src/keybinds.rs:82). Each is a mod key players rebind in Options › Mods
//! (`native_mods`, `native_mod_key`); a press while playing goes to the server as one byte.

use super::{Held, Module};

/// The channel a key press goes to the server on.
pub const CHANNEL: &str = "rustcraft:gear_key";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GearKey {
    Magnet,
    Backpack,
    Night,
}

impl GearKey {
    pub const ALL: [GearKey; 3] = [GearKey::Magnet, GearKey::Backpack, GearKey::Night];

    /// Its name among the mod's keys (`key.rustcraft.<name>` in the options).
    pub fn name(self) -> &'static str {
        match self {
            GearKey::Magnet => "magnet",
            GearKey::Backpack => "backpack",
            GearKey::Night => "night_vision",
        }
    }

    /// Its default key, by vanilla's name.
    pub fn default_key(self) -> &'static str {
        match self {
            GearKey::Magnet => "key.keyboard.z",
            GearKey::Backpack => "key.keyboard.right.bracket",
            GearKey::Night => "key.keyboard.semicolon",
        }
    }

    /// Its label in the options.
    pub fn label(self) -> &'static str {
        match self {
            GearKey::Magnet => "Магнит / Magnet",
            GearKey::Backpack => "Рюкзак / Backpack",
            GearKey::Night => "Ночное зрение / Night vision",
        }
    }

    pub fn byte(self) -> u8 {
        match self {
            GearKey::Magnet => 1,
            GearKey::Backpack => 2,
            GearKey::Night => 3,
        }
    }

    pub fn from_byte(b: u8) -> Option<GearKey> {
        GearKey::ALL.into_iter().find(|k| k.byte() == b)
    }

    /// The module a key switches, if it switches one.
    pub fn module(self) -> Option<Module> {
        match self {
            GearKey::Magnet => Some(Module::Magnet),
            GearKey::Night => Some(Module::Night),
            GearKey::Backpack => None,
        }
    }
}

/// Whether `m` is switched by a key (guide `pmodules`: «Ночное зрение и магнит включаются клавишей
/// и тратят заряд, пока работают»); the others work by themselves on their event.
pub fn toggled(m: Module) -> bool {
    matches!(m, Module::Night | Module::Magnet)
}

/// Whether module `m` of a piece holding `h` is on: a keyed one when switched on.
pub fn works(h: &Held, m: Module) -> bool {
    h.has(m) && (!toggled(m) || h.on.iter().any(|i| i == m.item()))
}

/// Switches keyed module `m` of a piece: its new state; false for a module not in the piece.
pub fn toggle(h: &mut Held, m: Module) -> bool {
    if !h.has(m) || !toggled(m) {
        return false;
    }
    if let Some(i) = h.on.iter().position(|i| i == m.item()) {
        h.on.remove(i);
        false
    } else {
        h.on.push(m.item().to_owned());
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(m: Module) -> Held {
        Held { modules: vec![m.item().to_owned()], ..Held::default() }
    }

    #[test]
    fn keyed_modules_wait_for_their_key() {
        let mut h = with(Module::Night);
        assert!(!works(&h, Module::Night), "night vision starts off");
        assert!(toggle(&mut h, Module::Night));
        assert!(works(&h, Module::Night));
        assert!(!toggle(&mut h, Module::Night));
        assert!(!works(&h, Module::Night));
        let run = with(Module::Run);
        assert!(works(&run, Module::Run), "the run module works by itself");
        let mut empty = Held::default();
        assert!(!toggle(&mut empty, Module::Magnet), "no magnet, nothing to switch");
        assert!(empty.on.is_empty());
    }

    #[test]
    fn keys_have_their_defaults_and_bytes() {
        assert_eq!(GearKey::Magnet.default_key(), "key.keyboard.z");
        assert_eq!(GearKey::Backpack.default_key(), "key.keyboard.right.bracket");
        assert_eq!(GearKey::Night.default_key(), "key.keyboard.semicolon");
        for k in GearKey::ALL {
            assert_eq!(GearKey::from_byte(k.byte()), Some(k));
        }
        assert_eq!(GearKey::from_byte(0), None);
        assert_eq!(GearKey::Night.module(), Some(Module::Night));
    }
}
