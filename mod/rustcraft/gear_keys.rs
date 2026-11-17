// Slice 18: the gear's keys (rustcraft_rules::gear::keys): Z the magnet, ] the backpack, ; night
// vision, each a mod key players rebind in Options › Mods.
//
// - `native_mods` lists RustCraft with its keys (name, label, default); Options › Mods shows them
//   as Key Binds rows and keeps the player's choice, which `native_mod_key` reads
//   (SRC crates/horizon-client/src/mod_api.rs:414-425, game/native_hooks.rs:3274 and 11252,
//   horizon-client-render/src/options_ui.rs:2624-2646).
// - `native_key` on the client: a press of one of them while playing (no screen over the world)
//   goes to the server as one byte on `rustcraft:gear_key` (API 40). A window that is its own
//   server (single player) has no connection: the press is queued for its own player.
// - The server's gear tick takes the presses (rustcraft_gear::keys_due).
use crate::game::Game;
use horizon_client::mod_api::NativeMod;
use rustcraft_rules::gear::keys::{CHANNEL, GearKey};
use std::sync::{Mutex, PoisonError};

const MOD_ID: &str = "rustcraft";

/// Presses the server has not taken yet: the player and the key.
static DUE: Mutex<Vec<(u128, GearKey)>> = Mutex::new(Vec::new());

/// `native_mods`: RustCraft's keys on its Options › Mods page.
pub(super) fn mods(mods: &mut Vec<NativeMod>) {
    let keys: Vec<[String; 3]> = GearKey::ALL
        .into_iter()
        .map(|k| [k.name().to_owned(), k.label().to_owned(), k.default_key().to_owned()])
        .collect();
    if let Some(m) = mods.iter_mut().find(|m| m.id == MOD_ID) {
        for k in keys {
            if !m.keys.iter().any(|have| have[0] == k[0]) {
                m.keys.push(k);
            }
        }
        return;
    }
    mods.push(NativeMod {
        id: MOD_ID.to_owned(),
        title: "RustCraft".to_owned(),
        description: "Магистраль, машины, база и снаряжение / The magistral, machines, base and gear".to_owned(),
        keys,
    });
}

/// `native_key` on a client: one of the gear's keys pressed while playing.
pub(super) fn on_key(game: &mut Game<'_>, key: &str, pressed: bool) {
    if !pressed || !game.native_playing() {
        return;
    }
    let Some(k) = GearKey::ALL.into_iter().find(|k| game.native_mod_key(MOD_ID, k.name(), k.default_key()) == key) else {
        return;
    };
    if game.remote.is_some() {
        if let Err(e) = game.native_net_send_to_server(CHANNEL, &[k.byte()]) {
            log::warn!("rustcraft: the {} key was not sent: {e}", k.name());
        }
    } else if let Some(p) = game.native_acting_player() {
        DUE.lock().unwrap_or_else(PoisonError::into_inner).push((p.uuid, k));
    }
}

/// `native_net_from_client`: a player's key press.
pub(super) fn on_client_message(player: u128, channel: &str, data: &[u8]) {
    if channel != CHANNEL {
        return;
    }
    if let [b] = data
        && let Some(k) = GearKey::from_byte(*b)
    {
        DUE.lock().unwrap_or_else(PoisonError::into_inner).push((player, k));
    }
}

/// The presses waiting for the server, taken.
pub(super) fn take() -> Vec<(u128, GearKey)> {
    std::mem::take(&mut *DUE.lock().unwrap_or_else(PoisonError::into_inner))
}
