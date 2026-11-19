//! Orders from the storage terminal (spec section 19). An order for an item is broken down into
//! recipes until it reaches what the storage holds (O3); those items are set aside for it. A step
//! may run on any machine of the mod on the line, not only the autocrafter (O9, O10). When an item
//! is short, the order says which and how many (O6); when no machine on the line can run a step,
//! it says which machine is missing (O13).
//!
//! The recipe book (O1) is the game's crafting and smelting recipes and the mod's machine recipes,
//! as `PlanRecipe`s; `PlanRecipe::crafting` builds the autocrafter's ones from a crafting grid. An
//! input of a recipe may take any of several items (planks of any wood): the planner takes the one
//! the storage has most of. `queue` runs the orders of one storage; `saved` writes a queue and
//! its running jobs into the storage's record and reads them back; `ship` lets a job's items ride the
//! item channel (slice 17).
//!
//! O12: fluids are keys of their own (`fluid_key`), counted in buckets. The mixer's recipes take
//! items and water and make a fluid. Round 31 (spec 29, FL5, FL6): the storage keeps fluids in its
//! cells, and its stock of a fluid is what they hold and what the tanks on its fluid line hold
//! (with the water the line's mixers can draw on); an order takes its fluid from there. An order
//! for a fluid is done when the mixer has made it, and the fluid nets carry it on. The kitchen's
//! copy of a recipe with a milk bucket takes milk from the stock instead (FL7).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::net::items::ItemKey;

pub mod held;
pub mod queue;
pub mod ride;
pub mod saved;
pub mod ship;

/// The machine kinds a step can name, in the order the terminal's list codes them.
/// Slice 18 adds the kitchen (guide `kitchen`: «заказ из терминала склада тоже проходит через
/// комбайн»): it runs the food recipes of the crafting table as the autocrafter does.
pub const KINDS: [&str; 6] = ["crafter", "furnace", "crusher", "alloy", "mixer", "kitchen"];

/// Slice 18 (handed over from stream 1): what an autocrafter's page still lacks for one craft:
/// each item of its pattern less what its buffer holds, none when the buffer has it all.
pub fn page_shortfall(pattern: &[Option<String>], buffer: &BTreeMap<String, u32>) -> Vec<(String, u32)> {
    let mut need: BTreeMap<&str, u32> = BTreeMap::new();
    for c in pattern.iter().flatten() {
        *need.entry(c.as_str()).or_insert(0) += 1;
    }
    need.into_iter()
        .filter_map(|(item, n)| {
            let short = n.saturating_sub(buffer.get(item).copied().unwrap_or(0));
            (short > 0).then(|| (item.to_owned(), short))
        })
        .collect()
}

/// 0.19.2 (the owner: «сделай так чтобы со склада электропечь не забирала сама руду, только
/// пыль»): what an order may put in the furnace for a smelting recipe that takes any of
/// `alternatives`: none of what the crusher grinds (`crushed`, ore and raw metal), which goes
/// crusher → two dust → furnace instead of one ingot for one. None when nothing is left: the
/// recipe is no way for an order.
pub fn furnace_inputs(alternatives: Vec<ItemKey>, crushed: &std::collections::BTreeSet<ItemKey>) -> Option<Vec<ItemKey>> {
    let left: Vec<ItemKey> = alternatives.into_iter().filter(|i| !crushed.contains(i)).collect();
    (!left.is_empty()).then_some(left)
}

/// Slice 18: the kitchen's copy of a crafting recipe whose result it cooks (the autocrafter keeps
/// its own): an order runs on whichever of the two the line has. Round 31 (FL7): the kitchen
/// takes a milk bucket's worth of milk from the fluid line instead of the bucket, so its copy
/// takes either, the one the stock has more of.
pub fn kitchen_copy(r: &PlanRecipe, kitchen_dishes: &[&str]) -> Option<PlanRecipe> {
    if r.machine != KINDS[0] || !kitchen_dishes.contains(&r.output.0.as_str()) {
        return None;
    }
    let inputs = r
        .inputs
        .iter()
        .map(|(alts, n)| {
            let mut alts = alts.clone();
            if alts.iter().any(|a| a == MILK_BUCKET) {
                alts.push(fluid_key(MILK));
            }
            (alts, *n)
        })
        .collect();
    Some(PlanRecipe {
        machine: KINDS[5].into(),
        inputs,
        ..r.clone()
    })
}

/// The milk bucket of the game's recipes, and the milk the kitchen takes for it (FL7).
const MILK_BUCKET: &str = "minecraft:milk_bucket";
const MILK: &str = "rustcraft:milk";

/// O12: a fluid's key starts with this, so no item id (`namespace:path`) is one.
pub const FLUID_PREFIX: &str = "fluid:";
/// The fluids an order can name, in the order the terminal's list numbers them. Round 31: milk
/// last, so the numbers before it stay.
pub const FLUIDS: [&str; 7] = [
    "minecraft:water",
    "minecraft:lava",
    "rustcraft:solution",
    "rustcraft:accelerator",
    "rustcraft:economizer",
    "rustcraft:enricher",
    "rustcraft:milk",
];
/// The terminal's list names fluid `k` of FLUIDS by this number plus `k`: past every item of
/// the item table, and short of a press's «no item» (0xFFFF).
pub const FLUID_NUMBERS: u16 = 0xFF00;

/// The key of `fluid` (`minecraft:water` → `fluid:minecraft:water`).
pub fn fluid_key(fluid: &str) -> ItemKey {
    format!("{FLUID_PREFIX}{fluid}")
}

/// The fluid a key names; none for an item's key.
pub fn fluid_of(key: &str) -> Option<&str> {
    key.strip_prefix(FLUID_PREFIX)
}

/// The number the terminal's list gives a fluid; none for a fluid it does not know.
pub fn fluid_number(fluid: &str) -> Option<u16> {
    FLUIDS
        .iter()
        .position(|f| *f == fluid)
        .map(|k| FLUID_NUMBERS + k as u16)
}

/// The fluid a number of the terminal's list stands for; none for an item's number.
pub fn fluid_of_number(number: u16) -> Option<&'static str> {
    FLUIDS
        .get(usize::from(number.checked_sub(FLUID_NUMBERS)?))
        .copied()
}

/// The most items the planner breaks down for one order before it takes the rest as missing: a
/// guard against a recipe book whose recipes branch without end (the order is planned again
/// every second while it waits).
const BUDGET: u32 = 3_000;
/// Recipes tried for one item, best first.
const MAX_TRIES: usize = 4;

/// A recipe any machine of the mod runs, as the recipe book knows it (O1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanRecipe {
    /// The machine kind that runs it: one of KINDS.
    pub machine: String,
    /// Each input: the items it takes (any one of them, the first preferred) and how many one run
    /// takes.
    pub inputs: Vec<(Vec<ItemKey>, u32)>,
    pub output: (ItemKey, u32),
    /// The autocrafter's 3×3 grid row by row: the input each cell takes. Empty for a machine.
    pub cells: Vec<Option<usize>>,
}

impl PlanRecipe {
    /// A recipe whose every input is one item.
    pub fn simple(machine: &str, inputs: &[(&str, u32)], output: (&str, u32)) -> PlanRecipe {
        PlanRecipe {
            machine: machine.into(),
            inputs: inputs
                .iter()
                .map(|(i, n)| (vec![(*i).to_string()], *n))
                .collect(),
            output: (output.0.into(), output.1),
            cells: Vec::new(),
        }
    }

    /// An autocrafter recipe from a crafting grid `width` cells wide, each cell the items it takes
    /// (none for an empty cell). Cells that take the same items are one input. None for a grid
    /// wider or taller than 3 or with no cell.
    pub fn crafting(
        width: usize,
        cells: &[Option<Vec<ItemKey>>],
        output: (ItemKey, u32),
    ) -> Option<PlanRecipe> {
        let width = width.max(1);
        let height = cells.len().div_ceil(width);
        if width > 3 || height > 3 {
            return None;
        }
        let mut inputs: Vec<(Vec<ItemKey>, u32)> = Vec::new();
        let mut grid = vec![None; 9];
        for (i, cell) in cells.iter().enumerate() {
            let Some(items) = cell.as_ref().filter(|items| !items.is_empty()) else {
                continue;
            };
            let k = match inputs.iter().position(|(alts, _)| alts == items) {
                Some(k) => k,
                None => {
                    inputs.push((items.clone(), 0));
                    inputs.len() - 1
                }
            };
            inputs[k].1 += 1;
            grid[(i / width) * 3 + i % width] = Some(k);
        }
        if inputs.is_empty() {
            return None;
        }
        Some(PlanRecipe {
            machine: KINDS[0].into(),
            inputs,
            output: (output.0, output.1.max(1)),
            cells: grid,
        })
    }
}

/// One step of an order: run a recipe of the book `times` times with these items.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    /// The recipe's place in the book.
    pub recipe: usize,
    pub machine: String,
    /// The item chosen for each input of the recipe, in its order, and how many one run takes
    /// (a fluid's key in buckets).
    pub inputs: Vec<(ItemKey, u32)>,
    pub output: (ItemKey, u32),
    pub times: u32,
}

impl Step {
    /// The inputs the storage hands over: the items, not the fluids, which the line's fluid nets
    /// bring (O12).
    pub fn items(&self) -> Vec<(ItemKey, u32)> {
        self.inputs
            .iter()
            .filter(|(k, _)| fluid_of(k).is_none())
            .cloned()
            .collect()
    }

    /// The autocrafter's pattern for this step: each cell the item chosen for its input.
    pub fn pattern(&self, book: &[PlanRecipe]) -> [Option<ItemKey>; 9] {
        let cells = book.get(self.recipe).map_or(&[][..], |r| &r.cells[..]);
        std::array::from_fn(|i| {
            cells
                .get(i)
                .copied()
                .flatten()
                .and_then(|k| self.inputs.get(k))
                .map(|(item, _)| item.clone())
        })
    }
}

/// The plan of an order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    /// Steps in the order they must run: every step's inputs are made by earlier steps or set aside.
    pub steps: Vec<Step>,
    /// Items from the storage set aside for the order (O3).
    pub reserved: BTreeMap<ItemKey, u64>,
    /// O6: items the storage lacks and no recipe makes, and how many are short.
    pub missing: BTreeMap<ItemKey, u64>,
    /// O13: machine kinds a step needs that the line does not have.
    pub missing_machines: Vec<String>,
}

impl Plan {
    /// The order can run now.
    pub fn ready(&self) -> bool {
        self.missing.is_empty() && self.missing_machines.is_empty()
    }
}

/// Plans an order for `count` of `item`, given the storage's stock, the recipe book and the
/// machine kinds on the line.
pub fn plan(
    item: &str,
    count: u64,
    stock: &BTreeMap<ItemKey, u64>,
    book: &[PlanRecipe],
    machines: &[&str],
) -> Plan {
    let mut planner = Planner {
        stock: stock.clone(),
        book,
        machines,
        plan: Plan::default(),
        making: Vec::new(),
        budget: BUDGET,
    };
    planner.need(item, count);
    planner.plan
}

/// How many items a plan takes from the storage, of every kind.
fn taken(plan: &Plan) -> u64 {
    plan.reserved.values().sum()
}

struct Planner<'a> {
    /// What the storage still has free, plus what earlier steps make beyond their need.
    stock: BTreeMap<ItemKey, u64>,
    book: &'a [PlanRecipe],
    machines: &'a [&'a str],
    plan: Plan,
    /// Items being broken down right now, so a recipe loop ends instead of recursing forever.
    making: Vec<ItemKey>,
    budget: u32,
}

impl Planner<'_> {
    fn free(&self, item: &str) -> u64 {
        self.stock.get(item).copied().unwrap_or(0)
    }

    fn short(&mut self, item: &str, qty: u64) {
        *self.plan.missing.entry(item.to_string()).or_insert(0) += qty;
    }

    /// Arranges `qty` of `item`: from stock first, then by a recipe, else it is missing.
    fn need(&mut self, item: &str, qty: u64) {
        let take = self.free(item).min(qty);
        if take > 0 {
            *self.stock.get_mut(item).expect("free > 0") -= take;
            *self.plan.reserved.entry(item.to_string()).or_insert(0) += take;
        }
        let rest = qty - take;
        if rest == 0 {
            return;
        }
        if self.budget == 0 {
            self.short(item, rest);
            return;
        }
        self.budget -= 1;
        let found: Vec<usize> = (0..self.book.len())
            .filter(|&i| self.book[i].output.0 == item)
            .collect();
        if found.is_empty() || self.making.iter().any(|m| m == item) {
            self.short(item, rest);
            return;
        }
        // Each recipe in turn on a copy of the plan: of those that need nothing the storage lacks,
        // the one that takes the fewest items from it (0.20, round 31: «надо, чтобы всегда умная
        // магистраль искала оптимизированный рецепт»); else the first that lacks only a machine
        // (O13); else the first that gathers some of its inputs, whose shortages are named. When
        // no recipe gathers any input, the item itself is what is short (O6: «не хватает:
        // железный слиток ×2», not the dust it could be smelted from).
        let mut ready: Option<(u64, Self)> = None;
        let mut lacks_machine: Option<Self> = None;
        let mut lacks_items: Option<Self> = None;
        for index in self.preferred(&found, rest).into_iter().take(MAX_TRIES) {
            // A way that works is in hand: another is weighed only while the budget lasts, so a
            // big order's tree does not branch four ways at every step.
            if ready.is_some() && self.budget < BUDGET / 2 {
                break;
            }
            let mut trial = self.copy();
            let hopeless = trial.run(item, rest, index);
            self.budget = trial.budget;
            // 0.19.1: a way that comes back to the item itself (iron from nuggets, nuggets from
            // smelted iron pickaxes, pickaxes from iron) is no way to make it, however much else
            // it gathers: the owner's 24 item channels waited for 264 iron instead of 21.
            let own = |p: &Self| p.plan.missing.get(item).copied().unwrap_or(0);
            if own(&trial) > own(self) {
                continue;
            }
            if trial.plan.missing != self.plan.missing {
                if !hopeless && lacks_items.is_none() {
                    lacks_items = Some(trial);
                }
                continue;
            }
            if trial.plan.missing_machines == self.plan.missing_machines {
                let taken = taken(&trial.plan) - taken(&self.plan);
                if ready.as_ref().is_none_or(|(best, _)| taken < *best) {
                    ready = Some((taken, trial));
                }
                continue;
            }
            lacks_machine.get_or_insert(trial);
        }
        if let Some((_, trial)) = ready {
            let budget = self.budget;
            *self = trial;
            self.budget = budget;
            return;
        }
        match lacks_machine.or(lacks_items) {
            Some(trial) => {
                let budget = self.budget;
                *self = trial;
                self.budget = budget;
            }
            None => self.short(item, rest),
        }
    }

    fn copy(&self) -> Self {
        Planner {
            stock: self.stock.clone(),
            book: self.book,
            machines: self.machines,
            plan: self.plan.clone(),
            making: self.making.clone(),
            budget: self.budget,
        }
    }

    /// Plans `rest` of `item` by the book's recipe `index`; true when not one of its inputs could
    /// be gathered at all.
    fn run(&mut self, item: &str, rest: u64, index: usize) -> bool {
        let recipe = &self.book[index];
        if !self.machines.contains(&recipe.machine.as_str())
            && !self.plan.missing_machines.contains(&recipe.machine)
        {
            self.plan.missing_machines.push(recipe.machine.clone());
        }
        let per_run = u64::from(recipe.output.1.max(1));
        let times = rest.div_ceil(per_run);
        self.making.push(item.to_string());
        let mut chosen = Vec::with_capacity(recipe.inputs.len());
        let mut hopeless = !recipe.inputs.is_empty();
        for (alts, n) in &recipe.inputs {
            let want = u64::from(*n) * times;
            let pick = self.pick(alts, want);
            let short_before = self.plan.missing.get(&pick).copied().unwrap_or(0);
            self.need(&pick, want);
            let short_after = self.plan.missing.get(&pick).copied().unwrap_or(0);
            if short_after - short_before < want {
                hopeless = false;
            }
            chosen.push((pick, *n));
        }
        self.making.pop();
        // What the runs make beyond the need is free for later needs.
        let surplus = times * per_run - rest;
        if surplus > 0 {
            *self.stock.entry(item.to_string()).or_insert(0) += surplus;
        }
        let times = u32::try_from(times).unwrap_or(u32::MAX);
        let same = |s: &&mut Step| s.recipe == index && s.inputs == chosen;
        match self.plan.steps.iter_mut().find(same) {
            Some(step) => step.times = step.times.saturating_add(times),
            None => self.plan.steps.push(Step {
                recipe: index,
                machine: recipe.machine.clone(),
                inputs: chosen,
                output: recipe.output.clone(),
                times,
            }),
        }
        hopeless
    }

    /// The recipes that make the item, best first: of those a machine on the line runs, the ones
    /// whose inputs are all in stock, then those with any input in stock, then the rest; then the
    /// ones no machine on the line runs, in the same order.
    fn preferred(&self, found: &[usize], qty: u64) -> Vec<usize> {
        let times = |i: usize| qty.div_ceil(u64::from(self.book[i].output.1.max(1)));
        // 0: the stock covers each input; 1: it has some of an input; 2: none.
        let cover = |i: usize| {
            let inputs = &self.book[i].inputs;
            let want = |n: u32| u64::from(n) * times(i);
            if inputs
                .iter()
                .all(|(alts, n)| alts.iter().any(|a| self.free(a) >= want(*n)))
            {
                0
            } else if inputs
                .iter()
                .any(|(alts, _)| alts.iter().any(|a| self.free(a) > 0))
            {
                1
            } else {
                2
            }
        };
        let mut order: Vec<usize> = found.to_vec();
        order.sort_by_key(|&i| {
            let absent = !self.machines.contains(&self.book[i].machine.as_str());
            (absent, cover(i))
        });
        order
    }

    /// The item of `alts` to take `want` of: the first the stock covers, else the one it has most
    /// of, else the first a recipe makes, else the first.
    fn pick(&self, alts: &[ItemKey], want: u64) -> ItemKey {
        if let Some(a) = alts.iter().find(|a| self.free(a) >= want) {
            return a.clone();
        }
        let mut best: Option<(&ItemKey, u64)> = None;
        for a in alts {
            let free = self.free(a);
            if free > 0 && best.is_none_or(|(_, b)| free > b) {
                best = Some((a, free));
            }
        }
        if let Some((a, _)) = best {
            return a.clone();
        }
        alts.iter()
            .find(|a| !self.making.contains(a) && self.book.iter().any(|r| &r.output.0 == *a))
            .or_else(|| alts.first())
            .cloned()
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_autocrafter_asks_for_what_its_page_lacks() {
        let pattern: Vec<Option<String>> = ["minecraft:oak_planks", "minecraft:oak_planks", "minecraft:stick"]
            .iter()
            .map(|s| Some((*s).to_owned()))
            .chain(std::iter::repeat_n(None, 6))
            .collect();
        let buffer = BTreeMap::from([("minecraft:oak_planks".to_owned(), 1)]);
        assert_eq!(
            page_shortfall(&pattern, &buffer),
            vec![("minecraft:oak_planks".to_owned(), 1), ("minecraft:stick".to_owned(), 1)]
        );
        let full = BTreeMap::from([("minecraft:oak_planks".to_owned(), 2), ("minecraft:stick".to_owned(), 5)]);
        assert!(page_shortfall(&pattern, &full).is_empty());
    }

    #[test]
    fn bread_is_ordered_through_the_kitchen_when_the_line_has_one() {
        // Slice 18: the kitchen's copy of the crafting recipe.
        let crafted = r("crafter", &[("minecraft:wheat", 3)], ("minecraft:bread", 1));
        let k = kitchen_copy(&crafted, &["minecraft:bread"]).unwrap();
        assert_eq!(k.machine, "kitchen");
        assert!(kitchen_copy(&crafted, &["minecraft:cake"]).is_none());
        let stock = BTreeMap::from([("minecraft:wheat".to_owned(), 9)]);
        let book = vec![k, crafted];
        let p = plan("minecraft:bread", 2, &stock, &book, &["kitchen"]);
        assert!(p.ready());
        assert_eq!(p.steps[0].machine, "kitchen");
        let p = plan("minecraft:bread", 2, &stock, &book, &["crafter"]);
        assert!(p.ready(), "no kitchen: the autocrafter makes it");
        assert_eq!(p.steps[0].machine, "crafter");
    }

    #[test]
    fn the_kitchen_takes_milk_from_the_storage_for_a_milk_bucket() {
        // Round 31, FL7: the cake's three milk buckets as three buckets of milk from the stock.
        let cake = r(
            "crafter",
            &[("minecraft:milk_bucket", 3), ("minecraft:sugar", 2), ("minecraft:egg", 1), ("minecraft:wheat", 3)],
            ("minecraft:cake", 1),
        );
        let k = kitchen_copy(&cake, &["minecraft:cake"]).unwrap();
        assert_eq!(k.inputs[0].0, vec!["minecraft:milk_bucket".to_owned(), fluid_key(MILK)]);
        let stock = BTreeMap::from([
            (fluid_key(MILK), 5),
            ("minecraft:sugar".to_owned(), 2),
            ("minecraft:egg".to_owned(), 1),
            ("minecraft:wheat".to_owned(), 3),
        ]);
        let p = plan("minecraft:cake", 1, &stock, &[k], &["kitchen"]);
        assert!(p.ready(), "{p:?}");
        assert_eq!(p.reserved[&fluid_key(MILK)], 3);
        assert!(!p.steps[0].items().iter().any(|(i, _)| i.contains("milk")));
    }

    fn r(machine: &str, inputs: &[(&str, u32)], output: (&str, u32)) -> PlanRecipe {
        PlanRecipe::simple(machine, inputs, output)
    }

    fn book() -> Vec<PlanRecipe> {
        vec![
            r(
                "crafter",
                &[
                    ("minecraft:oak_planks", 3),
                    ("minecraft:cobblestone", 4),
                    ("minecraft:iron_ingot", 1),
                    ("minecraft:redstone", 1),
                ],
                ("minecraft:piston", 1),
            ),
            r(
                "furnace",
                &[("rc:dust_iron", 1)],
                ("minecraft:iron_ingot", 1),
            ),
            r(
                "furnace",
                &[("rc:dust_copper", 1)],
                ("minecraft:copper_ingot", 1),
            ),
            r(
                "crafter",
                &[("minecraft:copper_ingot", 1), ("minecraft:iron_ingot", 1)],
                ("rc:coppersteel", 1),
            ),
            r(
                "alloy",
                &[
                    ("rc:coppersteel", 1),
                    ("minecraft:gold_ingot", 1),
                    ("minecraft:redstone", 1),
                ],
                ("rc:sparksteel", 1),
            ),
            r(
                "crafter",
                &[("rc:sparksteel", 2), ("minecraft:redstone", 1)],
                ("rc:module_speed", 1),
            ),
        ]
    }

    fn stock(items: &[(&str, u64)]) -> BTreeMap<ItemKey, u64> {
        items.iter().map(|(i, n)| (i.to_string(), *n)).collect()
    }

    const ALL: &[&str] = &["crafter", "furnace", "crusher", "alloy"];

    #[test]
    fn an_order_for_pistons_sets_their_ingredients_aside() {
        // Rules O3, O4: example «заказ поршня».
        let s = stock(&[
            ("minecraft:oak_planks", 20),
            ("minecraft:cobblestone", 30),
            ("minecraft:iron_ingot", 5),
            ("minecraft:redstone", 5),
        ]);
        let p = plan("minecraft:piston", 4, &s, &book(), ALL);
        assert!(p.ready());
        assert_eq!(p.steps.len(), 1);
        assert_eq!(p.steps[0].times, 4);
        assert_eq!(p.reserved["minecraft:oak_planks"], 12);
        assert_eq!(p.reserved["minecraft:cobblestone"], 16);
        assert_eq!(p.reserved["minecraft:iron_ingot"], 4);
    }

    #[test]
    fn a_short_item_is_named_with_how_many_are_missing() {
        // Rule O6: example «не хватает железа».
        let s = stock(&[
            ("minecraft:oak_planks", 20),
            ("minecraft:cobblestone", 30),
            ("minecraft:iron_ingot", 2),
            ("minecraft:redstone", 5),
        ]);
        let p = plan("minecraft:piston", 4, &s, &book(), ALL);
        assert!(!p.ready());
        // Iron could be smelted from dust, but there is none: the iron is what is short, as the
        // spec's example says («не хватает: железный слиток ×2»).
        assert_eq!(p.missing, stock(&[("minecraft:iron_ingot", 2)]));
    }

    #[test]
    fn an_order_runs_through_the_machines() {
        // Rules O9, O10: dust → furnace → crafter (coppersteel) → alloy smelter → crafter.
        let s = stock(&[
            ("rc:dust_iron", 2),
            ("rc:dust_copper", 2),
            ("minecraft:gold_ingot", 2),
            ("minecraft:redstone", 3),
        ]);
        let p = plan("rc:module_speed", 1, &s, &book(), ALL);
        assert!(p.ready(), "{p:?}");
        let machines: Vec<&str> = p.steps.iter().map(|s| s.machine.as_str()).collect();
        assert_eq!(machines.last(), Some(&"crafter"));
        let pos = |m: &str, out: &str| {
            p.steps
                .iter()
                .position(|s| s.machine == m && s.output.0 == out)
                .unwrap()
        };
        assert!(pos("furnace", "minecraft:iron_ingot") < pos("crafter", "rc:coppersteel"));
        assert!(pos("crafter", "rc:coppersteel") < pos("alloy", "rc:sparksteel"));
        assert!(pos("alloy", "rc:sparksteel") < pos("crafter", "rc:module_speed"));
        assert_eq!(p.steps[pos("alloy", "rc:sparksteel")].times, 2);
    }

    #[test]
    fn a_missing_machine_is_named() {
        // Rule O13.
        let s = stock(&[
            ("rc:coppersteel", 2),
            ("minecraft:gold_ingot", 2),
            ("minecraft:redstone", 3),
        ]);
        let p = plan("rc:module_speed", 1, &s, &book(), &["crafter", "furnace"]);
        assert_eq!(p.missing_machines, vec!["alloy".to_string()]);
        assert!(!p.ready());
    }

    #[test]
    fn stock_of_the_item_itself_is_used_first() {
        let s = stock(&[
            ("minecraft:piston", 3),
            ("minecraft:oak_planks", 3),
            ("minecraft:cobblestone", 4),
            ("minecraft:iron_ingot", 1),
            ("minecraft:redstone", 1),
        ]);
        let p = plan("minecraft:piston", 4, &s, &book(), ALL);
        assert_eq!(p.reserved["minecraft:piston"], 3);
        assert_eq!(p.steps[0].times, 1);
    }

    #[test]
    fn a_recipe_that_makes_several_rounds_up() {
        let book = vec![r(
            "crafter",
            &[("minecraft:sugar_cane", 3)],
            ("minecraft:paper", 3),
        )];
        let p = plan(
            "minecraft:paper",
            4,
            &stock(&[("minecraft:sugar_cane", 9)]),
            &book,
            ALL,
        );
        assert_eq!(p.steps[0].times, 2);
        assert_eq!(p.reserved["minecraft:sugar_cane"], 6);
    }

    #[test]
    fn a_recipe_that_needs_its_own_item_is_no_way_to_make_it() {
        // The owner: 24 item channels waited for 264 iron ingots. The planner made iron from
        // nuggets, the nuggets by smelting iron pickaxes, the pickaxes from iron: a loop, taken
        // because it gathered something (the sticks). 24 channels need 6 + 15 = 21 iron.
        let book = vec![
            r("crafter", &[("rustcraft:coppersteel", 4), ("minecraft:hopper", 1)], ("rustcraft:item_channel", 8)),
            r("crafter", &[("minecraft:copper_ingot", 1), ("minecraft:iron_ingot", 1)], ("rustcraft:coppersteel", 2)),
            r("crafter", &[("minecraft:iron_ingot", 5), ("minecraft:chest", 1)], ("minecraft:hopper", 1)),
            r("crafter", &[("minecraft:oak_planks", 8)], ("minecraft:chest", 1)),
            r("crafter", &[("minecraft:oak_planks", 2)], ("minecraft:stick", 4)),
            r("crafter", &[("minecraft:iron_nugget", 9)], ("minecraft:iron_ingot", 1)),
            r("crafter", &[("minecraft:iron_ingot", 3), ("minecraft:stick", 2)], ("minecraft:iron_pickaxe", 1)),
            r("furnace", &[("minecraft:iron_pickaxe", 1)], ("minecraft:iron_nugget", 1)),
            r("crafter", &[("minecraft:iron_block", 1)], ("minecraft:iron_ingot", 9)),
            r("crafter", &[("minecraft:iron_ingot", 9)], ("minecraft:iron_block", 1)),
            r("furnace", &[("minecraft:raw_iron", 1)], ("minecraft:iron_ingot", 1)),
        ];
        let s = stock(&[("minecraft:copper_ingot", 64), ("minecraft:oak_planks", 64)]);
        let p = plan("rustcraft:item_channel", 24, &s, &book, ALL);
        assert_eq!(p.missing, stock(&[("minecraft:iron_ingot", 21)]), "{p:?}");
        assert!(!p.steps.iter().any(|st| st.output.0 == "minecraft:iron_pickaxe"), "{:?}", p.steps);
        // With the iron in stock the order runs, and still without a pickaxe.
        let rich = stock(&[("minecraft:copper_ingot", 64), ("minecraft:oak_planks", 64), ("minecraft:iron_ingot", 21)]);
        let p = plan("rustcraft:item_channel", 24, &rich, &book, ALL);
        assert!(p.ready(), "{p:?}");
        assert_eq!(p.reserved["minecraft:iron_ingot"], 21);
    }

    #[test]
    fn ore_goes_through_the_crusher_never_whole_into_the_furnace() {
        // 0.19.2, the owner: «со склада электропечь не забирала сама руду, только пыль»: orders
        // shipped raw iron from the storage into the furnace, one ingot for one.
        let crushed: std::collections::BTreeSet<ItemKey> =
            ["minecraft:raw_iron", "minecraft:iron_ore", "minecraft:deepslate_iron_ore"].map(String::from).into();
        let ores: Vec<ItemKey> =
            ["minecraft:deepslate_iron_ore", "minecraft:iron_ore", "minecraft:raw_iron"].map(String::from).to_vec();
        assert_eq!(furnace_inputs(ores, &crushed), None, "smelting ore is no way for an order");
        let dust = vec!["rustcraft:dust_iron".to_owned()];
        assert_eq!(furnace_inputs(dust.clone(), &crushed), Some(dust));
        assert_eq!(
            furnace_inputs(vec!["minecraft:raw_iron".into(), "minecraft:sand".into()], &crushed),
            Some(vec!["minecraft:sand".to_owned()])
        );
        // The book without the ore's smelting: 10 raw iron make 20 ingots, two dust each.
        let book = vec![
            r("crusher", &[("minecraft:raw_iron", 1)], ("rustcraft:dust_iron", 2)),
            r("furnace", &[("rustcraft:dust_iron", 1)], ("minecraft:iron_ingot", 1)),
        ];
        let p = plan("minecraft:iron_ingot", 20, &stock(&[("minecraft:raw_iron", 10)]), &book, ALL);
        assert!(p.ready(), "{p:?}");
        assert_eq!(p.reserved["minecraft:raw_iron"], 10);
        assert!(p.steps.iter().any(|st| st.output.0 == "rustcraft:dust_iron"), "{:?}", p.steps);
    }

    fn planks() -> Vec<ItemKey> {
        ["minecraft:oak_planks", "minecraft:spruce_planks"]
            .map(String::from)
            .to_vec()
    }

    #[test]
    fn an_input_of_several_items_takes_the_one_in_stock() {
        // A chest takes planks of any wood: the storage holds spruce, so spruce goes.
        let cells: Vec<Option<Vec<ItemKey>>> = (0..9).map(|i| (i != 4).then(planks)).collect();
        let chest = PlanRecipe::crafting(3, &cells, ("minecraft:chest".into(), 1)).unwrap();
        assert_eq!(chest.inputs, vec![(planks(), 8)]);
        let book = vec![chest];
        let p = plan(
            "minecraft:chest",
            2,
            &stock(&[("minecraft:spruce_planks", 20)]),
            &book,
            ALL,
        );
        assert!(p.ready(), "{p:?}");
        assert_eq!(p.reserved["minecraft:spruce_planks"], 16);
        let pattern = p.steps[0].pattern(&book);
        assert_eq!(pattern[4], None);
        assert_eq!(pattern[0].as_deref(), Some("minecraft:spruce_planks"));
    }

    #[test]
    fn a_small_grid_lies_in_the_corner_of_the_crafter_grid() {
        // A 2×2 recipe: cells 0, 1, 3 and 4 of the 3×3 grid.
        let p = Some(vec!["minecraft:oak_planks".to_string()]);
        let cells = vec![p.clone(), p.clone(), p.clone(), p];
        let table =
            PlanRecipe::crafting(2, &cells, ("minecraft:crafting_table".into(), 1)).unwrap();
        let used: Vec<usize> = (0..9).filter(|&i| table.cells[i].is_some()).collect();
        assert_eq!(used, vec![0, 1, 3, 4]);
        assert_eq!(table.inputs[0].1, 4);
        assert!(PlanRecipe::crafting(4, &[None, None, None, None], ("x".into(), 1)).is_none());
        assert!(PlanRecipe::crafting(3, &[None, None], ("x".into(), 1)).is_none());
    }

    #[test]
    fn a_recipe_with_its_ingredients_in_stock_goes_before_the_first_one() {
        // Iron ingots from a block (first in the book) or from raw iron (in stock).
        let book = vec![
            r(
                "crafter",
                &[("minecraft:iron_block", 1)],
                ("minecraft:iron_ingot", 9),
            ),
            r(
                "furnace",
                &[("minecraft:raw_iron", 1)],
                ("minecraft:iron_ingot", 1),
            ),
        ];
        let p = plan(
            "minecraft:iron_ingot",
            3,
            &stock(&[("minecraft:raw_iron", 5)]),
            &book,
            ALL,
        );
        assert!(p.ready(), "{p:?}");
        assert_eq!(p.steps[0].machine, "furnace");
        assert_eq!(p.steps[0].times, 3);
    }

    #[test]
    fn of_the_ways_that_work_the_one_that_takes_least_from_the_storage_is_taken() {
        // 0.20 (round 31): gold from nuggets (first in the book) or from raw gold through the
        // crusher and the furnace: the nuggets cost 9 an ingot, the raw gold half of one.
        let book = vec![
            r("crafter", &[("minecraft:gold_nugget", 9)], ("minecraft:gold_ingot", 1)),
            r("furnace", &[("rc:dust_gold", 1)], ("minecraft:gold_ingot", 1)),
            r("crusher", &[("minecraft:raw_gold", 1)], ("rc:dust_gold", 2)),
        ];
        let s = stock(&[("minecraft:gold_nugget", 90), ("minecraft:raw_gold", 10)]);
        let p = plan("minecraft:gold_ingot", 4, &s, &book, ALL);
        assert!(p.ready(), "{p:?}");
        assert_eq!(p.reserved, stock(&[("minecraft:raw_gold", 2)]));
        assert!(p.steps.iter().all(|s| s.machine != "crafter"), "{p:?}");
        // Without a crusher on the line the nuggets are the way that works.
        let p = plan("minecraft:gold_ingot", 4, &s, &book, &["crafter", "furnace"]);
        assert!(p.ready(), "{p:?}");
        assert_eq!(p.reserved, stock(&[("minecraft:gold_nugget", 36)]));
    }

    #[test]
    fn a_shortage_deeper_in_a_chain_is_named_where_it_is() {
        // Copper comes from its dust; iron has no dust: the iron ingot is short, not coppersteel.
        let p = plan(
            "rc:coppersteel",
            1,
            &stock(&[("rc:dust_copper", 1)]),
            &book(),
            ALL,
        );
        assert_eq!(p.missing, stock(&[("minecraft:iron_ingot", 1)]));
        // Some dust in stock: the rest of the dust is what is short.
        let p = plan(
            "minecraft:iron_ingot",
            4,
            &stock(&[("rc:dust_iron", 2)]),
            &book(),
            ALL,
        );
        assert_eq!(p.missing, stock(&[("rc:dust_iron", 2)]));
    }

    #[test]
    fn a_book_that_branches_without_end_is_cut_short() {
        // 40 items, each made three ways from three others: planning ends within the budget.
        const N: usize = 40;
        let book: Vec<PlanRecipe> = (0..N)
            .flat_map(|i| {
                (1..=3).map(move |k| PlanRecipe {
                    machine: "crafter".into(),
                    inputs: (0..3)
                        .map(|j| (vec![format!("x:{}", (i + k + j) % N)], 1))
                        .collect(),
                    output: (format!("x:{i}"), 1),
                    cells: Vec::new(),
                })
            })
            .collect();
        let p = plan("x:0", 1, &BTreeMap::new(), &book, ALL);
        assert!(!p.ready());
    }

    fn solution_book() -> Vec<PlanRecipe> {
        vec![r(
            "mixer",
            &[
                ("minecraft:bone_meal", 1),
                ("minecraft:redstone", 1),
                ("fluid:minecraft:water", 1),
            ],
            ("fluid:rustcraft:solution", 1),
        )]
    }

    #[test]
    fn a_fluid_order_takes_water_from_the_line() {
        // Rule O12: the water of the line's tanks is the stock of water, in buckets.
        let s = stock(&[
            ("minecraft:bone_meal", 5),
            ("minecraft:redstone", 5),
            ("fluid:minecraft:water", 8),
        ]);
        let p = plan(
            "fluid:rustcraft:solution",
            3,
            &s,
            &solution_book(),
            &["mixer"],
        );
        assert!(p.ready(), "{p:?}");
        assert_eq!(p.steps[0].machine, "mixer");
        assert_eq!(p.steps[0].times, 3);
        assert_eq!(p.reserved["fluid:minecraft:water"], 3);
        assert_eq!(
            p.steps[0].items(),
            vec![
                ("minecraft:bone_meal".to_string(), 1),
                ("minecraft:redstone".to_string(), 1)
            ]
        );
    }

    #[test]
    fn missing_water_is_named() {
        // Rule O12: «не хватает: вода ×1».
        let s = stock(&[
            ("minecraft:bone_meal", 5),
            ("minecraft:redstone", 5),
            ("fluid:minecraft:water", 1),
        ]);
        let p = plan(
            "fluid:rustcraft:solution",
            2,
            &s,
            &solution_book(),
            &["mixer"],
        );
        assert_eq!(p.missing, stock(&[("fluid:minecraft:water", 1)]));
        // No mixer on the line: the machine is named (O13).
        let p = plan(
            "fluid:rustcraft:solution",
            1,
            &s,
            &solution_book(),
            &["crafter"],
        );
        assert_eq!(p.missing_machines, vec!["mixer".to_string()]);
    }

    #[test]
    fn fluids_have_keys_and_numbers_of_their_own() {
        assert_eq!(fluid_key("minecraft:water"), "fluid:minecraft:water");
        assert_eq!(
            fluid_of("fluid:rustcraft:solution"),
            Some("rustcraft:solution")
        );
        assert_eq!(fluid_of("minecraft:water_bucket"), None);
        for f in FLUIDS {
            assert_eq!(fluid_number(f).and_then(fluid_of_number), Some(f));
        }
        assert_eq!(fluid_number("minecraft:milk"), None);
        assert_eq!(fluid_number("rustcraft:milk"), Some(FLUID_NUMBERS + 6));
        assert_eq!(fluid_of_number(812), None);
        assert!(FLUID_NUMBERS + (FLUIDS.len() as u16) < 0xFFFF);
    }

    #[test]
    fn a_recipe_loop_ends_as_missing() {
        // Ingot and block make each other; neither is in stock.
        let book = vec![
            r(
                "crafter",
                &[("minecraft:iron_block", 1)],
                ("minecraft:iron_ingot", 9),
            ),
            r(
                "crafter",
                &[("minecraft:iron_ingot", 9)],
                ("minecraft:iron_block", 1),
            ),
        ];
        let p = plan("minecraft:iron_ingot", 3, &BTreeMap::new(), &book, ALL);
        assert!(!p.ready());
        assert_eq!(p.missing.len(), 1);
    }
}
