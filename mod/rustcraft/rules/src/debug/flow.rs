//! Items on the move, summed for the journal: every move out of (or into) a storage adds to its
//! route and item, and once a second each sum becomes one line, `storage#3 -> rustcraft:crusher(…)
//! net minecraft:iron_ore x12`, not one line an item.

use std::collections::BTreeMap;

/// The sums since the last drain, by route and item.
#[derive(Clone, Debug, Default)]
pub struct Flows {
    sums: BTreeMap<(String, String), u64>,
}

impl Flows {
    pub fn add(&mut self, route: &str, item: &str, n: u64) {
        if n == 0 {
            return;
        }
        *self
            .sums
            .entry((route.to_owned(), item.to_owned()))
            .or_insert(0) += n;
    }

    pub fn is_empty(&self) -> bool {
        self.sums.is_empty()
    }

    /// Each sum as a journal detail, the sums emptied.
    pub fn drain(&mut self) -> Vec<String> {
        std::mem::take(&mut self.sums)
            .into_iter()
            .map(|((route, item), n)| format!("{route} {item} x{n}"))
            .collect()
    }
}
