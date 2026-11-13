//! A machine's slots as the rules see them: an item and its count, or nothing. The bundle reads
//! them from the block entity's real slots and writes back what changed.

use crate::net::items::ItemKey;

/// One slot: an item and how many, or empty.
pub type Slot = Option<(ItemKey, u32)>;

/// How many of `item` the slots hold together.
pub fn count_in(slots: &[Slot], item: &str) -> u32 {
    slots
        .iter()
        .flatten()
        .filter(|(i, _)| i == item)
        .map(|(_, n)| n)
        .sum()
}

/// Takes up to `n` of `item` out of the slots, emptying slots that reach zero; returns how many
/// were still to take when the slots ran out.
pub fn take_from(slots: &mut [Slot], item: &str, n: u32) -> u32 {
    let mut left = n;
    for slot in slots.iter_mut() {
        if left == 0 {
            break;
        }
        if let Some((i, c)) = slot
            && i == item
        {
            let t = (*c).min(left);
            *c -= t;
            left -= t;
            if *c == 0 {
                *slot = None;
            }
        }
    }
    left
}

/// Puts `n` of `item` into the slots, onto stacks of the same item first and then into empty
/// slots, never past `stack` a slot; all or nothing: false, and the slots unchanged, when they
/// have no room for all of them.
pub fn place(slots: &mut [Slot], item: &str, n: u32, stack: u32) -> bool {
    let stack = stack.max(1);
    let room: u32 = slots
        .iter()
        .map(|s| match s {
            None => stack,
            Some((i, c)) if i == item => stack.saturating_sub(*c),
            Some(_) => 0,
        })
        .sum();
    if room < n {
        return false;
    }
    let mut left = n;
    for same in [true, false] {
        for slot in slots.iter_mut() {
            if left == 0 {
                return true;
            }
            match slot {
                Some((i, c)) if same && i == item => {
                    let t = stack.saturating_sub(*c).min(left);
                    *c += t;
                    left -= t;
                }
                None if !same => {
                    let t = stack.min(left);
                    *slot = Some((item.to_owned(), t));
                    left -= t;
                }
                _ => {}
            }
        }
    }
    left == 0
}

/// One kind a slot: whether `item` may go into `slots[index]`. A slot that holds `item` takes
/// more of it, a slot that holds something else takes nothing, and an empty slot takes `item`
/// only when no other slot holds it, so one ingredient cannot fill every slot and block the rest.
pub fn one_kind_takes(slots: &[Slot], index: usize, item: &str) -> bool {
    match slots.get(index) {
        Some(Some((i, _))) => i == item,
        Some(None) => !slots
            .iter()
            .enumerate()
            .any(|(k, s)| k != index && s.as_ref().is_some_and(|(i, _)| i == item)),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(item: &str, n: u32) -> Slot {
        Some((item.into(), n))
    }

    #[test]
    fn taking_empties_slots_and_says_what_is_missing() {
        let mut slots = [s("a", 2), s("b", 1), s("a", 3)];
        assert_eq!(take_from(&mut slots, "a", 4), 0);
        assert_eq!(slots, [None, s("b", 1), s("a", 1)]);
        assert_eq!(take_from(&mut slots, "b", 5), 4);
        assert_eq!(count_in(&slots, "a"), 1);
    }

    #[test]
    fn placing_tops_up_first_and_is_all_or_nothing() {
        let mut slots = [s("a", 60), None, s("b", 1)];
        assert!(place(&mut slots, "a", 10, 64));
        assert_eq!(slots, [s("a", 64), s("a", 6), s("b", 1)]);
        let before = slots.clone();
        assert!(!place(&mut slots, "c", 1, 64));
        assert_eq!(slots, before);
        // A tool stacks to one: two of them need two slots.
        let mut tools = [None, None];
        assert!(place(&mut tools, "pick", 2, 1));
        assert_eq!(tools, [s("pick", 1), s("pick", 1)]);
    }

    #[test]
    fn one_kind_a_slot() {
        let slots = [s("gold", 3), None, s("redstone", 1), None];
        assert!(one_kind_takes(&slots, 0, "gold"));
        assert!(!one_kind_takes(&slots, 1, "gold"));
        assert!(one_kind_takes(&slots, 1, "coppersteel"));
        assert!(!one_kind_takes(&slots, 2, "gold"));
        assert!(!one_kind_takes(&slots, 9, "gold"));
    }
}
