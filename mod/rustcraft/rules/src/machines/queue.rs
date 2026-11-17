//! Slice 18: the capacity and stack modules in a machine (guide `mod_cap`, `mod_stack`).
//!
//! A menu's slot holds at most 99 and never more than the item's own stack
//! (SRC/docs/native-mods.md:4617-4627), so a cell of 256 cannot be a real slot. A machine keeps its
//! extra cells in its block entity instead, and its window draws them:
//!
//! - **The queue.** Behind a machine's input slot, which stays its intake, lie hidden cells: two a
//!   capacity module, and one more with a stack module (the input's own cell of 256). Each holds
//!   [`crate::modules::machine_stack`] of its item. Every tick the intake moves into them; the
//!   machine works from the first cell with work in it, then from the intake. A cell that closes
//!   (a module taken out) only gives: what it holds goes back to the intake as room allows.
//! - **The reserve.** With a stack module the result slot holds 256 too: the real slot shows up
//!   to the item's stack, the rest waits in a reserve that refills it.

use super::slots::Slot;

/// The hidden queue cells a machine has open: two a capacity slot pair (`capacity_slots`, from
/// `modules::capacity_slots`), one more with a stack module.
pub fn open_cells(capacity_slots: usize, stacked: bool) -> usize {
    capacity_slots + usize::from(stacked)
}

/// Moves what `intake` holds into the first `open` cells: onto cells of its item with room first,
/// then into empty ones, each up to `size`. What does not fit stays in the intake.
pub fn intake(intake: &mut Slot, cells: &mut Vec<Slot>, open: usize, size: u32) {
    let Some((item, mut left)) = intake.take() else {
        return;
    };
    if cells.len() < open {
        cells.resize(open, None);
    }
    for pass in 0..2 {
        for cell in cells.iter_mut().take(open) {
            if left == 0 {
                break;
            }
            match cell {
                Some((i, n)) if pass == 0 && *i == item && *n < size => {
                    let put = (size - *n).min(left);
                    *n += put;
                    left -= put;
                }
                None if pass == 1 => {
                    let put = size.min(left);
                    *cell = Some((item.clone(), put));
                    left -= put;
                }
                _ => {}
            }
        }
    }
    if left > 0 {
        *intake = Some((item, left));
    }
}

/// Cells past `open` give back into `intake` (empty, or of their item) up to the item's stack
/// `max`; empty cells at the end are dropped.
pub fn spill(intake: &mut Slot, cells: &mut Vec<Slot>, open: usize, max: impl Fn(&str) -> u32) {
    for cell in cells.iter_mut().skip(open) {
        let Some((item, n)) = cell.as_mut() else { continue };
        let max = max(item);
        let room = match intake {
            None => max,
            Some((i, c)) if i == item => max.saturating_sub(*c),
            Some(_) => 0,
        };
        let give = room.min(*n);
        if give == 0 {
            continue;
        }
        match intake {
            Some((_, c)) => *c += give,
            None => *intake = Some((item.clone(), give)),
        }
        *n -= give;
        if *n == 0 {
            *cell = None;
        }
    }
    while cells.len() > open && cells.last().is_some_and(Option::is_none) {
        cells.pop();
    }
}

/// Slice 18 (guide `mod_stack`): with a stack module (`stacked`) each of `visible` (a machine's
/// input slots) has a hidden cell of `size(item)` (256 for what stacks to 64): what a slot holds
/// moves into its cells, so the slot takes more; without one, the cells give back into the slots
/// as far as they take (`spill`, `max` of an item a slot). What a slot and the cells hold together
/// is what the machine works from.
pub fn stash(
    visible: &mut [Slot],
    cells: &mut Vec<Slot>,
    stacked: bool,
    size: impl Fn(&str) -> u32,
    max: impl Fn(&str) -> u32,
) {
    let open = if stacked { visible.len() } else { 0 };
    for slot in visible.iter_mut() {
        if stacked {
            if let Some((item, _)) = slot.clone() {
                intake(slot, cells, open, size(&item));
            }
        } else {
            spill(slot, cells, open, &max);
        }
    }
}

/// The cell the machine works from: the first one holding an item `works` takes; none means the
/// intake itself.
pub fn front(cells: &[Slot], works: impl Fn(&str) -> bool) -> Option<usize> {
    cells
        .iter()
        .position(|c| c.as_ref().is_some_and(|(i, n)| *n > 0 && works(i)))
}

/// A result slot and its reserve together.
pub fn join(visible: &Slot, reserve: &Slot) -> Slot {
    match (visible, reserve) {
        (Some((i, a)), Some((j, b))) if i == j => Some((i.clone(), a + b)),
        (Some(v), _) => Some(v.clone()),
        (None, r) => r.clone(),
    }
}

/// `total` of an item that stacks to `max` split into the real slot and the reserve.
pub fn split(total: &Slot, max: u32) -> (Slot, Slot) {
    match total {
        None => (None, None),
        Some((item, n)) => {
            let shown = (*n).min(max.max(1));
            let rest = n - shown;
            (Some((item.clone(), shown)), (rest > 0).then(|| (item.clone(), rest)))
        }
    }
}

/// Everything the cells hold, for a machine broken: (item, count) stacks of at most `max` each.
pub fn spilled(cells: &[Slot], max: impl Fn(&str) -> u32) -> Vec<(String, u32)> {
    let mut out = Vec::new();
    for (item, n) in cells.iter().flatten() {
        let m = max(item).max(1);
        let mut left = *n;
        while left > 0 {
            let take = left.min(m);
            out.push((item.clone(), take));
            left -= take;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(item: &str, n: u32) -> Slot {
        Some((item.to_owned(), n))
    }

    #[test]
    fn capacity_and_stack_open_cells() {
        assert_eq!(open_cells(0, false), 0);
        assert_eq!(open_cells(2, false), 2);
        assert_eq!(open_cells(4, true), 5);
        assert_eq!(open_cells(0, true), 1);
    }

    #[test]
    fn the_intake_fills_its_item_first_then_empty_cells() {
        let mut cells = vec![s("iron", 60), None];
        let mut slot = s("iron", 10);
        intake(&mut slot, &mut cells, 2, 64);
        assert_eq!(cells, vec![s("iron", 64), s("iron", 6)]);
        assert_eq!(slot, None);
        let mut slot = s("gold", 64);
        intake(&mut slot, &mut cells, 3, 64);
        assert_eq!(cells[2], s("gold", 64));
        let mut slot = s("copper", 5);
        intake(&mut slot, &mut cells, 3, 64);
        assert_eq!(slot, s("copper", 5), "no room: it stays in the intake");
    }

    #[test]
    fn a_stack_module_makes_a_cell_of_256() {
        let mut cells = Vec::new();
        for _ in 0..4 {
            let mut slot = s("iron", 64);
            intake(&mut slot, &mut cells, 1, 256);
            assert_eq!(slot, None);
        }
        assert_eq!(cells, vec![s("iron", 256)]);
        let mut slot = s("iron", 1);
        intake(&mut slot, &mut cells, 1, 256);
        assert_eq!(slot, s("iron", 1));
    }

    #[test]
    fn a_closed_cell_gives_back_into_the_intake() {
        let mut cells = vec![s("iron", 3), s("gold", 100)];
        let mut slot = None;
        spill(&mut slot, &mut cells, 1, |_| 64);
        assert_eq!(slot, s("gold", 64));
        assert_eq!(cells, vec![s("iron", 3), s("gold", 36)]);
        spill(&mut slot, &mut cells, 1, |_| 64);
        assert_eq!(cells[1], s("gold", 36), "the intake is full");
        slot = None;
        spill(&mut slot, &mut cells, 1, |_| 64);
        assert_eq!(cells, vec![s("iron", 3)], "an emptied closed cell goes");
    }

    #[test]
    fn the_machine_works_from_the_first_cell_it_can() {
        let cells = vec![None, s("dirt", 4), s("iron", 2)];
        assert_eq!(front(&cells, |i| i == "iron"), Some(2));
        assert_eq!(front(&cells, |_| true), Some(1));
        assert_eq!(front(&[], |_| true), None);
    }

    #[test]
    fn a_result_and_its_reserve() {
        assert_eq!(split(&s("dust", 200), 64), (s("dust", 64), s("dust", 136)));
        assert_eq!(split(&s("dust", 10), 64), (s("dust", 10), None));
        assert_eq!(split(&None, 64), (None, None));
        assert_eq!(join(&s("dust", 64), &s("dust", 136)), s("dust", 200));
        assert_eq!(join(&None, &s("dust", 5)), s("dust", 5));
        assert_eq!(join(&s("dust", 5), &None), s("dust", 5));
    }

    #[test]
    fn a_broken_machine_spills_its_cells_in_stacks() {
        let out = spilled(&[s("iron", 130), None, s("gold", 3)], |_| 64);
        assert_eq!(out, vec![("iron".into(), 64), ("iron".into(), 64), ("iron".into(), 2), ("gold".into(), 3)]);
    }
}
