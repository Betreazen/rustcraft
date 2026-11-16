//! Slice 18: the technical enchantments at the anvil (guide `enchants`: «накладываешь её на
//! наковальне. Уровни складываются на наковальне, как у обычных книг»). A book carries its level
//! in its custom data (a crafted one is I); a piece carries its levels the same way, so both sides
//! of the anvil work its result out from the stacks alone, and the server takes the piece's
//! levels into what it keeps for it (rustcraft_gear).

use super::{BOOK_CAPACITY, BOOK_THRIFT, MAX_LEVEL};

/// A book's level in its custom data.
pub const LEVEL_KEY: &str = "rustcraft:level";
/// A piece's levels in its custom data.
pub const CAPACITY_KEY: &str = "rustcraft:capacity";
pub const THRIFT_KEY: &str = "rustcraft:thrift";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Book {
    Capacity,
    Thrift,
}

impl Book {
    pub fn of(item: &str) -> Option<Book> {
        match item {
            BOOK_CAPACITY => Some(Book::Capacity),
            BOOK_THRIFT => Some(Book::Thrift),
            _ => None,
        }
    }

    /// The custom-data key of this enchantment's level on a piece.
    pub fn piece_key(self) -> &'static str {
        match self {
            Book::Capacity => CAPACITY_KEY,
            Book::Thrift => THRIFT_KEY,
        }
    }

    pub fn name_key(self) -> &'static str {
        match self {
            Book::Capacity => "rustcraft.enchant.capacity",
            Book::Thrift => "rustcraft.enchant.thrift",
        }
    }
}

/// Two books of one kind at the anvil: two of a level make the next (up to III), else the higher.
pub fn combine(a: u8, b: u8) -> u8 {
    let (a, b) = (a.clamp(1, MAX_LEVEL), b.clamp(1, MAX_LEVEL));
    if a == b { (a + 1).min(MAX_LEVEL) } else { a.max(b) }
}

/// A book of `book` on a piece at `piece`: the piece's new level, or None when it gains nothing.
pub fn apply(piece: u8, book: u8) -> Option<u8> {
    let book = book.clamp(1, MAX_LEVEL);
    let new = if book > piece { book } else if book == piece { (piece + 1).min(MAX_LEVEL) } else { piece };
    (new > piece).then_some(new)
}

/// What the anvil charges for a result of `level`, experience levels.
pub fn cost(level: u8) -> i32 {
    2 * i32::from(level.clamp(1, MAX_LEVEL))
}

/// A level in Roman numerals, as the game writes enchantment levels.
pub fn roman(level: u8) -> &'static str {
    match level {
        0 | 1 => "I",
        2 => "II",
        _ => "III",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn books_add_up_at_the_anvil_like_the_games() {
        assert_eq!(combine(1, 1), 2);
        assert_eq!(combine(2, 1), 2);
        assert_eq!(combine(2, 2), 3);
        assert_eq!(combine(3, 3), 3);
        assert_eq!(Book::of(BOOK_THRIFT), Some(Book::Thrift));
        assert_eq!(Book::of("minecraft:book"), None);
    }

    #[test]
    fn a_book_raises_a_piece_or_gives_nothing() {
        assert_eq!(apply(0, 1), Some(1));
        assert_eq!(apply(1, 1), Some(2));
        assert_eq!(apply(1, 3), Some(3));
        assert_eq!(apply(2, 1), None);
        assert_eq!(apply(3, 3), None);
        assert_eq!(cost(3), 6);
        assert_eq!(roman(2), "II");
    }
}
