use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

use crate::config::Config;

#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub path: PathBuf,
}

pub fn save_entry(contents: &str) -> HistoryEntry {
    let dir = history_dir();
    let _ = fs::create_dir_all(&dir);

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = dir.join(format!("{stamp}.txt"));
    let _ = fs::write(&path, contents);

    HistoryEntry { path }
}

pub fn latest_entry() -> Result<HistoryEntry> {
    let dir = history_dir();
    let mut entries: Vec<PathBuf> = fs::read_dir(&dir)
        .with_context(|| format!("read history dir: {}", dir.display()))?
        .filter_map(|entry| entry.ok().map(|item| item.path()))
        .filter(|path| path.is_file())
        .collect();

    entries.sort();
    let path = entries
        .pop()
        .with_context(|| format!("no history entries in {}", dir.display()))?;

    Ok(HistoryEntry { path })
}

fn history_dir() -> PathBuf {
    Config::config_dir().join("history")
}
