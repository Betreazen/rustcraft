//! A one-input machine (electric furnace, crusher): it takes a recipe's input, works for a while on
//! energy, and gives the recipe's output. The work mode (A1) and the modules (A7, A9) scale its
//! speed and its energy; a circuit that gives less than it asks slows it by that share (E3). In any
//! state that is not work it shows one diagnosis (A4).

use crate::net::items::ItemKey;

/// A1: the work mode of a machine.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WorkMode {
    Economy,
    #[default]
    Normal,
    Boost,
}

impl WorkMode {
    pub const ALL: [WorkMode; 3] = [WorkMode::Economy, WorkMode::Normal, WorkMode::Boost];

    /// The byte a machine saves its mode as, and its button's id in the machine's window.
    pub fn code(self) -> u8 {
        match self {
            WorkMode::Economy => 0,
            WorkMode::Normal => 1,
            WorkMode::Boost => 2,
        }
    }

    /// The mode `code` names; none for a byte it does not know.
    pub fn from_code(code: u8) -> Option<WorkMode> {
        Self::ALL.into_iter().find(|m| m.code() == code)
    }

    /// (speed, energy) multipliers: economy ×0.5 for ×0.35, normal ×1, boost ×2 for ×3.
    pub fn factors(self) -> (f64, f64) {
        match self {
            WorkMode::Economy => (0.5, 0.35),
            WorkMode::Normal => (1.0, 1.0),
            WorkMode::Boost => (2.0, 3.0),
        }
    }
}

/// A machine module in a slot (A7). Identical modules stack fully (A9).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Module {
    /// Work ×2, energy ×2.5.
    Speed,
    /// Energy ×0.6, work ×0.85.
    Economy,
}

impl Module {
    fn factors(self) -> (f64, f64) {
        match self {
            Module::Speed => (2.0, 2.5),
            Module::Economy => (0.85, 0.6),
        }
    }
}

/// A recipe of a one-input machine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recipe {
    pub input: ItemKey,
    pub input_count: u32,
    pub output: ItemKey,
    pub output_count: u32,
}

/// The fixed numbers of a machine kind.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spec {
    /// Ticks one recipe takes in normal mode with full energy.
    pub ticks: f64,
    /// E a second in normal mode.
    pub energy: f64,
    /// How many items its output slot holds.
    pub output_capacity: u32,
}

/// A4: why the machine is not working, or that it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Working,
    /// Nothing to work on (the input is empty or has no recipe).
    Idle,
    /// «Готовое некуда отдать».
    OutputFull,
    /// «Нет энергии».
    NoEnergy,
}

/// What a machine shows a player looking at it (A4), from its state and the share of its request
/// the circuit gave (E3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shown {
    /// «Работает».
    Working,
    /// «Не хватает энергии: 50%»: it works, slower by that share.
    Slowed { percent: u32 },
    /// «Нет энергии».
    NoEnergy,
    /// «Готовое некуда отдать».
    OutputFull,
    /// Nothing to work on.
    Idle,
    /// «Нет топлива»: a generator, or an alloy smelter with neither fuel nor energy.
    NoFuel,
    /// «Работает на топливе»: an alloy smelter without energy.
    OnFuel,
    /// A generator with fuel whose circuit asks for no energy.
    NoDemand,
}

impl Shown {
    pub fn of(state: State, share: f64) -> Shown {
        match state {
            State::Working if share < 1.0 - 1e-9 => Shown::Slowed {
                // A working machine got something: never «0%», never «100%» while short.
                percent: ((share * 100.0).round() as u32).clamp(1, 99),
            },
            State::Working => Shown::Working,
            State::NoEnergy => Shown::NoEnergy,
            State::OutputFull => Shown::OutputFull,
            State::Idle => Shown::Idle,
        }
    }
}

/// A one-input machine's contents and settings.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Processor {
    pub input: Option<(ItemKey, u32)>,
    pub output: Option<(ItemKey, u32)>,
    /// Work done on the current recipe, 0 to 1.
    pub progress: f64,
    pub mode: WorkMode,
    pub modules: Vec<Module>,
}

impl Processor {
    /// Speed and energy multipliers of the mode and every module together.
    pub fn factors(&self) -> (f64, f64) {
        let mut speed = 1.0;
        let mut energy = 1.0;
        for (s, e) in
            std::iter::once(self.mode.factors()).chain(self.modules.iter().map(|m| m.factors()))
        {
            speed *= s;
            energy *= e;
        }
        (speed, energy)
    }

    /// The recipe the machine would run now and whether its output has room for it.
    fn job<'a>(&self, recipes: &'a [Recipe], spec: &Spec) -> Result<&'a Recipe, State> {
        let (item, count) = self.input.as_ref().ok_or(State::Idle)?;
        let recipe = recipes
            .iter()
            .find(|r| &r.input == item)
            .filter(|r| *count >= r.input_count)
            .ok_or(State::Idle)?;
        match &self.output {
            Some((out, n))
                if out != &recipe.output || n + recipe.output_count > spec.output_capacity =>
            {
                Err(State::OutputFull)
            }
            _ => Ok(recipe),
        }
    }

    /// The E a second the machine asks its circuit for this tick.
    pub fn request(&self, recipes: &[Recipe], spec: &Spec) -> f64 {
        match self.job(recipes, spec) {
            Ok(_) => spec.energy * self.factors().1,
            Err(_) => 0.0,
        }
    }

    /// One tick with `share` of the requested energy (E3); returns the machine's state.
    pub fn tick(&mut self, recipes: &[Recipe], spec: &Spec, share: f64) -> State {
        let recipe = match self.job(recipes, spec) {
            Ok(r) => r.clone(),
            Err(state) => {
                if state == State::Idle {
                    self.progress = 0.0;
                }
                return state;
            }
        };
        if share <= 0.0 {
            return State::NoEnergy;
        }
        self.progress += self.factors().0 * share.min(1.0) / spec.ticks;
        if self.progress >= 1.0 - 1e-9 {
            self.progress = 0.0;
            self.finish(&recipe);
        }
        State::Working
    }

    /// Takes the recipe's input and adds its output.
    fn finish(&mut self, recipe: &Recipe) {
        if let Some((_, n)) = self.input.as_mut() {
            *n -= recipe.input_count;
            if *n == 0 {
                self.input = None;
            }
        }
        match self.output.as_mut() {
            Some((_, n)) => *n += recipe.output_count,
            None => self.output = Some((recipe.output.clone(), recipe.output_count)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn smelt() -> Vec<Recipe> {
        vec![
            Recipe {
                input: "rc:dust_iron".into(),
                input_count: 1,
                output: "minecraft:iron_ingot".into(),
                output_count: 1,
            },
            Recipe {
                input: "minecraft:raw_iron".into(),
                input_count: 1,
                output: "minecraft:iron_ingot".into(),
                output_count: 1,
            },
        ]
    }

    fn crush() -> Vec<Recipe> {
        vec![Recipe {
            input: "minecraft:raw_iron".into(),
            input_count: 1,
            output: "rc:dust_iron".into(),
            output_count: 2,
        }]
    }

    const FURNACE: Spec = Spec {
        ticks: 100.0,
        energy: 8.0,
        output_capacity: 64,
    };

    fn with_input(item: &str, n: u32) -> Processor {
        Processor {
            input: Some((item.into(), n)),
            ..Processor::default()
        }
    }

    /// Ticks with full energy until the output holds `n` items; returns the ticks it took.
    fn ticks_until(p: &mut Processor, recipes: &[Recipe], spec: &Spec, share: f64, n: u32) -> u32 {
        for t in 1..=10_000 {
            p.tick(recipes, spec, share);
            if p.output.as_ref().is_some_and(|(_, c)| *c >= n) {
                return t;
            }
        }
        panic!("never produced {n}");
    }

    #[test]
    fn the_electric_furnace_smelts_in_a_hundred_ticks() {
        // Twice as fast as the base-game furnace's 200 ticks, at 8 E/s.
        let mut p = with_input("rc:dust_iron", 2);
        assert_eq!(p.request(&smelt(), &FURNACE), 8.0);
        assert_eq!(ticks_until(&mut p, &smelt(), &FURNACE, 1.0, 1), 100);
        assert_eq!(p.input, Some(("rc:dust_iron".into(), 1)));
        assert_eq!(p.output, Some(("minecraft:iron_ingot".into(), 1)));
    }

    #[test]
    fn half_the_energy_works_half_as_fast() {
        // Rule E3.
        let mut p = with_input("rc:dust_iron", 1);
        assert_eq!(ticks_until(&mut p, &smelt(), &FURNACE, 0.5, 1), 200);
    }

    #[test]
    fn the_crusher_gives_two_dusts_from_one_ore() {
        let mut p = with_input("minecraft:raw_iron", 1);
        ticks_until(&mut p, &crush(), &FURNACE, 1.0, 2);
        assert_eq!(p.output, Some(("rc:dust_iron".into(), 2)));
        assert_eq!(p.input, None);
    }

    #[test]
    fn boost_mode_is_twice_as_fast_for_three_times_the_energy() {
        // Rule A1.
        let mut p = Processor {
            mode: WorkMode::Boost,
            ..with_input("rc:dust_iron", 1)
        };
        assert_eq!(p.request(&smelt(), &FURNACE), 24.0);
        assert_eq!(ticks_until(&mut p, &smelt(), &FURNACE, 1.0, 1), 50);
    }

    #[test]
    fn economy_is_half_as_fast_for_a_third_of_the_energy_and_modes_survive_a_save() {
        // Rule A1: 8 E/s normal is 2.8 E/s economy, 24 E/s boost.
        let mut p = Processor {
            mode: WorkMode::Economy,
            ..with_input("rc:dust_iron", 1)
        };
        assert!((p.request(&smelt(), &FURNACE) - 2.8).abs() < 1e-9);
        assert_eq!(ticks_until(&mut p, &smelt(), &FURNACE, 1.0, 1), 200);
        for mode in WorkMode::ALL {
            assert_eq!(WorkMode::from_code(mode.code()), Some(mode));
        }
        assert_eq!(WorkMode::from_code(3), None);
    }

    #[test]
    fn identical_modules_stack_fully() {
        // Rule A9: two speed modules work ×4 for energy ×6.25.
        let p = Processor {
            modules: vec![Module::Speed, Module::Speed],
            ..with_input("rc:dust_iron", 1)
        };
        let (speed, energy) = p.factors();
        assert!((speed - 4.0).abs() < 1e-9);
        assert!((energy - 6.25).abs() < 1e-9);
        assert_eq!(p.request(&smelt(), &FURNACE), 50.0);
    }

    #[test]
    fn a_full_output_stops_the_machine_and_it_asks_for_nothing() {
        let mut p = Processor {
            output: Some(("minecraft:iron_ingot".into(), 64)),
            ..with_input("rc:dust_iron", 5)
        };
        let find = smelt();
        assert_eq!(p.request(&find, &FURNACE), 0.0);
        assert_eq!(p.tick(&find, &FURNACE, 1.0), State::OutputFull);
        assert_eq!(p.progress, 0.0);
    }

    #[test]
    fn an_output_of_another_item_blocks_the_machine() {
        let mut p = Processor {
            output: Some(("minecraft:gold_ingot".into(), 3)),
            ..with_input("rc:dust_iron", 1)
        };
        assert_eq!(p.tick(&smelt(), &FURNACE, 1.0), State::OutputFull);
    }

    #[test]
    fn a_short_circuit_shows_the_share_the_machine_gets() {
        // «Не хватает энергии: 50%» for a furnace on one crystal.
        assert_eq!(
            Shown::of(State::Working, 0.5),
            Shown::Slowed { percent: 50 }
        );
        assert_eq!(
            Shown::of(State::Working, 0.25),
            Shown::Slowed { percent: 25 }
        );
        assert_eq!(
            Shown::of(State::Working, 0.001),
            Shown::Slowed { percent: 1 }
        );
        assert_eq!(
            Shown::of(State::Working, 0.9999),
            Shown::Slowed { percent: 99 }
        );
        assert_eq!(Shown::of(State::Working, 1.0), Shown::Working);
        assert_eq!(Shown::of(State::NoEnergy, 0.0), Shown::NoEnergy);
        assert_eq!(Shown::of(State::OutputFull, 1.0), Shown::OutputFull);
        assert_eq!(Shown::of(State::Idle, 1.0), Shown::Idle);
    }

    #[test]
    fn no_energy_and_no_input_have_their_diagnoses() {
        let find = smelt();
        let mut p = with_input("rc:dust_iron", 1);
        assert_eq!(p.tick(&find, &FURNACE, 0.0), State::NoEnergy);
        let mut empty = Processor::default();
        assert_eq!(empty.tick(&find, &FURNACE, 1.0), State::Idle);
        assert_eq!(empty.request(&find, &FURNACE), 0.0);
        let mut unknown = with_input("minecraft:dirt", 4);
        assert_eq!(unknown.tick(&find, &FURNACE, 1.0), State::Idle);
    }
}
