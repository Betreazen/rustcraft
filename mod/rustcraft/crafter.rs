// The autocrafter `rustcraft:crafter` (guide `crafter`, spec 4.4, C1–C5): pages, each a 3×3
// pattern the player lays out in its window; the crafting table's own matcher says what a pattern
// makes (`Recipes::find`, SRC/crates/horizon-client/src/crafting.rs:1776-1803, shaped and
// shapeless alike, with the remainders a craft leaves, `Recipes::remainders`, crafting.rs:
// 1906-1959). The net fills its buffer with what its pages need (M17; rustcraft_machines::
// slot_takes, one kind a slot) and takes its products; it crafts the pages it can in turn (C3.a)
// and stops when its output has no room (C4). One page by default and one more for each capacity
// module in its module slots (C2), at most four.
//
// The pattern is ghost items: no slot holds them. A pattern cell is a button of the window
// (rustcraft_panels); a press reaches the server as `native_menu_button_v2` with the player's
// cursor stack (native_hooks.rs:4286-4297, fired by `mod_menu_button`, native_hooks.rs:
// 12133-12146), and the cell takes that stack's item, the stack staying where it is; a press with
// an empty cursor clears the cell. The window sees the pattern through its data slots, items as
// numbers (rustcraft_content::item_number).
use crate::game::{
    Game,
    rustcraft_content::{self as content, item_number},
    rustcraft_machines::{
        self as machines, MachineRecipes, read_f64, read_int, read_work, slot_of, write_f64,
        write_int, write_work,
    },
};
use horizon_client::{containers::ModBlockEntity, items::ItemStack};
use horizon_world::nbt::{Compound, JavaString, Tag};
use rustcraft_rules::{
    geom::Pos,
    machines::{
        crafter::{CraftRecipe, Crafter, CrafterSpec, Page, Product},
        processor::{Shown, WorkMode},
        slots::{self, Slot},
        windows::crafter::{self as window, CELLS, CrafterPanel, MAX_PAGES},
    },
    tuning::Tuning,
};

/// Saved compound keys.
const PAGES_KEY: &str = "pages";
const CELLS_KEY: &str = "cells";
const SHOWN_KEY: &str = "page";
const NEXT_KEY: &str = "next";
const CURRENT_KEY: &str = "current";
const PROGRESS_KEY: &str = "progress";

/// A page's pattern: an item a cell, row by row.
pub type Pattern = [Option<String>; CELLS];

fn read_patterns(nbt: &Compound) -> Vec<Pattern> {
    let Some(Tag::List(pages)) = nbt.get(&JavaString::from(PAGES_KEY)) else {
        return Vec::new();
    };
    pages
        .iter()
        .map(|page| {
            let cells = match page {
                Tag::Compound(c) => match c.get(&JavaString::from(CELLS_KEY)) {
                    Some(Tag::List(cells)) => cells.clone(),
                    _ => Vec::new(),
                },
                _ => Vec::new(),
            };
            std::array::from_fn(|i| match cells.get(i) {
                Some(Tag::String(s)) => Some(s.to_string_lossy()).filter(|s| !s.is_empty()),
                _ => None,
            })
        })
        .collect()
}

fn write_patterns(nbt: &mut Compound, patterns: &[Pattern]) {
    let list = patterns
        .iter()
        .map(|p| {
            let cells = p
                .iter()
                .map(|c| Tag::String(JavaString::from(c.as_deref().unwrap_or(""))))
                .collect();
            Tag::compound([(CELLS_KEY, Tag::List(cells))])
        })
        .collect();
    nbt.insert(JavaString::from(PAGES_KEY), Tag::List(list));
}

/// Pages it has now: the base, and one a capacity module in its module slots (C2).
pub(super) fn allowed_pages(entity: &ModBlockEntity, tuning: &Tuning) -> usize {
    let modules = content::CRAFTER_MODULES
        .filter(|&i| slot_of(entity, i).is_some_and(|(item, _)| item == content::MODULE_CAPACITY))
        .count();
    (tuning.crafter_pages + modules).clamp(1, MAX_PAGES)
}

/// The page the window shows, within the pages it has.
fn shown_page(nbt: &Compound, allowed: usize) -> usize {
    read_int(nbt, SHOWN_KEY)
        .and_then(|p| usize::try_from(p).ok())
        .unwrap_or(0)
        .min(allowed - 1)
}

/// What a pattern makes on the crafting table, and its result's stack; none when it makes
/// nothing. Asked once a pattern until the next rescan.
fn resolve<'r>(
    game: &Game<'_>,
    recipes: &'r mut MachineRecipes,
    pattern: &Pattern,
) -> &'r Option<(CraftRecipe, ItemStack)> {
    recipes
        .crafting
        .entry(pattern.clone())
        .or_insert_with(|| matched(game, pattern))
}

fn matched(game: &Game<'_>, pattern: &Pattern) -> Option<(CraftRecipe, ItemStack)> {
    if pattern.iter().all(Option::is_none) {
        return None;
    }
    let grid: Vec<Option<ItemStack>> = pattern
        .iter()
        .map(|c| c.as_ref().map(|item| ItemStack::new(item, 1)))
        .collect();
    let (name, result) = game.recipes().find(&grid, 3)?;
    let name = name.to_owned();
    if result.count == 0 {
        return None;
    }
    let mut products = vec![Product {
        item: result.item.to_string(),
        count: result.count,
        stack: machines::max_stack(game, &result.item),
    }];
    for rest in game
        .recipes()
        .remainders(&name, &grid, 3)
        .into_iter()
        .flatten()
    {
        match products.iter_mut().skip(1).find(|p| *p.item == *rest.item) {
            Some(p) => p.count += rest.count,
            None => products.push(Product {
                item: rest.item.to_string(),
                count: rest.count,
                stack: machines::max_stack(game, &rest.item),
            }),
        }
    }
    Some((CraftRecipe::of_pattern(pattern, products), result))
}

pub(super) struct Held {
    crafter: Crafter,
    patterns: Vec<Pattern>,
    shown: usize,
    /// Each page's result stack, components and all.
    results: Vec<Option<ItemStack>>,
    /// The buffer's slots as the rules see them.
    buffer: Vec<Slot>,
    /// E a second it asks for this tick, and in its mode when it has work.
    pub request: f64,
    nominal: f64,
    data: Vec<i32>,
}

fn spec(tuning: &Tuning) -> CrafterSpec {
    CrafterSpec {
        ticks: tuning.crafter_ticks,
        energy: tuning.crafter_energy,
    }
}

pub(super) fn read(
    game: &Game<'_>,
    pos: Pos,
    recipes: &mut MachineRecipes,
    tuning: &Tuning,
) -> Option<Held> {
    let entity = game.native_mod_block_entity(pos)?;
    let allowed = allowed_pages(&entity, tuning);
    let mut patterns = read_patterns(&entity.nbt);
    patterns.resize(allowed.max(patterns.len()), Default::default());
    let mut pages = Vec::new();
    let mut results = Vec::new();
    for pattern in patterns.iter().take(allowed) {
        let resolved = resolve(game, recipes, pattern).clone();
        results.push(resolved.as_ref().map(|(_, r)| r.clone()));
        pages.push(Page {
            recipe: resolved.map(|(r, _)| r),
            order: None,
        });
    }
    let buffer: Vec<Slot> = content::CRAFTER_BUFFER
        .map(|i| slot_of(&entity, i))
        .collect();
    let mut stock: Vec<(String, u32)> = Vec::new();
    for (item, n) in buffer.iter().flatten() {
        match stock.iter_mut().find(|(i, _)| i == item) {
            Some((_, c)) => *c += n,
            None => stock.push((item.clone(), *n)),
        }
    }
    let nbt = &entity.nbt;
    let current = read_int(nbt, CURRENT_KEY)
        .and_then(|c| usize::try_from(c).ok())
        .filter(|&c| c < allowed);
    let crafter = Crafter {
        pages,
        buffer: stock,
        output: content::CRAFTER_OUTPUTS
            .map(|i| slot_of(&entity, i))
            .collect(),
        next_page: read_int(nbt, NEXT_KEY)
            .and_then(|n| usize::try_from(n).ok())
            .unwrap_or(0)
            % allowed,
        current,
        progress: read_f64(nbt, PROGRESS_KEY).clamp(0.0, 1.0),
        mode: read_work(nbt),
    };
    let spec = spec(tuning);
    Some(Held {
        request: crafter.request(&spec),
        nominal: spec.energy * crafter.mode.factors().1,
        shown: shown_page(nbt, allowed),
        crafter,
        patterns,
        results,
        buffer,
        data: entity.data.clone(),
    })
}

/// One tick on `share` of its request; writes back the buffer, the output, its progress and its
/// window. What it shows.
pub(super) fn run(
    game: &mut Game<'_>,
    pos: Pos,
    held: Held,
    share: f64,
    tuning: &Tuning,
) -> Option<Shown> {
    let Held {
        mut crafter,
        patterns,
        shown: page,
        results,
        mut buffer,
        request,
        nominal,
        data: shown_data,
    } = held;
    let before = crafter.clone();
    let state = crafter.tick(&spec(tuning), share);
    let shown = Shown::of(state, share);
    let data = window::encode(&panel(
        game,
        &crafter,
        &patterns,
        &results,
        page,
        shown,
        share * request,
        nominal,
    ));
    if crafter == before && data == shown_data {
        return Some(shown);
    }
    let buffer_before = buffer.clone();
    for (item, n) in &before.buffer {
        let used = n.saturating_sub(crafter.has(item));
        if used > 0 {
            slots::take_from(&mut buffer, item, used);
        }
    }
    let made: Vec<ItemStack> = results.into_iter().flatten().collect();
    game.native_with_mod_block_entity(pos, move |entity| {
        machines::write_slots(
            entity,
            content::CRAFTER_BUFFER,
            &buffer_before,
            &buffer,
            &[],
        );
        machines::write_slots(
            entity,
            content::CRAFTER_OUTPUTS,
            &before.output,
            &crafter.output,
            &made,
        );
        let nbt = &mut entity.nbt;
        if crafter.progress != before.progress {
            write_f64(nbt, PROGRESS_KEY, crafter.progress);
        }
        if crafter.next_page != before.next_page {
            write_int(nbt, NEXT_KEY, crafter.next_page as i32);
        }
        if crafter.current != before.current {
            write_int(nbt, CURRENT_KEY, crafter.current.map_or(-1, |c| c as i32));
        }
        if entity.data != data {
            entity.data = data;
        }
        shown
    })
}

/// What the window shows: the open page's pattern and result, every page's tab.
#[allow(clippy::too_many_arguments)]
fn panel(
    game: &Game<'_>,
    crafter: &Crafter,
    patterns: &[Pattern],
    results: &[Option<ItemStack>],
    page: usize,
    shown: Shown,
    got: f64,
    asked: f64,
) -> CrafterPanel {
    let number = |item: &str| item_number(game, item);
    let result = results.get(page).cloned().flatten();
    CrafterPanel {
        progress: crafter.progress,
        got: if shown == Shown::Working || matches!(shown, Shown::Slowed { .. }) {
            got
        } else {
            0.0
        },
        asked,
        mode: crafter.mode,
        shown: Some(shown),
        page,
        pages: results.len(),
        crafting: crafter.current,
        cells: std::array::from_fn(|i| {
            patterns
                .get(page)
                .and_then(|p| p[i].as_deref())
                .and_then(number)
        }),
        result: result.as_ref().and_then(|r| number(&r.item)),
        result_count: result.as_ref().map_or(0, |r| r.count),
        tabs: std::array::from_fn(|i| {
            results
                .get(i)
                .and_then(Option::as_ref)
                .and_then(|r| number(&r.item))
        }),
    }
}

// ---- Its window's buttons ----

/// A work mode, or a page's tab (shown with the next data slots).
pub(super) fn on_menu_button(entity: &mut ModBlockEntity, id: i32, tuning: &Tuning) {
    if let Some(mode) = u8::try_from(id).ok().and_then(WorkMode::from_code) {
        write_work(&mut entity.nbt, mode);
        return;
    }
    let tab = id - window::TAB_BUTTON;
    let allowed = allowed_pages(entity, tuning);
    if let Ok(tab) = usize::try_from(tab)
        && tab < allowed
    {
        write_int(&mut entity.nbt, SHOWN_KEY, tab as i32);
    }
}

/// A pattern cell of the open page takes the item on the player's cursor, or is cleared when
/// the cursor is empty. The cursor keeps its stack.
pub(super) fn on_cell_button(
    entity: &mut ModBlockEntity,
    id: i32,
    carried: &Option<ItemStack>,
    tuning: &Tuning,
) {
    let Some(cell) = usize::try_from(id - window::CELL_BUTTON)
        .ok()
        .filter(|&c| c < CELLS)
    else {
        return;
    };
    let allowed = allowed_pages(entity, tuning);
    let page = shown_page(&entity.nbt, allowed);
    let mut patterns = read_patterns(&entity.nbt);
    if patterns.len() <= page {
        patterns.resize(page + 1, Default::default());
    }
    let item = carried
        .as_ref()
        .filter(|s| s.count > 0 && &*s.item != "minecraft:air")
        .map(|s| s.item.to_string());
    if patterns[page][cell] == item {
        return;
    }
    patterns[page][cell] = item;
    write_patterns(&mut entity.nbt, &patterns);
    // A changed page starts its craft again.
    if read_int(&entity.nbt, CURRENT_KEY) == Some(page as i32) {
        write_int(&mut entity.nbt, CURRENT_KEY, -1);
        write_f64(&mut entity.nbt, PROGRESS_KEY, 0.0);
    }
}

/// Whether buffer slot `slot` takes `item` from the net or a hopper: an item of a pattern of a
/// page it has, one kind a slot (M17).
pub(super) fn buffer_takes(
    entity: &ModBlockEntity,
    slot: usize,
    item: &str,
    tuning: &Tuning,
) -> bool {
    let allowed = allowed_pages(entity, tuning);
    let wanted = read_patterns(&entity.nbt)
        .iter()
        .take(allowed)
        .flatten()
        .any(|c| c.as_deref() == Some(item));
    if !wanted {
        return false;
    }
    let buffer: Vec<Slot> = content::CRAFTER_BUFFER
        .map(|i| slot_of(entity, i))
        .collect();
    slots::one_kind_takes(&buffer, slot - content::CRAFTER_BUFFER.start, item)
}
