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

/// Wave 2 (#15, ui.js `armor` «Ёмкость костюма: батарея костюма +50%»): a module's short name in
/// its window line and what it gives, by their keys and Russian fallbacks (the guide's words).
pub fn named(m: Module) -> ((&'static str, &'static str), (&'static str, &'static str)) {
    match m {
        Module::Dash => (("rustcraft.gear.window.name.dash", "Рывок"), ("rustcraft.gear.window.does.dash", "бросок на 4 блока по двойному нажатию")),
        Module::Run => (("rustcraft.gear.window.name.run", "Бег"), ("rustcraft.gear.window.does.run", "бег +50% на 5 с")),
        Module::Jump => (("rustcraft.gear.window.name.jump", "Прыжок"), ("rustcraft.gear.window.does.jump", "прыжок до 2,5 блока")),
        Module::Absorber => (("rustcraft.gear.window.name.absorber", "Амортизатор"), ("rustcraft.gear.window.does.absorber", "гасит урон от падения")),
        Module::Capacity => (("rustcraft.gear.window.name.capacity", "Ёмкость костюма"), ("rustcraft.gear.window.does.capacity", "батарея костюма +50%")),
        Module::Night => (("rustcraft.gear.window.name.night", "Ночное зрение"), ("rustcraft.gear.window.does.night", "видно в темноте")),
        Module::Magnet => (("rustcraft.gear.window.name.magnet", "Магнит"), ("rustcraft.gear.window.does.magnet", "притягивает предметы")),
        Module::Damper => (("rustcraft.gear.window.name.damper", "Демпфер"), ("rustcraft.gear.window.does.damper", "после удара 5 с урон −20%")),
        Module::Jetpack => (("rustcraft.gear.window.name.jetpack", "Джетпак"), ("rustcraft.gear.window.does.jetpack", "полёт за заряд")),
    }
}

#[cfg(test)]
mod wave2_tests {
    use super::*;

    #[test]
    fn every_module_line_says_what_it_gives() {
        assert_eq!(named(Module::Capacity), (("rustcraft.gear.window.name.capacity", "Ёмкость костюма"), ("rustcraft.gear.window.does.capacity", "батарея костюма +50%")));
        let mut keys: Vec<&str> = Module::ALL.iter().flat_map(|m| { let (a, b) = named(*m); [a.0, b.0] }).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), 2 * Module::ALL.len(), "a key each");
    }
}

#[cfg(test)]
mod panel_tests {
    use super::*;

    #[test]
    fn the_charge_panel_stays_on_the_screen() {
        // A 176-wide menu centred on a 427-wide GUI (1280 × 720 at scale 3): 121.5 pixels on
        // the right, so the panel narrows to that (the in-game check found it cut off).
        let menu = [125.5, 40.0, 176.0, 166.0];
        assert_eq!(panel(menu, 168.0, 4.0), (305.5, 121.5));
        // A wide screen: the whole panel on the right, as before.
        let wide = [372.0, 40.0, 176.0, 166.0];
        assert_eq!(panel(wide, 168.0, 4.0), (552.0, 168.0));
        // Too little room beside the menu: the whole panel against the screen's right edge.
        let narrow = [40.0, 40.0, 176.0, 166.0];
        assert_eq!(panel(narrow, 168.0, 4.0), (256.0 - 168.0, 168.0));
        for r in [menu, wide, narrow] {
            let (x, w) = panel(r, 168.0, 4.0);
            assert!(x >= 0.0 && x + w <= r[0] * 2.0 + r[2]);
        }
    }
}

/// Where the gear window's charge panel goes and how wide it is, in GUI pixels: right of the menu
/// `rect` (`[x, y, w, h]`, centred on the screen, so the screen is `2x + w` wide), narrowed to the
/// room there when the whole panel does not fit, else against the screen's right edge. It never
/// runs off the screen.
pub fn panel(rect: [f32; 4], width: f32, gap: f32) -> (f32, f32) {
    // The narrowest panel that still reads: the charge line and a module line cut with «…».
    const NARROWEST: f32 = 96.0;
    let screen = rect[0] * 2.0 + rect[2];
    let right = rect[0] + rect[2] + gap;
    let room = screen - right;
    if room >= width {
        (right, width)
    } else if room >= NARROWEST {
        (right, room)
    } else {
        ((screen - width).max(0.0), width.min(screen))
    }
}
