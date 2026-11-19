//! Slice 18 (guide `brewer`): the brewing vat's pages. «Выбери на странице зелье и его вид:
//! обычное, долгое, сильное, взрывное или туманное. Куб сам проходит все шаги: адский нарост для
//! грубого зелья, ингредиент эффекта, затем редстоун, светопыль или порох.» A page is an effect
//! and a kind; its chain is the ingredients in brewing order, and each brew the vat takes the
//! first of them the game's brewing recipes take for its bottles. When none applies, the bottles
//! are done. One page, and one more for each capacity module, worked in turn.

pub const WART: &str = "minecraft:nether_wart";

/// The effects a page chooses, each with the ingredient that brews it from an awkward potion.
pub const EFFECTS: [(&str, &str); 12] = [
    ("swiftness", "minecraft:sugar"),
    ("strength", "minecraft:blaze_powder"),
    ("healing", "minecraft:glistering_melon_slice"),
    ("regeneration", "minecraft:ghast_tear"),
    ("fire_resistance", "minecraft:magma_cream"),
    ("night_vision", "minecraft:golden_carrot"),
    ("water_breathing", "minecraft:pufferfish"),
    ("leaping", "minecraft:rabbit_foot"),
    ("slow_falling", "minecraft:phantom_membrane"),
    ("poison", "minecraft:spider_eye"),
    ("turtle_master", "minecraft:turtle_helmet"),
    ("invisibility", "minecraft:golden_carrot"),
];

/// The kinds of a potion, each with the ingredients that make it after its effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum Kind {
    #[default]
    Normal,
    Long,
    Strong,
    Splash,
    Lingering,
}

impl Kind {
    pub const ALL: [Kind; 5] = [Kind::Normal, Kind::Long, Kind::Strong, Kind::Splash, Kind::Lingering];

    pub fn name(self) -> &'static str {
        match self {
            Kind::Normal => "normal",
            Kind::Long => "long",
            Kind::Strong => "strong",
            Kind::Splash => "splash",
            Kind::Lingering => "lingering",
        }
    }

    fn after(self) -> &'static [&'static str] {
        match self {
            Kind::Normal => &[],
            Kind::Long => &["minecraft:redstone"],
            Kind::Strong => &["minecraft:glowstone_dust"],
            Kind::Splash => &["minecraft:gunpowder"],
            Kind::Lingering => &["minecraft:gunpowder", "minecraft:dragon_breath"],
        }
    }
}

/// One page: its effect (an EFFECTS place, none for an empty page) and its kind.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Page {
    pub effect: Option<usize>,
    pub kind: Kind,
}

/// The most pages: one and one a capacity module.
pub const PAGES_MAX: usize = 4;

/// Slice 18 (guide `mod_stack`: «Подходит: … варочный куб»): the brewer's slots a stack module
/// makes hold 256, each with a hidden cell of its own: the four reagents, and apart from them the
/// blaze powder (a cell gives back only into its own group's slots, so a reagent never lands in
/// the powder's slot). The bottles stack to one and stay as they are.
pub const STASH_GROUPS: [std::ops::Range<usize>; 2] = [super::BREW_INGREDIENTS, super::BREW_FUEL..super::BREW_FUEL + 1];

/// Wave 2 (guide `mod_cap`): the hidden cells each of [`STASH_GROUPS`] has open: one a slot
/// with a stack module, and the `queue` cells of its capacity modules (`utility::queue_cells`)
/// for the reagents (not for the powder).
pub fn group_open(group: usize, stacked: bool, queue: usize) -> usize {
    let slots = STASH_GROUPS.get(group).map_or(0, |g| g.len());
    let queue = if group == 0 { queue } else { 0 };
    super::kitchen::open_cells(slots, stacked, queue)
}

pub fn pages(capacity: u32) -> usize {
    (1 + capacity as usize).min(PAGES_MAX)
}

/// The next effect of a page's button, after the last none.
pub fn cycle_effect(effect: Option<usize>) -> Option<usize> {
    match effect {
        None => Some(0),
        Some(i) if i + 1 < EFFECTS.len() => Some(i + 1),
        Some(_) => None,
    }
}

/// A page's ingredients in brewing order: the wart, the effect's, then its kind's (invisibility
/// is night vision turned with a fermented spider eye).
pub fn chain(page: Page) -> Vec<&'static str> {
    let Some(e) = page.effect.filter(|e| *e < EFFECTS.len()) else {
        return Vec::new();
    };
    let mut out = vec![WART, EFFECTS[e].1];
    if EFFECTS[e].0 == "invisibility" {
        out.push("minecraft:fermented_spider_eye");
    }
    out.extend(page.kind.after());
    out
}

/// 0.20 (round 31): whether the magistral brings a vat whose first `usable` pages are `pages`
/// `item` into its reagent cells: only an ingredient of one of their chains. Before, it took any
/// brewing ingredient, and the net filled it with the redstone, glowstone and sugar that rode to
/// the storage. A hopper and a hand still give it any ingredient (`is_ingredient`).
pub fn pages_take(pages: &[Page], usable: usize, item: &str) -> bool {
    pages.iter().take(usable).any(|p| chain(*p).contains(&item))
}

/// Whether `item` is an ingredient of some page's chain.
pub fn is_ingredient(item: &str) -> bool {
    item == WART
        || item == "minecraft:fermented_spider_eye"
        || EFFECTS.iter().any(|(_, i)| *i == item)
        || Kind::ALL.iter().any(|k| k.after().contains(&item))
}

/// What a brew does with a page's bottles.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// Brew with this ingredient.
    Brew(&'static str),
    /// The next step needs this ingredient and the vat has none.
    Missing(&'static str),
    /// No ingredient of the chain takes the bottles any more: they are done.
    Done,
}

/// The step for bottles the game's recipes take `applies` ingredients for, with the vat holding
/// `has` ingredients: the first of the chain that applies.
pub fn next_step(chain: &[&'static str], applies: impl Fn(&str) -> bool, has: impl Fn(&str) -> bool) -> Step {
    match chain.iter().find(|i| applies(i)) {
        Some(i) if has(i) => Step::Brew(i),
        Some(i) => Step::Missing(i),
        None => Step::Done,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_magistral_brings_only_what_its_pages_brew_with() {
        // 0.20: the owner's redstone and sugar rode to the storage past a vat.
        let swiftness = EFFECTS.iter().position(|(_, i)| *i == "minecraft:sugar").expect("swiftness");
        let pages = [Page { effect: Some(swiftness), kind: Kind::default() }, Page::default()];
        assert!(pages_take(&pages, 1, WART));
        assert!(pages_take(&pages, 1, "minecraft:sugar"));
        assert!(!pages_take(&pages, 1, "minecraft:glowstone_dust"));
        assert!(!pages_take(&[Page::default()], 1, WART), "an empty page brews nothing");
        assert!(!pages_take(&pages, 0, "minecraft:sugar"), "no usable page");
    }

    #[test]
    fn a_stack_module_lets_the_brewers_reagents_and_powder_hold_256() {
        use crate::machines::{queue, slots::{Slot, count_in}};
        assert!(!STASH_GROUPS.iter().any(|g| g.clone().any(|i| super::super::BREW_BOTTLES.contains(&i))));
        let (powder, sugar) = ("minecraft:blaze_powder", "minecraft:sugar");
        let mut reagents: Vec<Slot> = vec![None; STASH_GROUPS[0].len()];
        let mut fuel: Vec<Slot> = vec![None; STASH_GROUPS[1].len()];
        let (mut reagent_cells, mut fuel_cells) = (Vec::new(), Vec::new());
        for _ in 0..4 {
            fuel[0] = Some((powder.into(), 64));
            reagents[0] = Some((sugar.into(), 64));
            queue::stash(&mut reagents, &mut reagent_cells, true, |_| 256, |_| 64);
            queue::stash(&mut fuel, &mut fuel_cells, true, |_| 256, |_| 64);
        }
        assert_eq!((fuel[0].clone(), reagents[0].clone()), (None, None));
        assert_eq!(count_in(&fuel_cells, powder), 256, "the powder's slot holds 256 with the module");
        assert_eq!(count_in(&reagent_cells, sugar), 256);
        // The module out: each group's cells give back into its own slots only.
        queue::stash(&mut fuel, &mut fuel_cells, false, |_| 256, |_| 64);
        queue::stash(&mut reagents, &mut reagent_cells, false, |_| 256, |_| 64);
        assert_eq!(count_in(&fuel, sugar), 0);
        assert_eq!(count_in(&fuel_cells, powder) + count_in(&fuel, powder), 256, "nothing lost");
        assert_eq!(count_in(&reagents, sugar), 256, "four slots of 64");
    }

    #[test]
    fn a_capacity_module_gives_the_reagents_two_cells() {
        assert_eq!(group_open(0, false, 2), 2);
        assert_eq!(group_open(0, true, 2), STASH_GROUPS[0].len() + 2);
        assert_eq!(group_open(1, false, 3), 0, "the powder's slot only grows with the stack");
        assert_eq!(group_open(1, true, 3), 1);
    }

    #[test]
    fn a_page_is_a_chain_of_ingredients() {
        let p = Page { effect: Some(1), kind: Kind::Long };
        assert_eq!(chain(p), vec![WART, "minecraft:blaze_powder", "minecraft:redstone"]);
        let p = Page { effect: Some(0), kind: Kind::Lingering };
        assert_eq!(chain(p), vec![WART, "minecraft:sugar", "minecraft:gunpowder", "minecraft:dragon_breath"]);
        assert!(chain(Page::default()).is_empty());
        assert_eq!(cycle_effect(Some(EFFECTS.len() - 1)), None);
        assert_eq!(pages(1), 2);
        assert!(is_ingredient("minecraft:redstone") && is_ingredient(WART) && !is_ingredient("minecraft:dirt"));
    }

    #[test]
    fn each_brew_takes_the_first_ingredient_that_applies() {
        let c = chain(Page { effect: Some(1), kind: Kind::Strong });
        // Water bottles: the wart applies first.
        let water = |i: &str| i == WART || i == "minecraft:gunpowder";
        assert_eq!(next_step(&c, water, |_| true), Step::Brew(WART));
        // Awkward potions with no blaze powder in the vat.
        let awkward = |i: &str| i == "minecraft:blaze_powder";
        assert_eq!(next_step(&c, awkward, |i| i != "minecraft:blaze_powder"), Step::Missing("minecraft:blaze_powder"));
        // Strong potions: nothing applies.
        assert_eq!(next_step(&c, |_| false, |_| true), Step::Done);
    }
}
