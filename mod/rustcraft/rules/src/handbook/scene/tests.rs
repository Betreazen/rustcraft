use super::*;
use crate::handbook::{self as hb, CONTENTS, HEIGHT, NEXT, PAGES, PREV, WIDTH};
use std::collections::BTreeMap;

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

const STILL: Step = Step {
    dur: 5.0,
    blocks: &[],
    movers: &[],
    flows: &[],
    beams: &[],
    tint: 0,
    labels: &[],
};

fn scene_of(steps: &'static [Step]) -> Scene {
    Scene { id: "test", steps }
}

fn near(a: [f32; 2], b: [f32; 2]) -> bool {
    (a[0] - b[0]).abs() < 1e-3 && (a[1] - b[1]).abs() < 1e-3
}

fn near3(a: [f32; 3], b: [f32; 3]) -> bool {
    (0..3).all(|i| (a[i] - b[i]).abs() < 1e-3)
}

#[test]
fn every_scene_is_a_page_once_and_has_steps() {
    let mut seen = std::collections::HashSet::new();
    for s in SCENES {
        assert!(seen.insert(s.id), "{} twice", s.id);
        let page = PAGES.iter().find(|p| p.id == s.id);
        assert!(page.is_some(), "the scene {} has no page", s.id);
        assert!(!s.steps.is_empty(), "{}", s.id);
        assert!(hb::scene_of(page.unwrap()).is_some());
        for st in s.steps {
            assert!(st.dur >= 3.0, "{}: a step too short to read", s.id);
        }
    }
    assert!(SCENES.len() > 90, "{}", SCENES.len());
}

#[test]
fn every_scene_of_the_guide_is_carried_over() {
    let Some(js) = repo_file("docs/showcase/js/scenes.js") else {
        return;
    };
    // scene('…', [ for the plain scenes; the helpers' scenes are counted by the table alone.
    for line in js.lines().map(str::trim_start) {
        if let Some(rest) = line.strip_prefix("scene('") {
            let id = rest.split('\'').next().unwrap();
            assert!(
                find(id).is_some(),
                "no scene {id}: run assets/models/handbook_mc.py"
            );
        }
    }
}

#[test]
fn every_look_is_an_item_the_mapping_knows() {
    for l in LOOKS {
        match *l {
            Look::Item(id) => {
                let (ns, name) = id.split_once(':').expect(id);
                assert!(ns == "minecraft" || ns == "rustcraft", "{id}");
                assert!(!name.is_empty() && !name.contains(':'), "{id}");
                if ns == "rustcraft"
                    && repo_file("mod/content/assets/rustcraft/lang/ru_ru.json").is_some()
                {
                    let path = format!("mod/content/assets/rustcraft/items/{name}.json");
                    assert!(
                        repo_file(&path).is_some(),
                        "{id}: the content mod has no {path}"
                    );
                }
            }
            Look::Tint(argb) | Look::Drop(argb) => assert!(argb >> 24 > 0, "an invisible tint"),
        }
    }
    for s in SCENES {
        for st in s.steps {
            let looks = st
                .blocks
                .iter()
                .map(|b| b.look)
                .chain(st.movers.iter().map(|m| m.look));
            for i in looks {
                assert!(usize::from(i) < LOOKS.len(), "{}: look {i}", s.id);
            }
            for bm in st.beams {
                if let End::Mover(i) = bm.to {
                    assert!(
                        usize::from(i) < st.movers.len(),
                        "{}: beam to mover {i}",
                        s.id
                    );
                }
            }
        }
    }
}

#[test]
fn every_caption_and_the_sheets_words_have_their_keys() {
    let ru = lang("ru_ru");
    let en = lang("en_us");
    for s in SCENES {
        for n in 0..s.steps.len() {
            let key = s.caption_key(n);
            assert!(
                ru.get(&key).is_some_and(|t| !t.trim().is_empty()),
                "no {key}: run assets/models/handbook_mc.py"
            );
        }
        assert!(
            !ru.contains_key(&s.caption_key(s.steps.len())),
            "{}: a caption too many",
            s.id
        );
    }
    for word in ["scene", "step"] {
        let key = hb::key(word);
        assert!(ru.contains_key(&key) && en.contains_key(&key), "{key}");
    }
    assert_eq!(ru[&hb::key("step")].matches("%s").count(), 2);
    assert_eq!(en[&hb::key("step")].matches("%s").count(), 2);
}

#[test]
fn every_caption_fits_its_lines() {
    let ru = lang("ru_ru");
    for s in SCENES {
        for n in 0..s.steps.len() {
            let text = &ru[&s.caption_key(n)];
            let lines = caption_lines(text, PANEL[2], &measure);
            assert!(lines.len() <= CAPTION_LINES, "{}.{n}", s.id);
            assert!(lines.iter().all(|l| measure(l) <= PANEL[2]), "{}.{n}", s.id);
        }
    }
    let bottom = CAPTION_TOP + hb::LINE * CAPTION_LINES as f32;
    assert!(bottom <= hb::BODY_BOTTOM, "{bottom}");
}

#[test]
fn a_caption_too_long_is_cut_on_its_last_line() {
    let long = "слово ".repeat(80);
    let lines = caption_lines(&long, 120.0, &measure);
    assert_eq!(lines.len(), CAPTION_LINES);
    assert!(lines.last().unwrap().ends_with('…'));
    assert_eq!(caption_lines("коротко", 120.0, &measure), vec!["коротко"]);
}

#[test]
fn the_view_is_the_games_block_icon() {
    // Along x: right and down; along y: left and down; up: straight up.
    let o = project([0.0, 0.0, 0.0]);
    let x = project([1.0, 0.0, 0.0]);
    let y = project([0.0, 1.0, 0.0]);
    let z = project([0.0, 0.0, 1.0]);
    assert!(x[0] > o[0] && x[1] > o[1]);
    assert!(y[0] < o[0] && y[1] > o[1]);
    assert!(near([z[0], z[1]], [0.0, -UP]));
    // A block edge is 10 of the icon's 16 pixels; its outline fits the icon.
    assert!((ACROSS * 2.0 - 14.142).abs() < 0.01);
    assert!(DOWN * 2.0 + UP <= ICON, "{}", DOWN * 2.0 + UP);
    // Neighbours along x and y meet edge to edge: their middles are a block's width apart.
    assert!(near([x[0] - y[0], x[1] - y[1]], [2.0 * ACROSS, 0.0]));
}

#[test]
fn a_nearer_cell_is_drawn_later() {
    const BLOCKS: &[Block] = &[
        b(0, [1.0, 0.0, 0.0]),
        b(0, [0.0, 0.0, 1.0]),
        b(0, [0.0, 0.0, 0.0]),
        b(0, [0.0, 1.0, 0.0]),
    ];
    const STEP: Step = Step {
        blocks: BLOCKS,
        ..STILL
    };
    let fit = Fit {
        scale: 1.0,
        shift: [0.0, 0.0],
    };
    let d = drawn(&STEP, 0.0, &fit);
    assert_eq!(d.len(), 4);
    assert!(d.windows(2).all(|w| w[0].depth <= w[1].depth));
    // The origin cell is behind its three neighbours.
    assert!(near(d[0].centre, fit.place(centre([0.0, 0.0, 0.0]))));
    // At one depth the lower goes first, so a block on top covers the one below's neighbours.
    let last = d.last().unwrap();
    assert!(near(last.centre, fit.place(centre([0.0, 0.0, 1.0]))));
}

#[test]
fn a_block_gone_in_the_step_vanishes_then() {
    const BLOCKS: &[Block] = &[b(0, [0.0; 3]), bg(1, [1.0, 0.0, 0.0], 2.0)];
    const STEP: Step = Step {
        blocks: BLOCKS,
        ..STILL
    };
    let fit = fit(&scene_of(&[STEP]), [100.0, 100.0]);
    assert_eq!(drawn(&STEP, 1.9, &fit).len(), 2);
    assert_eq!(drawn(&STEP, 2.0, &fit).len(), 1);
}

#[test]
fn movers_go_once_loop_and_ping() {
    const PATH: &[[f32; 3]] = &[[0.0, 0.0, 0.0], [4.0, 0.0, 0.0]];
    let m = |mode, since| Mover {
        look: 0,
        path: PATH,
        speed: 2.0,
        mode,
        since,
    };
    assert!(near3(m(Mode::Once, 0.0).at(1.0), [2.0, 0.0, 0.0]));
    assert!(near3(m(Mode::Once, 0.0).at(9.0), [4.0, 0.0, 0.0]));
    assert!(near3(m(Mode::Loop, 0.0).at(2.5), [1.0, 0.0, 0.0]));
    assert!(near3(m(Mode::Ping, 0.0).at(3.0), [2.0, 0.0, 0.0]));
    assert!(near3(m(Mode::Ping, 0.0).at(4.0), [0.0, 0.0, 0.0]));
    // A move begun in an earlier step goes on from where it was.
    assert!(near3(m(Mode::Once, 1.0).at(0.0), [2.0, 0.0, 0.0]));
    assert!((path_length(PATH) - 4.0).abs() < 1e-6);
    assert!(near3(point_on(&[[1.0, 2.0, 3.0]], 5.0), [1.0, 2.0, 3.0]));
}

#[test]
fn flow_dots_go_along_their_path_and_beams_reach_their_end() {
    const PATH: &[[f32; 3]] = &[[0.0, 0.0, 0.0], [3.0, 0.0, 0.0]];
    const FLOWS: &[Flow] = &[Flow {
        kind: Kind::Item,
        rgb: 0xffa53c,
        dots: 3,
        speed: 1.0,
        path: PATH,
    }];
    const MOVERS: &[Mover] = &[Mover {
        look: 0,
        path: &[[5.0, 0.0, 0.0]],
        speed: 1.0,
        mode: Mode::Once,
        since: 0.0,
    }];
    const BEAMS: &[Beam] = &[Beam {
        from: [0.5, 0.5, 1.1],
        to: End::Mover(0),
        rgb: 0xff5a4a,
    }];
    const STEP: Step = Step {
        movers: MOVERS,
        flows: FLOWS,
        beams: BEAMS,
        ..STILL
    };
    let fit = Fit {
        scale: 1.0,
        shift: [0.0, 0.0],
    };
    let d = dots(&STEP, 0.5, &fit);
    let flow: Vec<&Dot> = d.iter().filter(|d| !d.dash).collect();
    assert_eq!(flow.len(), 3);
    // Every dot is in the middle of a cell on the path: y and z stay, x is between 0.5 and 3.5.
    for dot in &flow {
        let ok = (0..=300).any(|i| {
            let x = i as f32 / 100.0;
            near(dot.centre, fit.place(centre([x, 0.0, 0.0])))
        });
        assert!(ok, "{:?}", dot.centre);
    }
    let dashes = d.iter().filter(|d| d.dash).count();
    // Five blocks long, a dash every 0.32.
    assert!((15..=16).contains(&dashes), "{dashes}");
    // Later the dots have moved on.
    assert_ne!(dots(&STEP, 0.9, &fit)[0].centre, d[0].centre);
}

#[test]
fn every_scene_fits_the_panel_at_a_readable_scale() {
    let size = [PANEL[2], PANEL[3]];
    for s in SCENES {
        let f = fit(s, size);
        assert!((MIN_SCALE..=MAX_SCALE).contains(&f.scale), "{}", s.id);
        assert_eq!(
            (f.scale * 16.0).fract(),
            0.0,
            "{}: an icon of part pixels",
            s.id
        );
        if f.scale == MIN_SCALE {
            continue;
        }
        let half = f.icon() / 2.0;
        for c in cells(s) {
            let p = f.place(centre(c));
            let inside = p[0] - half >= -0.01
                && p[1] - half >= -0.01
                && p[0] + half <= size[0] + 0.01
                && p[1] + half <= size[1] + 0.01;
            assert!(inside, "{}: {c:?} at {p:?} outside the panel", s.id);
        }
    }
}

#[test]
fn a_small_scene_is_centred_and_no_larger_than_the_largest_scale() {
    const STEP: Step = Step {
        blocks: &[b(0, [0.0; 3])],
        ..STILL
    };
    let f = fit(&scene_of(&[STEP]), [300.0, 120.0]);
    assert_eq!(f.scale, MAX_SCALE);
    assert!(near(f.place(centre([0.0; 3])), [150.0, 60.0]));
    let empty = fit(&scene_of(&[STILL]), [300.0, 120.0]);
    assert_eq!(empty.scale, MAX_SCALE);
}

#[test]
fn the_front_icon_is_picked() {
    let fit = Fit {
        scale: 1.0,
        shift: [0.0, 0.0],
    };
    let d = [
        Drawn {
            look: Look::Item("minecraft:stone"),
            centre: [0.0, 0.0],
            depth: 0.0,
        },
        Drawn {
            look: Look::Item("minecraft:dirt"),
            centre: [2.0, 0.0],
            depth: 1.0,
        },
    ];
    assert_eq!(pick(&d, &fit, [1.0, 0.0]), Some(1));
    assert_eq!(pick(&d, &fit, [-5.0, 0.0]), Some(0));
    assert_eq!(pick(&d, &fit, [40.0, 0.0]), None);
}

#[test]
fn a_tinted_cube_shows_three_faces_inside_its_icon() {
    let faces = cube_faces([8.0, 8.0], 1.0);
    for (corners, light) in faces {
        assert!((0.0..=1.0).contains(&light));
        for c in corners {
            assert!(
                (0.0..=ICON).contains(&c[0]) && (0.0..=ICON).contains(&c[1]),
                "{c:?}"
            );
        }
    }
    assert_eq!(shaded(0x80FF_8040, 0.5), 0x8080_4020);
    assert_eq!(shaded(0xFF12_3456, 1.0), 0xFF12_3456);
}

#[test]
fn steps_go_on_after_their_time_and_start_again_after_the_last() {
    const STEPS: &[Step] = &[Step { dur: 4.0, ..STILL }, Step { dur: 6.0, ..STILL }];
    let s = scene_of(STEPS);
    let mut p = Playback::start(3, 10.0);
    p.update(&s, 13.9);
    assert_eq!(p.step, 0);
    p.update(&s, 14.0);
    assert_eq!((p.step, p.since), (1, 14.0));
    assert_eq!(p.time(15.5), 1.5);
    p.update(&s, 20.0 + f64::from(END_HOLD) - 0.1);
    assert_eq!(p.step, 1, "the last step holds a little longer");
    p.update(&s, 20.0 + f64::from(END_HOLD));
    assert_eq!(p.step, 0);
}

#[test]
fn the_step_buttons_stop_at_the_ends() {
    const STEPS: &[Step] = &[STILL, STILL, STILL];
    let s = scene_of(STEPS);
    let mut p = Playback::start(0, 0.0);
    p.go(&s, false, 1.0);
    assert_eq!((p.step, p.since), (0, 1.0));
    p.go(&s, true, 2.0);
    p.go(&s, true, 3.0);
    p.go(&s, true, 4.0);
    assert_eq!((p.step, p.since), (2, 4.0));
    assert_eq!(p.time(3.0), 0.0, "no time before the step began");
}

#[test]
fn a_page_with_a_scene_has_its_sheet_last() {
    let with = PAGES.iter().find(|p| find(p.id).is_some()).unwrap();
    assert_eq!(hb::sheet_count(with, 2), 3);
    assert_eq!(hb::scene_sheet(with, 2), Some(2));
    assert_eq!(hb::scene_sheet(with, 0), Some(1));
    if let Some(without) = PAGES.iter().find(|p| find(p.id).is_none()) {
        assert_eq!(hb::sheet_count(without, 2), 2);
        assert_eq!(hb::scene_sheet(without, 2), None);
    }
    // Turning on from the scene's sheet goes to the next page.
    let page = PAGES.iter().position(|p| p.id == with.id).unwrap();
    let v = hb::View::Page { page, sheet: 2 };
    if page + 1 < PAGES.len() {
        assert_eq!(
            hb::turn(v, true, 3),
            hb::View::Page {
                page: page + 1,
                sheet: 0
            }
        );
    }
}

#[test]
fn the_scene_sheets_parts_fit_and_keep_clear_of_the_bottom_row() {
    let parts = [PANEL, SHEET, STEP_BACK, STEP_COUNT, STEP_ON];
    for r in parts {
        assert!(
            r[0] >= 0.0 && r[1] >= 0.0 && r[0] + r[2] <= WIDTH && r[1] + r[3] <= HEIGHT,
            "{r:?}"
        );
    }
    let overlap = |a: Rect, b: Rect| {
        a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]
    };
    let row = [PREV, CONTENTS, SHEET, STEP_BACK, STEP_COUNT, STEP_ON, NEXT];
    for (i, a) in row.iter().enumerate() {
        for b in &row[i + 1..] {
            assert!(!overlap(*a, *b), "{a:?} and {b:?}");
        }
    }
    assert!(PANEL[1] + PANEL[3] <= CAPTION_TOP);
    let middle = |r: Rect| [r[0] + r[2] / 2.0, r[1] + r[3] / 2.0];
    assert_eq!(part_at(middle(STEP_BACK)), Some(Part::StepBack));
    assert_eq!(part_at(middle(STEP_ON)), Some(Part::StepOn));
    assert_eq!(part_at(middle(PANEL)), None);
    // A step button press leaves the page where it is.
    let v = hb::View::Page { page: 0, sheet: 0 };
    assert_eq!(hb::press(v, Part::StepOn, 2), v);
}

const RUN: &[[f32; 3]] = &[[0.0, 0.0, 0.0], [4.0, 0.0, 0.0]];

#[test]
fn labels_follow_their_mover_and_stand_on_their_point() {
    const STEP: Step = Step {
        dur: 5.0,
        blocks: &[],
        movers: &[Mover {
            look: 0,
            path: RUN,
            speed: 1.0,
            mode: Mode::Once,
            since: 0.0,
        }],
        flows: &[],
        beams: &[],
        tint: 0,
        labels: &[
            Label {
                at: End::Mover(0),
                lift: 1.45,
                tone: Tone::Tag,
            },
            Label {
                at: End::Point([2.5, 0.5, 2.15]),
                lift: 0.0,
                tone: Tone::Bad,
            },
            Label {
                at: End::Mover(3),
                lift: 1.0,
                tone: Tone::Info,
            },
        ],
    };
    let at = labels_at(&STEP, 2.0);
    // The label on a mover that does not exist is left out.
    assert_eq!(at, vec![(0, [2.5, 0.5, 1.45]), (1, [2.5, 0.5, 2.15])]);
    let scene = scene_of(&[STEP]);
    assert_eq!(
        scene.label_key(0, 1),
        "rustcraft.handbook.scene.test.0.label.1"
    );
    // The badge's point is in the scene's bounds: the panel keeps room for it.
    assert!(
        cells(&scene)
            .iter()
            .any(|c| c[0] == 2.0 && c[1] == 0.0 && (c[2] - 1.65).abs() < 1e-5)
    );
}

#[test]
fn label_box_stays_in_the_panel() {
    let size = [100.0, 50.0];
    let b = label_box([50.0, 30.0], 40.0, size);
    assert_eq!(b[2], 40.0 * LABEL_SCALE + 2.0 * LABEL_PAD);
    assert!((b[0] + b[2] / 2.0 - 50.0).abs() <= 0.5 && b[1] + b[3] <= 30.0 + 0.5);
    let edge = label_box([2.0, 1.0], 40.0, size);
    assert_eq!((edge[0], edge[1]), (0.0, 0.0));
    let far = label_box([99.0, 49.0], 40.0, size);
    assert!(far[0] + far[2] <= size[0] + 0.5);
}

#[test]
fn label_colours_by_tone() {
    let (bad, back) = tone_colours(Tone::Bad);
    assert_eq!(bad, crate::machines::style::colour::BAD_TEXT);
    assert_eq!(back >> 24, 0xFF);
    for t in [Tone::Warn, Tone::Ok, Tone::Info, Tone::Tag] {
        let (text, back) = tone_colours(t);
        assert_eq!(text >> 24, 0, "{t:?}: text colours are RGB");
        assert!(back >> 24 > 0, "{t:?}: a visible plate");
    }
}

#[test]
fn every_label_has_its_text_in_russian() {
    let ru = lang("ru_ru");
    for s in SCENES {
        for (n, st) in s.steps.iter().enumerate() {
            for k in 0..st.labels.len() {
                assert!(
                    ru.contains_key(&s.label_key(n, k)),
                    "{} step {n} label {k}",
                    s.id
                );
            }
        }
    }
}
