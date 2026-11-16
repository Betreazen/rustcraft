//! Slice 18: the terminal's three sockets (guide `terminal`: «стак, книга рецептов, фильтр») and
//! the module items this stream reads by id. The items are stream 4's (rules/src/modules.rs); a
//! different id there is changed here, in one place.
//!
//! - The stack module (S3, guide `mod_stack`): in a terminal's socket it raises the stack of the
//!   whole storage, 256 for the spark-steel module, 1024 for the crystallite one, 4096 for the
//!   star-steel one; the best of all the storage's terminals counts.
//! - The recipe book (O1): orders.
//! - The filter module (TM1): the «своя» tab shows what it marks (rules::storage::filter).
//! - The capacity module (guide `mod_cap`): read by the craft machines.

/// The stack modules by id, and the tier each gives (`STACK_BY_MODULE`'s place).
pub const STACK_MODULES: [(&str, u8); 3] = [
    ("rustcraft:module_stack", 1),
    ("rustcraft:module_stack_1024", 2),
    ("rustcraft:module_stack_4096", 3),
];
pub const RECIPES: &str = "rustcraft:module_recipes";
pub const FILTER: &str = "rustcraft:module_filter";
pub const CAPACITY: &str = "rustcraft:module_capacity";
/// The storage module (guide `mod_storage`): the fishing station's catch buffer 9 → 18.
pub const STORAGE: &str = "rustcraft:module_storage";

/// The terminal's sockets, in the window's order.
pub const SOCKET_STACK: usize = 0;
pub const SOCKET_BOOK: usize = 1;
pub const SOCKET_FILTER: usize = 2;
pub const SOCKETS: usize = 3;

/// The tier of a stack module item, 0 for any other item.
pub fn stack_tier(item: &str) -> u8 {
    STACK_MODULES
        .iter()
        .find(|(i, _)| *i == item)
        .map_or(0, |(_, t)| *t)
}

/// Whether socket `k` of a terminal takes `item`.
pub fn socket_takes(k: usize, item: &str) -> bool {
    match k {
        SOCKET_STACK => stack_tier(item) > 0,
        SOCKET_BOOK => item == RECIPES,
        SOCKET_FILTER => item == FILTER,
        _ => false,
    }
}

/// What a press on a socket does with the cursor, as a slot of one item does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SocketMove {
    /// One of the cursor's stack goes in.
    In,
    /// The module comes out onto the empty cursor.
    Out,
    /// A single module on the cursor changes places with the one in the socket.
    Swap,
}

/// The move a press on socket `k` holding `held` makes with `cursor` (item, count); none when it
/// does nothing.
pub fn socket_move(k: usize, held: Option<&str>, cursor: Option<(&str, u32)>) -> Option<SocketMove> {
    match (held, cursor) {
        (None, Some((item, n))) if n > 0 && socket_takes(k, item) => Some(SocketMove::In),
        (Some(_), None) => Some(SocketMove::Out),
        (Some(_), Some((item, 1))) if socket_takes(k, item) => Some(SocketMove::Swap),
        _ => None,
    }
}

/// The count of capacity modules among a block's module items.
pub fn count_of(items: &[Option<String>], id: &str) -> u32 {
    items.iter().flatten().filter(|i| i.as_str() == id).count() as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Store;

    #[test]
    fn each_stack_module_gives_its_tier_and_the_storage_its_stack() {
        // S3: 256 / 1024 / 4096.
        assert_eq!(stack_tier("rustcraft:module_stack"), 1);
        assert_eq!(stack_tier("rustcraft:module_stack_1024"), 2);
        assert_eq!(stack_tier("minecraft:stone"), 0);
        let s = Store {
            stack_module: stack_tier("rustcraft:module_stack_4096"),
            ..Store::default()
        };
        assert_eq!(s.stack_of(64), 4096);
        assert_eq!(s.stack_of(16), 1024);
        assert_eq!(s.stack_of(1), 1);
    }

    #[test]
    fn a_socket_takes_only_its_own_module() {
        assert!(socket_takes(SOCKET_BOOK, RECIPES));
        assert!(!socket_takes(SOCKET_STACK, RECIPES));
        assert!(socket_takes(SOCKET_FILTER, FILTER));
        assert!(!socket_takes(3, FILTER));
        assert_eq!(socket_move(SOCKET_STACK, None, Some(("rustcraft:module_stack", 2))), Some(SocketMove::In));
        assert_eq!(socket_move(SOCKET_STACK, None, Some(("minecraft:stone", 2))), None);
        assert_eq!(socket_move(SOCKET_STACK, Some("rustcraft:module_stack"), None), Some(SocketMove::Out));
        assert_eq!(
            socket_move(SOCKET_STACK, Some("rustcraft:module_stack"), Some(("rustcraft:module_stack_1024", 1))),
            Some(SocketMove::Swap)
        );
        assert_eq!(
            socket_move(SOCKET_STACK, Some("rustcraft:module_stack"), Some(("rustcraft:module_stack_1024", 2))),
            None
        );
    }
}
