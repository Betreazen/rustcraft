//! 0.20 (round 31, 4): Path-of-craft-tree's tree acts on its owner's machines. RustCraft reads the
//! summary PCT keeps on every player (`pct:summary` in the player's persistent data, PCT's
//! `docs/combat-summary.md`), and a machine works faster by its owner's level and learned ranks in
//! the branch its work belongs to: the miner by the miner's branch, the furnace by the smith's,
//! the fishing station by the fisher's. Bounded: at most ×1.5, at the last level with every rank.
//! The energy a second stays; the work takes less time.
//!
//! The summary as PCT writes it (v1; `all_levels` and `all_ranks` since PCT 0.4.11):
//!
//! ```json
//! {"v":1,"levels":{…},"ranks":{…},
//!  "all_levels":{"mining":[31,75],"smithing":[14,75],…},
//!  "all_ranks":{"mining":[25,73],"smithing":[6,68],…},"general":[7,75]}
//! ```
//!
//! An unknown `v`, a summary without `all_levels`, or a branch it does not name count as none.

use std::collections::BTreeMap;

use serde_json::Value;

/// The persistent-data key PCT writes.
pub const KEY: &str = "pct:summary";
/// The summary shape this reader knows.
pub const VERSION: u64 = 1;

/// What the level share of a branch adds to its machines' speed at most. **Proposal.**
pub const LEVEL_BONUS: f64 = 0.25;
/// What the learned ranks of a branch add at most. **Proposal.**
pub const RANK_BONUS: f64 = 0.25;

/// A value and its highest, `[value, highest]` in the summary.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pair {
    pub value: f64,
    pub max: f64,
}

impl Pair {
    /// The value as a share of its highest, 0 to 1; 0 when the highest is 0.
    pub fn share(self) -> f64 {
        if self.max > 0.0 { (self.value / self.max).clamp(0.0, 1.0) } else { 0.0 }
    }
}

/// One player's tree, by branch id: the level and the learned ranks.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Summary {
    pub levels: BTreeMap<String, Pair>,
    pub ranks: BTreeMap<String, Pair>,
}

impl Summary {
    /// How much faster a machine of `branch` works for this owner: 1 + LEVEL_BONUS × the level's
    /// share + RANK_BONUS × the ranks' share.
    pub fn speed(&self, branch: &str) -> f64 {
        let share = |m: &BTreeMap<String, Pair>| m.get(branch).map_or(0.0, |p| p.share());
        1.0 + LEVEL_BONUS * share(&self.levels) + RANK_BONUS * share(&self.ranks)
    }
}

/// Reads the summary from what the key holds: a JSON string, or the same as a JSON object.
pub fn parse(held: &Value) -> Result<Summary, String> {
    match held {
        Value::String(text) => {
            let v: Value = serde_json::from_str(text).map_err(|e| format!("{KEY}: {e}"))?;
            parse_object(&v)
        }
        v @ Value::Object(_) => parse_object(v),
        other => Err(format!("{KEY}: neither a JSON string nor a compound: {other}")),
    }
}

fn parse_object(v: &Value) -> Result<Summary, String> {
    let version = v.get("v").and_then(Value::as_u64).ok_or(format!("{KEY}: no version v"))?;
    if version != VERSION {
        return Err(format!("{KEY}: version {version}, this reader knows {VERSION}"));
    }
    let levels = pairs(v.get("all_levels").ok_or(format!("{KEY}: no all_levels (PCT before 0.5.0)"))?)?;
    let ranks = v.get("all_ranks").map(pairs).transpose()?.unwrap_or_default();
    Ok(Summary { levels, ranks })
}

fn pairs(v: &Value) -> Result<BTreeMap<String, Pair>, String> {
    let map = v.as_object().ok_or(format!("{KEY}: a branch list is no object"))?;
    let num = |x: &Value| x.as_f64().filter(|n| n.is_finite() && *n >= 0.0);
    Ok(map
        .iter()
        .filter_map(|(branch, p)| {
            let list = p.as_array().filter(|l| l.len() == 2)?;
            Some((branch.clone(), Pair { value: num(&list[0])?, max: num(&list[1])? }))
        })
        .collect())
}

/// The machines and workers whose work a branch of the tree speeds up, as the handbook's table
/// names them: (branch, what it speeds). The bundle maps its blocks and tools to these branches.
pub const TABLE: [(&str, &str); 12] = [
    ("mining", "добытчик, разрушитель, захват, бур-проходчик, живая жила"),
    ("woodcutting", "пила"),
    ("farming", "жнец"),
    ("digging", "ковш, археологический стол"),
    ("fishing", "рыболовная станция"),
    ("husbandry", "забойщик, кормушка, стригаль-доильщик, пасечный сборщик"),
    ("smithing", "дробилка, электропечь, плавильня сплавов, ремонтный стенд"),
    ("crafting", "автокрафтер, дезинтегратор"),
    ("cooking", "кухонный комбайн"),
    ("alchemy", "варочный куб, смеситель"),
    ("enchanting", "зачаровательный автомат"),
    ("building", "установщик блоков, строительный автомат"),
];

/// A worker's tool, by its name in the tables (`worker::Tool`'s): the branch of its work.
pub fn tool_branch(tool: crate::worker::Tool) -> Option<&'static str> {
    use crate::worker::Tool;
    match tool {
        Tool::Saw => Some("woodcutting"),
        Tool::Reaper => Some("farming"),
        Tool::Grab | Tool::Bore => Some("mining"),
        Tool::Scoop => Some("digging"),
        // The cartographer marks what it rides past and the guard keeps its pace: no work time.
        Tool::Map | Tool::Hold | Tool::Shooter => None,
    }
}

/// A slice-12 machine's branch.
pub fn util_branch(u: crate::utility::Util) -> Option<&'static str> {
    use crate::utility::Util;
    match u {
        Util::Repair => Some("smithing"),
        Util::Kitchen => Some("cooking"),
        Util::Fishery => Some("fishing"),
        Util::Feeder | Util::Shearer | Util::Apiary => Some("husbandry"),
        Util::Brewer => Some("alchemy"),
        Util::Enchanter => Some("enchanting"),
        Util::Archeo => Some("digging"),
        Util::Builder => Some("building"),
        Util::Disintegrator => Some("crafting"),
        Util::Vein => Some("mining"),
        Util::Trash | Util::Charger | Util::ChargePlate | Util::XpTank | Util::Projector | Util::PortalDock => None,
    }
}

/// The summaries known on the server: read on its tick for the players online, kept for those
/// who left (their machines work on), and saved with the world so a restart keeps them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cache {
    known: BTreeMap<u128, Summary>,
}

impl Cache {
    /// What one read of `player`'s data gave: `Some(held)` the key's JSON, `None` no key. Returns
    /// whether what is known changed, with the error of a summary that does not read (counted as
    /// none: the player's machines work as anyone's).
    pub fn update(&mut self, player: u128, held: Option<&Value>) -> (bool, Option<String>) {
        let (now, error) = match held.map(parse) {
            None => (None, None),
            Some(Ok(s)) => (Some(s), None),
            Some(Err(e)) => (None, Some(e)),
        };
        let changed = match now {
            Some(s) => self.known.insert(player, s.clone()).filter(|b| *b == s).is_none(),
            None => self.known.remove(&player).is_some(),
        };
        (changed, error)
    }

    /// How much faster `owner`'s machine of `branch` works; 1 without an owner, a branch or a
    /// summary.
    pub fn speed(&self, owner: Option<u128>, branch: Option<&str>) -> f64 {
        match (owner.and_then(|o| self.known.get(&o)), branch) {
            (Some(s), Some(b)) => s.speed(b),
            _ => 1.0,
        }
    }

    pub fn get(&self, owner: u128) -> Option<&Summary> {
        self.known.get(&owner)
    }

    pub fn owners(&self) -> impl Iterator<Item = (&u128, &Summary)> {
        self.known.iter()
    }

    /// The cache as the world keeps it: `{"<uuid hex>": {"levels": {...}, "ranks": {...}}}`.
    pub fn to_json(&self) -> String {
        let pair = |p: &Pair| serde_json::json!([p.value, p.max]);
        let map: serde_json::Map<String, Value> = self
            .known
            .iter()
            .map(|(o, s)| {
                let side = |m: &BTreeMap<String, Pair>| -> Value {
                    m.iter().map(|(b, p)| (b.clone(), pair(p))).collect::<serde_json::Map<_, _>>().into()
                };
                (format!("{o:032x}"), serde_json::json!({"levels": side(&s.levels), "ranks": side(&s.ranks)}))
            })
            .collect();
        Value::Object(map).to_string()
    }

    /// What `to_json` wrote; entries that do not read are left out.
    pub fn from_json(text: &str) -> Cache {
        let Ok(Value::Object(map)) = serde_json::from_str::<Value>(text) else {
            return Cache::default();
        };
        let known = map
            .iter()
            .filter_map(|(o, v)| {
                let owner = u128::from_str_radix(o, 16).ok()?;
                let levels = pairs(v.get("levels")?).ok()?;
                let ranks = v.get("ranks").and_then(|r| pairs(r).ok()).unwrap_or_default();
                Some((owner, Summary { levels, ranks }))
            })
            .collect();
        Cache { known }
    }
}

/// A v1 summary as PCT writes it, every branch of `branches` at `level` of 75 with `ranks` of
/// `ranks_max` (a test fixture).
pub fn fixture(branches: &[&str], level: u32, ranks: u32, ranks_max: u32) -> String {
    let side = |a: u32, b: u32| {
        branches.iter().map(|br| format!("\"{br}\":[{a},{b}]")).collect::<Vec<_>>().join(",")
    };
    format!(
        "{{\"v\":1,\"all_levels\":{{{}}},\"all_ranks\":{{{}}},\"general\":[{level},75]}}",
        side(level, 75),
        side(ranks, ranks_max)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const PCT: &str = r#"{"v":1,"levels":{"archery":[6,75],"defense":[6,75],"melee":[27,75]},"ranks":{"archery":[0,68],"defense":[0,66],"melee":[21,69]},"all_levels":{"mining":[31,75],"smithing":[75,75],"fishing":[0,75]},"all_ranks":{"mining":[25,73],"smithing":[68,68],"fishing":[0,65]},"general":[7,75]}"#;

    #[test]
    fn a_branch_speeds_its_machines_up_to_one_and_a_half() {
        let s = parse(&Value::String(PCT.into())).unwrap();
        assert!((s.speed("smithing") - 1.5).abs() < 1e-9, "the last level, every rank");
        assert!((s.speed("fishing") - 1.0).abs() < 1e-9);
        let mining = 1.0 + 0.25 * 31.0 / 75.0 + 0.25 * 25.0 / 73.0;
        assert!((s.speed("mining") - mining).abs() < 1e-9);
        assert!((s.speed("cooking") - 1.0).abs() < 1e-9, "a branch it does not name");
    }

    #[test]
    fn an_old_or_unknown_summary_counts_as_none() {
        let old = r#"{"v":1,"levels":{"melee":[3,75]},"ranks":{"melee":[0,69]}}"#;
        assert!(parse(&Value::String(old.into())).is_err(), "no all_levels");
        assert!(parse(&Value::String(r#"{"v":2,"all_levels":{}}"#.into())).is_err());
        assert!(parse(&Value::from(5)).is_err());
        let mut c = Cache::default();
        let (changed, error) = c.update(7, Some(&Value::String(old.into())));
        assert!(!changed && error.is_some());
        assert_eq!(c.speed(Some(7), Some("mining")), 1.0);
    }

    #[test]
    fn the_cache_keeps_an_owner_who_left_and_survives_a_restart() {
        let mut c = Cache::default();
        assert_eq!(c.update(7, Some(&Value::String(PCT.into()))), (true, None));
        assert_eq!(c.update(7, Some(&Value::String(PCT.into()))), (false, None), "the same again");
        assert!((c.speed(Some(7), Some("smithing")) - 1.5).abs() < 1e-9);
        assert_eq!(c.speed(None, Some("smithing")), 1.0, "a machine nobody owns");
        assert_eq!(c.speed(Some(8), Some("smithing")), 1.0, "an owner never read");
        assert_eq!(c.speed(Some(7), None), 1.0, "a machine of no branch");
        let back = Cache::from_json(&c.to_json());
        assert_eq!(back, c);
        // PCT gone from the player: no key, no perk.
        assert_eq!(c.update(7, None), (true, None));
        assert_eq!(c.speed(Some(7), Some("smithing")), 1.0);
    }

    #[test]
    fn the_fixture_reads_as_pct_writes_it() {
        let s = parse(&Value::String(fixture(&["smithing", "fishing"], 75, 68, 68))).unwrap();
        assert!((s.speed("smithing") - 1.5).abs() < 1e-9);
        assert!((s.speed("mining") - 1.0).abs() < 1e-9);
    }

    #[test]
    fn every_branch_of_the_table_is_a_tool_or_machine_branch() {
        use crate::utility::Util;
        use crate::worker::Tool;
        let named: Vec<&str> = Tool::ALL.iter().filter_map(|t| tool_branch(*t)).chain(
            [Util::Repair, Util::Kitchen, Util::Fishery, Util::Feeder, Util::Brewer, Util::Enchanter, Util::Archeo, Util::Builder, Util::Disintegrator, Util::Vein]
                .into_iter()
                .filter_map(util_branch),
        ).collect();
        for (branch, _) in TABLE {
            assert!(named.contains(&branch) || ["smithing", "crafting", "alchemy", "building", "husbandry"].contains(&branch), "{branch}");
        }
    }
}
