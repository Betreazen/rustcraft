//! Slice 18 (TM1, guide `terminal`): the filter module's list. «Свою вкладку задаёт модуль
//! фильтра: в его окне отмечаются нужные предметы.» The list lives in the module item's custom
//! data (`rustcraft:filter`, a list of item ids), so it goes with the module from terminal to
//! terminal. Its window is a chest of three rows: a click with an item on the cursor marks that
//! item (the cursor keeps it), a click on a marked item unmarks it.

/// The custom data key of the list.
pub const DATA_KEY: &str = "rustcraft:filter";
/// The most items a filter marks: its window's three rows.
pub const MAX_MARKED: usize = 27;
/// The window's rows.
pub const ROWS: u8 = 3;

/// What a click in the filter's window did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Click {
    Marked,
    Unmarked,
    /// The list is full: nothing marked.
    Full,
    Nothing,
}

/// A click on place `slot` of the window showing `list`, with `cursor` (an item id) or nothing:
/// the list after it is written into `list`.
pub fn click(list: &mut Vec<String>, slot: usize, cursor: Option<&str>) -> Click {
    match cursor {
        Some(item) if list.iter().any(|i| i == item) => Click::Nothing,
        Some(_) if list.len() >= MAX_MARKED => Click::Full,
        Some(item) => {
            list.push(item.to_owned());
            Click::Marked
        }
        None if slot < list.len() => {
            list.remove(slot);
            Click::Unmarked
        }
        None => Click::Nothing,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_click_marks_with_the_cursor_and_unmarks_without() {
        let mut l = Vec::new();
        assert_eq!(click(&mut l, 0, Some("minecraft:stone")), Click::Marked);
        assert_eq!(click(&mut l, 5, Some("minecraft:stone")), Click::Nothing, "once");
        assert_eq!(click(&mut l, 1, Some("minecraft:dirt")), Click::Marked);
        assert_eq!(l, vec!["minecraft:stone", "minecraft:dirt"]);
        assert_eq!(click(&mut l, 0, None), Click::Unmarked);
        assert_eq!(l, vec!["minecraft:dirt"]);
        assert_eq!(click(&mut l, 9, None), Click::Nothing);
    }

    #[test]
    fn a_full_filter_marks_no_more() {
        let mut l: Vec<String> = (0..MAX_MARKED).map(|i| format!("m:i{i}")).collect();
        assert_eq!(click(&mut l, 0, Some("minecraft:stone")), Click::Full);
        assert_eq!(l.len(), MAX_MARKED);
    }
}
