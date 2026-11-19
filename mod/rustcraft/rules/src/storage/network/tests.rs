//! Round 32 (spec 33): the «Сеть» page's rules, rows, pages and message.

use super::*;
use crate::net::topology::{Housing, Layout, Node};
use crate::storage::{terminal, wire};
use crate::tuning::Tuning;

fn page() -> NetPage {
    NetPage {
        terminal: [10, 64, -3],
        magistral: Magistral {
            branches: 1,
            housings: 24,
            out_of_reach: 2,
            min_tier: 2,
            items: 16.0,
            fluid: 4.0,
            energy_cap: 128.0,
        },
        energy: Some(Energy {
            made: 40.0,
            used: 32.5,
            stored: 12_400.0,
            capacity: 40_000.0,
        }),
        storage: Some(StoreFacts {
            cells: 4,
            used: 12,
            total: 108,
            fluid_held: 48_000,
            fluid_capacity: 64_000,
            works: true,
        }),
        machines: group_machines([
            ("rustcraft:furnace".to_owned(), Activity::Working),
            ("rustcraft:crusher".to_owned(), Activity::NoEnergy),
            ("rustcraft:furnace".to_owned(), Activity::Idle),
        ]),
        containers: 3,
        storages: 0,
        tanks: vec![
            TankLine {
                pos: [11, 64, -4],
                fluid: Some("minecraft:water".to_owned()),
                amount: 12_500,
                capacity: 16_000,
                blocks: 1,
                joint: JointUse::Both,
                pin: Some("minecraft:water".to_owned()),
            },
            TankLine {
                pos: [13, 64, -4],
                fluid: None,
                amount: 0,
                capacity: 32_000,
                blocks: 2,
                joint: JointUse::Off,
                // 0.21 (FT5): empty, kept for milk.
                pin: Some("rustcraft:milk".to_owned()),
            },
        ],
        tank_count: 2,
        ends: group_ends(["rustcraft:pump".to_owned(), "rustcraft:mixer".to_owned(), "rustcraft:pump".to_owned()]),
    }
}

#[test]
fn a_page_comes_back_as_it_went() {
    let p = page();
    assert_eq!(decode(&encode(&p)), Some(p));
    let empty = NetPage::default();
    assert_eq!(decode(&encode(&empty)), Some(empty));
}

#[test]
fn a_cut_or_foreign_message_reads_as_none() {
    let bytes = encode(&page());
    assert_eq!(decode(&bytes[..bytes.len() - 1]), None);
    let mut other = bytes.clone();
    other[0] = 9;
    assert_eq!(decode(&other), None);
    assert_eq!(decode(&[]), None);
}

#[test]
fn a_huge_page_fits_one_message_and_says_how_many_tanks_there_are() {
    let long = "rustcraft:".to_owned() + &"x".repeat(300);
    let mut p = page();
    p.machines = (0..200)
        .map(|i| MachineGroup {
            block: format!("{long}{i}"),
            count: 1,
            ..MachineGroup::default()
        })
        .collect();
    p.tanks = (0..1000)
        .map(|i| TankLine {
            pos: [i, 64, 0],
            fluid: Some(long.clone()),
            amount: 1,
            capacity: 2,
            blocks: 1,
            joint: JointUse::Takes,
            pin: None,
        })
        .collect();
    p.tank_count = 1000;
    p.ends = (0..100)
        .map(|i| EndGroup {
            block: format!("{long}{i}"),
            count: 2,
        })
        .collect();
    let bytes = encode(&p);
    assert!(bytes.len() <= wire::MAX_BYTES, "{}", bytes.len());
    let back = decode(&bytes).unwrap();
    assert_eq!(back.machines.len(), MAX_MACHINES);
    assert_eq!(back.tanks.len(), MAX_TANKS);
    assert_eq!(back.ends.len(), MAX_ENDS);
    assert_eq!(back.tank_count, 1000);
    assert_eq!(back.tanks[0].fluid.as_ref().map(String::len), Some(MAX_ID));
    assert!(rows(&back).contains(&Row::MoreTanks));
}

#[test]
fn the_ask_comes_back_and_nothing_else_reads() {
    for a in [Ask::Terminal([-5, 70, 123_456]), Ask::Tablet] {
        assert_eq!(decode_ask(&encode_ask(a)), Some(a));
    }
    assert_eq!(decode_ask(&[0, 1, 2]), None);
    assert_eq!(decode_ask(&[2]), None);
    assert_eq!(decode_ask(&[]), None);
}

#[test]
fn machines_group_by_kind_with_their_states() {
    let p = page();
    assert_eq!(
        p.machines,
        vec![
            MachineGroup {
                block: "rustcraft:furnace".into(),
                count: 2,
                working: 1,
                idle: 1,
                no_energy: 0
            },
            MachineGroup {
                block: "rustcraft:crusher".into(),
                count: 1,
                working: 0,
                idle: 0,
                no_energy: 1
            },
        ]
    );
    assert_eq!(p.ends[0], EndGroup { block: "rustcraft:pump".into(), count: 2 });
    assert_eq!(Activity::of(Some(Shown::Slowed { percent: 50 })), Activity::Working);
    assert_eq!(Activity::of(Some(Shown::NoFuel)), Activity::NoEnergy);
    assert_eq!(Activity::of(None), Activity::Unknown);
    assert!(JointUse::of(true, true).counted());
    assert!(!JointUse::of(true, false).counted());
    assert_eq!(JointUse::of(false, false), JointUse::Off);
}

#[test]
fn the_rows_follow_the_page() {
    let p = page();
    assert_eq!(
        rows(&p),
        vec![
            Row::Header(Section::Magistral),
            Row::Housings,
            Row::Tier,
            Row::Items,
            Row::Fluid,
            Row::OutOfReach,
            Row::Energy,
            Row::Stored,
            Row::Header(Section::Storage),
            Row::Cells,
            Row::Slots,
            Row::FluidCells,
            Row::Header(Section::Connected),
            Row::Machine(0),
            Row::Machine(1),
            Row::Containers,
            Row::Header(Section::Tanks),
            Row::Tank(0),
            Row::Tank(1),
            Row::Header(Section::FluidEnds),
            Row::End(0),
            Row::End(1),
        ]
    );
    // No magistral, no storage, nothing on it: each part says so.
    let bare = rows(&NetPage::default());
    assert_eq!(
        bare,
        vec![
            Row::Header(Section::Magistral),
            Row::NoBranch,
            Row::Header(Section::Storage),
            Row::NoStorage,
            Row::Header(Section::Connected),
            Row::NothingConnected,
            Row::Header(Section::Tanks),
            Row::NoTanks,
        ]
    );
}

#[test]
fn pages_hold_ten_rows_and_keep_a_part_whole() {
    let r = rows(&page());
    // The magistral; the storage and what is connected; the tanks and the fluid ends.
    assert_eq!(pages(&r), vec![0..8, 8..16, 16..22]);
    assert_eq!(pages(&[]), vec![0..0]);
    let short = rows(&NetPage::default());
    assert_eq!(pages(&short), vec![0..short.len()]);
    // A long list of tanks fills its pages; no page ends on a header.
    let mut long = page();
    long.tanks = (0..25)
        .map(|i| TankLine {
            pos: [i, 0, 0],
            ..TankLine::default()
        })
        .collect();
    long.tank_count = 25;
    for r in [rows(&long), rows(&page()), short] {
        let p = pages(&r);
        assert!(p.iter().all(|x| x.len() <= ROWS && !x.is_empty()), "{p:?}");
        for x in &p {
            assert!(!matches!(r[x.end - 1], Row::Header(_)), "{p:?}");
        }
        // Every row on exactly one page, in order.
        assert_eq!(p.first().map(|x| x.start), Some(0));
        assert!(p.windows(2).all(|w| w[0].end == w[1].start));
        assert_eq!(p.last().map(|x| x.end), Some(r.len()));
    }
}

#[test]
fn the_rows_and_the_button_sit_inside_the_window() {
    // The last row ends above the inventory's label; every row is inside the window.
    let last = row(ROWS - 1);
    assert!(last[1] + last[3] <= terminal::INVENTORY_LABEL_Y);
    assert!(row(0)[1] >= terminal::GRID[1]);
    const { assert!(LEFT >= 8.0 && RIGHT <= terminal::WIDTH - 8.0) };
    assert_eq!(row_at([20.0, 46.0]), Some(0));
    assert_eq!(row_at([200.0, 45.0 + 10.0 * 9.0 + 2.0]), Some(9));
    assert_eq!(row_at([20.0, 45.0 + 10.0 * 10.0 + 2.0]), None);
    assert_eq!(row_at([4.0, 46.0]), None);
    // The button is on the title line, right of the title, above its rule.
    const { assert!(NET_MODE[0] + NET_MODE[2] <= terminal::WIDTH - 8.0) };
    const { assert!(NET_MODE[1] + NET_MODE[3] <= 14.0) };
    const { assert!(NET_MODE[1] >= 3.0, "inside the frame") };
    // It touches none of the top rows' parts.
    for r in terminal::TABS.iter().chain([&terminal::SEARCH, &terminal::LIMIT_MODE]) {
        assert!(NET_MODE[1] + NET_MODE[3] <= r[1]);
    }
}

/// A tier-1 node at the origin, housings of `tiers` along x after it, and an unserved line of
/// two housings far away.
fn topology(tiers: &[u8]) -> Topology {
    let mut layout = Layout::default();
    layout.nodes.insert([0, 0, 0], Node { tier: 1 });
    for (x, t) in tiers.iter().enumerate() {
        layout.housings.insert([x as i32 + 1, 0, 0], Housing::new(*t));
    }
    layout.housings.insert([0, 0, 50], Housing::new(1));
    layout.housings.insert([1, 0, 50], Housing::new(1));
    crate::net::topology::analyse(&layout, &Tuning::default())
}

#[test]
fn the_branch_is_what_the_storage_touches() {
    let t = topology(&[2, 2, 1, 3]);
    // A cell beside housing 2 and a terminal beside it.
    let b = branch(&t, &[[2, 1, 0], [3, 1, 0]]);
    assert_eq!(b.nets.len(), 1);
    assert_eq!(b.housings.len(), 4);
    assert_eq!(b.min_tier, 1, "the weakest housing");
    assert_eq!(b.out_of_reach, 0);
    // Off the magistral: nothing.
    assert_eq!(branch(&t, &[[2, 5, 0]]), Branch::default());
    // Beside the unserved line: no net, its two housings out of reach.
    let far = branch(&t, &[[0, 1, 50]]);
    assert!(far.nets.is_empty());
    assert_eq!(far.out_of_reach, 2);
}

#[test]
fn housings_past_the_node_range_count_as_out_of_reach() {
    // A tier-1 node serves its range; the housings after it on the same line do not work.
    let range = Tuning::default().node_range[0] as usize;
    let t = topology(&vec![1; range + 3]);
    let b = branch(&t, &[[1, 1, 0]]);
    assert!(b.out_of_reach > 0);
    assert_eq!(b.housings.len() + b.out_of_reach as usize, range + 3);
}
