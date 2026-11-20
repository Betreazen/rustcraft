//! 0.21 (spec 48, EW1–EW12; the owner: «окно у автомата вообще не понятное, как тут выбирать чары
//! и так далее или хотя бы получать их»): the enchanting machine's choice.
//!
//! The machine works as the enchanting table with 15 bookshelves at the level the player sets
//! (1–30, [`super::xp::step_level`]). For the item in its first slot it offers the three lists the
//! table's three rows would give at that level and its seed; the player picks one, or leaves
//! «Лучшее», the richest of the three, so items from the magistral are enchanted hands-off. The
//! seed stays while nothing is enchanted and is drawn anew after each enchanting, as the table's.
//!
//! An item or book that already has enchantments gets the ones it lacks and that go with what it
//! has (EW6). «Свои чары» raises one of its enchantments a level for an anvil-like price (EW7)
//! or takes one off for a lapis (EW15); [`anvil_level`] keeps a level above the most at the anvil
//! (EW16). 0.21.2: «Особые» lists the treasure enchantments the table never gives (Mending,
//! Frost Walker…) the item can take, at their most, for more experience and lapis (EW18–EW21).
//! The bundle (rustcraft_enchanter) asks the game for the lists; this module chooses, prices and
//! maps the window's presses.

use super::xp;

/// A list of enchantments and their levels, as the game's enchanting gives it.
pub type Levels = Vec<(String, u32)>;

/// The window's option rows: «Лучшее» and the three offers, or up to four enchantments to raise.
pub const OPTION_ROWS: usize = 4;
/// The machine's buttons, in its window's order: the four level steps (−10, −1, +1, +10), the
/// four option rows, the two views, the four «×» beside the option rows (only in «Свои чары»),
/// the result slot's «на склад» / «здесь» (EW17).
pub const LEVEL_BUTTONS: std::ops::Range<usize> = 0..4;
pub const OPTION_BUTTONS: std::ops::Range<usize> = 4..8;
pub const VIEW_NEW: usize = 8;
pub const VIEW_RAISE: usize = 9;
pub const REMOVE_BUTTONS: std::ops::Range<usize> = 10..14;
/// 0.21.2 (EW17; the owner: «нужно сделать тумблер уносить сразу на склад или нет»): the result
/// slot's label is a toggle: the magistral takes what is done (the default), or it stays in the
/// result slot for the player.
pub const KEEP_BUTTON: usize = 14;
/// 0.21.2 (EW18; the owner: «я хочу чтобы в зачаровательном столе можно было и уникальные
/// зачарования получить, например починка»): the view «Особые», the middle of the three segments
/// (the others keep their numbers). Pressed again in «Особые», it shows the next four.
pub const VIEW_TREASURE: usize = 15;
pub const BUTTONS: usize = 16;
/// EW15: what taking an enchantment off costs: one lapis, no experience.
pub const REMOVE_LAPIS: u32 = 1;

/// What the machine does with the next item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Pick {
    /// The richest of the three offers (the default, and what the magistral's items get).
    #[default]
    Best,
    /// The offer of the table's row 0, 1 or 2.
    Offer(usize),
    /// One level more of the enchantment the block entity names (`RAISE_KEY`).
    Raise,
    /// 0.21 (EW15): that enchantment taken off.
    Remove,
    /// 0.21.2 (EW19): the treasure enchantment the block entity names, at its most.
    Treasure,
}

impl Pick {
    /// Its number in the block entity: 0 «Лучшее», 1–3 an offer, 4 a raise, 5 a removal,
    /// 6 a treasure.
    pub fn code(self) -> i64 {
        match self {
            Pick::Best => 0,
            Pick::Offer(k) => 1 + k.min(2) as i64,
            Pick::Raise => 4,
            Pick::Remove => 5,
            Pick::Treasure => 6,
        }
    }

    pub fn of(code: i64) -> Pick {
        match code {
            1..=3 => Pick::Offer((code - 1) as usize),
            4 => Pick::Raise,
            5 => Pick::Remove,
            6 => Pick::Treasure,
            _ => Pick::Best,
        }
    }
}

/// Which list the window shows in its option rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum View {
    /// «Новые чары»: the level and the three offers.
    #[default]
    New,
    /// «Повысить»: the item's own enchantments, each a level up.
    Raise,
    /// 0.21.2 (EW18): «Особые», the treasure enchantments the item can take.
    Treasure,
}

impl View {
    pub fn code(self) -> i64 {
        match self {
            View::New => 0,
            View::Raise => 1,
            View::Treasure => 2,
        }
    }

    pub fn of(code: i64) -> View {
        match code {
            1 => View::Raise,
            2 => View::Treasure,
            _ => View::New,
        }
    }
}

/// What a press on the machine's button `i` does while the window shows `view`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// The level by this many (only in «Новые чары»).
    Step(i32),
    Pick(Pick),
    /// Raise the enchantment of option row `k` of «Свои чары».
    RaiseRow(usize),
    /// Take off the enchantment of option row `k` of «Свои чары» (its «×»).
    RemoveRow(usize),
    /// EW19: choose the treasure enchantment of option row `k` of «Особые» (on the shown page).
    TreasureRow(usize),
    Show(View),
    /// EW18: «Особые» pressed while it is shown: the next four treasures.
    NextPage,
    /// EW17: the result slot between «на склад» and «здесь», in either view.
    Keep,
}

pub fn press(i: usize, view: View) -> Option<Action> {
    if LEVEL_BUTTONS.contains(&i) {
        return (view == View::New).then(|| Action::Step(xp::LEVEL_STEPS[i]));
    }
    if OPTION_BUTTONS.contains(&i) {
        let row = i - OPTION_BUTTONS.start;
        return Some(match (view, row) {
            (View::New, 0) => Action::Pick(Pick::Best),
            (View::New, k) => Action::Pick(Pick::Offer(k - 1)),
            (View::Raise, k) => Action::RaiseRow(k),
            (View::Treasure, k) => Action::TreasureRow(k),
        });
    }
    if REMOVE_BUTTONS.contains(&i) {
        return (view == View::Raise).then(|| Action::RemoveRow(i - REMOVE_BUTTONS.start));
    }
    match i {
        VIEW_NEW => Some(Action::Show(View::New)),
        VIEW_RAISE => Some(Action::Show(View::Raise)),
        VIEW_TREASURE if view == View::Treasure => Some(Action::NextPage),
        VIEW_TREASURE => Some(Action::Show(View::Treasure)),
        KEEP_BUTTON => Some(Action::Keep),
        _ => None,
    }
}

/// EW17: whether the item net's own moves take the result slot's stack: not while the machine
/// keeps it for the player. A hopper under it is the game's and still pulls.
pub fn net_takes_result(keep: bool) -> bool {
    !keep
}

/// How rich a list is: its levels together, then how many enchantments it has.
fn worth(list: &Levels) -> (u32, usize) {
    (list.iter().map(|(_, l)| *l).sum(), list.len())
}

/// «Лучшее» (EW4): the offer whose list has the most levels together, then the most
/// enchantments, then the table's later row (its dearest); none when all three are empty.
pub fn best(offers: &[Levels; 3]) -> Option<usize> {
    (0..3)
        .filter(|k| !offers[*k].is_empty())
        .max_by_key(|k| (worth(&offers[*k]), *k))
}

/// The offer `pick` takes: the chosen row when it has a list, «Лучшее» otherwise (a row the item
/// has nothing for); none for a raise.
pub fn chosen(pick: Pick, offers: &[Levels; 3]) -> Option<usize> {
    match pick {
        Pick::Offer(k) if offers.get(k).is_some_and(|l| !l.is_empty()) => Some(k),
        Pick::Offer(_) | Pick::Best => best(offers),
        Pick::Raise | Pick::Remove | Pick::Treasure => None,
    }
}

/// EW6: the enchantments a table may give an item that already has `have`: those it lacks and
/// that go with each of them (`compatible(a, b)` is the game's `Enchantment.areCompatible`, false
/// for the same one).
pub fn open_pool<'a>(pool: &'a [String], have: &[(String, u32)], compatible: impl Fn(&str, &str) -> bool) -> Vec<&'a String> {
    pool.iter()
        .filter(|id| have.iter().all(|(h, _)| compatible(h, id)))
        .collect()
}

/// The seed a machine shows before its first enchanting: stable for its place, never 0.
pub fn first_seed(pos: crate::geom::Pos) -> i32 {
    let mut h: u32 = 0x811C_9DC5;
    for v in pos {
        for b in v.to_le_bytes() {
            h ^= u32::from(b);
            h = h.wrapping_mul(0x0100_0193);
        }
    }
    (h as i32).max(1)
}

/// The seed after an enchanting (EW5): from a random 0..1, never 0 and never `old`.
pub fn next_seed(old: i32, random: f64) -> i32 {
    let s = ((random.clamp(0.0, 1.0) * f64::from(i32::MAX)) as i32).max(1);
    if s == old { s.wrapping_add(1).max(1) } else { s }
}

/// EW7: what one more level of an enchantment costs: experience levels as the anvil counts a book
/// of the new level (its anvil cost × the new level, at least 1, at most 39: the anvil's «too
/// expensive» is 40) and lapis, the new level plus two.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RaisePrice {
    pub levels: u32,
    pub lapis: u32,
}

pub const RAISE_MAX_LEVELS: u32 = 39;
pub const RAISE_EXTRA_LAPIS: u32 = 2;

pub fn raise_price(anvil_cost: u32, new_level: u32) -> RaisePrice {
    RaisePrice {
        levels: (anvil_cost.max(1) * new_level.max(1)).clamp(1, RAISE_MAX_LEVELS),
        lapis: new_level.max(1) + RAISE_EXTRA_LAPIS,
    }
}

/// The condenser's points a raise takes: what its levels are worth up to level 30 or above, as
/// an enchanting's ([`xp::enchant_points`]).
pub fn raise_points(price: RaisePrice) -> u32 {
    xp::enchant_points(xp::LEVEL_MAX, price.levels)
}

/// One row of «Повысить»: an enchantment the item has, its level and its most.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RaiseRow {
    pub id: String,
    pub level: u32,
    pub max: u32,
}

impl RaiseRow {
    pub fn at_max(&self) -> bool {
        self.level >= self.max
    }
}

/// The rows of «Повысить» (EW8): those that can go up first, in the item's order, then those at
/// their most; four at most.
pub fn raise_rows(have: &[(String, u32)], max_of: impl Fn(&str) -> u32) -> Vec<RaiseRow> {
    let mut rows: Vec<RaiseRow> = have
        .iter()
        .map(|(id, level)| RaiseRow { id: id.clone(), level: *level, max: max_of(id) })
        .collect();
    rows.sort_by_key(RaiseRow::at_max);
    rows.truncate(OPTION_ROWS);
    rows
}

/// What a raise of `id` does to an item with `have` (EW9): its new level, or why not.
pub fn raise(have: &[(String, u32)], id: &str, max: u32) -> Result<u32, RaiseRefusal> {
    match have.iter().find(|(e, _)| e == id) {
        None => Err(RaiseRefusal::Missing),
        Some((_, l)) if *l >= max => Err(RaiseRefusal::AtMax),
        Some((_, l)) => Ok(l + 1),
    }
}

/// EW10 (the owner: «станция зачарования не брала сама лазурит из системы»): the lapis the machine
/// takes from the storage on its item net into its lapis slot: what the next enchanting lacks,
/// while it has something to enchant; never past a stack (`max`) with what the slot holds.
pub fn lapis_to_take(has_item: bool, held: u32, price: u32, max: u32) -> u32 {
    if !has_item || held >= price {
        return 0;
    }
    (price - held).min(max.saturating_sub(held))
}

/// EW15: an item's enchantments without `id`, none when it does not have it.
pub fn remove(have: &[(String, u32)], id: &str) -> Option<Levels> {
    have.iter().any(|(e, _)| e == id).then(|| have.iter().filter(|(e, _)| e != id).cloned().collect())
}

/// EW16 (the owner: «+1 уровень к максимальному на наковальне не работает»): the level the anvil
/// gives an enchantment that `left` has at `l` and `right` at `r` (0 for none) when the game's
/// anvil made `vanilla` of it (the game caps at the enchantment's most). Different levels keep
/// the higher, even above the most (a book of Efficiency VI from the skill tree); equal levels go
/// one up only up to the most, as the game's, so two books above it never climb further.
pub fn anvil_level(l: u32, r: u32, vanilla: u32) -> u32 {
    let wanted = if l == r { l } else { l.max(r) };
    vanilla.max(wanted)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RaiseRefusal {
    /// The item does not have it: it is enchanted as «Лучшее» instead.
    Missing,
    /// It is at its most: the machine stands and says so.
    AtMax,
}

// ---- «Особые» (EW18–EW21) ----

/// The game's tag of the enchantments the table never gives (`#minecraft:treasure`).
pub const TREASURE_TAG: &str = "minecraft:treasure";
/// Shown first: the one the owner asked for.
pub const MENDING: &str = "minecraft:mending";
/// EW20: a treasure costs twice a raise's experience (the anvil cost × its level × 2, at most 39
/// levels) and ten lapis.
pub const TREASURE_XP_FACTOR: u32 = 2;
pub const TREASURE_LAPIS: u32 = 10;

pub fn treasure_price(anvil_cost: u32, level: u32) -> RaisePrice {
    RaisePrice {
        levels: (anvil_cost.max(1) * level.max(1) * TREASURE_XP_FACTOR).clamp(1, RAISE_MAX_LEVELS),
        lapis: TREASURE_LAPIS,
    }
}

/// The treasures in the order «Особые» lists them (EW18): the tag's without its curses, Mending
/// first, then the tag's order, each once.
pub fn treasure_order(tag: &[String], is_curse: impl Fn(&str) -> bool) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mending = tag.iter().filter(|id| *id == MENDING);
    for id in mending.chain(tag.iter().filter(|id| *id != MENDING)) {
        if !is_curse(id) && !out.contains(id) {
            out.push(id.clone());
        }
    }
    out
}

/// The treasures an item with `have` can take (EW18), each at its most, in `order`: those it does
/// not have, that go on it (`fits`: the enchantment's `supported_items`, anything for a book) and
/// that go with each enchantment it has (`compatible`, the game's `Enchantment.areCompatible`).
pub fn treasures(
    order: &[String],
    have: &[(String, u32)],
    fits: impl Fn(&str) -> bool,
    compatible: impl Fn(&str, &str) -> bool,
    max_of: impl Fn(&str) -> u32,
) -> Levels {
    order
        .iter()
        .filter(|id| have.iter().all(|(h, _)| h != *id && compatible(h, id.as_str())) && fits(id.as_str()))
        .map(|id| (id.clone(), max_of(id).max(1)))
        .collect()
}

/// How many pages of four rows `n` treasures take (at least one).
pub fn treasure_pages(n: usize) -> usize {
    n.div_ceil(OPTION_ROWS).max(1)
}

/// The rows of page `page` (counted round: past the last page is the first again).
pub fn treasure_page(list: &[(String, u32)], page: usize) -> &[(String, u32)] {
    let start = (page % treasure_pages(list.len())) * OPTION_ROWS;
    &list[start.min(list.len())..(start + OPTION_ROWS).min(list.len())]
}

/// The page after `page` for `n` treasures, round.
pub fn next_page(page: usize, n: usize) -> usize {
    (page + 1) % treasure_pages(n)
}

/// EW19: the level a chosen treasure `id` gives an item that can take `list` (its [`treasures`])
/// and has `have`, or why not.
pub fn treasure(list: &[(String, u32)], have: &[(String, u32)], id: &str) -> Result<u32, TreasureRefusal> {
    if let Some((_, level)) = list.iter().find(|(e, _)| e == id) {
        return Ok(*level);
    }
    if have.iter().any(|(e, _)| e == id) {
        Err(TreasureRefusal::Has)
    } else {
        Err(TreasureRefusal::Unfit)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreasureRefusal {
    /// The item has it already: it waits, the status says so.
    Has,
    /// It does not go on the item or with what it has: it waits, the status says so.
    Unfit,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(e: &[(&str, u32)]) -> Levels {
        e.iter().map(|(i, l)| ((*i).to_owned(), *l)).collect()
    }

    #[test]
    fn best_is_the_richest_offer_and_the_later_row_on_a_tie() {
        let offers = [
            list(&[("minecraft:unbreaking", 1)]),
            list(&[("minecraft:sharpness", 3), ("minecraft:looting", 1)]),
            list(&[("minecraft:sharpness", 4)]),
        ];
        assert_eq!(best(&offers), Some(1), "4 levels each: the one with more enchantments");
        let tie = [list(&[("a", 2)]), list(&[("b", 2)]), list(&[])];
        assert_eq!(best(&tie), Some(1), "a full tie: the later row");
        let offers = [list(&[]), list(&[("minecraft:sharpness", 2), ("minecraft:knockback", 1)]), list(&[("minecraft:smite", 2)])];
        assert_eq!(best(&offers), Some(1));
        assert_eq!(best(&[list(&[]), list(&[]), list(&[])]), None);
    }

    #[test]
    fn a_chosen_row_is_kept_and_an_empty_one_falls_back_to_the_best() {
        let offers = [list(&[("a", 1)]), list(&[]), list(&[("b", 3)])];
        assert_eq!(chosen(Pick::Offer(0), &offers), Some(0));
        assert_eq!(chosen(Pick::Offer(1), &offers), Some(2), "nothing in row 1 for this item");
        assert_eq!(chosen(Pick::Best, &offers), Some(2));
        assert_eq!(chosen(Pick::Raise, &offers), None);
        assert_eq!(chosen(Pick::Remove, &offers), None);
        for p in [Pick::Best, Pick::Offer(0), Pick::Offer(2), Pick::Raise, Pick::Remove] {
            assert_eq!(Pick::of(p.code()), p);
        }
        assert_eq!(Pick::of(99), Pick::Best);
        assert_eq!(View::of(View::Raise.code()), View::Raise);
    }

    #[test]
    fn the_buttons_step_the_level_pick_and_switch_the_view() {
        assert_eq!(press(0, View::New), Some(Action::Step(-10)));
        assert_eq!(press(3, View::New), Some(Action::Step(10)));
        assert_eq!(press(0, View::Raise), None, "no level in «Повысить»");
        assert_eq!(press(4, View::New), Some(Action::Pick(Pick::Best)));
        assert_eq!(press(6, View::New), Some(Action::Pick(Pick::Offer(1))));
        assert_eq!(press(6, View::Raise), Some(Action::RaiseRow(2)));
        assert_eq!(press(VIEW_RAISE, View::New), Some(Action::Show(View::Raise)));
        assert_eq!(press(VIEW_NEW, View::Raise), Some(Action::Show(View::New)));
        assert_eq!(press(BUTTONS, View::New), None);
        assert_eq!(press(REMOVE_BUTTONS.start + 1, View::Raise), Some(Action::RemoveRow(1)));
        assert_eq!(press(REMOVE_BUTTONS.start, View::New), None, "no «×» among the offers");
        // EW17: the result slot's toggle in either view, the last button.
        assert_eq!(press(KEEP_BUTTON, View::New), Some(Action::Keep));
        assert_eq!(press(KEEP_BUTTON, View::Raise), Some(Action::Keep));
        assert_eq!(KEEP_BUTTON + 1, VIEW_TREASURE);
        assert!(net_takes_result(false) && !net_takes_result(true));
        // The level steps, kept in 1..=30.
        let mut level = xp::LEVEL_MAX;
        for i in [0, 0, 0, 1] {
            if let Some(Action::Step(by)) = press(i, View::New) {
                level = xp::step_level(level, by);
            }
        }
        assert_eq!(level, 1);
        assert_eq!(xp::step_level(level, 10), 11);
    }

    /// The game's 26.3 treasure tag, its curses, most levels, anvil costs and exclusive sets.
    mod vanilla {
        pub const TAG: [&str; 7] = [
            "minecraft:binding_curse",
            "minecraft:vanishing_curse",
            "minecraft:swift_sneak",
            "minecraft:soul_speed",
            "minecraft:frost_walker",
            "minecraft:mending",
            "minecraft:wind_burst",
        ];

        pub fn tag() -> Vec<String> {
            TAG.map(str::to_owned).to_vec()
        }

        pub fn is_curse(id: &str) -> bool {
            id.ends_with("_curse")
        }

        pub fn max_of(id: &str) -> u32 {
            match id {
                "minecraft:frost_walker" => 2,
                "minecraft:soul_speed" | "minecraft:swift_sneak" | "minecraft:wind_burst" | "minecraft:depth_strider" => 3,
                _ => 1,
            }
        }

        pub fn anvil(id: &str) -> u32 {
            match id {
                "minecraft:soul_speed" | "minecraft:swift_sneak" | "minecraft:infinity" => 8,
                _ => 4,
            }
        }

        /// `supported_items` by the item's kind: durability for all, the slot's own for the rest.
        pub fn fits(item: &str, id: &str) -> bool {
            if item == "minecraft:book" {
                return true;
            }
            match id {
                "minecraft:mending" => true,
                "minecraft:frost_walker" | "minecraft:soul_speed" => item.ends_with("_boots"),
                "minecraft:swift_sneak" => item.ends_with("_leggings"),
                "minecraft:wind_burst" => item == "minecraft:mace",
                _ => false,
            }
        }

        /// `exclusive_set/boots` (frost walker, depth strider) and `exclusive_set/bow` (infinity,
        /// mending).
        pub fn compatible(a: &str, b: &str) -> bool {
            let sets: [&[&str]; 2] = [
                &["minecraft:frost_walker", "minecraft:depth_strider"],
                &["minecraft:infinity", "minecraft:mending"],
            ];
            a != b && !sets.iter().any(|s| s.contains(&a) && s.contains(&b))
        }
    }

    fn treasures_of(item: &str, have: &[(&str, u32)]) -> Levels {
        let order = treasure_order(&vanilla::tag(), vanilla::is_curse);
        treasures(&order, &list(have), |id| vanilla::fits(item, id), vanilla::compatible, vanilla::max_of)
    }

    #[test]
    fn the_treasures_come_mending_first_without_the_curses() {
        let order = treasure_order(&vanilla::tag(), vanilla::is_curse);
        assert_eq!(
            order,
            ["minecraft:mending", "minecraft:swift_sneak", "minecraft:soul_speed", "minecraft:frost_walker", "minecraft:wind_burst"]
        );
        let twice: Vec<String> = ["minecraft:mending", "minecraft:mending"].map(str::to_owned).to_vec();
        assert_eq!(treasure_order(&twice, vanilla::is_curse), ["minecraft:mending"]);
    }

    #[test]
    fn an_item_is_offered_the_treasures_that_go_on_it_at_their_most() {
        assert_eq!(treasures_of("minecraft:diamond_sword", &[]), list(&[("minecraft:mending", 1)]));
        assert_eq!(treasures_of("minecraft:diamond_pickaxe", &[("minecraft:efficiency", 5)]), list(&[("minecraft:mending", 1)]));
        assert_eq!(
            treasures_of("minecraft:diamond_boots", &[]),
            list(&[("minecraft:mending", 1), ("minecraft:soul_speed", 3), ("minecraft:frost_walker", 2)])
        );
        // Depth Strider and Frost Walker exclude each other; Mending already there is not offered.
        assert_eq!(
            treasures_of("minecraft:diamond_boots", &[("minecraft:depth_strider", 3), ("minecraft:mending", 1)]),
            list(&[("minecraft:soul_speed", 3)])
        );
        assert_eq!(treasures_of("minecraft:netherite_leggings", &[]), list(&[("minecraft:mending", 1), ("minecraft:swift_sneak", 3)]));
        assert_eq!(treasures_of("minecraft:mace", &[]), list(&[("minecraft:mending", 1), ("minecraft:wind_burst", 3)]));
        // Infinity and Mending exclude each other: a bow with Infinity gets none.
        assert_eq!(treasures_of("minecraft:bow", &[("minecraft:infinity", 1)]), Vec::new());
        assert_eq!(treasures_of("minecraft:bow", &[("minecraft:power", 5)]), list(&[("minecraft:mending", 1)]));
        // A book takes all five; an enchanted book with Infinity all but Mending.
        let book = treasures_of("minecraft:book", &[]);
        assert_eq!(book.len(), 5);
        assert_eq!(book[0], ("minecraft:mending".to_owned(), 1));
        assert_eq!(treasures_of("minecraft:book", &[("minecraft:infinity", 1)]).len(), 4);
    }

    #[test]
    fn a_chosen_treasure_gives_its_level_or_says_why_not() {
        let boots = treasures_of("minecraft:diamond_boots", &[]);
        assert_eq!(treasure(&boots, &[], "minecraft:frost_walker"), Ok(2));
        assert_eq!(treasure(&boots, &[], "minecraft:swift_sneak"), Err(TreasureRefusal::Unfit));
        let have = list(&[("minecraft:mending", 1)]);
        let mended = treasures_of("minecraft:diamond_boots", &[("minecraft:mending", 1)]);
        assert_eq!(treasure(&mended, &have, "minecraft:mending"), Err(TreasureRefusal::Has));
        let bow = treasures_of("minecraft:bow", &[("minecraft:infinity", 1)]);
        assert_eq!(treasure(&bow, &list(&[("minecraft:infinity", 1)]), MENDING), Err(TreasureRefusal::Unfit));
    }

    #[test]
    fn a_treasure_costs_twice_a_raise_and_ten_lapis() {
        let price = |id: &str| treasure_price(vanilla::anvil(id), vanilla::max_of(id));
        assert_eq!(price("minecraft:mending"), RaisePrice { levels: 8, lapis: 10 });
        assert_eq!(price("minecraft:frost_walker"), RaisePrice { levels: 16, lapis: 10 });
        assert_eq!(price("minecraft:wind_burst"), RaisePrice { levels: 24, lapis: 10 });
        assert_eq!(price("minecraft:soul_speed").levels, RAISE_MAX_LEVELS, "8 × 3 × 2 = 48, capped");
        assert_eq!(treasure_price(0, 0), RaisePrice { levels: 2, lapis: TREASURE_LAPIS });
        for id in ["minecraft:mending", "minecraft:soul_speed"] {
            let raise = raise_price(vanilla::anvil(id), vanilla::max_of(id));
            assert!(price(id).levels >= raise.levels && price(id).lapis > raise.lapis);
        }
    }

    #[test]
    fn the_treasures_page_by_four_round() {
        let book = treasures_of("minecraft:book", &[]);
        assert_eq!(treasure_pages(book.len()), 2);
        assert_eq!(treasure_pages(0), 1);
        assert_eq!(treasure_pages(4), 1);
        assert_eq!(treasure_page(&book, 0).len(), OPTION_ROWS);
        assert_eq!(treasure_page(&book, 1), &book[4..]);
        assert_eq!(treasure_page(&book, 2), treasure_page(&book, 0), "past the last page: the first");
        assert!(treasure_page(&[], 3).is_empty());
        assert_eq!(next_page(0, book.len()), 1);
        assert_eq!(next_page(1, book.len()), 0);
        assert_eq!(next_page(0, 3), 0);
    }

    #[test]
    fn the_three_views_map_their_buttons() {
        assert_eq!(press(VIEW_TREASURE, View::New), Some(Action::Show(View::Treasure)));
        assert_eq!(press(VIEW_TREASURE, View::Raise), Some(Action::Show(View::Treasure)));
        assert_eq!(press(VIEW_TREASURE, View::Treasure), Some(Action::NextPage), "again: the next four");
        assert_eq!(press(VIEW_NEW, View::Treasure), Some(Action::Show(View::New)));
        assert_eq!(press(VIEW_RAISE, View::Treasure), Some(Action::Show(View::Raise)));
        assert_eq!(press(OPTION_BUTTONS.start, View::Treasure), Some(Action::TreasureRow(0)));
        assert_eq!(press(OPTION_BUTTONS.end - 1, View::Treasure), Some(Action::TreasureRow(3)));
        assert_eq!(press(0, View::Treasure), None, "no level in «Особые»");
        assert_eq!(press(REMOVE_BUTTONS.start, View::Treasure), None, "no «×» in «Особые»");
        assert_eq!(press(KEEP_BUTTON, View::Treasure), Some(Action::Keep));
        assert_eq!(VIEW_TREASURE + 1, BUTTONS);
        assert_eq!(View::of(View::Treasure.code()), View::Treasure);
        assert_eq!(View::of(7), View::New);
        assert_eq!(Pick::of(Pick::Treasure.code()), Pick::Treasure);
        assert_eq!(chosen(Pick::Treasure, &[list(&[("a", 1)]), list(&[]), list(&[])]), None);
    }

    #[test]
    fn the_seed_is_stable_for_a_place_and_changes_after_an_enchanting() {
        let a = first_seed([10, 64, -3]);
        assert_eq!(a, first_seed([10, 64, -3]));
        assert_ne!(a, first_seed([11, 64, -3]));
        assert!(a > 0);
        assert_ne!(next_seed(a, 0.25), a);
        let same = ((0.5 * f64::from(i32::MAX)) as i32).max(1);
        assert_ne!(next_seed(same, 0.5), same, "never the old seed again");
        assert!(next_seed(5, 0.0) >= 1);
    }

    #[test]
    fn an_enchanted_item_is_offered_only_what_it_lacks_and_goes_with_it() {
        let pool: Vec<String> = ["minecraft:sharpness", "minecraft:smite", "minecraft:looting", "minecraft:unbreaking"]
            .map(str::to_owned)
            .to_vec();
        let damage = ["minecraft:sharpness", "minecraft:smite"];
        let compatible = |a: &str, b: &str| a != b && !(damage.contains(&a) && damage.contains(&b));
        let have = list(&[("minecraft:sharpness", 2)]);
        let open = open_pool(&pool, &have, compatible);
        assert_eq!(open, vec!["minecraft:looting", "minecraft:unbreaking"]);
        assert_eq!(open_pool(&pool, &[], compatible).len(), 4);
    }

    #[test]
    fn it_takes_from_the_storage_only_the_lapis_the_next_enchanting_lacks() {
        assert_eq!(lapis_to_take(true, 0, 3, 64), 3);
        assert_eq!(lapis_to_take(true, 1, 7, 64), 6);
        assert_eq!(lapis_to_take(true, 3, 3, 64), 0);
        assert_eq!(lapis_to_take(false, 0, 3, 64), 0, "an idle machine keeps no lapis (0.21 intake)");
        assert_eq!(lapis_to_take(true, 62, 70, 64), 2, "never past a stack");
    }

    #[test]
    fn an_enchantment_comes_off_and_the_rest_stay() {
        let have = list(&[("minecraft:sharpness", 4), ("minecraft:looting", 2)]);
        assert_eq!(remove(&have, "minecraft:sharpness"), Some(list(&[("minecraft:looting", 2)])));
        assert_eq!(remove(&list(&[("minecraft:mending", 1)]), "minecraft:mending"), Some(Vec::new()));
        assert_eq!(remove(&have, "minecraft:smite"), None);
        assert_eq!(REMOVE_LAPIS, 1);
    }

    #[test]
    fn the_anvil_keeps_a_level_above_the_most() {
        // The owner: a book of Efficiency VI on a pickaxe of Efficiency V gave V.
        assert_eq!(anvil_level(5, 6, 5), 6);
        assert_eq!(anvil_level(6, 5, 5), 6);
        assert_eq!(anvil_level(6, 0, 5), 6, "a pickaxe of VI with a book of another enchantment");
        // Equal levels: one up to the most, as the game's, never past it.
        assert_eq!(anvil_level(4, 4, 5), 5);
        assert_eq!(anvil_level(5, 5, 5), 5);
        assert_eq!(anvil_level(6, 6, 5), 6);
        assert_eq!(anvil_level(2, 3, 3), 3);
    }

    #[test]
    fn a_raise_costs_like_the_anvil_and_stops_at_the_most() {
        // Sharpness (anvil cost 1) IV → V: 5 levels, 7 lapis; thorns (8) II → III: 24 levels.
        assert_eq!(raise_price(1, 5), RaisePrice { levels: 5, lapis: 7 });
        assert_eq!(raise_price(8, 3), RaisePrice { levels: 24, lapis: 5 });
        assert_eq!(raise_price(8, 9).levels, RAISE_MAX_LEVELS);
        assert_eq!(raise_price(0, 1), RaisePrice { levels: 1, lapis: 3 });
        assert!(raise_points(raise_price(1, 5)) > 0);
        let have = list(&[("minecraft:sharpness", 4), ("minecraft:unbreaking", 3)]);
        assert_eq!(raise(&have, "minecraft:sharpness", 5), Ok(5));
        assert_eq!(raise(&have, "minecraft:unbreaking", 3), Err(RaiseRefusal::AtMax));
        assert_eq!(raise(&have, "minecraft:looting", 3), Err(RaiseRefusal::Missing));
        let rows = raise_rows(&have, |id| if id == "minecraft:sharpness" { 5 } else { 3 });
        assert_eq!(rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["minecraft:sharpness", "minecraft:unbreaking"]);
        assert!(!rows[0].at_max() && rows[1].at_max());
        let many: Levels = (0..6).map(|i| (format!("e{i}"), 1)).collect();
        let rows = raise_rows(&many, |id| if id == "e5" { 3 } else { 1 });
        assert_eq!(rows.len(), OPTION_ROWS);
        assert_eq!(rows[0].id, "e5", "what can go up comes first");
    }
}
