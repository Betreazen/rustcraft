//! Slice 18 (guide `kitchen`): the kitchen's pages and its milk. «На каждой странице одно блюдо.
//! Его выбирают из списка рецептов еды… Страницы работают по очереди.» One page, and one more for
//! each capacity module. A recipe that asks for a bucket of milk takes a bucket's worth from the
//! tank the kitchen touches, and no bucket is needed or left over.

use crate::utility::KITCHEN_RECIPES;

/// The most pages: one and one a capacity module, three sockets.
pub const PAGES_MAX: usize = 4;
/// The fluid of milk in a tank.
pub const MILK: &str = "rustcraft:milk";
/// One bucket, in millibuckets.
pub const BUCKET_MB: u64 = 1000;
pub const MILK_BUCKET: &str = "minecraft:milk_bucket";

/// The pages a kitchen with `capacity` capacity modules has.
pub fn pages(capacity: u32) -> usize {
    (1 + capacity as usize).min(PAGES_MAX)
}

/// A press on a page's toggle: its dish becomes the next of the list, after the last none.
pub fn cycle(dish: Option<usize>) -> Option<usize> {
    match dish {
        None => Some(0),
        Some(i) if i + 1 < KITCHEN_RECIPES.len() => Some(i + 1),
        Some(_) => None,
    }
}

/// The dishes to try this step, each page in turn from `next` (the page after the last one that
/// cooked), only the `usable` first pages, empty pages left out: (page, dish).
pub fn in_turn(pages: &[Option<usize>], usable: usize, next: usize) -> Vec<(usize, usize)> {
    let n = usable.min(pages.len());
    (0..n)
        .map(|k| (next + k) % n.max(1))
        .filter_map(|p| pages.get(p).copied().flatten().map(|d| (p, d)))
        .filter(|(_, d)| *d < KITCHEN_RECIPES.len())
        .collect()
}

/// How many buckets of milk a recipe's ingredients ask for.
pub fn milk_buckets(need: &[(String, u32)]) -> u32 {
    need.iter().filter(|(i, _)| i == MILK_BUCKET).map(|(_, n)| *n).sum()
}

/// Slice 18 (cross, guide `mod_stack`): the input slots the kitchen has, each a hidden cell of
/// its own with a stack module.
pub const INPUTS: usize = 9;

/// Wave 2 (guide `mod_cap`: «+2 ячейки очереди за модуль»; `kitchen`: «больше ячеек под
/// ингредиенты»): the hidden cells a group of `slots` input slots has open: one a slot with a
/// stack module (`stacked`), and the `queue` cells its capacity modules give
/// (`utility::queue_cells`, two a module).
pub fn open_cells(slots: usize, stacked: bool, queue: usize) -> usize {
    (if stacked { slots } else { 0 }) + queue
}

/// What the visible slots hold moves into the first `open` hidden cells, `size(item)` a cell (256
/// for what stacks to 64 with a stack module, its stack without), so the slots take more; cells
/// past `open` (a module taken out) give back into the slots as far as they take
/// (`queue::spill`). What the cells and the slots hold together is what the machine works from.
pub fn stash_open(
    visible: &mut [crate::machines::slots::Slot],
    cells: &mut Vec<crate::machines::slots::Slot>,
    open: usize,
    size: impl Fn(&str) -> u32,
    max: impl Fn(&str) -> u32,
) {
    use crate::machines::queue::{intake, spill};
    for slot in visible.iter_mut() {
        if open > 0
            && let Some((item, _)) = slot.clone()
        {
            intake(slot, cells, open, size(&item));
        }
        spill(slot, cells, open, &max);
    }
}

/// With a stack module (`stacked`) the kitchen's inputs hold `size(item)` each (256 for what
/// stacks to 64), and its capacity modules add `queue` hidden cells: what the visible
/// input slots hold moves into the hidden cells, so the slots take more; without them, the cells
/// give back into the slots as far as they take. What a cell and a slot hold together is what the
/// kitchen cooks from.
pub fn stash(
    visible: &mut [crate::machines::slots::Slot],
    cells: &mut Vec<crate::machines::slots::Slot>,
    stacked: bool,
    queue: usize,
    size: impl Fn(&str) -> u32,
    max: impl Fn(&str) -> u32,
) {
    let n = visible.len().min(INPUTS);
    stash_open(&mut visible[..n], cells, open_cells(n, stacked, queue), size, max);
}

/// w2: what a dish takes, each of `need` from the hidden `cells` first; what the visible slots
/// must still give, item by item.
pub fn take_ingredients(cells: &mut [crate::machines::slots::Slot], need: &[(String, u32)]) -> Vec<u32> {
    need.iter()
        .map(|(item, n)| crate::machines::slots::take_from(cells, item, *n))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ingredients_come_from_the_cells_first_then_the_slots() {
        let wheat = "minecraft:wheat";
        let need = vec![(wheat.to_owned(), 3)];
        let mut cells = vec![Some((wheat.to_owned(), 1))];
        assert_eq!(take_ingredients(&mut cells, &need), vec![2], "two still from the slots");
        assert_eq!(cells, vec![None]);
        let mut none: Vec<crate::machines::slots::Slot> = Vec::new();
        assert_eq!(take_ingredients(&mut none, &need), vec![3], "all from the slots");
    }

    #[test]
    fn a_stack_module_lets_each_input_hold_256() {
        let wheat = "minecraft:wheat";
        let mut visible: Vec<crate::machines::slots::Slot> = vec![None; INPUTS];
        let mut cells = Vec::new();
        visible[0] = Some((wheat.into(), 64));
        stash(&mut visible, &mut cells, true, 0, |_| 256, |_| 64);
        assert_eq!(visible[0], None, "the slot takes more");
        assert_eq!(crate::machines::slots::count_in(&cells, wheat), 64);
        for _ in 0..4 {
            visible[0] = Some((wheat.into(), 64));
            stash(&mut visible, &mut cells, true, 0, |_| 256, |_| 64);
        }
        // 5 × 64 = 320: the first cell holds 256, the rest goes on in the next cell.
        assert_eq!(crate::machines::slots::count_in(&cells, wheat), 320);
        assert_eq!(cells[0], Some((wheat.into(), 256)));
        // The module taken out: the cells give back as far as the slots take.
        stash(&mut visible, &mut cells, false, 0, |_| 256, |_| 64);
        let total = crate::machines::slots::count_in(&cells, wheat) + crate::machines::slots::count_in(&visible, wheat);
        assert_eq!(total, 320, "nothing lost");
        assert_eq!(crate::machines::slots::count_in(&visible, wheat), 320, "all back in five slots");
    }

    #[test]
    fn a_capacity_module_gives_two_hidden_cells() {
        // Wave 2: no stack module, one capacity module: two cells of a stack each.
        let wheat = "minecraft:wheat";
        assert_eq!(open_cells(INPUTS, false, 2), 2);
        assert_eq!(open_cells(INPUTS, true, 4), INPUTS + 4);
        let mut visible: Vec<crate::machines::slots::Slot> = vec![None; INPUTS];
        let mut cells = Vec::new();
        for _ in 0..3 {
            visible[0] = Some((wheat.into(), 64));
            stash(&mut visible, &mut cells, false, 2, |_| 64, |_| 64);
        }
        assert_eq!(crate::machines::slots::count_in(&cells, wheat), 128, "two cells of 64");
        assert_eq!(visible[0], Some((wheat.into(), 64)), "the third stack stays in its slot");
        // The module taken out: they give back into the slots.
        stash(&mut visible, &mut cells, false, 0, |_| 64, |_| 64);
        let back = crate::machines::slots::count_in(&visible, wheat);
        assert_eq!(back + crate::machines::slots::count_in(&cells, wheat), 192, "nothing lost");
        assert_eq!(back, 192);
    }

    #[test]
    fn a_page_a_capacity_module() {
        assert_eq!(pages(0), 1);
        assert_eq!(pages(2), 3);
        assert_eq!(pages(9), PAGES_MAX);
    }

    #[test]
    fn a_page_cycles_through_the_dishes() {
        assert_eq!(cycle(None), Some(0));
        assert_eq!(cycle(Some(0)), Some(1));
        assert_eq!(cycle(Some(KITCHEN_RECIPES.len() - 1)), None);
    }

    #[test]
    fn the_pages_work_in_turn() {
        let pages = [Some(0), None, Some(3), Some(1)];
        assert_eq!(in_turn(&pages, 4, 0), vec![(0, 0), (2, 3), (3, 1)]);
        assert_eq!(in_turn(&pages, 4, 2), vec![(2, 3), (3, 1), (0, 0)]);
        assert_eq!(in_turn(&pages, 1, 3), vec![(0, 0)], "only the pages it has");
        assert!(in_turn(&[], 1, 0).is_empty());
    }

    #[test]
    fn milk_is_counted_in_buckets() {
        let cake = vec![(MILK_BUCKET.to_owned(), 3), ("minecraft:sugar".to_owned(), 2)];
        assert_eq!(milk_buckets(&cake), 3);
        assert_eq!(milk_buckets(&[]), 0);
    }
}
