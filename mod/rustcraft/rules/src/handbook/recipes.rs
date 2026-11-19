//! A page's recipes (guide: every entry's `recipes`, app.js `recipeEl`): the crafting table's,
//! the transmuting ones that raise a tier, and the machines' (the crusher, the alloy smelter, the
//! mixer, the electric furnace's smelting).
//!
//! The window reads the game's own recipes and turns each into a `Shown`: a 3 × 3 grid, or a
//! machine's inputs with the machine over the arrow; what it makes is an item or a fluid. A page
//! shows those whose result its guide entry's recipes make (`Page::makes`), each tier of it
//! included: the alloys' page its four alloys, the blade's page the blade of tiers 2 to 4.
//! Two recipes stand side by side on a row of the page.

use super::Page;
use serde_json::Value;

/// The recipe types of the mod's machines, as the content mod's files name them.
pub const CRUSHING: &str = "rustcraft:crushing";
pub const ALLOYING: &str = "rustcraft:alloying";
pub const MIXING: &str = "rustcraft:mixing";
pub const SMELTING: &str = "smelting";
pub const MACHINE_TYPES: [&str; 3] = [CRUSHING, ALLOYING, MIXING];
/// Water a mixing recipe takes shows as buckets of water.
pub const WATER_BUCKET: &str = "minecraft:water_bucket";
pub const BUCKET_MB: u32 = 1000;
/// A page's `makes` entry that takes every fluid (the guide's `fluids` page).
pub const ANY_FLUID: &str = "#fluid";

/// How a recipe is made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Made {
    /// On the crafting table: the cells row by row, each the item it shows.
    Grid([Option<String>; 9]),
    /// In a machine (its item): what goes in and how many of each.
    Machine {
        machine: &'static str,
        inputs: Vec<(String, u32)>,
    },
}

/// What a recipe makes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Out {
    Item(String, u32),
    /// A fluid and how much of it, in millibuckets.
    Fluid(String, u32),
}

impl Out {
    pub fn id(&self) -> &str {
        match self {
            Out::Item(id, _) | Out::Fluid(id, _) => id,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shown {
    pub made: Made,
    pub out: Out,
}

impl Shown {
    /// The block's height on the page.
    pub fn height(&self) -> f32 {
        match self.made {
            Made::Grid(_) => GRID_HEIGHT,
            Made::Machine { .. } => MACHINE_HEIGHT,
        }
    }
}

/// The machine that runs a recipe type.
pub fn machine_of(kind: &str) -> Option<&'static str> {
    match kind {
        CRUSHING => Some("rustcraft:crusher"),
        ALLOYING => Some("rustcraft:alloy"),
        MIXING => Some("rustcraft:mixer"),
        SMELTING => Some("rustcraft:furnace"),
        _ => None,
    }
}

/// An ingredient as a recipe file names it: an id, `{"item"}` or `{"tag"}` (as `#tag`).
fn ingredient_id(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Object(o) => o
            .get("item")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| {
                o.get("tag")
                    .and_then(Value::as_str)
                    .map(|t| format!("#{t}"))
            }),
        Value::Array(a) => a.first().and_then(ingredient_id),
        _ => None,
    }
}

fn count(v: &Value, key: &str, default: u64) -> u32 {
    let n = v.get(key).and_then(Value::as_u64).unwrap_or(default);
    u32::try_from(n).unwrap_or(u32::MAX).max(1)
}

/// A machine's recipe file (`rustcraft:crushing`, `alloying`, `mixing`) as the page shows it.
/// `resolve` turns an ingredient's id or `#tag` into the item to draw (None: the game has
/// nothing for it, and the recipe is left out).
pub fn custom(kind: &str, json: &Value, resolve: &dyn Fn(&str) -> Option<String>) -> Option<Shown> {
    let machine = machine_of(kind)?;
    let mut inputs = Vec::new();
    if let Some(one) = json.get("ingredient") {
        inputs.push((resolve(&ingredient_id(one)?)?, 1));
    }
    for i in json
        .get("ingredients")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        inputs.push((resolve(&ingredient_id(i)?)?, count(i, "count", 1)));
    }
    let water = json.get("water").and_then(Value::as_u64).unwrap_or(0);
    if water > 0 {
        let buckets = u32::try_from(water.div_ceil(u64::from(BUCKET_MB))).unwrap_or(1);
        inputs.push((WATER_BUCKET.to_owned(), buckets));
    }
    if inputs.is_empty() {
        return None;
    }
    let result = json.get("result")?;
    let out = if let Some(id) = result.get("id").and_then(Value::as_str) {
        Out::Item(id.to_owned(), count(result, "count", 1))
    } else {
        let fluid = result.get("fluid").and_then(Value::as_str)?;
        Out::Fluid(
            fluid.to_owned(),
            count(result, "amount", u64::from(BUCKET_MB)),
        )
    };
    Some(Shown {
        made: Made::Machine { machine, inputs },
        out,
    })
}

/// A smelting recipe (the electric furnace smelts what the game's furnace does).
pub fn smelting(input: String, out: String, n: u32) -> Shown {
    Shown {
        made: Made::Machine {
            machine: "rustcraft:furnace",
            inputs: vec![(input, 1)],
        },
        out: Out::Item(out, n.max(1)),
    }
}

/// Where a transmuting recipe's materials go round the input in the middle: the cross first,
/// as the guide draws a tier's blade (` a |aba| a `), then the corners.
const AROUND: [usize; 8] = [1, 3, 5, 7, 0, 2, 6, 8];

/// A transmuting recipe (`crafting_transmute`: the item of the tier below and `count`
/// materials) as a grid.
pub fn transmute(input: String, material: String, count: usize, out: String, n: u32) -> Shown {
    let mut grid: [Option<String>; 9] = Default::default();
    grid[4] = Some(input);
    for &i in AROUND.iter().take(count.clamp(1, 8)) {
        grid[i] = Some(material.clone());
    }
    Shown {
        made: Made::Grid(grid),
        out: Out::Item(out, n.max(1)),
    }
}

/// An id without its tier: `rustcraft:blade_3` → `rustcraft:blade`.
pub fn base_of(id: &str) -> &str {
    match id.rsplit_once('_') {
        Some((base, n)) if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => base,
        _ => id,
    }
}

/// An id's tier from its name: `_<n>` at its end, else 1.
pub fn tier_of(id: &str) -> u32 {
    match id.rsplit_once('_') {
        Some((_, n)) if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => {
            n.parse().unwrap_or(1)
        }
        _ => 1,
    }
}

impl Page {
    /// Where among the page's `makes` the result `out` stands: None when the page does not
    /// show it. Every tier of a listed item counts (`blade_2` lists `blade_3` and `blade_4`).
    pub fn makes_at(&self, out: &Out) -> Option<usize> {
        self.makes.iter().position(|&m| match out {
            Out::Fluid(..) if m == ANY_FLUID => true,
            _ => base_of(m) == base_of(out.id()),
        })
    }
}

impl Shown {
    /// The items that go into it: a grid's cells, a machine's inputs.
    pub fn inputs(&self) -> Vec<&str> {
        match &self.made {
            Made::Grid(cells) => cells.iter().flatten().map(String::as_str).collect(),
            Made::Machine { inputs, .. } => inputs.iter().map(|(i, _)| i.as_str()).collect(),
        }
    }
}

/// The recipes the page shows, out of every recipe the game has: those whose result it makes,
/// in the order of its `makes`, then by tier, then grids before machines; each once. 0.21 (spec
/// 45, CR5): a recipe that takes an old channel item (the `_apart` ones) is never shown.
pub fn for_page(page: &Page, all: Vec<Shown>) -> Vec<Shown> {
    let mut picked: Vec<(usize, u32, bool, usize, Shown)> = Vec::new();
    for (n, s) in all.into_iter().enumerate() {
        let Some(at) = page.makes_at(&s.out) else {
            continue;
        };
        if s.inputs().into_iter().any(crate::net::old_channels::is_old_channel) {
            continue;
        }
        if picked.iter().any(|p| p.4 == s) {
            continue;
        }
        let machine = matches!(s.made, Made::Machine { .. });
        picked.push((at, tier_of(s.out.id()), machine, n, s));
    }
    picked.sort_by_key(|p| (p.0, p.1, p.2, p.3));
    picked.into_iter().map(|p| p.4).collect()
}

/// The colour (ARGB) a fluid's square shows, as the guide paints its flows.
pub fn fluid_colour(id: &str) -> u32 {
    match id.rsplit(':').next().unwrap_or(id) {
        "water" => 0xFF3F_76E4,
        "lava" => 0xFFE8_601C,
        "solution" => 0xFF59_D38C,
        _ => 0xFF8A_8F99,
    }
}

// ---- Where a recipe's parts go on the page ----

/// A recipe's block is this wide: two stand side by side in the body.
pub const CELL: f32 = 150.0;
pub const GRID_HEIGHT: f32 = 58.0;
pub const MACHINE_HEIGHT: f32 = 40.0;
/// A grid's arrow and result, from the block's corner (its cells: `super::recipe_cell`).
pub const GRID_ARROW: [f32; 4] = [62.0, 25.0, 22.0, 6.0];
pub const GRID_RESULT: [f32; 2] = [98.0, 20.0];
/// A machine recipe: inputs in a row, the machine's icon over the arrow, the result.
pub const MACHINE_INPUT: [f32; 2] = [1.0, 14.0];
pub const MACHINE_ICON: [f32; 2] = [79.0, 0.0];
pub const MACHINE_ARROW: [f32; 4] = [76.0, 20.0, 22.0, 6.0];
pub const MACHINE_RESULT: [f32; 2] = [104.0, 14.0];
/// At most this many inputs show (a machine takes up to four).
pub const INPUTS: usize = 4;

/// The left of the recipe in column `col` (0 or 1), from the body's left.
pub fn column_x(col: usize) -> f32 {
    CELL * col as f32
}

/// Where a machine recipe's input `k` goes, from the block's corner.
pub fn input_at(k: usize) -> [f32; 2] {
    [MACHINE_INPUT[0] + 18.0 * k as f32, MACHINE_INPUT[1]]
}

/// Where a grid's cell `i` goes, from the block's corner.
pub fn grid_cell(i: usize) -> [f32; 2] {
    [1.0 + 18.0 * (i % 3) as f32, 2.0 + 18.0 * (i / 3) as f32]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn same(id: &str) -> Option<String> {
        Some(id.to_owned())
    }

    fn page(makes: &'static [&'static str]) -> Page {
        Page {
            id: "x",
            section: 0,
            tier: 1,
            item: None,
            text: 0,
            stats: 0,
            diagnoses: 0,
            parts: 0,
            makes,
            tables: &[],
            see: &[],
        }
    }

    fn item(out: &str, n: u32) -> Shown {
        Shown {
            made: Made::Grid(Default::default()),
            out: Out::Item(out.into(), n),
        }
    }

    #[test]
    fn custom_crushing_reads_ingredient_and_result() {
        let j = json!({"type": CRUSHING, "ingredient": "minecraft:raw_iron",
            "result": {"id": "rustcraft:dust_iron", "count": 2}, "time": 100});
        let s = custom(CRUSHING, &j, &same).unwrap();
        assert_eq!(
            s,
            Shown {
                made: Made::Machine {
                    machine: "rustcraft:crusher",
                    inputs: vec![("minecraft:raw_iron".into(), 1)]
                },
                out: Out::Item("rustcraft:dust_iron".into(), 2),
            }
        );
        assert_eq!(s.height(), MACHINE_HEIGHT);
    }

    #[test]
    fn custom_alloying_keeps_counts_and_resolves_tags() {
        let j = json!({"ingredients": [{"item": "rustcraft:coppersteel", "count": 2},
            {"tag": "c:ingots/gold"}, {"item": "minecraft:redstone", "count": 1}],
            "result": {"id": "rustcraft:sparksteel", "count": 2}});
        let resolve = |id: &str| match id {
            "#c:ingots/gold" => Some("minecraft:gold_ingot".to_owned()),
            other => Some(other.to_owned()),
        };
        let s = custom(ALLOYING, &j, &resolve).unwrap();
        let Made::Machine { machine, inputs } = s.made else {
            panic!("not a machine")
        };
        assert_eq!(machine, "rustcraft:alloy");
        assert_eq!(
            inputs,
            vec![
                ("rustcraft:coppersteel".into(), 2),
                ("minecraft:gold_ingot".into(), 1),
                ("minecraft:redstone".into(), 1)
            ]
        );
        // An ingredient the game has nothing for leaves the recipe out.
        assert_eq!(custom(ALLOYING, &j, &|_| None), None);
        // A type that is not a machine's is not shown.
        assert_eq!(custom("rustcraft:other", &j, &same), None);
    }

    #[test]
    fn custom_mixing_reads_water_and_fluid() {
        let j = json!({"ingredients": [{"item": "minecraft:bone_meal", "count": 1}],
            "water": 1000, "result": {"fluid": "rustcraft:solution", "amount": 1000}});
        let s = custom(MIXING, &j, &same).unwrap();
        assert_eq!(
            s.made,
            Made::Machine {
                machine: "rustcraft:mixer",
                inputs: vec![("minecraft:bone_meal".into(), 1), (WATER_BUCKET.into(), 1)]
            }
        );
        assert_eq!(s.out, Out::Fluid("rustcraft:solution".into(), 1000));
        assert_eq!(fluid_colour("rustcraft:solution"), 0xFF59_D38C);
    }

    #[test]
    fn transmute_puts_input_in_the_middle() {
        let s = transmute(
            "rustcraft:blade_2".into(),
            "rustcraft:crystallite".into(),
            4,
            "rustcraft:blade_3".into(),
            1,
        );
        let Made::Grid(g) = &s.made else {
            panic!("not a grid")
        };
        assert_eq!(g[4].as_deref(), Some("rustcraft:blade_2"));
        for i in [1, 3, 5, 7] {
            assert_eq!(g[i].as_deref(), Some("rustcraft:crystallite"), "cell {i}");
        }
        for i in [0, 2, 6, 8] {
            assert_eq!(g[i], None, "cell {i}");
        }
        assert_eq!(s.height(), GRID_HEIGHT);
    }

    #[test]
    fn page_makes_its_tiers_and_extras() {
        assert_eq!(base_of("rustcraft:blade_3"), "rustcraft:blade");
        assert_eq!(base_of("rustcraft:dust_iron"), "rustcraft:dust_iron");
        assert_eq!(tier_of("rustcraft:monorail_4"), 4);
        assert_eq!(tier_of("rustcraft:monorail"), 1);
        let blade = page(&["rustcraft:blade_2"]);
        assert_eq!(
            blade.makes_at(&Out::Item("rustcraft:blade_4".into(), 1)),
            Some(0)
        );
        assert_eq!(
            blade.makes_at(&Out::Item("rustcraft:eshield_3".into(), 1)),
            None
        );
        let crystal = page(&["rustcraft:crystal"]);
        assert_eq!(
            crystal.makes_at(&Out::Item("rustcraft:crystallite".into(), 1)),
            None
        );
        let fluids = page(&[ANY_FLUID]);
        assert_eq!(
            fluids.makes_at(&Out::Fluid("rustcraft:solution".into(), 1000)),
            Some(0)
        );
        assert_eq!(
            fluids.makes_at(&Out::Item("rustcraft:solution".into(), 1)),
            None
        );
        assert_eq!(
            page(&[]).makes_at(&Out::Item("rustcraft:blade_2".into(), 1)),
            None
        );
    }

    #[test]
    fn for_page_leaves_out_what_takes_old_channels_apart() {
        // 0.21 (spec 45, CR5): two old channels gave an alloy on the crafting table; the alloys'
        // page shows only the alloy's own recipes.
        let alloys = page(&["rustcraft:coppersteel", "rustcraft:sparksteel"]);
        let mut apart = [None, None, None, None, None, None, None, None, None];
        apart[3] = same("rustcraft:item_channel");
        apart[5] = same("rustcraft:item_channel");
        let mut apart_2 = apart.clone();
        apart_2[3] = same("rustcraft:energy_channel_2");
        apart_2[5] = same("rustcraft:energy_channel_2");
        let mut own = apart.clone();
        own[3] = same("minecraft:copper_ingot");
        own[5] = same("minecraft:iron_ingot");
        let all = vec![
            Shown { made: Made::Grid(apart), out: Out::Item("rustcraft:coppersteel".into(), 1) },
            Shown { made: Made::Grid(own.clone()), out: Out::Item("rustcraft:coppersteel".into(), 2) },
            Shown { made: Made::Grid(apart_2), out: Out::Item("rustcraft:sparksteel".into(), 1) },
        ];
        let got = for_page(&alloys, all);
        assert_eq!(got, vec![Shown { made: Made::Grid(own), out: Out::Item("rustcraft:coppersteel".into(), 2) }]);
    }

    #[test]
    fn for_page_orders_by_makes_and_tier_and_drops_repeats() {
        let alloys = page(&["rustcraft:coppersteel", "rustcraft:sparksteel"]);
        let all = vec![
            item("rustcraft:sparksteel", 2),
            item("minecraft:stone", 1),
            item("rustcraft:coppersteel", 2),
            item("rustcraft:sparksteel", 2),
        ];
        let got: Vec<String> = for_page(&alloys, all)
            .iter()
            .map(|s| s.out.id().to_owned())
            .collect();
        assert_eq!(got, ["rustcraft:coppersteel", "rustcraft:sparksteel"]);
        let blade = page(&["rustcraft:blade_2"]);
        let all = vec![
            item("rustcraft:blade_4", 1),
            item("rustcraft:blade_2", 1),
            item("rustcraft:blade_3", 1),
        ];
        let got: Vec<u32> = for_page(&blade, all)
            .iter()
            .map(|s| tier_of(s.out.id()))
            .collect();
        assert_eq!(got, [2, 3, 4]);
    }

    #[test]
    fn recipes_lay_out_two_to_a_row() {
        let pieces = vec![super::super::Piece::Recipes(vec![
            GRID_HEIGHT,
            MACHINE_HEIGHT,
            MACHINE_HEIGHT,
        ])];
        let rows =
            super::super::lay_out(&pieces, super::super::BODY_WIDTH, &|s| 6.0 * s.len() as f32);
        assert_eq!(
            rows,
            vec![
                super::super::Row::Recipes {
                    first: 0,
                    n: 2,
                    height: GRID_HEIGHT
                },
                super::super::Row::Recipes {
                    first: 2,
                    n: 1,
                    height: MACHINE_HEIGHT
                },
            ]
        );
    }
}
