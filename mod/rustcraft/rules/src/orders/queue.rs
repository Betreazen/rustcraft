//! The orders of one storage, in the order they came (O8). Once a second the bundle asks the queue
//! for a round: each order not running is planned again on what the storage holds now (an earlier
//! order's items set aside first), and every step whose inputs are all set aside goes to free
//! machines of its kind on the line, split between them (O4, O5). The bundle hands each job its
//! items from the storage and, when the machine has run it, says how many runs were done; the next
//! round plans the rest, so a step made of another step's products starts once they are back in
//! the storage. An order short of an item or a machine waits and says which (O6, O13), and goes on
//! by itself when the storage has it. «Держать N» (O14) orders the difference whenever the stock of
//! an item falls below N and no such order is open.
//!
//! O11: what a step makes for a later step of the same order comes back into the storage's count
//! and is the order's own there (`Order::made`): no other order plans on it, and the round in the
//! same second hands it to the next machine. A step takes a free machine with energy or fuel
//! first; an order whose jobs all stand on machines without either waits for energy and says so.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::geom::Pos;
use crate::machines::slots::{self, Slot};
use crate::net::items::ItemKey;
use crate::orders::{PlanRecipe, Step, plan};

/// M15: the smart net of orders opens on a line with a node of this tier or higher. Proposal: the
/// playtest may move it earlier.
pub const SMART_TIER: u8 = 3;
/// Proposal: orders one storage keeps at once, «держать» ones among them.
pub const MAX_ORDERS: usize = 12;
/// Proposal: the most items one order asks for.
pub const MAX_COUNT: u32 = 4096;

/// What an order waits for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Wait {
    /// Not planned yet.
    New,
    /// Steps of it run on machines.
    Running,
    /// O6: the items the storage lacks and no recipe makes, and how many.
    Items(Vec<(ItemKey, u64)>),
    /// O13: the machine kinds the line lacks.
    Machines(Vec<String>),
    /// Every machine of this kind on the line is busy.
    Busy(String),
    /// Its jobs stand on machines of this kind that have neither energy nor fuel: «ждёт
    /// энергию».
    Energy(String),
}

/// One order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Order {
    pub id: u16,
    pub item: ItemKey,
    pub count: u32,
    /// Made so far and back in the storage.
    pub done: u32,
    /// O14: made by «держать N», not by a player's order.
    pub keep: bool,
    pub wait: Wait,
    /// What the storage still holds set aside for its steps not started yet.
    pub reserved: BTreeMap<ItemKey, u64>,
    /// O11: what its steps made for its later steps and the storage holds for it.
    #[serde(default)]
    pub made: BTreeMap<ItemKey, u64>,
}

impl Order {
    /// What the order holds in the storage: of each item, the more of its set-aside and its own.
    fn claim(&self) -> BTreeMap<ItemKey, u64> {
        let mut claim = self.reserved.clone();
        for (item, &n) in &self.made {
            let c = claim.entry(item.clone()).or_insert(0);
            *c = (*c).max(n);
        }
        claim
    }
}

/// Runs of a step handed to one machine.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Job {
    pub order: u16,
    pub machine: Pos,
    pub step: Step,
    pub runs: u32,
}

/// A machine on the line that has no job: where it stands and its kind (one of `orders::KINDS`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FreeMachine {
    pub pos: Pos,
    pub kind: String,
    /// It has energy (a source on its circuit or touching it) or, an alloy smelter, fuel.
    pub powered: bool,
}

/// The orders of one storage.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Queue {
    pub orders: Vec<Order>,
    next_id: u16,
}

impl Queue {
    /// A queue read back from the storage's record: its orders and the id it gave last.
    pub fn restored(orders: Vec<Order>, next_id: u16) -> Queue {
        let mut ids = BTreeSet::new();
        let orders: Vec<Order> = orders
            .into_iter()
            .filter(|o| o.id != 0 && ids.insert(o.id))
            .take(MAX_ORDERS)
            .collect();
        let highest = orders.iter().map(|o| o.id).max().unwrap_or(0);
        Queue {
            orders,
            next_id: next_id.max(highest),
        }
    }

    /// The id the queue gave last.
    pub fn next_id(&self) -> u16 {
        self.next_id
    }

    /// A new order for `count` of `item` (at most MAX_COUNT); none when the queue is full or the
    /// count is zero.
    pub fn add(&mut self, item: &str, count: u32, keep: bool) -> Option<u16> {
        if count == 0 || item.is_empty() || self.orders.len() >= MAX_ORDERS {
            return None;
        }
        self.next_id = self.next_id.wrapping_add(1).max(1);
        let id = self.next_id;
        self.orders.push(Order {
            id,
            item: item.to_owned(),
            count: count.min(MAX_COUNT),
            done: 0,
            keep,
            wait: Wait::New,
            reserved: BTreeMap::new(),
            made: BTreeMap::new(),
        });
        Some(id)
    }

    /// O11: `n` of `item` that a job of the order `id` made for a later step came into the
    /// storage: they are the order's own there.
    pub fn keep_for(&mut self, id: u16, item: &str, n: u64) {
        if let Some(o) = self.orders.iter_mut().find(|o| o.id == id)
            && item != o.item
            && n > 0
        {
            *o.made.entry(item.to_owned()).or_insert(0) += n;
        }
    }

    /// A job went to its machine: the order's own items it took are no longer in the storage.
    pub fn started(&mut self, job: &Job) {
        let Some(o) = self.orders.iter_mut().find(|o| o.id == job.order) else {
            return;
        };
        for (item, n) in &job.step.inputs {
            if let Some(m) = o.made.get_mut(item) {
                *m = m.saturating_sub(u64::from(*n) * u64::from(job.runs));
            }
        }
        o.made.retain(|_, n| *n > 0);
    }

    /// A running order whose every job stands on a machine without energy or fuel waits for
    /// energy, and says on which kind of machine, instead of standing still unsaid.
    pub fn wait_for_energy(&mut self, jobs: &[Job], powered: &dyn Fn(Pos) -> bool) {
        for o in &mut self.orders {
            if o.wait != Wait::Running {
                continue;
            }
            let own: Vec<&Job> = jobs.iter().filter(|j| j.order == o.id).collect();
            if let Some(first) = own.first()
                && own.iter().all(|j| !powered(j.machine))
            {
                o.wait = Wait::Energy(first.step.machine.clone());
            }
        }
    }

    /// O7: the order leaves the queue. What it set aside and what its steps made stay in the
    /// storage; the bundle takes back what its jobs did not use yet.
    pub fn cancel(&mut self, id: u16) -> Option<Order> {
        let i = self.orders.iter().position(|o| o.id == id)?;
        Some(self.orders.remove(i))
    }

    /// A job's machine ran `runs` of its runs: the order's own item, when the job made it, counts
    /// as done.
    pub fn finished(&mut self, job: &Job, runs: u32) {
        let Some(o) = self.orders.iter_mut().find(|o| o.id == job.order) else {
            return;
        };
        if job.step.output.0 == o.item {
            let made = runs.saturating_mul(job.step.output.1.max(1));
            o.done = o.done.saturating_add(made).min(o.count);
        }
    }

    /// Orders whose count is made leave the queue; they are returned.
    pub fn remove_done(&mut self) -> Vec<Order> {
        let (done, open): (Vec<Order>, Vec<Order>) = std::mem::take(&mut self.orders)
            .into_iter()
            .partition(|o| o.done >= o.count);
        self.orders = open;
        done
    }

    /// O14: for each item kept at N whose stock is below N, an order for the difference unless
    /// one is open; a «держать» order whose N was cleared and that runs nothing leaves.
    pub fn keep(
        &mut self,
        keeps: &BTreeMap<ItemKey, u64>,
        stock: &BTreeMap<ItemKey, u64>,
        running: &dyn Fn(u16) -> bool,
    ) {
        self.orders
            .retain(|o| !o.keep || keeps.get(&o.item).is_some_and(|&n| n > 0) || running(o.id));
        for (item, &n) in keeps {
            let open = self.orders.iter().any(|o| o.keep && o.item == *item);
            let have = stock.get(item).copied().unwrap_or(0);
            if n > have && !open {
                let short = u32::try_from(n - have).unwrap_or(u32::MAX);
                self.add(item, short, true);
            }
        }
    }

    /// One round: the jobs to start now. `stock` is what the storage holds, `kinds` the machine
    /// kinds on the line, `free` its machines without a job, `running` whether an order has jobs
    /// on machines, and `fits` how many of `want` runs of a step a machine can take in.
    pub fn round(
        &mut self,
        stock: &BTreeMap<ItemKey, u64>,
        book: &[PlanRecipe],
        kinds: &[&str],
        free: &[FreeMachine],
        running: &dyn Fn(u16) -> bool,
        fits: &mut dyn FnMut(Pos, &Step, u32) -> u32,
    ) -> Vec<Job> {
        // An order's own products count as far as the storage still holds them (a player may
        // have taken some out).
        for o in &mut self.orders {
            for (item, n) in &mut o.made {
                *n = (*n).min(stock.get(item).copied().unwrap_or(0));
            }
            o.made.retain(|_, n| *n > 0);
        }
        let mut left = stock.clone();
        for o in &self.orders {
            if running(o.id) {
                take_all(&mut left, &o.claim());
            } else {
                take_all(&mut left, &o.made);
            }
        }
        let mut taken: BTreeSet<Pos> = BTreeSet::new();
        let mut jobs = Vec::new();
        for o in &mut self.orders {
            if running(o.id) {
                o.wait = Wait::Running;
                continue;
            }
            let wanted = o.count.saturating_sub(o.done);
            if wanted == 0 {
                continue;
            }
            // The item's own stock is not the order: an order makes new ones. Its own products
            // are its stock too (O11).
            let mut free_stock = left.clone();
            for (item, n) in &o.made {
                *free_stock.entry(item.clone()).or_insert(0) += n;
            }
            free_stock.remove(&o.item);
            let p = plan(&o.item, u64::from(wanted), &free_stock, book, kinds);
            o.reserved.clear();
            if !p.missing_machines.is_empty() {
                o.wait = Wait::Machines(p.missing_machines);
                continue;
            }
            if !p.missing.is_empty() {
                o.wait = Wait::Items(p.missing.into_iter().collect());
                continue;
            }
            // What the plan takes beyond the order's own products is no later order's; its own
            // products the plan does not use are free from the next round.
            let beyond: BTreeMap<ItemKey, u64> = p
                .reserved
                .iter()
                .map(|(k, n)| {
                    let own = o.made.get(k).copied().unwrap_or(0);
                    (k.clone(), n.saturating_sub(own))
                })
                .collect();
            take_all(&mut left, &beyond);
            o.made = o
                .made
                .iter()
                .filter_map(|(k, n)| {
                    let used = (*n).min(p.reserved.get(k).copied().unwrap_or(0));
                    (used > 0).then(|| (k.clone(), used))
                })
                .collect();
            let mut avail = p.reserved;
            let mut busy: Option<String> = None;
            let before = jobs.len();
            for step in &p.steps {
                let covered = step.inputs.iter().all(|(item, n)| {
                    avail.get(item).copied().unwrap_or(0) >= u64::from(*n) * u64::from(step.times)
                });
                if !covered {
                    continue;
                }
                // Machines with energy or fuel first: one without goes only when no other is free.
                let of_kind: Vec<&FreeMachine> = free
                    .iter()
                    .filter(|m| m.kind == step.machine && !taken.contains(&m.pos))
                    .collect();
                let any_powered = of_kind.iter().any(|m| m.powered);
                let machines: Vec<Pos> = of_kind
                    .iter()
                    .filter(|m| m.powered || !any_powered)
                    .map(|m| m.pos)
                    .collect();
                let share = step.times.div_ceil(machines.len().max(1) as u32);
                let mut runs_left = step.times;
                for pos in machines {
                    if runs_left == 0 {
                        break;
                    }
                    let runs = fits(pos, step, share.min(runs_left)).min(runs_left);
                    if runs == 0 {
                        continue;
                    }
                    taken.insert(pos);
                    runs_left -= runs;
                    for (item, n) in &step.inputs {
                        let used = u64::from(*n) * u64::from(runs);
                        if let Some(a) = avail.get_mut(item) {
                            *a = a.saturating_sub(used);
                        }
                    }
                    jobs.push(Job {
                        order: o.id,
                        machine: pos,
                        step: step.clone(),
                        runs,
                    });
                }
                if runs_left == step.times {
                    busy.get_or_insert_with(|| step.machine.clone());
                }
            }
            avail.retain(|_, n| *n > 0);
            o.reserved = avail;
            o.wait = if jobs.len() > before {
                Wait::Running
            } else {
                Wait::Busy(busy.unwrap_or_else(|| crate::orders::KINDS[0].to_owned()))
            };
        }
        jobs
    }
}

/// `stock` less what `taken` names, never below zero.
fn take_all(stock: &mut BTreeMap<ItemKey, u64>, taken: &BTreeMap<ItemKey, u64>) {
    for (item, n) in taken {
        if let Some(have) = stock.get_mut(item) {
            *have = have.saturating_sub(*n);
        }
    }
}

/// How many of `want` runs of `inputs` the slots take in, each input onto its own stacks or into
/// empty slots, `stack` of an item a slot.
pub fn runs_that_fit(
    slots: &[Slot],
    inputs: &[(ItemKey, u32)],
    want: u32,
    stack: &dyn Fn(&str) -> u32,
) -> u32 {
    let fits = |runs: u32| {
        let mut after = slots.to_vec();
        inputs
            .iter()
            .all(|(item, n)| slots::place(&mut after, item, n.saturating_mul(runs), stack(item)))
    };
    let (mut lo, mut hi) = (0, want);
    while lo < hi {
        let mid = lo + (hi - lo).div_ceil(2);
        if fits(mid) {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    lo
}

#[cfg(test)]
mod tests {
    use super::*;

    const PISTON: &str = "minecraft:piston";
    const IRON: &str = "minecraft:iron_ingot";
    const RAW: &str = "minecraft:raw_iron";

    fn book() -> Vec<PlanRecipe> {
        vec![
            PlanRecipe::simple(
                "crafter",
                &[
                    ("minecraft:oak_planks", 3),
                    ("minecraft:cobblestone", 4),
                    (IRON, 1),
                    ("minecraft:redstone", 1),
                ],
                (PISTON, 1),
            ),
            PlanRecipe::simple("furnace", &[(RAW, 1)], (IRON, 1)),
        ]
    }

    fn stock(iron: u64, raw: u64) -> BTreeMap<ItemKey, u64> {
        [
            ("minecraft:oak_planks", 64),
            ("minecraft:cobblestone", 64),
            (IRON, iron),
            (RAW, raw),
            ("minecraft:redstone", 64),
        ]
        .iter()
        .filter(|(_, n)| *n > 0)
        .map(|(i, n)| (i.to_string(), *n))
        .collect()
    }

    fn machine(x: i32, kind: &str) -> FreeMachine {
        FreeMachine {
            pos: [x, 0, 0],
            kind: kind.into(),
            powered: true,
        }
    }

    fn unpowered(x: i32, kind: &str) -> FreeMachine {
        FreeMachine {
            powered: false,
            ..machine(x, kind)
        }
    }

    #[test]
    fn a_machine_with_energy_goes_first() {
        // The alloy smelter's order: a free machine with energy or fuel takes the step, and the
        // one without stays free.
        let mut q = Queue::default();
        q.add(PISTON, 4, false);
        let free = [unpowered(1, "crafter"), machine(2, "crafter")];
        let jobs = round(&mut q, &stock(8, 0), &free);
        assert_eq!(jobs.len(), 1);
        assert_eq!((jobs[0].machine, jobs[0].runs), ([2, 0, 0], 4));
    }

    #[test]
    fn an_order_on_machines_without_energy_waits_for_energy() {
        // Only a machine without energy is free: the step goes to it, and the order says it
        // waits for energy instead of «в работе».
        let mut q = Queue::default();
        let id = q.add(PISTON, 2, false).unwrap();
        let jobs = round(&mut q, &stock(8, 0), &[unpowered(1, "crafter")]);
        assert_eq!(jobs.len(), 1);
        q.wait_for_energy(&jobs, &|_| false);
        assert_eq!(q.orders[0].wait, Wait::Energy("crafter".into()));
        // Energy came: the next round finds it running, and so it says.
        let running = |o: u16| o == id;
        q.round(
            &stock(8, 0),
            &book(),
            &["crafter"],
            &[],
            &running,
            &mut |_, _, w| w,
        );
        q.wait_for_energy(&jobs, &|_| true);
        assert_eq!(q.orders[0].wait, Wait::Running);
        // The machine lost its energy again: the order says so again.
        q.wait_for_energy(&jobs, &|p| p == [9, 9, 9]);
        assert_eq!(q.orders[0].wait, Wait::Energy("crafter".into()));
    }

    #[test]
    fn an_orders_own_products_are_no_other_orders() {
        // Rule O11: the iron the second order's furnace step made is its own in the storage:
        // the first order, which also lacks iron, does not take it.
        let mut q = Queue::default();
        let a = q.add(PISTON, 1, false).unwrap();
        let b = q.add(PISTON, 1, false).unwrap();
        q.keep_for(b, IRON, 1);
        // An order's own final item is never «its own» stock.
        q.keep_for(b, PISTON, 5);
        assert_eq!(q.orders[1].made, BTreeMap::from([(IRON.to_string(), 1)]));
        let jobs = round(&mut q, &stock(1, 0), &[machine(1, "crafter")]);
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].order, b);
        assert_eq!(q.orders[0].id, a);
        assert_eq!(q.orders[0].wait, Wait::Items(vec![(IRON.into(), 1)]));
        // Started: the iron left the storage, so it is no longer the order's there.
        q.started(&jobs[0]);
        assert!(q.orders[1].made.is_empty());
    }

    #[test]
    fn own_products_count_only_as_far_as_the_storage_holds_them() {
        let mut q = Queue::default();
        let b = q.add(PISTON, 1, false).unwrap();
        q.keep_for(b, IRON, 3);
        // A player took the iron out: the order plans on what is there, and lacks the rest.
        assert!(round(&mut q, &stock(0, 0), &[machine(1, "crafter")]).is_empty());
        assert!(q.orders[0].made.is_empty());
        assert_eq!(q.orders[0].wait, Wait::Items(vec![(IRON.into(), 1)]));
    }

    #[test]
    fn a_restored_queue_goes_on_numbering_its_orders() {
        let mut q = Queue::default();
        q.add(PISTON, 1, false);
        let b = q.add(PISTON, 1, false).unwrap();
        let mut back = Queue::restored(q.orders.clone(), q.next_id());
        assert_eq!(back, q);
        assert_eq!(back.add(PISTON, 1, false), Some(b + 1));
        // Doubled ids and an id the record lost are dropped or counted past.
        let doubled = vec![q.orders[1].clone(), q.orders[1].clone()];
        let back = Queue::restored(doubled, 0);
        assert_eq!(back.orders.len(), 1);
        assert_eq!(back.next_id(), b);
    }

    fn idle(_: u16) -> bool {
        false
    }

    fn round(q: &mut Queue, stock: &BTreeMap<ItemKey, u64>, free: &[FreeMachine]) -> Vec<Job> {
        q.round(
            stock,
            &book(),
            &["crafter", "furnace"],
            free,
            &idle,
            &mut |_, _, want| want,
        )
    }

    #[test]
    fn an_order_goes_to_a_free_crafter_and_is_done_when_its_runs_are() {
        // Rules O3, O4.
        let mut q = Queue::default();
        let id = q.add(PISTON, 4, false).unwrap();
        let jobs = round(&mut q, &stock(8, 0), &[machine(1, "crafter")]);
        assert_eq!(jobs.len(), 1);
        assert_eq!((jobs[0].order, jobs[0].runs), (id, 4));
        assert_eq!(q.orders[0].wait, Wait::Running);
        q.finished(&jobs[0], 3);
        assert_eq!(q.orders[0].done, 3);
        assert!(q.remove_done().is_empty());
        q.finished(&jobs[0], 1);
        assert_eq!(q.remove_done().len(), 1);
        assert!(q.orders.is_empty());
    }

    #[test]
    fn an_order_is_shared_between_free_crafters() {
        // Rule O5.
        let mut q = Queue::default();
        q.add(PISTON, 5, false);
        let free = [machine(1, "crafter"), machine(2, "crafter")];
        let jobs = round(&mut q, &stock(8, 0), &free);
        let runs: Vec<u32> = jobs.iter().map(|j| j.runs).collect();
        assert_eq!(runs, vec![3, 2]);
    }

    #[test]
    fn a_short_order_waits_and_goes_on_when_the_items_come() {
        // Rule O6: example «не хватает железа».
        let mut q = Queue::default();
        q.add(PISTON, 4, false);
        let free = [machine(1, "crafter")];
        assert!(round(&mut q, &stock(2, 0), &free).is_empty());
        assert_eq!(q.orders[0].wait, Wait::Items(vec![(IRON.into(), 2)]));
        let jobs = round(&mut q, &stock(4, 0), &free);
        assert_eq!(jobs.len(), 1);
    }

    #[test]
    fn a_missing_machine_is_named_and_a_busy_one_waited_for() {
        // Rule O13: iron from raw iron needs a furnace.
        let mut q = Queue::default();
        q.add(IRON, 2, false);
        let jobs = q.round(
            &stock(0, 4),
            &book(),
            &["crafter"],
            &[machine(1, "crafter")],
            &idle,
            &mut |_, _, w| w,
        );
        assert!(jobs.is_empty());
        assert_eq!(q.orders[0].wait, Wait::Machines(vec!["furnace".into()]));
        // A furnace on the line, but none free.
        assert!(round(&mut q, &stock(0, 4), &[machine(1, "crafter")]).is_empty());
        assert_eq!(q.orders[0].wait, Wait::Busy("furnace".into()));
    }

    #[test]
    fn steps_run_one_after_another_through_the_storage() {
        // Rule O10: raw iron → furnace → iron → crafter → piston.
        let mut q = Queue::default();
        q.add(PISTON, 2, false);
        let free = [machine(1, "crafter"), machine(2, "furnace")];
        let jobs = round(&mut q, &stock(0, 2), &free);
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].step.machine, "furnace");
        assert_eq!(jobs[0].runs, 2);
        // The crafter's step waits for the ingots, which stay set aside for nobody else.
        assert_eq!(q.orders[0].reserved.get(IRON), None);
        q.finished(&jobs[0], 2);
        assert_eq!(q.orders[0].done, 0);
        let jobs = round(&mut q, &stock(2, 0), &free);
        assert_eq!(jobs[0].step.machine, "crafter");
    }

    #[test]
    fn an_earlier_order_has_the_items_first() {
        let mut q = Queue::default();
        let first = q.add(PISTON, 2, false).unwrap();
        q.add(PISTON, 2, false);
        let free = [machine(1, "crafter"), machine(2, "crafter")];
        let jobs = round(&mut q, &stock(3, 0), &free);
        // The first order's two runs take both crafters.
        assert_eq!(jobs.len(), 2);
        assert!(jobs.iter().all(|j| j.order == first));
        assert_eq!(q.orders[1].wait, Wait::Items(vec![(IRON.into(), 1)]));
    }

    #[test]
    fn a_running_order_keeps_what_it_set_aside() {
        let mut q = Queue::default();
        let first = q.add(PISTON, 2, false).unwrap();
        q.orders[0].reserved = BTreeMap::from([(IRON.to_string(), 2)]);
        q.add(PISTON, 2, false);
        let running = |id: u16| id == first;
        let jobs = q.round(
            &stock(3, 0),
            &book(),
            &["crafter", "furnace"],
            &[machine(1, "crafter")],
            &running,
            &mut |_, _, w| w,
        );
        assert!(jobs.is_empty());
        assert_eq!(q.orders[0].wait, Wait::Running);
        assert_eq!(q.orders[1].wait, Wait::Items(vec![(IRON.into(), 1)]));
    }

    #[test]
    fn a_machine_takes_only_the_runs_that_fit() {
        let mut q = Queue::default();
        q.add(PISTON, 8, false);
        let jobs = q.round(
            &stock(8, 0),
            &book(),
            &["crafter"],
            &[machine(1, "crafter")],
            &idle,
            &mut |_, _, w| w.min(5),
        );
        assert_eq!(jobs[0].runs, 5);
    }

    #[test]
    fn cancel_takes_the_order_out() {
        // Rule O7.
        let mut q = Queue::default();
        let a = q.add(PISTON, 1, false).unwrap();
        let b = q.add(IRON, 1, false).unwrap();
        assert_eq!(q.cancel(a).map(|o| o.item), Some(PISTON.into()));
        assert_eq!(q.cancel(a), None);
        assert_eq!(q.orders.len(), 1);
        assert_eq!(q.orders[0].id, b);
    }

    #[test]
    fn the_queue_has_room_for_so_many_orders() {
        let mut q = Queue::default();
        for _ in 0..MAX_ORDERS {
            assert!(q.add(PISTON, 1, false).is_some());
        }
        assert_eq!(q.add(PISTON, 1, false), None);
        assert_eq!(Queue::default().add(PISTON, 0, false), None);
        let mut big = Queue::default();
        big.add(PISTON, u32::MAX, false);
        assert_eq!(big.orders[0].count, MAX_COUNT);
    }

    #[test]
    fn keep_orders_the_difference_once() {
        // Rule O14: keep 64 books, 61 in stock.
        let mut q = Queue::default();
        let keeps = BTreeMap::from([("minecraft:book".to_string(), 64)]);
        let s = BTreeMap::from([("minecraft:book".to_string(), 61)]);
        q.keep(&keeps, &s, &idle);
        q.keep(&keeps, &s, &idle);
        assert_eq!(q.orders.len(), 1);
        assert_eq!((q.orders[0].count, q.orders[0].keep), (3, true));
        // Enough in stock: nothing more; keep cleared: the open order leaves.
        let full = BTreeMap::from([("minecraft:book".to_string(), 64)]);
        q.orders.clear();
        q.keep(&keeps, &full, &idle);
        assert!(q.orders.is_empty());
        q.keep(&keeps, &s, &idle);
        q.keep(&BTreeMap::new(), &s, &idle);
        assert!(q.orders.is_empty());
    }

    #[test]
    fn runs_that_fit_count_the_room_of_the_slots() {
        let piston = book()[0].clone();
        let inputs: Vec<(ItemKey, u32)> = piston
            .inputs
            .iter()
            .map(|(a, n)| (a[0].clone(), *n))
            .collect();
        let stack = |_: &str| 64;
        let empty: Vec<Slot> = vec![None; 9];
        // 3 planks, 4 cobblestone, 1 iron, 1 redstone a run in 9 slots of 64: cobblestone fills
        // first (4 a run), and the others need a slot each.
        let runs = runs_that_fit(&empty, &inputs, 1000, &stack);
        assert!(runs > 0);
        let mut after = empty.clone();
        for (item, n) in &inputs {
            assert!(slots::place(&mut after, item, n * runs, 64));
        }
        let mut more = empty.clone();
        let fit_more = inputs
            .iter()
            .all(|(item, n)| slots::place(&mut more, item, n * (runs + 1), 64));
        assert!(!fit_more);
        assert_eq!(runs_that_fit(&empty, &inputs, 4, &stack), 4);
        let full: Vec<Slot> = vec![Some(("minecraft:dirt".into(), 64)); 9];
        assert_eq!(runs_that_fit(&full, &inputs, 4, &stack), 0);
    }
}
