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

#[cfg(test)]
mod tests {
    use super::*;

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
