//! Orders from the storage terminal (spec section 19). An order for an item is broken down into
//! recipes until it reaches what the storage holds (O3); those items are set aside for it. A step
//! may run on any machine of the mod on the line, not only the autocrafter (O9, O10). When an item
//! is short, the order says which and how many (O6); when no machine on the line can run a step,
//! it says which machine is missing (O13).

use std::collections::BTreeMap;

use crate::net::items::ItemKey;

/// A recipe any machine of the mod runs, as the recipe book knows it (O1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanRecipe {
    /// The machine kind that runs it: "crafter", "furnace", "crusher", "alloy", "mixer"…
    pub machine: String,
    pub inputs: Vec<(ItemKey, u32)>,
    pub output: (ItemKey, u32),
}

/// One step of an order: run `recipe` `times` times.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    pub recipe: PlanRecipe,
    pub times: u32,
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
    };
    planner.need(item, count);
    planner.plan
}

struct Planner<'a> {
    /// What the storage still has free, plus what earlier steps make beyond their need.
    stock: BTreeMap<ItemKey, u64>,
    book: &'a [PlanRecipe],
    machines: &'a [&'a str],
    plan: Plan,
    /// Items being broken down right now, so a recipe loop ends instead of recursing forever.
    making: Vec<ItemKey>,
}

impl Planner<'_> {
    /// Arranges `qty` of `item`: from stock first, then by a recipe, else it is missing.
    fn need(&mut self, item: &str, qty: u64) {
        let have = self.stock.get(item).copied().unwrap_or(0);
        let take = have.min(qty);
        if take > 0 {
            *self.stock.get_mut(item).expect("have > 0") -= take;
            *self.plan.reserved.entry(item.to_string()).or_insert(0) += take;
        }
        let rest = qty - take;
        if rest == 0 {
            return;
        }
        let recipes: Vec<&PlanRecipe> = self.book.iter().filter(|r| r.output.0 == item).collect();
        if recipes.is_empty() || self.making.iter().any(|m| m == item) {
            *self.plan.missing.entry(item.to_string()).or_insert(0) += rest;
            return;
        }
        // A recipe a machine on the line can run; otherwise the first one, and its machine is missing.
        let recipe = recipes
            .iter()
            .find(|r| self.machines.contains(&r.machine.as_str()))
            .copied()
            .unwrap_or_else(|| {
                if !self.plan.missing_machines.contains(&recipes[0].machine) {
                    self.plan.missing_machines.push(recipes[0].machine.clone());
                }
                recipes[0]
            });
        let per_craft = u64::from(recipe.output.1.max(1));
        let times = rest.div_ceil(per_craft);
        self.making.push(item.to_string());
        for (ingredient, n) in &recipe.inputs {
            self.need(ingredient, u64::from(*n) * times);
        }
        self.making.pop();
        // What the crafts make beyond the need is free for later needs.
        let surplus = times * per_craft - rest;
        if surplus > 0 {
            *self.stock.entry(item.to_string()).or_insert(0) += surplus;
        }
        let times = u32::try_from(times).unwrap_or(u32::MAX);
        match self.plan.steps.iter_mut().find(|s| s.recipe == *recipe) {
            Some(step) => step.times = step.times.saturating_add(times),
            None => self.plan.steps.push(Step {
                recipe: recipe.clone(),
                times,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(machine: &str, inputs: &[(&str, u32)], output: (&str, u32)) -> PlanRecipe {
        PlanRecipe {
            machine: machine.into(),
            inputs: inputs.iter().map(|(i, n)| (i.to_string(), *n)).collect(),
            output: (output.0.into(), output.1),
        }
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
        // Iron could come from dust, but there is none: the dust is what is short.
        assert_eq!(p.missing, stock(&[("rc:dust_iron", 2)]));
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
        let machines: Vec<&str> = p.steps.iter().map(|s| s.recipe.machine.as_str()).collect();
        assert_eq!(machines.last(), Some(&"crafter"));
        let pos = |m: &str, out: &str| {
            p.steps
                .iter()
                .position(|s| s.recipe.machine == m && s.recipe.output.0 == out)
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
        // Coppersteel is made one at a time here, so 2 sparksteel need 2 crafts of coppersteel.
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
}
