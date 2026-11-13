//! Slice 7: the turret (spec 4.13, 27). It shoots a hostile mob in range for its energy and never
//! a peaceful one (P1, P3); effect modules act together, poison spares the undead, slowness stops
//! at its cap (TR1, TR2, TR5); a support module makes it a support turret, which shoots nothing and
//! names an idle effect module (TR3); without energy it says so; its kill is reported (X8).
//! SoulsCraft's resistances are checked on its brute when SoulsCraft is in the build (27.1).

use super::{Bench, Build, Memo, MobView, Scenario, at, near, plot_area, same};
use crate::geom::Pos;
use crate::modules::Module;
use crate::tuning::Tuning;
use crate::turret::{self, Loadout, Shown};

pub const ALL: [Build; 10] = [
    shoots_a_hostile_mob_only,
    effects_combine,
    poison_spares_the_undead,
    slowness_stops_at_its_cap,
    support_shoots_nothing,
    no_energy,
    kill_is_reported,
    soulscraft_resistances,
    role_in_its_block_entity,
    testkit_turret_shoots,
];

/// SoulsCraft's contract: a turret's block entity names its role whenever its modules change.
fn role_in_its_block_entity(o: Pos, _: &Tuning) -> Scenario {
    let turret = at(o, [4, 1, 0]);
    Scenario::new("turret_role_in_its_block_entity", plot_area(o))
        .then("a turret with a heal module", move |b, _| {
            b.place(turret, TURRET, &[])?;
            b.set_modules(turret, &[Module::Heal.item()])
        })
        .check("role support", move |b, _| {
            same("role", b.role(turret), Some("support".to_owned()))
        })
        .then("a poison module instead", move |b, _| {
            b.set_modules(turret, &[Module::Poison.item()])
        })
        .check("role attack", move |b, _| {
            same("role", b.role(turret), Some("attack".to_owned()))
        })
}

/// `rustcraft_testkit turret`: a turret with no energy net shoots at once and keeps its buffer.
fn testkit_turret_shoots(o: Pos, t: &Tuning) -> Scenario {
    let turret = at(o, [10, 1, 0]);
    let full = t.turret_buffer;
    let shot = loadout(&[Module::Poison]).shot_energy(t);
    Scenario::new("turret_testkit_shoots_without_a_net", plot_area(o))
        .then("a floor, a test-kit turret and a spider", move |b, m| {
            for x in 6..16 {
                b.place(at(o, [x, 0, 0]), "minecraft:stone", &[])?;
            }
            let answer = b.testkit(&format!(
                "turret {} {} {} modules=poison owner={OWNER:032x}",
                turret[0], turret[1], turret[2]
            ));
            if answer != "ran true" {
                return Err(answer);
            }
            let id = b.spawn_mob(
                SPIDER,
                [
                    f64::from(turret[0]) + 4.5,
                    f64::from(turret[1]),
                    f64::from(turret[2]) + 0.5,
                ],
                None,
            )?;
            m.mark("mob0", f64::from(id));
            Ok(())
        })
        .until("it is placed with its role", 40, move |b, _| {
            Ok(b.block(turret) == TURRET && b.role(turret).as_deref() == Some("attack"))
        })
        .until("it shot the spider", 200, move |b, m| {
            let id = m.get("mob0") as u32;
            Ok(b.mob(id).is_none_or(|v| v.health < 16.0))
        })
        .wait(40)
        .check("its buffer is topped up", move |b, _| {
            // Full each tick before it shoots; a shot of this tick may have just left it.
            near("buffer", b.buffer(turret), full, shot)
        })
}

const TURRET: &str = "rustcraft:turret";
const SPIDER: &str = "minecraft:spider";
const PIG: &str = "minecraft:pig";
const HUSK: &str = "minecraft:husk";
/// A hostile mob that is neither undead nor an arthropod: poison works on it.
const PILLAGER: &str = "minecraft:pillager";
const OWNER: u128 = super::workers::OWNER;

fn mods(m: &[Module]) -> Vec<&'static str> {
    m.iter().map(|m| m.item()).collect()
}

fn loadout(m: &[Module]) -> Loadout {
    let slots: Vec<Option<Module>> = m.iter().copied().map(Some).collect();
    Loadout::of(&slots)
}

/// A floor, the turret at its middle with `modules` and a full buffer, and the mobs at their
/// offsets (kind, dx, health).
fn arena(
    name: &'static str,
    o: Pos,
    t: &Tuning,
    modules: Vec<&'static str>,
    mobs: Vec<(&'static str, i32, Option<f32>)>,
    energy: Option<f64>,
) -> Scenario {
    let turret = at(o, [10, 1, 0]);
    let full = energy.unwrap_or(t.turret_buffer);
    Scenario::new(name, plot_area(o))
        .then("a floor and the turret with its modules", move |b, _| {
            for x in 0..24 {
                for z in -3..=3 {
                    b.place(at(o, [x, 0, z]), "minecraft:stone", &[])?;
                }
            }
            b.place(turret, TURRET, &[])?;
            b.set_owner(turret, OWNER, 1)?;
            b.set_modules(turret, &modules)?;
            b.set_buffer(turret, full)
        })
        .then("the mobs", move |b, m| {
            for (i, (kind, dx, health)) in mobs.iter().enumerate() {
                let p = at(turret, [*dx, 0, 0]);
                let id = b.spawn_mob(
                    kind,
                    [
                        f64::from(p[0]) + 0.5,
                        f64::from(p[1]),
                        f64::from(p[2]) + 0.5,
                    ],
                    *health,
                )?;
                m.mark(["mob0", "mob1", "mob2"][i.min(2)], f64::from(id));
            }
            Ok(())
        })
}

fn mob(b: &dyn Bench, m: &Memo, key: &str) -> Result<MobView, String> {
    b.mob(m.get(key) as u32)
        .ok_or_else(|| format!("{key} is gone"))
}

fn turret_at(o: Pos) -> Pos {
    at(o, [10, 1, 0])
}

fn shoots_a_hostile_mob_only(o: Pos, t: &Tuning) -> Scenario {
    let start = t.turret_buffer;
    let cost = loadout(&[]).shot_energy(t);
    arena(
        "turret_shoots_a_hostile_mob_only",
        o,
        t,
        Vec::new(),
        vec![(SPIDER, 6, None), (PIG, 2, None)],
        None,
    )
    .until("the spider is hurt", 60, |b, m| {
        Ok(mob(b, m, "mob0")?.health < 16.0)
    })
    .check("the pig is not (P3), and a shot cost 40 E", move |b, m| {
        same("pig's health", mob(b, m, "mob1")?.health, 10.0)?;
        let spent = start - b.buffer(turret_at(o));
        let shots = (spent / cost).round();
        near("energy a shot", spent / shots.max(1.0), cost, 1.0)
    })
}

fn effects_combine(o: Pos, t: &Tuning) -> Scenario {
    let m3 = [Module::Bleed, Module::Poison, Module::Slow];
    arena(
        "turret_effects_combine",
        o,
        t,
        mods(&m3),
        vec![(PILLAGER, 6, None)],
        None,
    )
    .until("one shot bleeds, poisons and slows", 60, |b, m| {
        let s = mob(b, m, "mob0")?;
        Ok(s.effect("minecraft:poison").is_some()
            && s.effect("minecraft:slowness").is_some()
            && b.bleeding(m.get("mob0") as u32))
    })
    .check("as the rules give them", move |b, m| {
        let s = mob(b, m, "mob0")?;
        let given = turret::effects(
            &loadout(&m3),
            &Tuning::default(),
            &Default::default(),
            false,
        );
        let slow = given
            .iter()
            .find(|g| g.effect == turret::Effect::Slow)
            .map(|g| g.amplifier);
        same(
            "slowness amplifier",
            s.effect("minecraft:slowness").map(|e| e.1),
            slow,
        )
    })
}

fn poison_spares_the_undead(o: Pos, t: &Tuning) -> Scenario {
    arena(
        "turret_poison_spares_the_undead",
        o,
        t,
        mods(&[Module::Poison, Module::Slow]),
        vec![(HUSK, 6, None)],
        None,
    )
    .until("the husk is slowed", 60, |b, m| {
        Ok(mob(b, m, "mob0")?.effect("minecraft:slowness").is_some())
    })
    .check("but not poisoned (TR5)", |b, m| {
        same(
            "poison",
            mob(b, m, "mob0")?.effect("minecraft:poison"),
            None,
        )
    })
}

fn slowness_stops_at_its_cap(o: Pos, t: &Tuning) -> Scenario {
    let m3 = [Module::Slow, Module::Slow, Module::Slow];
    let want = turret::slowness_amplifier(t.slow_cap);
    arena(
        "turret_slowness_stops_at_its_cap",
        o,
        t,
        mods(&m3),
        vec![(PILLAGER, 6, None)],
        None,
    )
    .until(
        "three slow modules give the cap's level",
        60,
        move |b, m| Ok(mob(b, m, "mob0")?.effect("minecraft:slowness").map(|e| e.1) == Some(want)),
    )
}

fn support_shoots_nothing(o: Pos, t: &Tuning) -> Scenario {
    arena(
        "turret_support_shoots_nothing",
        o,
        t,
        mods(&[Module::Heal, Module::Poison]),
        vec![(SPIDER, 4, None)],
        None,
    )
    .wait(60)
    .check(
        "TR3: no shot, and the idle effect module is named",
        move |b, m| {
            same("spider's health", mob(b, m, "mob0")?.health, 16.0)?;
            same("shown", b.turret(turret_at(o)), Some(Shown::IdleEffects))
        },
    )
}

fn no_energy(o: Pos, t: &Tuning) -> Scenario {
    arena(
        "turret_without_energy",
        o,
        t,
        Vec::new(),
        vec![(SPIDER, 6, None)],
        Some(0.0),
    )
    .wait(40)
    .check("no shot, «Нет энергии»", move |b, m| {
        same("spider's health", mob(b, m, "mob0")?.health, 16.0)?;
        same("shown", b.turret(turret_at(o)), Some(Shown::NoEnergy))
    })
}

fn kill_is_reported(o: Pos, t: &Tuning) -> Scenario {
    arena(
        "turret_kill_is_reported",
        o,
        t,
        Vec::new(),
        vec![(SPIDER, 6, Some(2.0))],
        None,
    )
    .until("the spider dies", 60, |b, m| {
        Ok(b.mob(m.get("mob0") as u32).is_none())
    })
    .until("its kill goes to Path-of-craft-tree", 40, |b, m| {
        let line = format!(
            "kill {OWNER:032x} {} minecraft:spider",
            m.get("mob0") as u32
        );
        Ok(b.reports().iter().any(|l| l.contains(&line)))
    })
}

fn soulscraft_resistances(o: Pos, t: &Tuning) -> Scenario {
    let m3 = [Module::Bleed, Module::Poison, Module::Slow];
    // Slowness: it acts on the undead too, and the brute may be one.
    let base = (t.slow_seconds * t.ticks_per_second) as u32;
    let turret = turret_at(o);
    Scenario::new("turret_soulscraft_resistances", plot_area(o))
        .then("a turret with three effect modules", move |b, m| {
            for x in 0..24 {
                for z in -3..=3 {
                    b.place(at(o, [x, 0, z]), "minecraft:stone", &[])?;
                }
            }
            b.place(turret, TURRET, &[])?;
            b.set_modules(turret, &mods(&m3))?;
            b.set_buffer(turret, 800.0)?;
            // Without SoulsCraft in the build there is nothing to check.
            let at = [
                f64::from(turret[0]) + 6.5,
                f64::from(turret[1]),
                f64::from(turret[2]) + 0.5,
            ];
            match b.spawn_mob("soulscraft:brute", at, None) {
                Ok(id) => m.mark("mob0", f64::from(id)),
                Err(_) => m.mark("none", 1.0),
            }
            Ok(())
        })
        .until(
            "the brute's slowness is half as long (27.1)",
            80,
            move |b, m| {
                if m.get("none") > 0.0 {
                    return Ok(true);
                }
                if !b.souls_found() {
                    return Err("SoulsCraft is in the build but its mobs.json was not found".into());
                }
                let Some(p) = mob(b, m, "mob0")?.effect("minecraft:slowness") else {
                    return Ok(false);
                };
                if p.0 > base / 2 {
                    return Err(format!(
                        "slowness {} ticks, expected at most {}",
                        p.0,
                        base / 2
                    ));
                }
                Ok(true)
            },
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cap_is_a_level_the_rules_give() {
        let t = Tuning::default();
        assert_eq!(turret::slowness_amplifier(t.slow_cap), 3);
    }
}
