//! Round 32 (spec 33): the «Сеть» page of the storage terminal. What the storage sees on its
//! magistral: the housings its cells and terminals touch and their weakest tier, what that
//! branch carries, the energy of its circuits, the experience its condensers keep (0.21, spec
//! 44), the storage's own cells, the machines and the
//! vanilla containers on its item joints, and every tank on its fluid lane. A tank in the list is
//! connected right (the owner: «если терминал склада их видит, значит всё подключено правильно»).
//!
//! The page costs the server nothing while nobody looks at it: the client asks for it on
//! ASK_CHANNEL each ASK_EVERY_MS while the page is shown, and the server answers each ask with a
//! fresh page on CHANNEL (the terminal) or TABLET_CHANNEL (the storage tablet), at most once in
//! ANSWER_EVERY_MS for a player.
//!
//! The window: a «Сеть» button at the right of the title line switches the page on and off; the
//! page takes the place of the grid, the row under it and the status, ROWS rows of ROW_H a
//! page, and the page buttons of the top row turn its pages. Each row is a header or a key on the
//! left and a value on the right (`docs/design/04-window-style.md`, «Строки „ключ — значение“»).

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use crate::geom::{Face, Pos, step};
use crate::machines::panel::{Rect, inside};
use crate::machines::processor::Shown;
use crate::net::topology::{ChannelKind, Topology};

/// Server to client: a terminal's network page.
pub const CHANNEL: &str = "rustcraft:terminal_net";
/// Server to client: the storage tablet's network page.
pub const TABLET_CHANNEL: &str = "rustcraft:tablet_net";
/// Client to server: «the network page is shown», again each ASK_EVERY_MS while it is.
pub const ASK_CHANNEL: &str = "rustcraft:terminal_net_ask";
/// How often a client with the page shown asks for it again, ms.
pub const ASK_EVERY_MS: u64 = 1000;
/// The page counts as shown this long after the window last drew it, ms.
pub const SHOWN_FOR_MS: u64 = 400;
/// The server answers one player at most this often, ms.
pub const ANSWER_EVERY_MS: u64 = 500;
/// How far from a terminal a player asking for its page may stand, blocks.
pub const REACH: f64 = 12.0;

// ---- The window ----

/// The «Сеть» button, at the right of the title line (the top rows are full).
pub const NET_MODE: Rect = [166.0, 4.0, 40.0, 10.0];
/// Rows a page shows.
pub const ROWS: usize = 10;
pub const ROW_H: f32 = 10.0;
/// The first row's top: where the grid starts.
pub const TOP: f32 = 45.0;
pub const LEFT: f32 = 8.0;
pub const RIGHT: f32 = 206.0;
/// Room between a row's key and its value, px.
pub const GAP: f32 = 6.0;

/// Row `i` of the page, its whole width.
pub fn row(i: usize) -> Rect {
    [LEFT, TOP + ROW_H * i as f32, RIGHT - LEFT, ROW_H - 1.0]
}

/// The row at `at`, in window pixels.
pub fn row_at(at: [f32; 2]) -> Option<usize> {
    (0..ROWS).find(|&i| inside(row(i), at))
}

/// What the client asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ask {
    /// The page of the terminal at this place, whose window the player has open.
    Terminal(Pos),
    /// The page of the storage the player's tablet shows.
    Tablet,
}

pub fn encode_ask(ask: Ask) -> Vec<u8> {
    match ask {
        Ask::Terminal(p) => {
            let mut out = vec![0];
            for c in p {
                out.extend(c.to_le_bytes());
            }
            out
        }
        Ask::Tablet => vec![1],
    }
}

/// What `encode_ask` wrote; none for anything else.
pub fn decode_ask(bytes: &[u8]) -> Option<Ask> {
    match bytes {
        [1] => Some(Ask::Tablet),
        [0, rest @ ..] if rest.len() == 12 => {
            let mut r = Reader(rest);
            Some(Ask::Terminal(r.pos()?))
        }
        _ => None,
    }
}

// ---- What the page shows ----

/// The magistral the storage sees: the item nets its blocks touch (since round 31 every housing
/// carries every channel, so the fluid, energy and experience nets are the same housings).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Magistral {
    /// Working nets the storage touches: 0 when it touches no working housing.
    pub branches: u8,
    pub housings: u32,
    /// Housings on the same lines that no node serves (out of its range).
    pub out_of_reach: u32,
    /// The weakest housing's tier, 0 without a net.
    pub min_tier: u8,
    /// Items, buckets and E a second the branch carries (its slowest net, with the nodes'
    /// line-boost modules).
    pub items: f32,
    pub fluid: f32,
    pub energy_cap: f32,
    /// 0.21 (XC8): experience points a second the branch carries, and the condensers on its
    /// experience nets with the points they hold.
    pub xp: f32,
    pub condensers: u16,
    pub xp_held: u32,
}

/// The energy of the circuits on the branch, as the node's window counts it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Energy {
    pub made: f32,
    pub used: f32,
    pub stored: f32,
    pub capacity: f32,
}

/// The storage's own cells.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StoreFacts {
    pub cells: u32,
    pub used: u64,
    pub total: u64,
    /// Fluid in the cells, all fluids, mB; and what the cells hold of each fluid at most.
    pub fluid_held: u64,
    pub fluid_capacity: u64,
    /// S8: the cells are a solid box.
    pub works: bool,
}

/// What a machine does now, as the page sums it up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Activity {
    Working,
    Idle,
    NoEnergy,
    /// No status of its own (a dock, a block the machines' tick does not run).
    Unknown,
}

impl Activity {
    pub fn of(shown: Option<Shown>) -> Activity {
        match shown {
            Some(Shown::Working | Shown::Slowed { .. } | Shown::OnFuel) => Activity::Working,
            Some(Shown::Idle | Shown::OutputFull | Shown::NoDemand) => Activity::Idle,
            Some(Shown::NoEnergy | Shown::NoFuel) => Activity::NoEnergy,
            None => Activity::Unknown,
        }
    }
}

/// The machines of one kind on the storage's item joints.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MachineGroup {
    /// Its block id: the window names it.
    pub block: String,
    pub count: u16,
    pub working: u16,
    pub idle: u16,
    pub no_energy: u16,
}

/// How the fluid lane works a tank's joints: whether the lane may take from it (the storage
/// counts its fluid, FL5) and put into it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum JointUse {
    /// The default of a tank (FL3): it gives and takes.
    #[default]
    Both,
    /// Set «из блока».
    Gives,
    /// Set «в блок»: it only takes; the storage does not count its fluid.
    Takes,
    /// Switched off.
    Off,
}

impl JointUse {
    /// From what the joints on a tank allow, all of them together.
    pub fn of(takes: bool, gives: bool) -> JointUse {
        match (takes, gives) {
            (true, true) => JointUse::Both,
            (false, true) => JointUse::Gives,
            (true, false) => JointUse::Takes,
            (false, false) => JointUse::Off,
        }
    }

    /// FL5: the storage counts the tank's fluid as its stock.
    pub fn counted(self) -> bool {
        matches!(self, JointUse::Both | JointUse::Gives)
    }

    fn code(self) -> u8 {
        match self {
            JointUse::Both => 0,
            JointUse::Gives => 1,
            JointUse::Takes => 2,
            JointUse::Off => 3,
        }
    }

    fn of_code(code: u8) -> JointUse {
        match code {
            1 => JointUse::Gives,
            2 => JointUse::Takes,
            3 => JointUse::Off,
            _ => JointUse::Both,
        }
    }
}

/// A tank on the fluid lane: touching tank blocks are one tank (its lowest block names it).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TankLine {
    pub pos: Pos,
    pub fluid: Option<String>,
    /// mB.
    pub amount: u64,
    pub capacity: u64,
    pub blocks: u16,
    pub joint: JointUse,
    /// 0.21 (spec 42, FT5): the fluid the tank is kept for, the one it holds while it holds
    /// some; none while it is free.
    pub pin: Option<String>,
}

/// The fluid sources and consumers of one kind on the lane (pumps, mixers, the kitchen…).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EndGroup {
    pub block: String,
    pub count: u16,
}

/// The whole page.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NetPage {
    /// The terminal it is for (the tablet: a block of its storage).
    pub terminal: Pos,
    pub magistral: Magistral,
    /// None while the level has not counted its energy yet, or with no working net.
    pub energy: Option<Energy>,
    /// None when the terminal touches no cell.
    pub storage: Option<StoreFacts>,
    pub machines: Vec<MachineGroup>,
    pub containers: u16,
    /// Other storages on the item joints.
    pub storages: u16,
    pub tanks: Vec<TankLine>,
    /// Tanks on the lane in all, of which `tanks` lists MAX_TANKS at most.
    pub tank_count: u16,
    pub ends: Vec<EndGroup>,
}

// ---- What the server works out ----

/// The housings a storage's blocks touch, as the page counts them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Branch {
    /// The item nets (places in `Topology::nets`) the blocks touch.
    pub nets: Vec<usize>,
    /// Their housings.
    pub housings: BTreeSet<Pos>,
    /// Housings on the lines the blocks touch that no node serves.
    pub out_of_reach: u32,
    /// The weakest housing of those nets, 0 with none.
    pub min_tier: u8,
}

/// The branch of the magistral that the blocks `blocks` (a storage's cells and terminals) touch.
pub fn branch(topology: &Topology, blocks: &[Pos]) -> Branch {
    let touched: BTreeSet<Pos> = blocks
        .iter()
        .flat_map(|b| Face::ALL.map(|f| step(*b, f)))
        .collect();
    let nets: Vec<usize> = topology
        .nets
        .iter()
        .enumerate()
        .filter(|(_, n)| n.kind == ChannelKind::Item && !n.housings.is_disjoint(&touched))
        .map(|(i, _)| i)
        .collect();
    let housings: BTreeSet<Pos> = nets
        .iter()
        .flat_map(|&i| topology.nets[i].housings.iter().copied())
        .collect();
    let min_tier = nets.iter().map(|&i| topology.nets[i].min_tier).min().unwrap_or(0);
    let out_of_reach = topology
        .lines
        .iter()
        .filter(|l| !l.is_disjoint(&touched) || !l.is_disjoint(&housings))
        .flat_map(|l| l.iter())
        .collect::<BTreeSet<&Pos>>()
        .into_iter()
        .filter(|h| !topology.reach.get(*h).is_some_and(|r| r.works()))
        .count() as u32;
    Branch {
        nets,
        housings,
        out_of_reach,
        min_tier,
    }
}

/// The machines found on the joints, by kind: most numerous first, then by block.
pub fn group_machines(found: impl IntoIterator<Item = (String, Activity)>) -> Vec<MachineGroup> {
    let mut by: BTreeMap<String, MachineGroup> = BTreeMap::new();
    for (block, activity) in found {
        let g = by.entry(block.clone()).or_insert_with(|| MachineGroup {
            block,
            ..MachineGroup::default()
        });
        g.count = g.count.saturating_add(1);
        let n = match activity {
            Activity::Working => &mut g.working,
            Activity::Idle => &mut g.idle,
            Activity::NoEnergy => &mut g.no_energy,
            Activity::Unknown => continue,
        };
        *n = n.saturating_add(1);
    }
    let mut out: Vec<MachineGroup> = by.into_values().collect();
    out.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.block.cmp(&b.block)));
    out
}

/// The fluid ends found on the lane, by kind, as `group_machines` orders them.
pub fn group_ends(found: impl IntoIterator<Item = String>) -> Vec<EndGroup> {
    let mut by: BTreeMap<String, u16> = BTreeMap::new();
    for block in found {
        let n = by.entry(block).or_insert(0);
        *n = n.saturating_add(1);
    }
    let mut out: Vec<EndGroup> = by
        .into_iter()
        .map(|(block, count)| EndGroup { block, count })
        .collect();
    out.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.block.cmp(&b.block)));
    out
}

// ---- The rows ----

/// The page's parts, each under its header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Magistral,
    Storage,
    Connected,
    Tanks,
    FluidEnds,
}

/// One row of the page; the window writes it from the page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    Header(Section),
    /// The storage touches no working housing.
    NoBranch,
    Housings,
    OutOfReach,
    Tier,
    Items,
    Fluid,
    /// 0.21 (XC8): the experience channel and its condensers.
    Xp,
    Energy,
    Stored,
    /// The terminal touches no cell.
    NoStorage,
    Cells,
    Slots,
    FluidCells,
    /// A group of `NetPage::machines`, by its place.
    Machine(usize),
    Containers,
    Storages,
    NothingConnected,
    /// A tank of `NetPage::tanks`, by its place.
    Tank(usize),
    /// «и ещё N»: tanks past the list.
    MoreTanks,
    /// «Баков на магистрали нет».
    NoTanks,
    /// A group of `NetPage::ends`, by its place.
    End(usize),
}

/// Every row of the page, in its order.
pub fn rows(page: &NetPage) -> Vec<Row> {
    let mut out = vec![Row::Header(Section::Magistral)];
    let m = &page.magistral;
    if m.branches == 0 {
        out.push(Row::NoBranch);
    } else {
        out.extend([Row::Housings, Row::Tier, Row::Items, Row::Fluid, Row::Xp]);
    }
    if m.out_of_reach > 0 {
        out.push(Row::OutOfReach);
    }
    if let Some(e) = page.energy.filter(|_| m.branches > 0) {
        out.push(Row::Energy);
        if e.capacity > 0.0 {
            out.push(Row::Stored);
        }
    }
    out.push(Row::Header(Section::Storage));
    match page.storage {
        None => out.push(Row::NoStorage),
        Some(_) => out.extend([Row::Cells, Row::Slots, Row::FluidCells]),
    }
    out.push(Row::Header(Section::Connected));
    out.extend((0..page.machines.len()).map(Row::Machine));
    if page.containers > 0 {
        out.push(Row::Containers);
    }
    if page.storages > 0 {
        out.push(Row::Storages);
    }
    if page.machines.is_empty() && page.containers == 0 && page.storages == 0 {
        out.push(Row::NothingConnected);
    }
    out.push(Row::Header(Section::Tanks));
    out.extend((0..page.tanks.len()).map(Row::Tank));
    if usize::from(page.tank_count) > page.tanks.len() {
        out.push(Row::MoreTanks);
    }
    if page.tank_count == 0 && page.tanks.is_empty() {
        out.push(Row::NoTanks);
    }
    if !page.ends.is_empty() {
        out.push(Row::Header(Section::FluidEnds));
        out.extend((0..page.ends.len()).map(Row::End));
    }
    out
}

/// The rows each page shows: ROWS at most. A part (a header and its rows) that fits on a page
/// starts a new page rather than break over two; a longer one fills its pages, and a header is
/// never last on its page. At least one page.
pub fn pages(rows: &[Row]) -> Vec<Range<usize>> {
    let mut starts: Vec<usize> = rows
        .iter()
        .enumerate()
        .filter(|(_, r)| matches!(r, Row::Header(_)))
        .map(|(i, _)| i)
        .collect();
    if starts.first() != Some(&0) {
        starts.insert(0, 0);
    }
    let mut out = Vec::new();
    let mut page = 0;
    for (k, &start) in starts.iter().enumerate() {
        let end = starts.get(k + 1).copied().unwrap_or(rows.len());
        if start > page && end - page > ROWS && end - start <= ROWS {
            out.push(page..start);
            page = start;
        }
        while end - page > ROWS {
            let mut cut = page + ROWS;
            if matches!(rows[cut - 1], Row::Header(_)) && cut - 1 > page {
                cut -= 1;
            }
            out.push(page..cut);
            page = cut;
        }
    }
    if page < rows.len() || out.is_empty() {
        out.push(page..rows.len());
    }
    out
}

// ---- The message ----

/// 0.21 (spec 42): 2, with the empty tanks' pins; (spec 44) 3, with the experience channel.
const VERSION: u8 = 3;
/// The lists a message carries at most; the page says how many tanks there are in all.
pub const MAX_MACHINES: usize = 48;
pub const MAX_TANKS: usize = 96;
pub const MAX_ENDS: usize = 32;
/// An id longer than this is cut (ids are ASCII and far shorter).
const MAX_ID: usize = 96;

fn put_str(out: &mut Vec<u8>, s: &str) {
    let mut end = s.len().min(MAX_ID);
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    out.push(end as u8);
    out.extend(&s.as_bytes()[..end]);
}

fn put_f32(out: &mut Vec<u8>, v: f32) {
    out.extend(v.to_le_bytes());
}

/// The message for `page`; lists past their MAX_* are cut.
pub fn encode(page: &NetPage) -> Vec<u8> {
    let mut out = vec![VERSION];
    for c in page.terminal {
        out.extend(c.to_le_bytes());
    }
    let m = &page.magistral;
    out.push(m.branches);
    out.extend(m.housings.to_le_bytes());
    out.extend(m.out_of_reach.to_le_bytes());
    out.push(m.min_tier);
    for v in [m.items, m.fluid, m.energy_cap, m.xp] {
        put_f32(&mut out, v);
    }
    out.extend(m.condensers.to_le_bytes());
    out.extend(m.xp_held.to_le_bytes());
    match page.energy {
        Some(e) => {
            out.push(1);
            for v in [e.made, e.used, e.stored, e.capacity] {
                put_f32(&mut out, v);
            }
        }
        None => out.push(0),
    }
    match page.storage {
        Some(s) => {
            out.push(1 + u8::from(s.works));
            out.extend(s.cells.to_le_bytes());
            for v in [s.used, s.total, s.fluid_held, s.fluid_capacity] {
                out.extend(v.to_le_bytes());
            }
        }
        None => out.push(0),
    }
    let machines = &page.machines[..page.machines.len().min(MAX_MACHINES)];
    out.push(machines.len() as u8);
    for g in machines {
        put_str(&mut out, &g.block);
        for v in [g.count, g.working, g.idle, g.no_energy] {
            out.extend(v.to_le_bytes());
        }
    }
    out.extend(page.containers.to_le_bytes());
    out.extend(page.storages.to_le_bytes());
    let tanks = &page.tanks[..page.tanks.len().min(MAX_TANKS)];
    out.push(tanks.len() as u8);
    out.extend(page.tank_count.to_le_bytes());
    for t in tanks {
        for c in t.pos {
            out.extend(c.to_le_bytes());
        }
        put_str(&mut out, t.fluid.as_deref().unwrap_or(""));
        // FT5: a tank that holds a fluid is kept for it; an empty one says its pin.
        if t.fluid.is_none() {
            put_str(&mut out, t.pin.as_deref().unwrap_or(""));
        }
        out.extend(t.amount.to_le_bytes());
        out.extend(t.capacity.to_le_bytes());
        out.extend(t.blocks.to_le_bytes());
        out.push(t.joint.code());
    }
    let ends = &page.ends[..page.ends.len().min(MAX_ENDS)];
    out.push(ends.len() as u8);
    for e in ends {
        put_str(&mut out, &e.block);
        out.extend(e.count.to_le_bytes());
    }
    out
}

/// A reader over a message's bytes.
struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> Option<[u8; N]> {
        let (head, rest) = self.0.split_first_chunk::<N>()?;
        self.0 = rest;
        Some(*head)
    }
    fn u8(&mut self) -> Option<u8> {
        self.take::<1>().map(|b| b[0])
    }
    fn u16(&mut self) -> Option<u16> {
        self.take().map(u16::from_le_bytes)
    }
    fn u32(&mut self) -> Option<u32> {
        self.take().map(u32::from_le_bytes)
    }
    fn i32(&mut self) -> Option<i32> {
        self.take().map(i32::from_le_bytes)
    }
    fn u64(&mut self) -> Option<u64> {
        self.take().map(u64::from_le_bytes)
    }
    fn f32(&mut self) -> Option<f32> {
        self.take().map(f32::from_le_bytes)
    }
    fn pos(&mut self) -> Option<Pos> {
        Some([self.i32()?, self.i32()?, self.i32()?])
    }
    fn str(&mut self) -> Option<String> {
        let len = usize::from(self.u8()?);
        let (text, rest) = self.0.split_at_checked(len)?;
        self.0 = rest;
        Some(String::from_utf8_lossy(text).into_owned())
    }
}

/// What `encode` wrote; none for another version or a message cut short.
pub fn decode(bytes: &[u8]) -> Option<NetPage> {
    let mut r = Reader(bytes);
    if r.u8()? != VERSION {
        return None;
    }
    let terminal = r.pos()?;
    let magistral = Magistral {
        branches: r.u8()?,
        housings: r.u32()?,
        out_of_reach: r.u32()?,
        min_tier: r.u8()?,
        items: r.f32()?,
        fluid: r.f32()?,
        energy_cap: r.f32()?,
        xp: r.f32()?,
        condensers: r.u16()?,
        xp_held: r.u32()?,
    };
    let energy = match r.u8()? {
        0 => None,
        _ => Some(Energy {
            made: r.f32()?,
            used: r.f32()?,
            stored: r.f32()?,
            capacity: r.f32()?,
        }),
    };
    let storage = match r.u8()? {
        0 => None,
        flag => Some(StoreFacts {
            works: flag == 2,
            cells: r.u32()?,
            used: r.u64()?,
            total: r.u64()?,
            fluid_held: r.u64()?,
            fluid_capacity: r.u64()?,
        }),
    };
    let machines = (0..r.u8()?)
        .map(|_| {
            Some(MachineGroup {
                block: r.str()?,
                count: r.u16()?,
                working: r.u16()?,
                idle: r.u16()?,
                no_energy: r.u16()?,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let containers = r.u16()?;
    let storages = r.u16()?;
    let listed = r.u8()?;
    let tank_count = r.u16()?;
    let tanks = (0..listed)
        .map(|_| {
            let pos = r.pos()?;
            let fluid = Some(r.str()?).filter(|f| !f.is_empty());
            let pin = match &fluid {
                Some(f) => Some(f.clone()),
                None => Some(r.str()?).filter(|f| !f.is_empty()),
            };
            Some(TankLine {
                pos,
                fluid,
                pin,
                amount: r.u64()?,
                capacity: r.u64()?,
                blocks: r.u16()?,
                joint: JointUse::of_code(r.u8()?),
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let ends = (0..r.u8()?)
        .map(|_| {
            Some(EndGroup {
                block: r.str()?,
                count: r.u16()?,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(NetPage {
        terminal,
        magistral,
        energy,
        storage,
        machines,
        containers,
        storages,
        tanks,
        tank_count,
        ends,
    })
}

#[cfg(test)]
mod tests;
