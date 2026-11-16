use super::*;
use std::collections::{BTreeMap, HashSet};

/// The game's font, near enough: 6 pixels a character.
fn measure(s: &str) -> f32 {
    6.0 * s.chars().count() as f32
}

fn repo_file(rest: &str) -> Option<String> {
    std::fs::read_to_string(format!("{}/../../../{rest}", env!("CARGO_MANIFEST_DIR"))).ok()
}

fn lang(code: &str) -> BTreeMap<String, String> {
    let path = format!("mod/content/assets/rustcraft/lang/{code}.json");
    let text = repo_file(&path).unwrap_or_else(|| panic!("no {path}"));
    serde_json::from_str(&text).unwrap()
}

#[test]
fn every_entry_of_the_guide_has_a_page_once() {
    let ids: Vec<&str> = PAGES.iter().map(|p| p.id).collect();
    let unique: HashSet<&str> = ids.iter().copied().collect();
    assert_eq!(unique.len(), ids.len(), "a page twice");
    let Some(data) = repo_file("docs/showcase/js/data.js") else {
        return;
    };
    // E({ id: '…' for the plain entries, effectMod('…' and supportMod('…' for the turret's.
    let mut guide = Vec::new();
    for line in data.lines().map(str::trim_start) {
        for start in ["E({ id: '", "effectMod('", "supportMod('"] {
            if let Some(rest) = line.strip_prefix(start) {
                guide.push(rest.split('\'').next().unwrap().to_owned());
            }
        }
    }
    assert!(guide.len() > 100, "{}", guide.len());
    for id in &guide {
        assert!(
            unique.contains(id.as_str()),
            "no page for the guide's {id}: run assets/models/handbook_mc.py"
        );
    }
    assert_eq!(guide.len(), PAGES.len(), "pages the guide no longer has");
}

#[test]
fn every_section_has_pages_and_every_page_a_section() {
    for p in PAGES {
        assert!(p.section < SECTIONS.len(), "{}", p.id);
        assert!(p.tier <= 4, "{}", p.id);
    }
    for (s, name) in SECTIONS.iter().enumerate() {
        assert!(!pages_of(s).is_empty(), "{name}");
    }
}

#[test]
fn the_handbook_and_the_alloys_have_their_items() {
    let page = |id: &str| PAGES.iter().find(|p| p.id == id).unwrap();
    assert_eq!(page("guide").item, Some(ITEM));
    assert_eq!(page("alloys").item, Some(ALLOYS[0]));
    assert_eq!(page("overview").item, None);
    assert_eq!(page("overview").icon(), ITEM);
    assert!(
        PAGES
            .iter()
            .filter_map(|p| p.item)
            .all(|i| i.starts_with("rustcraft:"))
    );
}

#[test]
fn every_page_has_its_lines_in_russian_and_its_name_in_english() {
    let (ru, en) = (lang("ru_ru"), lang("en_us"));
    for p in PAGES {
        let mut keys = vec![p.key("name"), p.key("sum")];
        keys.extend((0..p.text).map(|i| p.key(&format!("text.{i}"))));
        keys.extend((0..p.stats).flat_map(|i| {
            [
                p.key(&format!("stat.{i}.key")),
                p.key(&format!("stat.{i}.value")),
            ]
        }));
        keys.extend((0..p.diagnoses).map(|i| p.key(&format!("diag.{i}"))));
        keys.extend((0..p.parts).map(|i| p.key(&format!("part.{i}"))));
        for k in keys {
            assert!(ru.contains_key(&k), "ru_ru has no {k}");
        }
        assert!(
            en.contains_key(&p.key("name")) && en.contains_key(&p.key("sum")),
            "en_us has no {}",
            p.id
        );
    }
    for s in 0..SECTIONS.len() {
        assert!(ru.contains_key(&section_key(s)) && en.contains_key(&section_key(s)));
    }
    for w in [
        "title",
        "contents",
        "sections",
        "recipe",
        "diagnoses",
        "window",
        "sheet",
        "given",
    ] {
        assert!(ru.contains_key(&key(w)) && en.contains_key(&key(w)), "{w}");
    }
    assert!(
        ru.contains_key("item.rustcraft.handbook") && en.contains_key("item.rustcraft.handbook")
    );
}

#[test]
fn every_page_lays_out_into_sheets_that_fit() {
    let ru = lang("ru_ru");
    let lookup = |k: &str| ru.get(k).cloned();
    for p in PAGES {
        let rows = lay_out(
            &pieces(p, &[recipes::GRID_HEIGHT; 3], &lookup),
            BODY_WIDTH,
            &measure,
        );
        assert!(!rows.is_empty(), "{}", p.id);
        let all = sheets(&rows);
        assert_eq!(all.first().unwrap().start, 0);
        let mut next = 0;
        for (s, r) in all.iter().enumerate() {
            assert!(r.start >= next && r.end > r.start, "{} sheet {s}", p.id);
            next = r.end;
            let used: f32 = rows[r.clone()].iter().map(Row::height).sum();
            assert!(
                sheet_top(s) + used <= BODY_BOTTOM,
                "{} sheet {s} runs over",
                p.id
            );
            if r.end < rows.len() {
                assert!(
                    !is_heading(&rows[r.end - 1]),
                    "{} sheet {s} ends on a heading",
                    p.id
                );
            }
        }
        assert_eq!(next, rows.len(), "{} loses rows", p.id);
        for row in &rows {
            if let Row::Text { x, text, .. } = row {
                assert!(x + measure(text) <= BODY_WIDTH, "{}: {text}", p.id);
            }
        }
    }
}

#[test]
fn an_english_page_is_its_name_and_summary() {
    let page = PAGES.iter().find(|p| p.id == "furnace").unwrap();
    let en: BTreeMap<String, String> = [
        (page.key("name"), "Electric Furnace".to_owned()),
        (page.key("sum"), "Smelts fast.".to_owned()),
        (key("recipe"), "Recipe".to_owned()),
    ]
    .into();
    let got = pieces(page, &[recipes::GRID_HEIGHT], &|k| en.get(k).cloned());
    assert_eq!(
        got,
        vec![
            Piece::Lead("Smelts fast.".into()),
            Piece::Heading("Recipe".into()),
            Piece::Recipes(vec![recipes::GRID_HEIGHT])
        ]
    );
    assert_eq!(pieces(page, &[], &|_| None), Vec::<Piece>::new());
}

#[test]
fn pieces_keep_the_pages_order() {
    let page = Page {
        id: "x",
        section: 0,
        tier: 1,
        item: None,
        text: 1,
        stats: 1,
        diagnoses: 1,
        parts: 2,
        makes: &[],
    };
    let lines: BTreeMap<String, String> = [
        ("sum", "Sum"),
        ("text.0", "Para"),
        ("stat.0.key", "K"),
        ("stat.0.value", "V"),
        ("diag.0", "No power"),
        ("diag.0.fix", "Fix"),
        ("part.0.label", "Input"),
        ("part.0", "Note"),
        ("part.1", "Status"),
    ]
    .into_iter()
    .map(|(k, v)| (page.key(k), v.to_owned()))
    .chain([
        (key("diagnoses"), "D".to_owned()),
        (key("window"), "W".to_owned()),
    ])
    .collect();
    let got = pieces(&page, &[], &|k| lines.get(k).cloned());
    assert_eq!(
        got,
        vec![
            Piece::Lead("Sum".into()),
            Piece::Paragraph("Para".into()),
            Piece::Stat {
                key: "K".into(),
                value: "V".into()
            },
            Piece::Heading("D".into()),
            Piece::Diagnosis {
                name: "No power".into(),
                fix: "Fix".into()
            },
            Piece::Heading("W".into()),
            Piece::Part {
                n: 1,
                label: "Input".into(),
                note: "Note".into()
            },
            Piece::Part {
                n: 2,
                label: String::new(),
                note: "Status".into()
            },
        ]
    );
    let rows = lay_out(&got, 120.0, &measure);
    assert!(rows.contains(&Row::Pair {
        key: "K".into(),
        value: "V".into()
    }));
    assert!(rows.contains(&Row::Number {
        n: 1,
        text: "INPUT".into(),
        role: Role::Key
    }));
    assert!(rows.contains(&Row::Number {
        n: 2,
        text: "Status".into(),
        role: Role::Text
    }));
    assert!(rows.contains(&Row::Text {
        x: INDENT,
        text: "Fix".into(),
        role: Role::Text
    }));
    // A heading has no gap under it; pieces of another kind have one between them.
    let heading = rows
        .iter()
        .position(|r| {
            matches!(
                r,
                Row::Text {
                    role: Role::Heading,
                    ..
                }
            )
        })
        .unwrap();
    assert_ne!(rows[heading + 1], Row::Gap);
    assert_eq!(rows[heading - 1], Row::Gap);
}

#[test]
fn a_long_stat_goes_under_its_key() {
    let p = [Piece::Stat {
        key: "Key".into(),
        value: "a value much too long for one line".into(),
    }];
    let rows = lay_out(&p, 60.0, &measure);
    assert_eq!(
        rows[0],
        Row::Text {
            x: 0.0,
            text: "Key".into(),
            role: Role::Key
        }
    );
    assert!(
        rows[1..]
            .iter()
            .all(|r| matches!(r, Row::Text { x, role: Role::Value, .. } if *x == INDENT))
    );
}

#[test]
fn wrap_breaks_at_spaces_and_a_long_word_at_its_letters() {
    assert_eq!(
        wrap("one two three", 42.0, &measure),
        vec!["one two", "three"]
    );
    assert_eq!(
        wrap("abcdefghij", 24.0, &measure),
        vec!["abcd", "efgh", "ij"]
    );
    assert_eq!(wrap("  ", 24.0, &measure), Vec::<String>::new());
    for line in wrap(
        "Магистраль несёт предметы, жидкость и энергию по одному корпусу.",
        90.0,
        &measure,
    ) {
        assert!(measure(&line) <= 90.0, "{line}");
    }
}

#[test]
fn a_name_too_long_is_clipped() {
    assert_eq!(clip("Short", 60.0, &measure), "Short");
    let c = clip("Инструмент: грузовой отсек", 60.0, &measure);
    assert!(c.ends_with('…') && measure(&c) <= 60.0, "{c}");
}

#[test]
fn sheets_never_start_with_a_gap_and_keep_a_heading_with_its_lines() {
    let text = |t: &str| Row::Text {
        x: 0.0,
        text: t.into(),
        role: Role::Text,
    };
    let fits = ((BODY_BOTTOM - FIRST_TOP) / LINE) as usize;
    let mut rows: Vec<Row> = (0..fits - 1).map(|_| text("a")).collect();
    rows.push(Row::Text {
        x: 0.0,
        text: "H".into(),
        role: Role::Heading,
    });
    rows.push(text("b"));
    let all = sheets(&rows);
    assert_eq!(all, vec![0..fits - 1, fits - 1..fits + 1]);
    let all = sheets(&[Row::Gap, text("a")]);
    assert_eq!(all, vec![1..2]);
    assert_eq!(sheets(&[]), vec![0..0]);
}

#[test]
fn the_contents_open_a_page_and_its_button_comes_back_to_its_place() {
    let module = SECTIONS.iter().position(|s| *s == "module").unwrap();
    assert!(
        pages_of(module).len() > ENTRIES,
        "the modules' list has more than one sheet"
    );
    let v = press(View::default(), Part::Section(module), 1);
    assert_eq!(
        v,
        View::Contents {
            section: module,
            sheet: 0
        }
    );
    let v = press(v, Part::Next, 1);
    assert_eq!(
        v,
        View::Contents {
            section: module,
            sheet: 1
        }
    );
    let v = press(v, Part::Entry(2), 1);
    let page = pages_of(module)[ENTRIES + 2];
    assert_eq!(v, View::Page { page, sheet: 0 });
    assert_eq!(
        press(v, Part::Contents, 1),
        View::Contents {
            section: module,
            sheet: 1
        }
    );
    // A row past the sheet's list does nothing.
    let last = contents_sheets(module) - 1;
    let short = View::Contents {
        section: module,
        sheet: last,
    };
    let rows = listed(module, last).len();
    assert_eq!(press(short, Part::Entry(ENTRIES), 1), short);
    assert!(rows <= ENTRIES);
}

#[test]
fn turning_goes_through_a_pages_sheets_and_on_to_the_next_page() {
    let v = View::Page { page: 3, sheet: 0 };
    assert_eq!(turn(v, true, 2), View::Page { page: 3, sheet: 1 });
    assert_eq!(
        turn(View::Page { page: 3, sheet: 1 }, true, 2),
        View::Page { page: 4, sheet: 0 }
    );
    assert_eq!(
        turn(v, false, 2),
        View::Page {
            page: 2,
            sheet: LAST
        }
    );
    assert_eq!(
        View::Page {
            page: 2,
            sheet: LAST
        }
        .clamped(3),
        View::Page { page: 2, sheet: 2 }
    );
    let first = View::Page { page: 0, sheet: 0 };
    assert_eq!(turn(first, false, 1), first);
    let end = View::Page {
        page: PAGES.len() - 1,
        sheet: 0,
    };
    assert_eq!(turn(end, true, 1), end);
    assert_eq!(turn(View::default(), false, 1), View::default());
}

#[test]
fn parts_are_where_the_window_draws_them() {
    let contents = View::Contents {
        section: 0,
        sheet: 0,
    };
    let middle = |r: Rect| [r[0] + r[2] / 2.0, r[1] + r[3] / 2.0];
    assert_eq!(part_at(contents, middle(PREV)), Some(Part::Prev));
    assert_eq!(part_at(contents, middle(NEXT)), Some(Part::Next));
    assert_eq!(part_at(contents, middle(CONTENTS)), Some(Part::Contents));
    assert_eq!(
        part_at(contents, middle(section_rect(4))),
        Some(Part::Section(4))
    );
    assert_eq!(
        part_at(contents, middle(entry_rect(0))),
        Some(Part::Entry(0))
    );
    let rows = listed(0, 0).len();
    assert_eq!(part_at(contents, middle(entry_rect(rows))), None);
    let page = View::Page { page: 0, sheet: 0 };
    assert_eq!(part_at(page, middle(section_rect(4))), None);
    assert_eq!(part_at(page, middle(NEXT)), Some(Part::Next));
}

#[test]
fn the_window_fits_a_small_screen_and_its_parts_fit_the_window() {
    // GUI scale 4 on a 1920 × 1080 screen is 480 × 270 GUI pixels.
    const { assert!(WIDTH <= 480.0 && HEIGHT <= 270.0) };
    let face = [3.0, 3.0, WIDTH - 6.0, HEIGHT - 6.0];
    let within = |r: Rect| {
        r[0] >= face[0]
            && r[1] >= face[1]
            && r[0] + r[2] <= face[0] + face[2]
            && r[1] + r[3] <= face[1] + face[3]
    };
    for r in [
        PREV,
        NEXT,
        CONTENTS,
        SHEET,
        section_rect(SECTIONS.len() - 1),
        entry_rect(ENTRIES - 1),
    ] {
        assert!(within(r), "{r:?}");
    }
    assert!(section_rect(SECTIONS.len() - 1)[1] + SECTION_SIZE[1] <= BODY_BOTTOM);
    assert!(entry_rect(ENTRIES - 1)[1] + ENTRY_SIZE[1] <= BODY_BOTTOM);
    const { assert!(BODY_BOTTOM <= PREV[1]) };
    assert!(recipes::grid_cell(8)[1] + 17.0 <= recipes::GRID_HEIGHT);
    assert!(recipes::input_at(recipes::INPUTS - 1)[1] + 17.0 <= recipes::MACHINE_HEIGHT);
    const { assert!(BODY_X + recipes::CELL + recipes::GRID_RESULT[0] + 17.0 <= WIDTH) };
    const { assert!(BODY_X + recipes::CELL + recipes::MACHINE_RESULT[0] + 17.0 <= WIDTH) };
    assert!(recipes::input_at(recipes::INPUTS - 1)[0] + 17.0 <= recipes::MACHINE_ARROW[0]);
    assert_eq!(origin([480.0, 270.0]), [80.0, 30.0]);
    assert_eq!(origin([200.0, 100.0]), [0.0, 0.0]);
}

#[test]
fn the_first_alloy_gives_the_handbook_once() {
    assert!(!due(false, ["minecraft:dirt"]));
    assert!(due(false, ["minecraft:dirt", "rustcraft:sparksteel"]));
    assert!(!due(true, ["rustcraft:coppersteel"]));
    let mut given = Given::default();
    let player = 0x1234_5678_9abc_def0_1234_5678_9abc_def0;
    assert!(!given.has(player));
    assert!(given.add(player));
    assert!(!given.add(player));
    assert!(given.has(player) && !given.has(player + 1));
    let text = serde_json::to_string(&given).unwrap();
    assert_eq!(text, r#"{"players":["123456789abcdef0123456789abcdef0"]}"#);
    assert_eq!(serde_json::from_str::<Given>(&text).unwrap(), given);
    assert_eq!(
        serde_json::from_str::<Given>("{}").unwrap(),
        Given::default()
    );
    assert!(given.remove(player) && !given.has(player));
}

#[test]
fn a_recipe_goes_into_the_grid_by_its_width() {
    let s = |x: &str| Some(x.to_owned());
    let shaped = recipe_grid(2, &[s("a"), None, s("b"), s("c")]).unwrap();
    assert_eq!(
        shaped,
        [s("a"), None, None, s("b"), s("c"), None, None, None, None]
    );
    let shapeless = recipe_grid(3, &[s("book"), s("rustcraft:coppersteel")]).unwrap();
    assert_eq!(shapeless[..2], [s("book"), s("rustcraft:coppersteel")]);
    assert_eq!(recipe_grid(4, &[s("a")]), None);
    assert_eq!(recipe_grid(1, &[s("a"), s("b"), s("c"), s("d")]), None);
    assert_eq!(recipe_grid(3, &[None]), None);
}
