//! What the terminal's window shows of a storage (S4, TM1): the stored items with the quick
//! filter's tab and the search applied, most numerous first, in pages of 27 (a cell's worth of
//! slots). The window does this itself from the list the server sends, so a tab, a page or a
//! letter typed costs the server nothing.

use crate::storage::Limit;

/// Items a page of the window shows: three rows of nine.
pub const PER_PAGE: usize = 27;

/// The quick filter's tabs (TM1). «Своя» comes with the filter module, which is not in the mod
/// yet.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    All,
    Armor,
    Tools,
    Blocks,
}

impl Tab {
    pub const ALL: [Tab; 4] = [Tab::All, Tab::Armor, Tab::Tools, Tab::Blocks];

    /// Whether the tab shows an item with these facts.
    pub fn shows(self, facts: Facts) -> bool {
        match self {
            Tab::All => true,
            Tab::Armor => facts.armor,
            Tab::Tools => facts.tool && !facts.armor,
            Tab::Blocks => facts.block,
        }
    }
}

/// What the tabs need to know of an item: it is worn as armour, it wears out as a tool or a
/// weapon does, it places a block.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Facts {
    pub armor: bool,
    pub tool: bool,
    pub block: bool,
}

impl Facts {
    /// The facts of an item from its equipment slot, its durability and its block.
    pub fn of(equip_slot: Option<&str>, max_damage: u32, places_block: bool) -> Facts {
        Facts {
            armor: matches!(equip_slot, Some("head" | "chest" | "legs" | "feet")),
            tool: max_damage > 0,
            block: places_block,
        }
    }
}

/// One line of the list: an item as the storage keys it, how many there are, its limit. An item
/// with a limit and none stored is listed too, so its limit can be changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry<K> {
    pub key: K,
    pub count: u64,
    pub limit: Option<Limit>,
}

/// Whether `search` is found in any of `names`, letters of any case alike; an empty search finds
/// everything.
pub fn matches(search: &str, names: &[&str]) -> bool {
    let search = search.trim().to_lowercase();
    search.is_empty() || names.iter().any(|n| n.to_lowercase().contains(&search))
}

/// The entries the tab and the search let through, most numerous first, then by key.
pub fn visible<'a, K: Ord>(
    entries: &'a [Entry<K>],
    tab: Tab,
    search: &str,
    facts: impl Fn(&K) -> Facts,
    names: impl Fn(&K) -> Vec<String>,
) -> Vec<&'a Entry<K>> {
    let mut out: Vec<&Entry<K>> = entries
        .iter()
        .filter(|e| tab.shows(facts(&e.key)))
        .filter(|e| {
            let names = names(&e.key);
            let names: Vec<&str> = names.iter().map(String::as_str).collect();
            matches(search, &names)
        })
        .collect();
    out.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.key.cmp(&b.key)));
    out
}

/// How many pages `shown` entries fill; one even for none.
pub fn pages(shown: usize) -> usize {
    shown.div_ceil(PER_PAGE).max(1)
}

/// The entries of page `page` (from 0), the last page when past it.
pub fn page<T>(shown: &[T], page: usize) -> &[T] {
    let page = page.min(pages(shown.len()) - 1);
    let start = page * PER_PAGE;
    &shown[start.min(shown.len())..(start + PER_PAGE).min(shown.len())]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(key: &str, count: u64) -> Entry<String> {
        Entry {
            key: key.into(),
            count,
            limit: None,
        }
    }

    fn facts_of(key: &str) -> Facts {
        match key {
            "helmet" => Facts::of(Some("head"), 165, false),
            "pickaxe" => Facts::of(Some("mainhand"), 250, false),
            "stone" => Facts::of(None, 0, true),
            _ => Facts::default(),
        }
    }

    fn names_of(key: &str) -> Vec<String> {
        let shown = match key {
            "helmet" => "Железный шлем",
            "pickaxe" => "Железная кирка",
            "stone" => "Камень",
            _ => "Палка",
        };
        vec![shown.to_owned(), key.to_owned()]
    }

    fn list() -> Vec<Entry<String>> {
        vec![
            entry("stick", 5),
            entry("stone", 300),
            entry("helmet", 1),
            entry("pickaxe", 2),
        ]
    }

    fn keys(v: Vec<&Entry<String>>) -> Vec<&str> {
        v.into_iter().map(|e| e.key.as_str()).collect()
    }

    #[test]
    fn the_tabs_filter_armour_tools_and_blocks() {
        // Rule TM1.
        let l = list();
        assert_eq!(
            keys(visible(&l, Tab::All, "", |k| facts_of(k), |k| names_of(k))),
            ["stone", "stick", "pickaxe", "helmet"]
        );
        assert_eq!(
            keys(visible(
                &l,
                Tab::Armor,
                "",
                |k| facts_of(k),
                |k| names_of(k)
            )),
            ["helmet"]
        );
        assert_eq!(
            keys(visible(
                &l,
                Tab::Tools,
                "",
                |k| facts_of(k),
                |k| names_of(k)
            )),
            ["pickaxe"]
        );
        assert_eq!(
            keys(visible(
                &l,
                Tab::Blocks,
                "",
                |k| facts_of(k),
                |k| names_of(k)
            )),
            ["stone"]
        );
    }

    #[test]
    fn the_search_finds_a_name_in_any_case_or_the_id() {
        // Rule S4.
        let l = list();
        assert_eq!(
            keys(visible(
                &l,
                Tab::All,
                "ЖЕЛЕЗ",
                |k| facts_of(k),
                |k| names_of(k)
            )),
            ["pickaxe", "helmet"]
        );
        assert_eq!(
            keys(visible(
                &l,
                Tab::All,
                "stic",
                |k| facts_of(k),
                |k| names_of(k)
            )),
            ["stick"]
        );
        assert!(visible(&l, Tab::Blocks, "кирка", |k| facts_of(k), |k| names_of(k)).is_empty());
    }

    #[test]
    fn pages_hold_twenty_seven_and_the_last_is_partial() {
        let shown: Vec<u32> = (0..60).collect();
        assert_eq!(pages(shown.len()), 3);
        assert_eq!(pages(0), 1);
        assert_eq!(page(&shown, 0).len(), 27);
        assert_eq!(page(&shown, 2), &shown[54..]);
        assert_eq!(page(&shown, 9), &shown[54..]);
        assert!(page::<u32>(&[], 0).is_empty());
    }
}
