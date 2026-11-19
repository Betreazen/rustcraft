use super::*;
use crate::handbook::wrap;
use std::collections::BTreeMap;

fn repo() -> String {
    format!("{}/../../..", env!("CARGO_MANIFEST_DIR"))
}

fn lang(code: &str) -> BTreeMap<String, String> {
    let path = format!("{}/mod/content/assets/rustcraft/lang/{code}.json", repo());
    let text = std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("no {path}"));
    serde_json::from_str(&text).unwrap()
}

/// Every item the content mod registers: each has its item model in `items/` (the handbook's
/// script checks pages against the same files).
fn content_items() -> Vec<String> {
    let dir = format!("{}/mod/content/assets/rustcraft/items", repo());
    let mut ids: Vec<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|_| panic!("no {dir}"))
        .filter_map(|e| {
            let name = e.ok()?.file_name().into_string().ok()?;
            Some(format!("rustcraft:{}", name.strip_suffix(".json")?))
        })
        .collect();
    ids.sort();
    ids
}

/// The game's font, near enough: 6 pixels a character.
fn measure(s: &str) -> f32 {
    6.0 * s.chars().count() as f32
}

fn stats_of(page: &str) -> &'static [usize] {
    STATS
        .iter()
        .find(|(id, _)| *id == page)
        .map_or(&[], |s| s.1)
}

#[test]
fn every_item_of_the_content_mod_has_a_description_in_both_languages() {
    let items = content_items();
    assert!(items.len() > 150, "{}", items.len());
    for code in ["ru_ru", "en_us"] {
        let words = lang(code);
        let lookup = |k: &str| words.get(k).cloned();
        for item in &items {
            let pages = pages_of(item);
            assert!(!pages.is_empty(), "{item} has no page");
            let want: usize = pages.iter().map(|p| 1 + stats_of(p.id).len()).sum();
            let got = lines(item, &lookup);
            assert_eq!(got.len(), want, "{code}: {item} misses a line: {got:?}");
            assert!(got.iter().all(|l| !l.trim().is_empty()), "{code}: {item}");
        }
    }
}

#[test]
fn every_tooltip_stays_short() {
    let ru = lang("ru_ru");
    let lookup = |k: &str| ru.get(k).cloned();
    for item in content_items() {
        let rows: usize = lines(&item, &lookup)
            .iter()
            .map(|l| wrap(l, WIDTH, &measure).len())
            .sum();
        assert!(rows <= 9, "{item}: {rows} rows");
    }
}

#[test]
fn a_tier_takes_its_familys_page() {
    let id = |item: &str| pages_of(item).iter().map(|p| p.id).collect::<Vec<_>>();
    assert_eq!(id("rustcraft:furnace"), ["furnace"]);
    assert_eq!(id("rustcraft:furnace_3"), ["furnace"]);
    assert_eq!(id("rustcraft:housing_4"), ["housing"]);
    assert_eq!(id("rustcraft:node_2"), ["node"]);
    assert_eq!(id("rustcraft:trash_4"), ["trash"]);
    assert_eq!(id("rustcraft:monorail_4"), ["monorail"]);
    assert_eq!(id("rustcraft:chassis_3"), ["chassis"]);
    assert_eq!(id("rustcraft:backpack_3"), ["backpack"]);
    assert_eq!(id("rustcraft:blade_4"), ["blade"]);
    assert_eq!(id("rustcraft:tech_boots_3"), ["armor"]);
    assert_eq!(id("rustcraft:module_stack_3"), ["mod_stack"]);
    assert_eq!(id("rustcraft:alloy_2"), ["alloy"]);
    assert_eq!(id("rustcraft:coppersteel"), ["alloys"]);
    assert_eq!(id("rustcraft:crystal"), ["crystal"]);
    assert_eq!(id("rustcraft:tool_shooter"), ["tool_gun"]);
    assert_eq!(id("rustcraft:handbook"), ["guide"]);
    assert_eq!(id("rustcraft:crystal_shard"), ["shard", "grow"]);
    assert_eq!(id("rustcraft:zip_anchor"), ["zipline"]);
}

#[test]
fn other_items_get_nothing() {
    let ru = lang("ru_ru");
    let lookup = |k: &str| ru.get(k).cloned();
    for item in [
        "minecraft:stone",
        "minecraft:furnace",
        "rustcraft:nothing",
        "rustcraft:nothing_3",
        "soulscraft:altar",
    ] {
        assert!(pages_of(item).is_empty(), "{item}");
        assert!(lines(item, &lookup).is_empty(), "{item}");
    }
}

#[test]
fn the_yield_module_names_its_chance_and_its_machines() {
    for (code, chance, fits) in [("ru_ru", "15%", "Подходит: "), ("en_us", "15%", "Fits: ")]
    {
        let words = lang(code);
        let got = lines("rustcraft:module_yield", &|k| words.get(k).cloned());
        assert!(got[0].contains(chance), "{code}: {got:?}");
        assert!(got.last().unwrap().starts_with(fits), "{code}: {got:?}");
    }
}

/// The digits of a line, in order: «2,5» and «2.5» both give [2, 5].
fn numbers(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_ascii_digit())
        .filter(|n| !n.is_empty())
        .map(str::to_owned)
        .collect()
}

#[test]
fn every_chosen_stat_is_the_handbooks_and_english_says_the_same_numbers() {
    let (ru, en) = (lang("ru_ru"), lang("en_us"));
    for (id, stats) in STATS {
        let p = page(id).unwrap_or_else(|| panic!("no page {id}"));
        assert!(!stats.is_empty(), "{id}");
        for &s in *stats {
            assert!(s < p.stats, "{id} has no stat {s}");
            let key = &ru[&p.key(&format!("stat.{s}.key"))];
            let value = &ru[&p.key(&format!("stat.{s}.value"))];
            let tip = en
                .get(&stat_key(id, s))
                .unwrap_or_else(|| panic!("en_us has no {}", stat_key(id, s)));
            assert_eq!(
                numbers(&format!("{key}: {value}")),
                numbers(tip),
                "{id} stat {s}: «{key}: {value}» and «{tip}»"
            );
        }
    }
    // No English line is left over from a stat no longer chosen.
    for k in en.keys().filter(|k| k.starts_with(PREFIX)) {
        let chosen = STATS
            .iter()
            .any(|(id, stats)| stats.iter().any(|&s| stat_key(id, s) == *k));
        assert!(chosen, "en_us has {k}, which no stat of STATS gives");
    }
    // Russian reads the handbook's own stats: no copies of them to drift.
    assert!(!ru.keys().any(|k| k.starts_with(PREFIX)));
}
