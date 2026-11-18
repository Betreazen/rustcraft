use super::journal::{self, Buffer};
use super::text;
use super::track::{Seen, Tracker, next_batch};
use super::*;
use serde_json::json;
use std::collections::BTreeSet;

#[test]
fn the_command_reads_every_subcommand() {
    assert_eq!(
        parse("at 1 -2 3"),
        Ok(Command::At { pos: [1, -2, 3], dimension: None })
    );
    assert_eq!(
        parse("at 1 2 3 nether"),
        Ok(Command::At {
            pos: [1, 2, 3],
            dimension: Some("minecraft:the_nether".into())
        })
    );
    assert_eq!(
        parse("near Steve"),
        Ok(Command::Near { player: "Steve".into(), radius: NEAR_RADIUS })
    );
    assert_eq!(
        parse("near Steve 9999"),
        Ok(Command::Near { player: "Steve".into(), radius: NEAR_MAX })
    );
    assert_eq!(parse("workers"), Ok(Command::Workers { owner: None }));
    assert_eq!(
        parse("workers Alex"),
        Ok(Command::Workers { owner: Some("Alex".into()) })
    );
    assert_eq!(parse("log"), Ok(Command::Log { n: LOG_LINES, filter: None }));
    assert_eq!(parse("log 5"), Ok(Command::Log { n: 5, filter: None }));
    assert_eq!(
        parse("log 5 status 1 2"),
        Ok(Command::Log { n: 5, filter: Some("status 1 2".into()) })
    );
    assert_eq!(
        parse("log worker"),
        Ok(Command::Log { n: LOG_LINES, filter: Some("worker".into()) })
    );
    assert_eq!(parse("log 0"), Ok(Command::Log { n: 1, filter: None }));
}

#[test]
fn bad_words_get_the_usage_or_say_what_is_wrong() {
    assert_eq!(parse(""), Err(usage()));
    assert_eq!(parse("at 1 2"), Err(usage()));
    assert_eq!(parse("frobnicate"), Err(usage()));
    assert!(parse("at 1 x 3").unwrap_err().contains("x"));
    assert!(parse("near Steve far").unwrap_err().contains("far"));
}

#[test]
fn dimensions_by_short_name() {
    assert_eq!(dimension_id("overworld"), "minecraft:overworld");
    assert_eq!(dimension_id("end"), "minecraft:the_end");
    assert_eq!(dimension_id("mymod:moon"), "mymod:moon");
}

#[test]
fn a_journal_line_has_every_field_and_stays_one_line() {
    let l = journal::line("2026-01-01T00:00:00Z", 42, Some("minecraft:overworld"), Some([1, 2, 3]), "status", "a\nb");
    assert_eq!(l, "2026-01-01T00:00:00Z 42 minecraft:overworld 1 2 3 status a b");
    let l = journal::line("T", 0, None, None, "warn x", "oops");
    assert_eq!(l, "T 0 - - - - warn_x oops");
    let long = "x".repeat(journal::MAX_DETAIL + 50);
    assert!(journal::line("T", 0, None, None, "k", &long).ends_with("..."));
}

#[test]
fn utc_from_unix_seconds() {
    assert_eq!(journal::utc(0), "1970-01-01T00:00:00Z");
    assert_eq!(journal::utc(951_782_400), "2000-02-29T00:00:00Z");
    assert_eq!(journal::utc(1_795_030_205), "2026-11-18T19:30:05Z");
}

#[test]
fn the_file_rotates_past_two_megabytes_only() {
    assert!(!journal::should_rotate(0, 10 * journal::ROTATE_AT));
    assert!(!journal::should_rotate(journal::ROTATE_AT - 10, 10));
    assert!(journal::should_rotate(journal::ROTATE_AT - 10, 11));
}

#[test]
fn the_buffer_goes_out_once_a_second_or_when_full() {
    let mut b = Buffer::default();
    assert!(!b.due(10_000));
    b.push("one".into());
    assert!(b.due(1000));
    let mut b = Buffer { flushed_at: 5000, ..Buffer::default() };
    b.push("one".into());
    assert!(!b.due(5500));
    for i in 1..journal::FLUSH_LINES {
        b.push(i.to_string());
    }
    assert!(b.due(5500));
    let text = b.take(5500);
    assert!(text.starts_with("one\n1\n") && text.ends_with('\n'));
    assert!(b.pending.is_empty() && !b.due(9999));
}

#[test]
fn the_buffer_keeps_the_newest_lines_while_it_waits() {
    let mut b = Buffer::default();
    for i in 0..journal::MAX_PENDING + 5 {
        b.push(i.to_string());
    }
    assert_eq!(b.pending.len(), journal::MAX_PENDING);
    assert_eq!(b.pending[0], "5");
}

#[test]
fn the_tail_takes_the_last_matching_lines_in_order() {
    let text = "a 1\nb 2\na 3\n\nb 4\na 5\n";
    assert_eq!(journal::tail(text.lines(), 2, None), vec!["b 4", "a 5"]);
    assert_eq!(journal::tail(text.lines(), 2, Some("a")), vec!["a 3", "a 5"]);
    assert_eq!(journal::tail(text.lines(), 10, Some("z")), Vec::<&str>::new());
}

#[test]
fn a_long_answer_is_clipped_with_a_note() {
    let lines: Vec<String> = (0..100).map(|i| format!("line {i}")).collect();
    let out = clip(lines);
    assert!(out.len() <= MAX_LINES);
    assert!(out.last().unwrap().contains("more lines not shown"));
    let wide: Vec<String> = (0..30).map(|_| "y".repeat(500)).collect();
    let out = clip(wide);
    assert!(out.iter().map(|l| l.len() + 1).sum::<usize>() <= MAX_CHARS);
    assert_eq!(clip(vec!["one".into()]), vec!["one".to_owned()]);
}

#[test]
fn a_long_tail_keeps_its_newest_lines_and_leaves_room_for_a_heading() {
    let lines: Vec<String> = (0..200).map(|i| format!("event {i:03} {}", "z".repeat(100))).collect();
    let out = clip_newest(lines);
    assert!(out[0].contains("older lines not shown"));
    assert!(out.last().unwrap().starts_with("event 199"));
    let mut with_heading = vec!["heading".to_owned()];
    with_heading.extend(out.clone());
    assert_eq!(clip(with_heading).len(), out.len() + 1);
    assert_eq!(clip_newest(vec!["a".into(), "b".into()]), vec!["a".to_owned(), "b".to_owned()]);
}

#[test]
fn a_status_reads_as_its_keys_and_its_fallbacks() {
    let c = json!({
        "translate": "rustcraft.machine.slowed",
        "fallback": "Not enough energy: %s",
        "with": [{"text": "50%"}],
        "extra": [{"text": " · "}, {"translate": "rustcraft.joint.receiver_full", "fallback": "Receiver full"}]
    });
    assert_eq!(text::keys(&c), vec!["rustcraft.machine.slowed", "rustcraft.joint.receiver_full"]);
    assert_eq!(text::plain(&c), "Not enough energy: 50% · Receiver full");
    assert_eq!(text::signature(&c), "rustcraft.machine.slowed+rustcraft.joint.receiver_full");
    assert_eq!(
        text::status_line(&c),
        "rustcraft.machine.slowed, rustcraft.joint.receiver_full | Not enough energy: 50% · Receiver full"
    );
    assert_eq!(text::signature(&json!({"text": "plain"})), "plain");
}

#[test]
fn a_translation_fills_its_arguments_as_the_game_does() {
    let a = vec!["x".to_owned(), "y".to_owned()];
    assert_eq!(text::fill("%s of %s", &a), "x of y");
    assert_eq!(text::fill("%2$s then %1$s", &a), "y then x");
    assert_eq!(text::fill("100%% %s", &a), "100% x");
    assert_eq!(text::fill("odd % sign", &a), "odd % sign");
}

#[test]
fn the_tracker_reports_changes_only() {
    let mut t: Tracker<(String, [i32; 3])> = Tracker::default();
    let k = ("minecraft:overworld".to_owned(), [1, 2, 3]);
    assert_eq!(t.observe(k.clone(), "idle"), Seen::New);
    assert_eq!(t.observe(k.clone(), "idle"), Seen::Same);
    assert_eq!(t.observe(k.clone(), "no_energy"), Seen::Changed("idle".into()));
    assert_eq!(t.last(&k), Some("no_energy"));
    let other = ("minecraft:overworld".to_owned(), [9, 9, 9]);
    t.observe(other.clone(), "working");
    assert_eq!(t.retain(|key| key.1 != [9, 9, 9]), vec![other]);
    assert_eq!(t.len(), 1);
}

#[test]
fn batches_go_round_the_whole_set() {
    let all: BTreeSet<u32> = (1..=5).collect();
    let (a, c) = next_batch(&all, None, 2);
    assert_eq!(a, vec![1, 2]);
    let (b, c) = next_batch(&all, c.as_ref(), 2);
    assert_eq!(b, vec![3, 4]);
    let (d, c) = next_batch(&all, c.as_ref(), 2);
    assert_eq!(d, vec![5, 1]);
    let (e, _) = next_batch(&all, c.as_ref(), 2);
    assert_eq!(e, vec![2, 3]);
    // A batch larger than the set takes each once.
    assert_eq!(next_batch(&all, Some(&3), 10).0, vec![4, 5, 1, 2, 3]);
    // A cursor that is gone still finds its place.
    let small: BTreeSet<u32> = [10, 20].into();
    assert_eq!(next_batch(&small, Some(&15), 1).0, vec![20]);
    assert_eq!(next_batch(&BTreeSet::<u32>::new(), None, 3).0, Vec::<u32>::new());
}

#[test]
fn the_water_grid_marks_the_station_and_the_line() {
    let mut depths = vec![2; 25];
    depths[0] = 0;
    let mut line = vec![false; 25];
    line[4] = true;
    let rows = water_grid(&depths, &line, 5);
    assert_eq!(rows.len(), 5);
    assert_eq!(rows[0], "  dx -2:  0  2  2  2 L2");
    assert_eq!(rows[2], "  dx +0:  2  2 @2  2  2");
    assert!(water_grid(&[1, 2], &[], 5)[0].contains("not 5x5"));
}

#[test]
fn near_is_a_sphere_round_the_player() {
    assert!(within([0.5, 64.0, 0.5], [0, 64, 0], 1));
    assert!(within([0.0, 64.0, 0.0], [10, 64, 0], 16));
    assert!(!within([0.0, 64.0, 0.0], [12, 64, 12], 16));
}

#[test]
fn a_worker_row_says_everything_on_one_line() {
    let row = WorkerRow {
        id: 7,
        owner: "Alex".into(),
        cell: [1, 2, 3],
        tool: "Saw".into(),
        charge_percent: 49.6,
        hold_used: 3,
        hold_size: 9,
        errand: "Work".into(),
        note_key: "rustcraft.worker.working",
        loaded: false,
    };
    assert_eq!(
        row.line(),
        "worker #7 owner Alex at 1 2 3 tool Saw charge 50% hold 3/9 errand Work note rustcraft.worker.working (chunk not loaded)"
    );
}

#[test]
fn storage_reads_like_at() {
    assert_eq!(
        parse("storage 4 5 6 end"),
        Ok(Command::Storage {
            pos: [4, 5, 6],
            dimension: Some("minecraft:the_end".into())
        })
    );
    assert_eq!(parse("storage 4 5"), Err(usage()));
}

#[test]
fn the_top_items_are_the_most_numerous() {
    let items: std::collections::BTreeMap<String, u64> =
        [("a".into(), 5), ("b".into(), 50), ("c".into(), 5), ("d".into(), 0)].into();
    assert_eq!(
        top_items(&items, 2),
        vec![("b".to_owned(), 50), ("a".to_owned(), 5)]
    );
    assert_eq!(top_items(&items, 10).len(), 3);
}

#[test]
fn packed_entries_share_lines() {
    let entries: Vec<String> = (0..10).map(|i| format!("item{i} x{i}")).collect();
    let lines = pack("  ", &entries, 30);
    assert!(lines.len() > 1 && lines.len() < 10);
    assert!(lines.iter().all(|l| l.starts_with("  ")));
    assert_eq!(lines.join(", ").matches("item").count(), 10);
    assert!(pack("x", &[], 10).is_empty());
}

#[test]
fn flows_sum_by_route_and_item_until_drained() {
    let mut f = super::flow::Flows::default();
    f.add("storage#1 -> crusher net", "minecraft:iron_ore", 4);
    f.add("storage#1 -> crusher net", "minecraft:iron_ore", 8);
    f.add("storage#1 -> crusher net", "minecraft:gold_ore", 1);
    f.add("r", "i", 0);
    let lines = f.drain();
    assert_eq!(
        lines,
        vec![
            "storage#1 -> crusher net minecraft:gold_ore x1".to_owned(),
            "storage#1 -> crusher net minecraft:iron_ore x12".to_owned(),
        ]
    );
    assert!(f.is_empty() && f.drain().is_empty());
}

#[test]
fn orders_start_wait_finish_and_go() {
    use super::track::{OrderSeen, order_events, set_diff};
    use std::collections::BTreeMap;
    let seen = |done: u32, wait: &str| OrderSeen {
        item: "minecraft:iron_ingot".into(),
        count: 8,
        done,
        wait: wait.into(),
    };
    let none = BTreeMap::new();
    let one: BTreeMap<(u32, u16), OrderSeen> = [((1, 2), seen(0, "New"))].into();
    let ev = order_events(&none, &one);
    assert_eq!(ev.len(), 1);
    assert_eq!(ev[0].1, "order_started");
    assert!(ev[0].2.contains("storage#1 order#2 minecraft:iron_ingot 0/8"));
    let running: BTreeMap<(u32, u16), OrderSeen> = [((1, 2), seen(3, "Running"))].into();
    let ev = order_events(&one, &running);
    assert_eq!(ev[0].1, "order_wait");
    assert!(ev[0].2.ends_with("New -> Running"));
    assert!(order_events(&running, &running).is_empty());
    assert_eq!(order_events(&running, &none)[0].1, "order_gone");
    let made: BTreeMap<(u32, u16), OrderSeen> = [((1, 2), seen(8, "Running"))].into();
    assert_eq!(order_events(&made, &none)[0].1, "order_finished");
    let a: BTreeSet<u32> = [1, 2].into();
    let b: BTreeSet<u32> = [2, 3].into();
    assert_eq!(set_diff(&a, &b), (vec![3], vec![1]));
}

#[test]
fn a_component_key_is_shortened_the_same_way_every_time() {
    let long = format!("minecraft:stone#{}", "0a0b".repeat(40));
    let short = short_key(&long);
    assert!(short.starts_with("minecraft:stone#") && short.len() == "minecraft:stone#".len() + 8);
    assert_eq!(short, short_key(&long));
    assert_ne!(short, short_key(&format!("minecraft:stone#{}", "0a0c".repeat(40))));
    assert_eq!(short_key("minecraft:iron_pickaxe@12"), "minecraft:iron_pickaxe@12");
    assert_eq!(short_key("minecraft:book#"), "minecraft:book#");
}
