//! A storage's orders kept over a restart of the server: its queue, the jobs its orders have on
//! machines and the autocrafters' temporary pages go into the storage's record (the bundle's level
//! attachment) as a list of short JSON strings, one for the head, each order and each job, so no
//! string comes near the 65 535 bytes an NBT string holds.
//!
//! What a machine already holds for a job stays in it and is neither made twice nor lost: on load
//! the job takes its machine again, a crafter's page owing no more than its buffer can still make
//! (`owed_after_load`); a job whose order is gone gives its items back to the storage (O7), and a
//! job whose machine is gone ends, as it would without a restart.

use serde::{Deserialize, Serialize};

use crate::machines::slots::{self, Slot};
use crate::net::items::ItemKey;
use crate::orders::fluid_of;
use crate::orders::queue::{Job, Order, Queue};

/// The record's version; a record of a newer one is not read.
pub const VERSION: u32 = 1;

/// An autocrafter's temporary page (O4): its pattern, cell by cell, and the crafts it still owes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedPage {
    pub pattern: Vec<Option<ItemKey>>,
    pub owed: u32,
}

/// A job on a machine, and its page when the machine is an autocrafter.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedJob {
    pub job: Job,
    #[serde(default)]
    pub page: Option<SavedPage>,
}

/// What one storage keeps of its orders.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Record {
    pub queue: Queue,
    pub jobs: Vec<SavedJob>,
}

impl Record {
    pub fn is_empty(&self) -> bool {
        self.queue.orders.is_empty() && self.jobs.is_empty()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
enum Entry {
    Head { v: u32, next: u16 },
    Order(Order),
    Job(SavedJob),
}

/// The record as the strings the storage's record keeps; none for an empty record.
pub fn encode(record: &Record) -> Vec<String> {
    if record.is_empty() {
        return Vec::new();
    }
    let head = Entry::Head {
        v: VERSION,
        next: record.queue.next_id(),
    };
    let orders = record.queue.orders.iter().cloned().map(Entry::Order);
    let jobs = record.jobs.iter().cloned().map(Entry::Job);
    std::iter::once(head)
        .chain(orders)
        .chain(jobs)
        .filter_map(|e| serde_json::to_string(&e).ok())
        .collect()
}

/// What `encode` wrote: none without a head or for a newer version. An entry that does not read
/// is left out; a job whose order is not in the queue is kept, so its items go back.
pub fn decode(entries: &[String]) -> Option<Record> {
    let mut parsed = entries
        .iter()
        .filter_map(|s| serde_json::from_str::<Entry>(s).ok());
    let Some(Entry::Head { v, next }) = parsed.next() else {
        return None;
    };
    if v > VERSION {
        return None;
    }
    let mut orders = Vec::new();
    let mut jobs = Vec::new();
    for entry in parsed {
        match entry {
            Entry::Head { .. } => {}
            Entry::Order(o) => orders.push(o),
            Entry::Job(j) => jobs.push(j),
        }
    }
    Some(Record {
        queue: Queue::restored(orders, next),
        jobs,
    })
}

/// The crafts a crafter's page owes after a load: no more than it saved, and no more than its
/// buffer holds the items for (crafts made after the record was last written spent theirs).
pub fn owed_after_load(saved: u32, inputs: &[(ItemKey, u32)], buffer: &[Slot]) -> u32 {
    inputs
        .iter()
        .filter(|(item, n)| fluid_of(item).is_none() && *n > 0)
        .map(|(item, n)| slots::count_in(buffer, item) / n)
        .fold(saved, u32::min)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orders::Step;
    use crate::orders::queue::Wait;
    use std::collections::BTreeMap;

    const PISTON: &str = "minecraft:piston";
    const IRON: &str = "minecraft:iron_ingot";

    fn step() -> Step {
        Step {
            recipe: 7,
            machine: "crafter".into(),
            inputs: vec![(IRON.into(), 1), ("minecraft:cobblestone".into(), 4)],
            output: (PISTON.into(), 1),
            times: 3,
        }
    }

    fn record() -> Record {
        let mut queue = Queue::default();
        let waits = [
            Wait::New,
            Wait::Running,
            Wait::Items(vec![(IRON.into(), 2), ("fluid:minecraft:water".into(), 1)]),
            Wait::Machines(vec!["alloy".into()]),
            Wait::Busy("furnace".into()),
            Wait::Energy("alloy".into()),
        ];
        for (k, wait) in waits.into_iter().enumerate() {
            queue.add(PISTON, 4 + k as u32, k % 2 == 0);
            let o = queue.orders.last_mut().unwrap();
            o.wait = wait;
            o.done = k as u32;
            o.reserved = BTreeMap::from([(IRON.to_string(), 2)]);
            o.made = BTreeMap::from([("minecraft:copper_ingot".to_string(), k as u64)]);
        }
        let id = queue.orders[1].id;
        let job = |x: i32| Job {
            order: id,
            machine: [x, 64, -3],
            step: step(),
            runs: 3,
        };
        let mut pattern = vec![None; 9];
        pattern[0] = Some(IRON.to_string());
        Record {
            queue,
            jobs: vec![
                SavedJob {
                    job: job(1),
                    page: Some(SavedPage { pattern, owed: 2 }),
                },
                SavedJob {
                    job: job(2),
                    page: None,
                },
            ],
        }
    }

    #[test]
    fn a_queue_and_its_jobs_come_back_as_they_went() {
        let r = record();
        let saved = encode(&r);
        assert_eq!(saved.len(), 1 + 6 + 2);
        assert!(saved.iter().all(|s| s.len() < 2_000), "{saved:?}");
        assert_eq!(decode(&saved), Some(r.clone()));
        // The queue goes on numbering where it was.
        let mut back = decode(&saved).unwrap();
        let next = back.queue.add(PISTON, 1, false).unwrap();
        assert!(r.queue.orders.iter().all(|o| o.id < next));
    }

    #[test]
    fn an_empty_record_is_nothing_and_nothing_reads_as_none() {
        assert!(encode(&Record::default()).is_empty());
        assert_eq!(decode(&[]), None);
    }

    #[test]
    fn a_record_that_does_not_read_is_left_out() {
        let mut saved = encode(&record());
        // A newer version is not read; an entry cut short is left out.
        let newer = saved[0].replace("\"v\":1", "\"v\":99");
        let mut cut = saved.clone();
        cut[0] = newer;
        assert_eq!(decode(&cut), None);
        saved[2].truncate(5);
        saved.push("{\"t\":\"spaceship\"}".into());
        let back = decode(&saved).unwrap();
        assert_eq!(back.queue.orders.len(), 5);
        assert_eq!(back.jobs.len(), 2);
        // Without a head nothing is read.
        assert_eq!(decode(&saved[1..]), None);
    }

    #[test]
    fn an_older_order_without_its_own_products_reads() {
        // A record written before `made` was kept.
        let head = r#"{"t":"head","v":1,"next":4}"#.to_string();
        let order = r#"{"t":"order","id":4,"item":"minecraft:piston","count":2,"done":0,"keep":false,"wait":"New","reserved":{}}"#.to_string();
        let back = decode(&[head, order]).unwrap();
        assert_eq!(back.queue.orders[0].id, 4);
        assert!(back.queue.orders[0].made.is_empty());
    }

    #[test]
    fn a_page_owes_no_more_than_the_buffer_can_make() {
        let inputs = step().inputs;
        let buffer: Vec<Slot> = vec![
            Some((IRON.into(), 3)),
            Some(("minecraft:cobblestone".into(), 8)),
            None,
        ];
        // The record said 3, but one craft ran after it was written: 8 cobblestone make 2.
        assert_eq!(owed_after_load(3, &inputs, &buffer), 2);
        assert_eq!(owed_after_load(1, &inputs, &buffer), 1);
        assert_eq!(owed_after_load(3, &inputs, &[]), 0);
    }
}
