// The prototype's files: one CSV per attempt and role, a summary CSV and an event log, all in
// `config/traversal/` of the server's or the client's folder. Game-free: the caller passes it.
use crate::game::trv_rules::{Kind, Method, V3};
use std::{
    fs::{self, File, OpenOptions},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub const FOLDER: &str = "traversal";
pub const SUMMARY_FILE: &str = "summary.csv";
pub const EVENTS_FILE: &str = "events.log";

pub const SERVER_HEADER: &str = "tick,ms,phase,server_x,server_y,server_z,target_x,target_y,target_z,off_path,lag,seat_x,seat_y,seat_z,vehicle,teleports,setbacks,above_ground,floating,note";
pub const RIDER_HEADER: &str = "tick,ms,client_x,client_y,client_z,vel_x,vel_y,vel_z,target_x,target_y,target_z,push_x,push_y,push_z,error,step,position_packets,riding,snap";
pub const OBSERVER_HEADER: &str = "tick,ms,rider_x,rider_y,rider_z,step,note";

/// Seconds since 1970, to keep files of different runs apart.
pub fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// `<unix>-<attempt>-<kind>-<method>-<role>-<pid>.csv`: sorts by time, and two game windows
/// sharing one folder never write the same file.
pub fn file_name(started: u64, attempt: u64, kind: Kind, method: Method, role: &str) -> String {
    format!(
        "{started}-{attempt:03}-{}-{}-{role}-{}.csv",
        kind.id(),
        method.id(),
        std::process::id()
    )
}

/// "x,y,z" with millimetres; empty cells for none.
pub fn cells(v: Option<V3>) -> String {
    match v {
        Some([x, y, z]) => format!("{x:.3},{y:.3},{z:.3}"),
        None => ",,".to_owned(),
    }
}

pub fn cell(v: Option<f64>) -> String {
    v.map_or(String::new(), |v| format!("{v:.3}"))
}

/// One CSV file, written as lines arrive.
pub struct Journal {
    path: PathBuf,
    out: BufWriter<File>,
    failed: Option<String>,
}

impl Journal {
    pub fn create(dir: &Path, name: &str, header: &str) -> Result<Journal, String> {
        fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let path = dir.join(name);
        let file = File::create(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut journal = Journal {
            path,
            out: BufWriter::new(file),
            failed: None,
        };
        journal.line(header);
        Ok(journal)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Writes a line; the first failure is kept for `finish` to report.
    pub fn line(&mut self, text: &str) {
        if self.failed.is_some() {
            return;
        }
        if let Err(e) = writeln!(self.out, "{text}") {
            self.failed = Some(format!("{}: {e}", self.path.display()));
        }
    }

    /// Flushes the file: an error names the file and the first failed write.
    pub fn finish(mut self) -> Result<PathBuf, String> {
        if let Some(failed) = self.failed.take() {
            return Err(failed);
        }
        self.out
            .flush()
            .map_err(|e| format!("{}: {e}", self.path.display()))?;
        Ok(self.path)
    }
}

/// Appends `line` to `name` in `dir`, writing `header` first when the file is new.
pub fn append(dir: &Path, name: &str, header: Option<&str>, line: &str) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let path = dir.join(name);
    let new = !path.exists();
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let mut text = String::new();
    if let (true, Some(header)) = (new, header) {
        text.push_str(header);
        text.push('\n');
    }
    text.push_str(line);
    text.push('\n');
    file.write_all(text.as_bytes())
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// A timestamped line in the event log.
pub fn event(dir: &Path, text: &str) -> Result<(), String> {
    append(
        dir,
        EVENTS_FILE,
        None,
        &format!("{} {text}", unix_seconds()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("traversal-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_journal_writes_its_header_and_lines() {
        let dir = scratch("journal");
        let mut journal = Journal::create(&dir, "a.csv", "tick,x").unwrap();
        journal.line("0,1.000");
        let path = journal.finish().unwrap();
        assert_eq!(fs::read_to_string(path).unwrap(), "tick,x\n0,1.000\n");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn the_summary_gets_its_header_once() {
        let dir = scratch("summary");
        append(&dir, SUMMARY_FILE, Some("h"), "1").unwrap();
        append(&dir, SUMMARY_FILE, Some("h"), "2").unwrap();
        event(&dir, "started").unwrap();
        assert_eq!(
            fs::read_to_string(dir.join(SUMMARY_FILE)).unwrap(),
            "h\n1\n2\n"
        );
        assert!(
            fs::read_to_string(dir.join(EVENTS_FILE))
                .unwrap()
                .ends_with(" started\n")
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn names_and_cells_read_as_expected() {
        let name = file_name(1_700_000_000, 4, Kind::Hook, Method::B2, "server");
        assert!(name.starts_with("1700000000-004-hook-b2-server-") && name.ends_with(".csv"));
        assert_eq!(cells(Some([1.0, -2.5, 0.0004])), "1.000,-2.500,0.000");
        assert_eq!(cells(None), ",,");
        assert_eq!(cell(None), "");
        assert_eq!(SERVER_HEADER.split(',').count(), 20);
    }

    #[test]
    fn a_folder_that_cannot_be_made_is_reported() {
        let dir = scratch("blocked");
        fs::create_dir_all(dir.parent().unwrap()).unwrap();
        fs::write(&dir, "a file where the folder should be").unwrap();
        let error = Journal::create(&dir, "a.csv", "h").err().unwrap();
        assert!(error.contains("traversal-test-blocked"), "{error}");
        let _ = fs::remove_file(dir);
    }
}
