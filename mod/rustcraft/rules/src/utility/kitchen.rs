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

/// With a stack module (`stacked`) the kitchen's inputs hold `size(item)` each (256 for what
/// stacks to 64): what the visible input slots hold moves into hidden cells, one a slot, so the
/// slots take more; without one, the cells give back into the slots as far as they take
/// (`queue::spill`). What a cell and a slot hold together is what the kitchen cooks from.
pub fn stash(
    visible: &mut [crate::machines::slots::Slot],
    cells: &mut Vec<crate::machines::slots::Slot>,
    stacked: bool,
    size: impl Fn(&str) -> u32,
    max: impl Fn(&str) -> u32,
) {
    let n = visible.len().min(INPUTS);
    crate::machines::queue::stash(&mut visible[..n], cells, stacked, size, max);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stack_module_lets_each_input_hold_256() {
        let wheat = "minecraft:wheat";
        let mut visible: Vec<crate::machines::slots::Slot> = vec![None; INPUTS];
        let mut cells = Vec::new();
        visible[0] = Some((wheat.into(), 64));
        stash(&mut visible, &mut cells, true, |_| 256, |_| 64);
        assert_eq!(visible[0], None, "the slot takes more");
        assert_eq!(crate::machines::slots::count_in(&cells, wheat), 64);
        for _ in 0..4 {
            visible[0] = Some((wheat.into(), 64));
            stash(&mut visible, &mut cells, true, |_| 256, |_| 64);
        }
        // 5 × 64 = 320: the first cell holds 256, the rest goes on in the next cell.
        assert_eq!(crate::machines::slots::count_in(&cells, wheat), 320);
        assert_eq!(cells[0], Some((wheat.into(), 256)));
        // The module taken out: the cells give back as far as the slots take.
        stash(&mut visible, &mut cells, false, |_| 256, |_| 64);
        let total = crate::machines::slots::count_in(&cells, wheat) + crate::machines::slots::count_in(&visible, wheat);
        assert_eq!(total, 320, "nothing lost");
        assert_eq!(crate::machines::slots::count_in(&visible, wheat), 320, "all back in five slots");
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
