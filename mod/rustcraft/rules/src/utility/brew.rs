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
