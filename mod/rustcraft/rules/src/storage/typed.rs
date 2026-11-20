//! 0.21.2 (spec 49, «Числа в терминале вводятся с клавиатуры»; the owner: «сделай чтобы лимит
//! можно было прописывать руками, а то долго кликать»): the numbers of the terminal's window are
//! typed, not only stepped by «−» and «+».
//!
//! Three fields take digits: the chosen item's limit (the «Лимиты» row), the order's count and
//! «Держать: N» (the «Заказы» row). A click on a field's number turns it into a text field with
//! a caret, its number selected: the first digit typed replaces it, Backspace takes the last digit
//! off (the whole selected number at once). Enter applies the number; Escape, an empty field or a
//! click elsewhere leaves the old one. Only digits go in, at most MAX_DIGITS of them; a number past
//! the field's most is that most.
//!
//! What a number means follows the buttons: a limit of 0 takes none of the item in (as «−» down
//! to zero), «×» still clears the limit; «Держать: 0» clears N (as «×»); an order count of 0 is 1.
//!
//! The order's count is the window's own (the «Заказ» button sends it, `terminal::Press::Order`,
//! up to `terminal::MAX_ORDER`). A limit or an N goes to the server as a message, not a menu
//! button: a key press cannot press one, and 32 bits hold no item with a number. The terminal's on
//! CHANNEL (`encode_terminal`: the terminal's place and the `Set`), the tablet's as its own press
//! (`tablet::Press::Set`). The server takes it only from a player whose window is open on that
//! terminal (the tablet's: on that storage), only for an item the storage lists (it holds some, it
//! has a limit or an N, or the book makes it) and only up to MAX_VALUE.

use crate::geom::Pos;
use crate::storage::terminal::{ItemRef, LimitChange, MAX_ORDER, Press, VARIANT};

/// Client to server: a number typed into a terminal's window.
pub const CHANNEL: &str = "rustcraft:terminal_typed";
/// The most digits a field takes.
pub const MAX_DIGITS: usize = 9;
/// The largest limit or N a field takes (nine nines).
pub const MAX_VALUE: u64 = 999_999_999;
/// The item field of no item (`terminal`'s NO_ITEM).
const NO_ITEM: u16 = 0xFFFF;

/// A field of the window that takes typed digits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    /// The chosen item's limit (the «Лимиты» row).
    Limit,
    /// «Держать: N» of the chosen item (the «Заказы» row).
    Keep,
    /// The count «Заказ» orders.
    OrderCount,
}

impl Field {
    /// The largest number the field takes.
    pub fn max(self) -> u64 {
        match self {
            Field::OrderCount => u64::from(MAX_ORDER),
            Field::Limit | Field::Keep => MAX_VALUE,
        }
    }

    /// The smallest: an order is of one at least.
    pub fn min(self) -> u64 {
        u64::from(self == Field::OrderCount)
    }
}

/// The number `text` names in `field`: none for an empty text or one with anything but digits;
/// past the field's most, its most; an order's count at least 1.
pub fn parse(text: &str, field: Field) -> Option<u64> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    // Only digits: the one failure left is a number too long for u64.
    let n = text.parse::<u64>().unwrap_or(u64::MAX);
    Some(n.clamp(field.min(), field.max()))
}

/// A field being typed into: its digits, and whether they are still the number it opened with,
/// selected (the first digit replaces them, Backspace takes them all).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Typing {
    pub field: Field,
    pub text: String,
    selected: bool,
}

impl Typing {
    /// The field opened on its number now (none: empty).
    pub fn start(field: Field, now: Option<u64>) -> Self {
        Typing {
            field,
            text: now.map(|n| n.to_string()).unwrap_or_default(),
            selected: true,
        }
    }

    /// Whether the text is the number it opened with, selected.
    pub fn selected(&self) -> bool {
        self.selected && !self.text.is_empty()
    }

    /// A typed character: a digit goes in (in place of the selected number, after a lone 0, up
    /// to MAX_DIGITS); anything else is dropped. Whether the text changed.
    pub fn type_char(&mut self, c: char) -> bool {
        if !c.is_ascii_digit() {
            return false;
        }
        if std::mem::replace(&mut self.selected, false) || self.text == "0" {
            self.text.clear();
        }
        if self.text.len() >= MAX_DIGITS {
            return false;
        }
        self.text.push(c);
        true
    }

    /// Backspace: the selected number goes whole, else the last digit.
    pub fn backspace(&mut self) {
        if std::mem::replace(&mut self.selected, false) {
            self.text.clear();
        } else {
            self.text.pop();
        }
    }

    /// The number Enter applies; none for an empty field (Enter then leaves the old one).
    pub fn value(&self) -> Option<u64> {
        parse(&self.text, self.field)
    }
}

/// What a typed number sets on the server.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
    Limit,
    Keep,
}

/// A typed limit or N of an item, for the server.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Set {
    pub setting: Setting,
    /// The item as a press names it; «держать» only names plain items (damage 0).
    pub item: ItemRef,
    pub value: u64,
}

impl Set {
    /// The press a button would make of it, with the number as it is (`LimitChange::Set`), the
    /// value no more than MAX_VALUE.
    pub fn press(self) -> Press {
        let change = LimitChange::Set(self.value.min(MAX_VALUE));
        match self.setting {
            Setting::Limit => Press::Limit {
                item: self.item,
                change,
            },
            Setting::Keep => Press::Keep {
                item: self.item.0,
                change,
            },
        }
    }
}

/// `set` in 13 bytes: what it sets, the item's number and damage, the value.
pub fn encode_set(set: Set) -> Vec<u8> {
    let mut out = Vec::with_capacity(13);
    out.push(match set.setting {
        Setting::Limit => 0,
        Setting::Keep => 1,
    });
    out.extend(set.item.0.to_le_bytes());
    out.extend(set.item.1.to_le_bytes());
    out.extend(set.value.to_le_bytes());
    out
}

/// What `encode_set` wrote; none for anything else, no item, a «держать» on a worn or component
/// stack, or a value past MAX_VALUE.
pub fn decode_set(bytes: &[u8]) -> Option<Set> {
    let [what, n0, n1, d0, d1, rest @ ..] = bytes else {
        return None;
    };
    let value = u64::from_le_bytes(rest.try_into().ok()?);
    let item = (u16::from_le_bytes([*n0, *n1]), u16::from_le_bytes([*d0, *d1]));
    let setting = match what {
        0 => Setting::Limit,
        1 if item.1 == 0 => Setting::Keep,
        _ => return None,
    };
    let damage = item.1 & !VARIANT;
    let named = item.0 != NO_ITEM && crate::storage::terminal::addressable(u32::from(damage));
    (named && value <= MAX_VALUE).then_some(Set {
        setting,
        item,
        value,
    })
}

/// The terminal's message on CHANNEL: the terminal's place, then the `Set`.
pub fn encode_terminal(terminal: Pos, set: Set) -> Vec<u8> {
    let mut out: Vec<u8> = terminal.iter().flat_map(|c| c.to_le_bytes()).collect();
    out.extend(encode_set(set));
    out
}

/// What `encode_terminal` wrote.
pub fn decode_terminal(bytes: &[u8]) -> Option<(Pos, Set)> {
    if bytes.len() < 12 {
        return None;
    }
    let (place, set) = bytes.split_at(12);
    let c = |i: usize| i32::from_le_bytes(place[i * 4..i * 4 + 4].try_into().unwrap_or_default());
    Some(([c(0), c(1), c(2)], decode_set(set)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_field_takes_digits_only_and_clamps_to_its_most() {
        assert_eq!(parse("3938", Field::Limit), Some(3938));
        assert_eq!(parse("0", Field::Limit), Some(0));
        assert_eq!(parse("007", Field::Keep), Some(7));
        assert_eq!(parse("", Field::Limit), None);
        assert_eq!(parse("12a", Field::Limit), None);
        assert_eq!(parse("-5", Field::Limit), None);
        assert_eq!(parse(" 5", Field::Limit), None);
        // Past u64 and past the field's most: the most.
        assert_eq!(parse("99999999999999999999999", Field::Limit), Some(MAX_VALUE));
        assert_eq!(parse("1000000000", Field::Keep), Some(MAX_VALUE));
        assert_eq!(parse("5000", Field::OrderCount), Some(u64::from(MAX_ORDER)));
        assert_eq!(parse("0", Field::OrderCount), Some(1));
        assert_eq!(parse("250", Field::OrderCount), Some(250));
    }

    #[test]
    fn the_first_digit_replaces_the_number_it_opened_with() {
        let mut t = Typing::start(Field::Limit, Some(128));
        assert!(t.selected());
        assert!(t.type_char('3'));
        for c in "938".chars() {
            t.type_char(c);
        }
        assert_eq!(t.text, "3938");
        assert_eq!(t.value(), Some(3938));
        // Letters, signs and spaces are dropped.
        assert!(!t.type_char('x'));
        assert!(!t.type_char('-'));
        assert_eq!(t.text, "3938");
        t.backspace();
        assert_eq!(t.value(), Some(393));
    }

    #[test]
    fn backspace_takes_the_selected_number_whole() {
        let mut t = Typing::start(Field::Keep, Some(64));
        t.backspace();
        assert_eq!(t.text, "");
        assert_eq!(t.value(), None);
        t.backspace();
        assert_eq!(t.text, "");
        let mut none = Typing::start(Field::Limit, None);
        assert!(!none.selected());
        none.type_char('5');
        assert_eq!(none.value(), Some(5));
    }

    #[test]
    fn a_field_holds_nine_digits_and_no_leading_zero() {
        let mut t = Typing::start(Field::Limit, Some(0));
        t.type_char('0');
        t.type_char('0');
        assert_eq!(t.text, "0");
        t.type_char('7');
        assert_eq!(t.text, "7");
        for _ in 0..20 {
            t.type_char('9');
        }
        assert_eq!(t.text.len(), MAX_DIGITS);
        assert_eq!(t.value(), Some(799_999_999));
    }

    #[test]
    fn a_typed_number_goes_to_the_server_and_back() {
        let sets = [
            Set {
                setting: Setting::Limit,
                item: (812, 0),
                value: 3938,
            },
            Set {
                setting: Setting::Limit,
                item: (812, VARIANT | 3),
                value: 0,
            },
            Set {
                setting: Setting::Keep,
                item: (65_534, 0),
                value: MAX_VALUE,
            },
        ];
        for set in sets {
            assert_eq!(decode_set(&encode_set(set)), Some(set));
            let terminal = [-120, 64, 300_000];
            assert_eq!(decode_terminal(&encode_terminal(terminal, set)), Some((terminal, set)));
        }
        assert_eq!(
            sets[0].press(),
            Press::Limit {
                item: (812, 0),
                change: LimitChange::Set(3938)
            }
        );
        assert_eq!(
            sets[2].press(),
            Press::Keep {
                item: 65_534,
                change: LimitChange::Set(MAX_VALUE)
            }
        );
    }

    #[test]
    fn a_message_out_of_its_ranges_does_not_read() {
        let ok = Set {
            setting: Setting::Limit,
            item: (5, 0),
            value: 7,
        };
        let bytes = encode_set(ok);
        assert_eq!(decode_set(&bytes[..12]), None);
        assert_eq!(decode_set(&[bytes.clone(), vec![0]].concat()), None);
        let mut what = bytes.clone();
        what[0] = 2;
        assert_eq!(decode_set(&what), None);
        let past = Set { value: MAX_VALUE + 1, ..ok };
        assert_eq!(decode_set(&encode_set(past)), None);
        let nothing = Set { item: (NO_ITEM, 0), ..ok };
        assert_eq!(decode_set(&encode_set(nothing)), None);
        // «Держать» names plain items only.
        let worn = Set {
            setting: Setting::Keep,
            item: (5, 3),
            value: 1,
        };
        assert_eq!(decode_set(&encode_set(worn)), None);
        let too_worn = Set { item: (5, 4000), ..ok };
        assert_eq!(decode_set(&encode_set(too_worn)), None);
        assert_eq!(decode_terminal(&[0; 11]), None);
        // A press with a number has no menu button id: old servers never see one.
        assert_eq!(crate::storage::terminal::encode(ok.press()), None);
    }
}
