//! The event journal: `rustcraft-debug.log` in the world folder, one line an event,
//! `<UTC time> <game tick> <dimension> <x y z> <kind> <details>`, with `-` for what an event does
//! not know. Lines wait in a buffer and go to the file once a second or every
//! [`FLUSH_LINES`] lines; past [`ROTATE_AT`] bytes the file becomes `.1` (one backup kept).

use crate::geom::Pos;

/// The journal's file name, in the world folder.
pub const FILE: &str = "rustcraft-debug.log";
/// Past this size the file is moved to `<file>.1` before more is written.
pub const ROTATE_AT: u64 = 2 * 1024 * 1024;
/// The buffer goes to the file when it holds this many lines, or when [`FLUSH_MS`] passed.
pub const FLUSH_LINES: usize = 64;
pub const FLUSH_MS: u128 = 1000;
/// Lines kept while the file is not known yet (a client, or before the world's folder is): the
/// oldest go first.
pub const MAX_PENDING: usize = 2000;
/// A detail longer than this is cut: a journal line stays one readable line.
pub const MAX_DETAIL: usize = 300;

/// One journal line, without its newline.
pub fn line(
    utc: &str,
    tick: u64,
    dimension: Option<&str>,
    pos: Option<Pos>,
    kind: &str,
    details: &str,
) -> String {
    let pos = pos.map_or_else(|| "- - -".to_owned(), super::pos_text);
    let mut detail: String = details
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    if detail.chars().count() > MAX_DETAIL {
        detail = detail.chars().take(MAX_DETAIL).collect::<String>() + "...";
    }
    format!(
        "{utc} {tick} {} {pos} {} {}",
        dimension.unwrap_or("-"),
        word(kind),
        detail.trim()
    )
}

/// A kind as one word.
fn word(kind: &str) -> String {
    let w: String = kind
        .chars()
        .map(|c| if c.is_whitespace() { '_' } else { c })
        .collect();
    if w.is_empty() { "-".to_owned() } else { w }
}

/// `2026-11-18T19:30:05Z` for seconds since the Unix epoch.
pub fn utc(unix_secs: u64) -> String {
    let days = i64::try_from(unix_secs / 86_400).unwrap_or(0);
    let secs = unix_secs % 86_400;
    let (y, m, d) = civil(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        secs / 3600,
        secs / 60 % 60,
        secs % 60
    )
}

/// Year, month and day of `days` since 1970-01-01 (H. Hinnant's `civil_from_days`).
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

/// Whether the file of `size` bytes is moved aside before `adding` more bytes go in.
pub fn should_rotate(size: u64, adding: u64) -> bool {
    size > 0 && size + adding > ROTATE_AT
}

/// The lines waiting for the file, and when they last went.
#[derive(Clone, Debug, Default)]
pub struct Buffer {
    pub pending: Vec<String>,
    pub flushed_at: u128,
}

impl Buffer {
    /// A line added; the oldest dropped past [`MAX_PENDING`].
    pub fn push(&mut self, line: String) {
        self.pending.push(line);
        if self.pending.len() > MAX_PENDING {
            let over = self.pending.len() - MAX_PENDING;
            self.pending.drain(..over);
        }
    }

    /// Whether the buffer goes to the file now (`now_ms` on the same clock as `flushed_at`).
    pub fn due(&self, now_ms: u128) -> bool {
        !self.pending.is_empty()
            && (self.pending.len() >= FLUSH_LINES || now_ms.saturating_sub(self.flushed_at) >= FLUSH_MS)
    }

    /// The text that goes to the file, the buffer emptied.
    pub fn take(&mut self, now_ms: u128) -> String {
        self.flushed_at = now_ms;
        let mut text = self.pending.join("\n");
        if !text.is_empty() {
            text.push('\n');
        }
        self.pending.clear();
        text
    }
}

/// The last `n` of `lines` that contain `filter` (all when none), oldest first.
pub fn tail<'a>(lines: impl DoubleEndedIterator<Item = &'a str>, n: usize, filter: Option<&str>) -> Vec<&'a str> {
    let mut out: Vec<&str> = lines
        .rev()
        .filter(|l| !l.trim().is_empty())
        .filter(|l| filter.is_none_or(|f| l.contains(f)))
        .take(n)
        .collect();
    out.reverse();
    out
}
