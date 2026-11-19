//! 0.21 (spec 47, OT1–OT5): an input that takes any item of a group (the game's tags: planks of
//! any wood, coal or charcoal, wool of any colour) takes what the storage has or can make, not
//! the group's first item. The owner: «склад при заказе книжных полок почему-то ждёт только
//! определённое бревно, а не берёт то, что есть в складу» — thousands of birch logs, and the
//! order for bookshelves waited for oak logs, since it always made the planks of the first wood.
//!
//! OT1: an item the stock covers whole, the one it has most of. OT2: else each item in turn —
//! the most in stock first, then the ones a recipe makes from something in stock — as much of it
//! as its plan lacks nothing for, from stock or made, first on the machines the line has. OT3:
//! so an input mixes items: on the autocrafter's grid cell by cell (planks of two woods in one
//! bookshelf), on a machine run by run. OT4: what is left goes to the item that gathers most of
//! it, its own shortages named; when no item gathers anything more, the group is short («доски
//! любого дерева ×2»).

use super::groups::{group_key, group_of};
use super::{ItemKey, KINDS, Planner, PlanRecipe, taken};

/// Items of one input tried in turn before it is taken as short.
const MAX_ALT_TRIES: usize = 6;

/// What one input takes over all of a step's runs: each item and how many, in the order the runs
/// use them.
pub(super) type Share = Vec<(ItemKey, u64)>;

/// One part of a step whose runs all take the same items: the runs, each input's item and how
/// many one run takes, and the grid's cells when an input mixes items (OT3).
pub(super) type Segment = (u64, Vec<(ItemKey, u32)>, Vec<Option<usize>>);

impl Planner<'_> {
    fn short_of(&self, item: &str) -> u64 {
        self.plan.missing.get(item).copied().unwrap_or(0)
    }

    /// Takes the plan of a trial, keeping the budget the trials have left.
    fn adopt(&mut self, trial: Self) {
        let budget = self.budget;
        *self = trial;
        self.budget = budget;
    }

    /// Plans `qty` of `item` on a copy: the copy, and whether it lacks nothing more.
    fn try_need(&mut self, item: &str, qty: u64) -> (Self, bool) {
        let mut trial = self.copy();
        trial.need(item, qty);
        self.budget = trial.budget;
        let whole = trial.plan.missing == self.plan.missing;
        (trial, whole)
    }

    /// The items of an input worth trying, in the order they are tried: the most in stock
    /// first; then those a recipe makes from something in stock, then those a recipe makes;
    /// ties in the recipe's own order. An item neither in stock nor made by a recipe is left out.
    fn ranked(&self, alts: &[ItemKey]) -> Vec<ItemKey> {
        let reach = |a: &ItemKey| -> u8 {
            if self.free(a) > 0 {
                return 0;
            }
            if self.making.contains(a) {
                return 3;
            }
            let mut makes = false;
            for r in self.book.iter().filter(|r| &r.output.0 == a) {
                makes = true;
                if r.inputs.iter().any(|(xs, _)| xs.iter().any(|x| self.free(x) > 0)) {
                    return 1;
                }
            }
            if makes { 2 } else { 3 }
        };
        let mut order: Vec<(u64, u8, usize, &ItemKey)> = alts
            .iter()
            .enumerate()
            .map(|(i, a)| (self.free(a), reach(a), i, a))
            .filter(|&(_, r, _, _)| r < 3)
            .collect();
        order.sort_by_key(|&(free, r, i, _)| (std::cmp::Reverse(free), r, i));
        order.into_iter().map(|(_, _, _, a)| a.clone()).collect()
    }

    /// Arranges `want` of an input that takes any of `alts`, `grain` at a time (a run's worth on
    /// a machine, one item on the autocrafter's grid): its share, and whether not one of its
    /// items could be gathered at all.
    pub(super) fn gather(&mut self, alts: &[ItemKey], want: u64, grain: u64) -> (Share, bool) {
        let first = alts.first().cloned().unwrap_or_default();
        if alts.len() < 2 || want == 0 {
            let before = self.short_of(&first);
            self.need(&first, want);
            let none = want > 0 && self.short_of(&first) - before >= want;
            return (vec![(first, want)], none);
        }
        let order = self.ranked(alts);
        // OT1.
        if let Some(a) = order.iter().find(|a| self.free(a) >= want) {
            self.need(a, want);
            return (vec![(a.clone(), want)], false);
        }
        // OT2, OT3: each item in turn, as much as its plan lacks nothing for: first on the
        // machines the line has, then, when an item lacks only a machine, on any (the order then
        // names the machine, O13).
        let grain = grain.max(1);
        let mut share: Share = Vec::new();
        let mut left = want;
        let mut seen = Seen::default();
        for strict in [true, false] {
            if !strict && !seen.machine_only {
                break;
            }
            let mut tries = 0;
            for a in &order {
                if left == 0 || self.budget == 0 {
                    break;
                }
                if self.free(a) == 0 {
                    if tries == MAX_ALT_TRIES {
                        break;
                    }
                    tries += 1;
                }
                let units = self.most_of(a, left, grain, strict, &mut seen);
                if units > 0 {
                    self.need(a, units);
                    match share.iter_mut().find(|(i, _)| i == a) {
                        Some((_, n)) => *n += units,
                        None => share.push((a.clone(), units)),
                    }
                    left -= units;
                }
            }
        }
        if left == 0 {
            return (share, false);
        }
        // OT4.
        let mut best: Option<(u64, ItemKey, Self)> = None;
        for a in order.iter().take(if seen.more { MAX_ALT_TRIES } else { 0 }) {
            let (trial, _) = self.try_need(a, left);
            let got = taken(&trial.plan) - taken(&self.plan);
            if got > 0 && best.as_ref().is_none_or(|(b, _, _)| got > *b) {
                best = Some((got, a.clone(), trial));
            }
        }
        if let Some((_, a, trial)) = best {
            self.adopt(trial);
            share.push((a, left));
            return (share, false);
        }
        let short = group_of(alts).map_or_else(|| first.clone(), group_key);
        self.short(&short, left);
        let none = share.is_empty();
        share.push((first, left));
        (share, none)
    }

    /// The most of `item`, `grain` at a time and at most `left`, its plan lacks nothing for (nor
    /// a machine, when `strict`): all of it, else what the stock has, else halving between.
    fn most_of(&mut self, item: &str, left: u64, grain: u64, strict: bool, seen: &mut Seen) -> u64 {
        let in_stock = self.free(item).min(left) / grain;
        let fits = |p: &mut Self, qty: u64| -> (bool, bool, u64) {
            let (trial, whole) = p.try_need(item, qty);
            let machines = trial.plan.missing_machines == p.plan.missing_machines;
            (whole && (machines || !strict), whole && !machines, taken(&trial.plan) - taken(&p.plan))
        };
        let (whole, machine_only, got) = fits(self, left);
        seen.machine_only |= machine_only;
        if whole {
            return left;
        }
        // Nothing gathers beyond the stock: no halving.
        if got <= in_stock * grain || machine_only {
            return in_stock * grain;
        }
        seen.more = true;
        let (mut lo, mut hi) = (in_stock, left / grain);
        while hi - lo > 1 && self.budget > 0 {
            let mid = lo + (hi - lo) / 2;
            if fits(self, mid * grain).0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        lo * grain
    }
}

/// What the trials of an input found.
#[derive(Default)]
struct Seen {
    /// An item could be made whole but on a machine the line lacks.
    machine_only: bool,
    /// An item's plan gathered more than the stock held of it, though not all.
    more: bool,
}

/// The items one run of an input takes, run `run` of a share whose runs take `n` each.
fn run_items(share: &Share, n: u64, run: u64) -> Vec<&ItemKey> {
    let mut out = Vec::with_capacity(usize::try_from(n).unwrap_or(0));
    let (from, to) = (run * n, run * n + n);
    let mut at = 0;
    for (item, k) in share {
        let (a, b) = (at.max(from), (at + k).min(to));
        for _ in a..b {
            out.push(item);
        }
        at += k;
    }
    out
}

/// A step's `times` runs of `recipe` cut where an input's items change (OT3): runs that take the
/// same items are one segment.
pub(super) fn segments(recipe: &PlanRecipe, shares: &[Share], times: u64) -> Vec<Segment> {
    let per_run: Vec<u64> = recipe.inputs.iter().map(|(_, n)| u64::from(*n)).collect();
    let mut cuts = vec![0, times];
    for (share, &n) in shares.iter().zip(&per_run) {
        if n == 0 {
            continue;
        }
        let mut at = 0;
        for (_, k) in share {
            at += k;
            cuts.push(at / n);
            if at % n != 0 {
                cuts.push(at / n + 1);
            }
        }
    }
    cuts.retain(|&c| c <= times);
    cuts.sort_unstable();
    cuts.dedup();
    let grid = recipe.machine == KINDS[0] && !recipe.cells.is_empty();
    cuts.windows(2)
        .map(|w| {
            let items: Vec<Vec<&ItemKey>> = shares
                .iter()
                .zip(&per_run)
                .map(|(share, &n)| run_items(share, n, w[0]))
                .collect();
            let (inputs, cells) = one_run(recipe, &items, grid);
            (w[1] - w[0], inputs, cells)
        })
        .collect()
}

/// A run's inputs from each input's items: an input of one item is one entry, as the recipe
/// has it; when one mixes items on the grid, the step's own entries, one per item, and its
/// cells.
fn one_run(recipe: &PlanRecipe, items: &[Vec<&ItemKey>], grid: bool) -> (Vec<(ItemKey, u32)>, Vec<Option<usize>>) {
    let uniform = items.iter().all(|xs| xs.windows(2).all(|w| w[0] == w[1]));
    if uniform || !grid {
        let inputs = items
            .iter()
            .zip(&recipe.inputs)
            .map(|(xs, (alts, n))| {
                let item = xs.first().copied().or(alts.first()).cloned().unwrap_or_default();
                (item, *n)
            })
            .collect();
        return (inputs, Vec::new());
    }
    let mut entries: Vec<(ItemKey, u32)> = Vec::new();
    let mut used = vec![0usize; items.len()];
    let cells = recipe
        .cells
        .iter()
        .map(|cell| {
            let k = (*cell)?;
            let item = items.get(k)?.get(used[k])?;
            used[k] += 1;
            let e = match entries.iter().position(|(i, _)| i == *item) {
                Some(e) => e,
                None => {
                    entries.push(((*item).clone(), 0));
                    entries.len() - 1
                }
            };
            entries[e].1 += 1;
            Some(e)
        })
        .collect();
    (entries, cells)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::super::{Plan, PlanRecipe, plan};
    use super::*;

    const WOODS: [&str; 3] = ["oak", "spruce", "birch"];
    const ALL: &[&str] = &["crafter", "furnace"];
    const SHELF: &str = "minecraft:bookshelf";
    const BOOK: &str = "minecraft:book";

    fn planks() -> Vec<ItemKey> {
        WOODS.iter().map(|w| format!("minecraft:{w}_planks")).collect()
    }

    fn logs_of(wood: &str) -> Vec<ItemKey> {
        vec![
            format!("minecraft:{wood}_log"),
            format!("minecraft:{wood}_wood"),
            format!("minecraft:stripped_{wood}_log"),
            format!("minecraft:stripped_{wood}_wood"),
        ]
    }

    fn all_logs() -> Vec<ItemKey> {
        WOODS.iter().flat_map(|w| logs_of(w)).collect()
    }

    fn one(item: &str) -> Option<Vec<ItemKey>> {
        Some(vec![item.to_string()])
    }

    fn any(items: &[&str]) -> Option<Vec<ItemKey>> {
        Some(items.iter().map(|i| (*i).to_string()).collect())
    }

    /// The game's recipes in small, as the book builds them: planks of each wood from its logs,
    /// bookshelves, sticks, torches with coal or charcoal, charcoal from any log, a painting
    /// with wool of any colour and a stone pickaxe of any stone.
    fn book() -> Vec<PlanRecipe> {
        let mut book = Vec::new();
        for w in WOODS {
            let cells = [Some(logs_of(w))];
            book.push(PlanRecipe::crafting(1, &cells, (format!("minecraft:{w}_planks"), 4)).unwrap());
        }
        let p = Some(planks());
        let b = one(BOOK);
        let shelf = [p.clone(), p.clone(), p.clone(), b.clone(), b.clone(), b, p.clone(), p.clone(), p.clone()];
        book.push(PlanRecipe::crafting(3, &shelf, (SHELF.into(), 1)).unwrap());
        book.push(PlanRecipe::crafting(1, &[p.clone(), p], ("minecraft:stick".into(), 4)).unwrap());
        let coals = any(&["minecraft:coal", "minecraft:charcoal"]);
        let st = one("minecraft:stick");
        book.push(PlanRecipe::crafting(1, &[coals, st.clone()], ("minecraft:torch".into(), 4)).unwrap());
        book.push(PlanRecipe {
            machine: "furnace".into(),
            inputs: vec![(all_logs(), 1)],
            output: ("minecraft:charcoal".into(), 1),
            cells: Vec::new(),
        });
        let wool = any(&["minecraft:white_wool", "minecraft:red_wool"]);
        let painting = [st.clone(), st.clone(), st.clone(), st.clone(), wool, st.clone(), st.clone(), st.clone(), st.clone()];
        book.push(PlanRecipe::crafting(3, &painting, ("minecraft:painting".into(), 1)).unwrap());
        let stone = any(&["minecraft:cobblestone", "minecraft:blackstone", "minecraft:cobbled_deepslate"]);
        let pick = [stone.clone(), stone.clone(), stone, None, st.clone(), None, None, st, None];
        book.push(PlanRecipe::crafting(3, &pick, ("minecraft:stone_pickaxe".into(), 1)).unwrap());
        book
    }

    fn stock(items: &[(&str, u64)]) -> BTreeMap<ItemKey, u64> {
        items.iter().map(|(i, n)| (i.to_string(), *n)).collect()
    }

    /// What the plan's steps make, by item.
    fn made(p: &Plan) -> BTreeMap<&str, u64> {
        let mut out = BTreeMap::new();
        for s in &p.steps {
            *out.entry(s.output.0.as_str()).or_insert(0) += u64::from(s.times) * u64::from(s.output.1);
        }
        out
    }

    #[test]
    fn bookshelves_take_planks_from_the_birch_logs_the_storage_holds() {
        // OT2, the owner's storage: thousands of birch logs, no oak.
        let book = book();
        let s = stock(&[("minecraft:birch_log", 3000), (BOOK, 30)]);
        let p = plan(SHELF, 2, &s, &book, ALL);
        assert!(p.ready(), "{p:?}");
        assert_eq!(p.reserved, stock(&[("minecraft:birch_log", 3), (BOOK, 6)]));
        assert_eq!(made(&p)["minecraft:birch_planks"], 12);
        let shelf = p.steps.iter().find(|s| s.output.0 == SHELF).unwrap();
        assert_eq!(shelf.pattern(&book)[0].as_deref(), Some("minecraft:birch_planks"));
        assert_eq!(shelf.pattern(&book)[4].as_deref(), Some(BOOK));
    }

    #[test]
    fn the_wood_the_storage_has_most_of_goes_first() {
        // OT1: planks of two woods in stock, either covers: the more plentiful one is taken.
        let book = book();
        let s = stock(&[("minecraft:oak_planks", 10), ("minecraft:spruce_planks", 40), (BOOK, 3)]);
        let p = plan(SHELF, 1, &s, &book, ALL);
        assert!(p.ready(), "{p:?}");
        assert_eq!(p.reserved, stock(&[("minecraft:spruce_planks", 6), (BOOK, 3)]));
    }

    #[test]
    fn planks_of_two_woods_mix_on_the_grid() {
        // OT3: 3 oak and 4 birch planks, no logs: one bookshelf of both.
        let book = book();
        let s = stock(&[("minecraft:oak_planks", 3), ("minecraft:birch_planks", 4), (BOOK, 3)]);
        let p = plan(SHELF, 1, &s, &book, ALL);
        assert!(p.ready(), "{p:?}");
        assert_eq!(
            p.reserved,
            stock(&[("minecraft:birch_planks", 4), ("minecraft:oak_planks", 2), (BOOK, 3)])
        );
        assert_eq!(p.steps.len(), 1, "{p:?}");
        let step = &p.steps[0];
        let mut items = step.items();
        items.sort();
        assert_eq!(
            items,
            vec![
                ("minecraft:birch_planks".into(), 4),
                (BOOK.into(), 3),
                ("minecraft:oak_planks".into(), 2)
            ]
        );
        let pattern = step.pattern(&book);
        let count = |item: &str| pattern.iter().flatten().filter(|c| *c == item).count();
        assert_eq!((count("minecraft:birch_planks"), count("minecraft:oak_planks"), count(BOOK)), (4, 2, 3));
        assert!(pattern[3..6].iter().all(|c| c.as_deref() == Some(BOOK)), "{pattern:?}");
    }

    #[test]
    fn stock_planks_first_then_planks_from_logs() {
        // OT3: 6 oak planks and spruce logs: the first bookshelf of oak, the second of spruce.
        let book = book();
        let s = stock(&[("minecraft:oak_planks", 6), ("minecraft:spruce_log", 5), (BOOK, 6)]);
        let p = plan(SHELF, 2, &s, &book, ALL);
        assert!(p.ready(), "{p:?}");
        assert_eq!(
            p.reserved,
            stock(&[("minecraft:oak_planks", 6), ("minecraft:spruce_log", 2), (BOOK, 6)])
        );
        let shelves: Vec<_> = p.steps.iter().filter(|s| s.output.0 == SHELF).collect();
        assert_eq!(shelves.len(), 2, "{p:?}");
        assert!(shelves.iter().all(|s| s.times == 1 && s.cells.is_empty()), "{p:?}");
    }

    #[test]
    fn with_no_wood_the_order_names_planks_of_any_wood() {
        // OT4: «нужны доски любого дерева», not oak logs.
        let book = book();
        let p = plan(SHELF, 1, &stock(&[(BOOK, 3)]), &book, ALL);
        assert!(!p.ready());
        assert_eq!(p.missing, stock(&[("group:planks", 6)]));
    }

    #[test]
    fn too_little_wood_names_the_planks_still_short() {
        // OT4: a birch log gives 4 planks of the 6: 2 planks of any wood are short.
        let book = book();
        let p = plan(SHELF, 1, &stock(&[("minecraft:birch_log", 1), (BOOK, 3)]), &book, ALL);
        assert_eq!(p.missing, stock(&[("group:planks", 2)]), "{p:?}");
        assert_eq!(p.reserved["minecraft:birch_log"], 1);
    }

    #[test]
    fn sticks_and_torches_take_any_planks_and_charcoal() {
        // Sticks from spruce logs; torches with charcoal smelted from birch logs.
        let book = book();
        let s = stock(&[("minecraft:spruce_log", 1), ("minecraft:birch_log", 2)]);
        let p = plan("minecraft:torch", 8, &s, &book, ALL);
        assert!(p.ready(), "{p:?}");
        assert_eq!(made(&p)["minecraft:charcoal"], 2);
        assert_eq!(made(&p)["minecraft:spruce_planks"], 4);
        assert_eq!(made(&p)["minecraft:stick"], 4);
        assert_eq!(p.reserved, stock(&[("minecraft:birch_log", 2), ("minecraft:spruce_log", 1)]));
        // Coal in stock goes before smelting logs.
        let s = stock(&[("minecraft:coal", 5), ("minecraft:stick", 10), ("minecraft:oak_log", 9)]);
        let p = plan("minecraft:torch", 8, &s, &book, ALL);
        assert_eq!(p.reserved, stock(&[("minecraft:coal", 2), ("minecraft:stick", 2)]));
    }

    #[test]
    fn a_machine_takes_one_item_a_run_mixed_run_by_run() {
        // Charcoal of 3 oak and 2 birch logs: the furnace's runs, not a grid, so each run one log.
        let book = book();
        let s = stock(&[("minecraft:oak_log", 3), ("minecraft:birch_log", 2)]);
        let p = plan("minecraft:charcoal", 5, &s, &book, ALL);
        assert!(p.ready(), "{p:?}");
        let runs: BTreeMap<&str, u32> = p.steps.iter().map(|s| (s.inputs[0].0.as_str(), s.times)).collect();
        assert_eq!(runs, BTreeMap::from([("minecraft:birch_log", 2), ("minecraft:oak_log", 3)]));
        let p = plan("minecraft:charcoal", 6, &s, &book, ALL);
        assert_eq!(p.missing, stock(&[("group:logs", 1)]), "{p:?}");
    }

    #[test]
    fn wool_of_any_colour_and_stone_of_any_kind() {
        let book = book();
        let s = stock(&[("minecraft:red_wool", 1), ("minecraft:stick", 8)]);
        let p = plan("minecraft:painting", 1, &s, &book, ALL);
        assert!(p.ready(), "{p:?}");
        assert_eq!(p.reserved["minecraft:red_wool"], 1);
        let p = plan("minecraft:painting", 1, &stock(&[("minecraft:stick", 8)]), &book, ALL);
        assert_eq!(p.missing, stock(&[("group:wool", 1)]));
        let s = stock(&[("minecraft:blackstone", 3), ("minecraft:stick", 2)]);
        let p = plan("minecraft:stone_pickaxe", 1, &s, &book, ALL);
        assert!(p.ready(), "{p:?}");
        assert_eq!(p.reserved["minecraft:blackstone"], 3);
        let p = plan("minecraft:stone_pickaxe", 1, &stock(&[("minecraft:stick", 2)]), &book, ALL);
        assert_eq!(p.missing, stock(&[("group:stone", 3)]));
    }

    #[test]
    fn segments_cut_where_an_input_changes() {
        let book = book();
        let shelf = book.iter().find(|r| r.output.0 == SHELF).unwrap();
        // 2 bookshelves: 8 oak then 4 birch planks; books of one kind.
        let shares = vec![
            vec![("minecraft:oak_planks".to_string(), 8), ("minecraft:birch_planks".to_string(), 4)],
            vec![(BOOK.to_string(), 6)],
        ];
        let segs = segments(shelf, &shares, 2);
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0].1, vec![("minecraft:oak_planks".into(), 6), (BOOK.into(), 3)]);
        assert!(segs[0].2.is_empty());
        assert_eq!(
            segs[1].1,
            vec![("minecraft:oak_planks".into(), 2), ("minecraft:birch_planks".into(), 4), (BOOK.into(), 3)]
        );
        assert_eq!(segs[1].2.iter().flatten().count(), 9);
    }
}
