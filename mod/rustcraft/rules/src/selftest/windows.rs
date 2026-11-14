//! Slice 9: the drawn windows of the slice-7 and slice-8 blocks, checked without a client. Each
//! scenario reads the sheet the server would send a player with the window open (what the window
//! draws) and presses the window's parts as the client sends them (menu buttons with the cursor):
//! a module socket takes a module from the cursor and gives it back as a slot does, refuses what
//! its block does not take, the miner's mode and ore toggles change what it does, and the turret's
//! sheet shows its role, energy and diagnosis.

use super::{Bench, Build, Scenario, at, plot_area, same};
use crate::geom::Pos;
use crate::machines::processor::WorkMode;
use crate::machines::sheet::{Press, SOCKETS, Sheet};
use crate::modules::Module;
use crate::tuning::Tuning;

pub const ALL: [Build; 4] = [
    turret_sheet,
    socket_takes_and_gives,
    miner_mode_and_ores,
    battery_sheet,
];

const TURRET: &str = "rustcraft:turret";
const ANCHOR: &str = "rustcraft:anchor";
const MINER: &str = "rustcraft:miner";
const BATTERY: &str = "rustcraft:battery";
const CRYSTAL: &str = "rustcraft:crystal";
/// The windows' scenarios' owner.
const OWNER: u128 = 0x5e1f_7e57_0000_0000_0000_0000_0000_00c1;
/// The server builds a sheet from what the block showed on its last tick.
const LIMIT: u64 = 60;

fn sheet(b: &dyn Bench, pos: Pos) -> Result<Sheet, String> {
    b.sheet(pos)
        .ok_or_else(|| format!("no window sheet at {pos:?}"))
}

fn module_in(s: &Sheet, k: usize) -> Option<&str> {
    s.modules.get(k).and_then(|m| m.as_deref())
}

fn turret_sheet(o: Pos, t: &Tuning) -> Scenario {
    let turret = at(o, [4, 2, 4]);
    let buffer = t.turret_buffer;
    Scenario::new("window_turret_shows_role_energy_and_state", plot_area(o))
        .then(
            "a turret with a poison module and half its buffer",
            move |b, _| {
                b.place(turret, TURRET, &[])?;
                b.set_owner(turret, OWNER, 1)?;
                b.set_modules(turret, &[Module::Poison.item()])?;
                b.set_buffer(turret, buffer / 2.0)
            },
        )
        .until(
            "its sheet: attack, the module, the energy bar, a state",
            LIMIT,
            move |b, _| {
                let s = sheet(b, turret)?;
                Ok(s.status.is_some()
                    && s.support == Some(false)
                    && module_in(&s, 0) == Some(Module::Poison.item())
                    && s.modules.len() == SOCKETS
                    && s.rows.len() >= 2
                    && s.bars
                        .first()
                        .is_some_and(|bar| bar.energy && bar.share > 0.0))
            },
        )
        .then("a healing module instead", move |b, _| {
            b.set_modules(turret, &[Module::Heal.item()])
        })
        .until("its sheet says support", LIMIT, move |b, _| {
            Ok(sheet(b, turret)?.support == Some(true))
        })
}

fn socket_takes_and_gives(o: Pos, _: &Tuning) -> Scenario {
    let anchor = at(o, [4, 2, 4]);
    let socket = |k| Press::Socket(k).id();
    Scenario::new("window_socket_takes_and_gives_a_module", plot_area(o))
        .then("an anchor", move |b, _| {
            b.place(anchor, ANCHOR, &[])?;
            b.set_owner(anchor, OWNER, 1)
        })
        .then("two radius modules pressed on socket 2", move |b, _| {
            let left = b.press_cursor(anchor, socket(1), Some((Module::Radius.item(), 2)))?;
            same(
                "the cursor after",
                left,
                Some((Module::Radius.item().to_owned(), 1)),
            )
        })
        .check("the module is in socket 2", move |b, _| {
            same(
                "socket 2",
                module_in(&sheet(b, anchor)?, 1),
                Some(Module::Radius.item()),
            )
        })
        .then("a turret's module is refused", move |b, _| {
            let left = b.press_cursor(anchor, socket(0), Some((Module::Poison.item(), 1)))?;
            same(
                "the cursor after",
                left,
                Some((Module::Poison.item().to_owned(), 1)),
            )?;
            same("socket 1", module_in(&sheet(b, anchor)?, 0), None)
        })
        .then("an empty cursor takes it back", move |b, _| {
            let left = b.press_cursor(anchor, socket(1), None)?;
            same(
                "the cursor after",
                left,
                Some((Module::Radius.item().to_owned(), 1)),
            )?;
            same("socket 2", module_in(&sheet(b, anchor)?, 1), None)
        })
}

fn miner_mode_and_ores(o: Pos, _: &Tuning) -> Scenario {
    // No energy: the miner digs nothing under the plot while its window is pressed.
    let miner = at(o, [4, 2, 4]);
    Scenario::new("window_miner_mode_and_ore_toggles", plot_area(o))
        .then("a miner without energy", move |b, _| {
            b.place(miner, MINER, &[])?;
            b.set_owner(miner, OWNER, 1)
        })
        .until(
            "its sheet: nine ores taken, nine slots of 27",
            LIMIT,
            move |b, _| {
                let s = sheet(b, miner)?;
                Ok(s.toggles.len() == 9 && s.toggles.iter().all(|t| t.on) && s.usable == Some(9))
            },
        )
        .then("Boost and coal left in the ground", move |b, _| {
            b.press_cursor(miner, Press::Mode(WorkMode::Boost).id(), None)?;
            b.press_cursor(miner, Press::Toggle(0).id(), None)?;
            Ok(())
        })
        .check("the sheet shows both", move |b, _| {
            let s = sheet(b, miner)?;
            same("mode", s.mode, Some(WorkMode::Boost.code()))?;
            same("coal taken", s.toggles.first().map(|t| t.on), Some(false))?;
            same("copper taken", s.toggles.get(1).map(|t| t.on), Some(true))
        })
        .then("a storage module from the cursor", move |b, _| {
            b.press_cursor(
                miner,
                Press::Socket(0).id(),
                Some((Module::Storage.item(), 1)),
            )?;
            Ok(())
        })
        .check("18 slots now", move |b, _| {
            same("usable slots", sheet(b, miner)?.usable, Some(18))
        })
}

fn battery_sheet(o: Pos, _: &Tuning) -> Scenario {
    let battery = at(o, [4, 2, 4]);
    Scenario::new("window_battery_shows_its_store", plot_area(o))
        .then("a battery beside a crystal", move |b, _| {
            b.place(battery, BATTERY, &[])?;
            b.set_owner(battery, OWNER, 1)?;
            b.place(at(battery, [1, 0, 0]), CRYSTAL, &[])
        })
        .until(
            "its sheet: an energy bar, three rows, a state",
            LIMIT,
            move |b, _| {
                let s = sheet(b, battery)?;
                Ok(s.status.is_some()
                    && s.rows.len() == 3
                    && s.modules.len() == SOCKETS
                    && s.bars.first().is_some_and(|bar| bar.energy))
            },
        )
}
