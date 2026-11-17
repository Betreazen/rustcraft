//! RustCraft's server settings file (spec W8, guide `chassis`: «Число работников на игрока
//! ограничено настройкой сервера»): `config/rustcraft-server.toml`, or the world's
//! `serverconfig/rustcraft-server.toml`, which Horizon writes from `DEFAULT_TEXT` when missing
//! and hands over as text (API 397). Kept to what the server's owner sets; the balance numbers
//! stay in `Tuning`.

/// The file's name in `config/`, as Horizon names a `Server` config of mod `rustcraft`.
pub const FILE: &str = "rustcraft-server.toml";

/// What a missing file is written with.
pub const DEFAULT_TEXT: &str = "\
# RustCraft: server settings.
# How many workers (carts with a tool on the monorail) one player may have on the server.
# 0 means no limit.
workers_per_player = 16
";

/// The settings as read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ServerConfig {
    /// W8: workers one player may have on the server; 0 for no limit.
    pub workers_per_player: u32,
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            workers_per_player: 16,
        }
    }
}

impl ServerConfig {
    /// Whether an owner with `owned` workers may place one more.
    pub fn may_add(&self, owned: usize) -> bool {
        self.workers_per_player == 0 || owned < self.workers_per_player as usize
    }
}

/// Reads the file's text: `key = value` lines, `#` comments; a key that does not read keeps its
/// default, unknown keys are skipped. The keys it did not read are named in the second value.
pub fn parse(text: &str) -> (ServerConfig, Vec<String>) {
    let mut out = ServerConfig::default();
    let mut bad = Vec::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        if key == "workers_per_player" {
            match value.parse::<u32>() {
                Ok(n) => out.workers_per_player = n,
                Err(_) => bad.push(key.to_owned()),
            }
        }
    }
    (out, bad)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_limit() {
        assert_eq!(parse(DEFAULT_TEXT).0, ServerConfig::default());
        let (c, bad) = parse("# x\nworkers_per_player = 3 # three\nother = 1\n");
        assert_eq!((c.workers_per_player, bad.len()), (3, 0));
        assert!(c.may_add(2) && !c.may_add(3));
        assert!(parse("workers_per_player=0").0.may_add(10_000), "0 is no limit");
    }

    #[test]
    fn a_bad_file_keeps_the_default() {
        let (c, bad) = parse("workers_per_player = many\n[[[");
        assert_eq!(c, ServerConfig::default());
        assert_eq!(bad, vec!["workers_per_player".to_owned()]);
    }
}
