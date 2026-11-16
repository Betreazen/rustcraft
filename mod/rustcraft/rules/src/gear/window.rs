//! Slice 18: what a gear piece's window shows beside its sockets (guide `armor`, ui.js: the charge
//! bar «Заряд 11 000 из 20 000 Е» and a line a module, «Магнит: включён · клавиша Z»). The server
//! sends it to the player when the window opens; the client draws it over the menu.

use serde::{Deserialize, Serialize};

use super::{Gear, Held, Module, charger_capacity, keys};

/// The channel the server sends a window's view on.
pub const CHANNEL: &str = "rustcraft:gear_window";
/// The window's title key: how the client knows the menu.
pub const TITLE_KEY: &str = "rustcraft.gear.window";

/// A module's line: switched by its key and on or off, or working by itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum State {
    Auto,
    On,
    Off,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct View {
    pub charge: f64,
    pub capacity: f64,
    /// The modules in its sockets, by item id.
    pub modules: Vec<(String, State)>,
    /// Its technical enchantments' levels.
    pub capacity_level: u8,
    pub thrift_level: u8,
}

/// The view of a piece `g` holding `h`.
pub fn view(g: Gear, h: &Held) -> View {
    let capacity = charger_capacity(g, h).unwrap_or_else(|| g.base_capacity());
    let modules = h
        .modules
        .iter()
        .filter_map(|item| {
            let m = Module::of(item)?;
            let state = if !keys::toggled(m) {
                State::Auto
            } else if keys::works(h, m) {
                State::On
            } else {
                State::Off
            };
            Some((item.clone(), state))
        })
        .collect();
    View { charge: h.charge.min(capacity), capacity, modules, capacity_level: h.capacity, thrift_level: h.thrift }
}

/// The key that switches `m`, if one does.
pub fn key_of(m: Module) -> Option<keys::GearKey> {
    keys::GearKey::ALL.into_iter().find(|k| k.module() == Some(m))
}

/// A key's label from vanilla's name: `key.keyboard.z` → `Z`, `key.keyboard.semicolon` → `;`.
pub fn key_label(key: &str) -> String {
    let last = key.rsplit('.').next().unwrap_or(key);
    match last {
        "semicolon" => ";".to_owned(),
        "comma" => ",".to_owned(),
        "period" => ".".to_owned(),
        "apostrophe" => "'".to_owned(),
        "slash" => "/".to_owned(),
        "unknown" => "—".to_owned(),
        other => other.to_uppercase(),
    }
}

/// A whole number of energy with its thousands apart, as the guide writes them: «11 000».
pub fn grouped(n: f64) -> String {
    let digits = (n.max(0.0).round() as u64).to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push('\u{a0}');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gear::{Kind, Piece};

    #[test]
    fn the_window_shows_the_charge_and_each_modules_state() {
        let chest = Gear { kind: Kind::Armor(Piece::Chestplate), tier: 3 };
        let mut h = Held {
            charge: 11_000.0,
            modules: vec![Module::Magnet.item().to_owned(), Module::Damper.item().to_owned()],
            ..Held::default()
        };
        let v = view(chest, &h);
        assert_eq!((v.charge, v.capacity), (11_000.0, 20_000.0));
        assert_eq!(v.modules[0], (Module::Magnet.item().to_owned(), State::Off));
        assert_eq!(v.modules[1].1, State::Auto);
        keys::toggle(&mut h, Module::Magnet);
        assert_eq!(view(chest, &h).modules[0].1, State::On);
        assert_eq!(key_of(Module::Magnet).map(|k| k.default_key()), Some("key.keyboard.z"));
        assert_eq!(key_of(Module::Run), None);
    }

    #[test]
    fn keys_are_labelled_as_the_keyboard_shows_them() {
        assert_eq!(key_label("key.keyboard.z"), "Z");
        assert_eq!(key_label("key.keyboard.semicolon"), ";");
        assert_eq!(key_label("key.keyboard.f9"), "F9");
    }

    #[test]
    fn energy_is_written_in_thousands() {
        assert_eq!(grouped(11_000.0), "11\u{a0}000");
        assert_eq!(grouped(800.0), "800");
        assert_eq!(grouped(1_234_567.4), "1\u{a0}234\u{a0}567");
        assert_eq!(grouped(-5.0), "0");
    }
}
