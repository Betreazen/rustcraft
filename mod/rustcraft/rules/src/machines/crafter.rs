//! The autocrafter: pages, each holding one recipe (C1). It crafts a page whose ingredients are in
//! its buffer (C3), going round the pages when several can run (C3.a), and stops with a diagnosis
//! when its output has no room (C4). A temporary page from an order goes before the ordinary pages
//! and disappears once its count is crafted (O4). The work mode (A1) scales its speed and energy.

use crate::machines::processor::{State, WorkMode};
use crate::machines::slots::{self, Slot};
use crate::net::items::ItemKey;

/// One thing a craft gives: the result, or a remainder such as an empty bucket.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Product {
    pub item: ItemKey,
    pub count: u32,
    /// How many of it one slot holds.
    pub stack: u32,
}

/// What a page crafts: the ingredients one craft takes, and what it gives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CraftRecipe {
    pub ingredients: Vec<(ItemKey, u32)>,
    /// The result first, then the remainders.
    pub products: Vec<Product>,
}

impl CraftRecipe {
    /// The recipe of a 3×3 pattern whose result the game's crafting table gives: one of each
    /// cell's item, summed by item.
    pub fn of_pattern(cells: &[Option<ItemKey>], products: Vec<Product>) -> CraftRecipe {
        let mut ingredients: Vec<(ItemKey, u32)> = Vec::new();
        for item in cells.iter().flatten() {
            match ingredients.iter_mut().find(|(i, _)| i == item) {
                Some((_, n)) => *n += 1,
                None => ingredients.push((item.clone(), 1)),
            }
        }
        CraftRecipe {
            ingredients,
            products,
        }
    }
}

/// A page of the autocrafter. Its recipe is none while its pattern makes nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page {
    pub recipe: Option<CraftRecipe>,
    /// O4: a temporary page from an order, with how many crafts it still owes.
    pub order: Option<u32>,
}

/// The fixed numbers of the autocrafter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CrafterSpec {
    /// Ticks one craft takes in normal mode at full energy.
    pub ticks: f64,
    /// E a second in normal mode.
    pub energy: f64,
}

/// The autocrafter's contents.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Crafter {
    pub pages: Vec<Page>,
    /// Ingredients the net brought, by item.
    pub buffer: Vec<(ItemKey, u32)>,
    /// Its output slots.
    pub output: Vec<Slot>,
    /// The ordinary page to try first next time (C3.a).
    pub next_page: usize,
    /// The page being crafted and the work done on it.
    pub current: Option<usize>,
    pub progress: f64,
    pub mode: WorkMode,
}

impl Crafter {
    /// The E a second it asks for while it has a page it can craft and room for what it gives.
    pub fn request(&self, spec: &CrafterSpec) -> f64 {
        match self.pick() {
            Some(p) if self.room_for(p) => spec.energy * self.mode.factors().1,
            _ => 0.0,
        }
    }

    /// One tick with `share` of the requested energy.
    pub fn tick(&mut self, spec: &CrafterSpec, share: f64) -> State {
        let Some(p) = self.pick() else {
            self.current = None;
            self.progress = 0.0;
            return State::Idle;
        };
        if !self.room_for(p) {
            return State::OutputFull;
        }
        if share <= 0.0 {
            return State::NoEnergy;
        }
        if self.current != Some(p) {
            self.current = Some(p);
            self.progress = 0.0;
        }
        self.progress += self.mode.factors().0 * share.min(1.0) / spec.ticks;
        if self.progress >= 1.0 - 1e-9 {
            self.craft(p);
        }
        State::Working
    }

    /// The page to craft: the current one while it can still run, else an order page, else the
    /// next ordinary page in turn that can run.
    pub fn pick(&self) -> Option<usize> {
        if let Some(c) = self
            .current
            .filter(|&c| c < self.pages.len() && self.can_craft(c))
        {
            return Some(c);
        }
        if let Some(o) =
            (0..self.pages.len()).find(|&i| self.pages[i].order.is_some() && self.can_craft(i))
        {
            return Some(o);
        }
        let n = self.pages.len();
        (0..n)
            .map(|k| (self.next_page + k) % n)
            .find(|&i| self.pages[i].order.is_none() && self.can_craft(i))
    }

    fn can_craft(&self, page: usize) -> bool {
        self.pages[page].recipe.as_ref().is_some_and(|r| {
            !r.products.is_empty() && r.ingredients.iter().all(|(i, n)| self.has(i) >= *n)
        })
    }

    /// Whether the output slots have room for everything a craft of `page` gives.
    fn room_for(&self, page: usize) -> bool {
        let Some(recipe) = &self.pages[page].recipe else {
            return false;
        };
        let mut after = self.output.clone();
        recipe
            .products
            .iter()
            .all(|p| slots::place(&mut after, &p.item, p.count, p.stack))
    }

    /// Finishes a craft of `page`: takes the ingredients, adds the products, counts down an order.
    fn craft(&mut self, page: usize) {
        let Some(recipe) = self.pages[page].recipe.clone() else {
            return;
        };
        for (item, n) in &recipe.ingredients {
            let mut left = *n;
            for (i, c) in self.buffer.iter_mut() {
                if i == item && left > 0 {
                    let t = (*c).min(left);
                    *c -= t;
                    left -= t;
                }
            }
        }
        self.buffer.retain(|(_, c)| *c > 0);
        for p in &recipe.products {
            slots::place(&mut self.output, &p.item, p.count, p.stack);
        }
        self.progress = 0.0;
        self.current = None;
        match self.pages[page].order.as_mut() {
            Some(owed) => {
                *owed -= 1;
                if *owed == 0 {
                    self.pages.remove(page);
                    if self.next_page > page {
                        self.next_page -= 1;
                    }
                }
            }
            None => self.next_page = (page + 1) % self.pages.len(),
        }
    }

    /// How many of `item` the buffer holds.
    pub fn has(&self, item: &str) -> u32 {
        self.buffer
            .iter()
            .filter(|(i, _)| i == item)
            .map(|(_, n)| n)
            .sum()
    }

    /// Every ingredient its pages take: what the net brings it (M17).
    pub fn wants(&self) -> Vec<ItemKey> {
        let mut out: Vec<ItemKey> = Vec::new();
        for (item, _) in self
            .pages
            .iter()
            .filter_map(|p| p.recipe.as_ref())
            .flat_map(|r| r.ingredients.iter())
        {
            if !out.contains(item) {
                out.push(item.clone());
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC: CrafterSpec = CrafterSpec {
        ticks: 20.0,
        energy: 8.0,
    };

    fn product(item: &str, count: u32) -> Vec<Product> {
        vec![Product {
            item: item.into(),
            count,
            stack: 64,
        }]
    }

    fn paper() -> CraftRecipe {
        CraftRecipe {
            ingredients: vec![("minecraft:sugar_cane".into(), 3)],
            products: product("minecraft:paper", 3),
        }
    }

    fn book() -> CraftRecipe {
        CraftRecipe {
            ingredients: vec![
                ("minecraft:paper".into(), 3),
                ("minecraft:leather".into(), 1),
            ],
            products: product("minecraft:book", 1),
        }
    }

    fn piston() -> CraftRecipe {
        CraftRecipe {
            ingredients: vec![
                ("minecraft:oak_planks".into(), 3),
                ("minecraft:cobblestone".into(), 4),
                ("minecraft:iron_ingot".into(), 1),
                ("minecraft:redstone".into(), 1),
            ],
            products: product("minecraft:piston", 1),
        }
    }

    fn page(recipe: CraftRecipe) -> Page {
        Page {
            recipe: Some(recipe),
            order: None,
        }
    }

    fn crafter(pages: Vec<Page>, buffer: &[(&str, u32)]) -> Crafter {
        Crafter {
            pages,
            buffer: buffer.iter().map(|(i, n)| ((*i).into(), *n)).collect(),
            output: vec![None; 3],
            ..Crafter::default()
        }
    }

    fn run(c: &mut Crafter, ticks: u32) -> State {
        let mut last = State::Idle;
        for _ in 0..ticks {
            last = c.tick(&SPEC, 1.0);
        }
        last
    }

    fn out(c: &Crafter, item: &str) -> u32 {
        slots::count_in(&c.output, item)
    }

    #[test]
    fn a_pattern_counts_its_cells_by_item() {
        let s = |i: &str| Some(i.to_owned());
        let cells = [
            s("minecraft:oak_planks"),
            s("minecraft:oak_planks"),
            s("minecraft:oak_planks"),
            s("minecraft:cobblestone"),
            s("minecraft:iron_ingot"),
            s("minecraft:cobblestone"),
            s("minecraft:cobblestone"),
            s("minecraft:redstone"),
            s("minecraft:cobblestone"),
        ];
        let r = CraftRecipe::of_pattern(&cells, product("minecraft:piston", 1));
        assert_eq!(r, piston());
        let empty = CraftRecipe::of_pattern(&[None, None], Vec::new());
        assert!(empty.ingredients.is_empty());
    }

    #[test]
    fn it_crafts_a_page_when_the_ingredients_are_in() {
        // Rule C3.
        let mut c = crafter(vec![page(paper())], &[("minecraft:sugar_cane", 6)]);
        assert_eq!(c.request(&SPEC), 8.0);
        run(&mut c, 20);
        assert_eq!(out(&c, "minecraft:paper"), 3);
        assert_eq!(c.has("minecraft:sugar_cane"), 3);
        assert_eq!(c.wants(), ["minecraft:sugar_cane"]);
    }

    #[test]
    fn without_ingredients_it_waits_and_asks_for_nothing() {
        let mut c = crafter(vec![page(paper())], &[("minecraft:sugar_cane", 2)]);
        assert_eq!(c.request(&SPEC), 0.0);
        assert_eq!(run(&mut c, 5), State::Idle);
        // A page whose pattern makes nothing is never crafted.
        let mut blank = crafter(
            vec![Page {
                recipe: None,
                order: None,
            }],
            &[("minecraft:sugar_cane", 9)],
        );
        assert_eq!(run(&mut blank, 5), State::Idle);
    }

    #[test]
    fn it_goes_round_the_pages_it_can_craft() {
        // Rule C3.a.
        let mut c = crafter(
            vec![page(paper()), page(book())],
            &[
                ("minecraft:sugar_cane", 6),
                ("minecraft:paper", 6),
                ("minecraft:leather", 2),
            ],
        );
        run(&mut c, 20);
        assert_eq!(c.current, None);
        assert_eq!(c.next_page, 1);
        run(&mut c, 20);
        assert_eq!(out(&c, "minecraft:paper"), 3);
        assert_eq!(out(&c, "minecraft:book"), 1);
        run(&mut c, 40);
        assert_eq!(out(&c, "minecraft:paper"), 6);
        assert_eq!(out(&c, "minecraft:book"), 2);
    }

    #[test]
    fn a_full_output_stops_it() {
        // Rule C4.
        let mut c = crafter(vec![page(paper())], &[("minecraft:sugar_cane", 6)]);
        c.output = vec![
            Some(("a".into(), 64)),
            Some(("b".into(), 64)),
            Some(("minecraft:paper".into(), 62)),
        ];
        assert_eq!(c.request(&SPEC), 0.0);
        assert_eq!(run(&mut c, 5), State::OutputFull);
        assert_eq!(c.has("minecraft:sugar_cane"), 6);
    }

    #[test]
    fn remainders_need_room_too() {
        // A cake gives back three buckets; a cake stacks to one.
        let cake = CraftRecipe {
            ingredients: vec![("minecraft:milk_bucket".into(), 3)],
            products: vec![
                Product {
                    item: "minecraft:cake".into(),
                    count: 1,
                    stack: 1,
                },
                Product {
                    item: "minecraft:bucket".into(),
                    count: 3,
                    stack: 16,
                },
            ],
        };
        let mut c = crafter(vec![page(cake)], &[("minecraft:milk_bucket", 6)]);
        run(&mut c, 20);
        assert_eq!(out(&c, "minecraft:cake"), 1);
        assert_eq!(out(&c, "minecraft:bucket"), 3);
        // The second cake needs a slot of its own; the buckets stack onto the first three.
        run(&mut c, 20);
        assert_eq!(out(&c, "minecraft:cake"), 2);
        assert_eq!(run(&mut c, 5), State::Idle);
        c.buffer.push(("minecraft:milk_bucket".into(), 3));
        assert_eq!(run(&mut c, 5), State::OutputFull);
    }

    #[test]
    fn an_order_page_goes_first_and_disappears_when_done() {
        // Rule O4: 2 pistons ordered; the ordinary paper page waits until the order is done.
        let mut c = crafter(
            vec![
                page(paper()),
                Page {
                    recipe: Some(piston()),
                    order: Some(2),
                },
            ],
            &[
                ("minecraft:sugar_cane", 3),
                ("minecraft:oak_planks", 6),
                ("minecraft:cobblestone", 8),
                ("minecraft:iron_ingot", 2),
                ("minecraft:redstone", 2),
            ],
        );
        run(&mut c, 40);
        assert_eq!(out(&c, "minecraft:piston"), 2);
        assert_eq!(out(&c, "minecraft:paper"), 0);
        assert_eq!(c.pages.len(), 1);
        run(&mut c, 20);
        assert_eq!(out(&c, "minecraft:paper"), 3);
    }

    #[test]
    fn half_the_energy_crafts_half_as_fast_and_boost_twice_as_fast() {
        let mut c = crafter(vec![page(paper())], &[("minecraft:sugar_cane", 3)]);
        for _ in 0..20 {
            c.tick(&SPEC, 0.5);
        }
        assert_eq!(out(&c, "minecraft:paper"), 0);
        for _ in 0..20 {
            c.tick(&SPEC, 0.5);
        }
        assert_eq!(out(&c, "minecraft:paper"), 3);
        let mut fast = crafter(vec![page(paper())], &[("minecraft:sugar_cane", 3)]);
        fast.mode = WorkMode::Boost;
        assert_eq!(fast.request(&SPEC), 24.0);
        run(&mut fast, 10);
        assert_eq!(out(&fast, "minecraft:paper"), 3);
    }
}
