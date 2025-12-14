//! Simple transcript history manager for CodeScribe
//!
//! Saves transcripts to ~/.CodeScribe/Transcripts/YYYY-MM-DD/HHMMSS.txt

use chrono::{DateTime, Local};
use directories::BaseDirs;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use tracing::{debug, error, warn};

/// A single history entry
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub path: PathBuf,
    #[allow(dead_code)] // Used for future menu display
    pub timestamp: DateTime<Local>,
    #[allow(dead_code)] // Used for future menu display
    pub preview: String,
}

impl HistoryEntry {
    /// Get a formatted label for display in menus
    #[allow(dead_code)] // Prepared for dynamic menu updates
    pub fn label(&self) -> String {
        let ts = self.timestamp.format("%H:%M:%S").to_string();
        if self.preview.is_empty() {
            ts
        } else {
            format!("{} – {}", ts, self.preview)
        }
    }
}

/// Get the history directory, creating it if needed
pub fn history_dir() -> PathBuf {
    let home = BaseDirs::new()
        .map(|b| b.home_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    let dir = home.join(".CodeScribe").join("Transcripts");

    if !dir.exists() {
        if let Err(e) = fs::create_dir_all(&dir) {
            error!("Failed to create history directory: {}", e);
        }
    }

    dir
}

/// Save a transcript to history and return the entry
pub fn save_entry(text: &str) -> HistoryEntry {
    let text = text.trim();
    let now = Local::now();

    // Create day directory
    let day_dir = history_dir().join(now.format("%Y-%m-%d").to_string());
    if let Err(e) = fs::create_dir_all(&day_dir) {
        error!("Failed to create history day directory: {}", e);
    }

    // Create file
    let filename = now.format("%H%M%S.txt").to_string();
    let path = day_dir.join(&filename);

    match fs::File::create(&path) {
        Ok(mut file) => {
            if let Err(e) = file.write_all(text.as_bytes()) {
                error!(
                    "Failed to write transcript history '{}': {}",
                    path.display(),
                    e
                );
            } else {
                debug!("Saved transcript to history: {}", path.display());
            }
        }
        Err(e) => {
            error!("Failed to create history file '{}': {}", path.display(), e);
        }
    }

    // Extract preview (first line, max 60 chars)
    let preview = text.lines().next().unwrap_or("").chars().take(60).collect();

    HistoryEntry {
        path,
        timestamp: now,
        preview,
    }
}

/// Get recent history entries, sorted by modification time (newest first)
pub fn recent_entries(limit: usize) -> Vec<HistoryEntry> {
    let dir = history_dir();
    let mut entries = Vec::new();
    let mut files: Vec<PathBuf> = Vec::new();

    // Collect all .txt files from day subdirectories
    if let Ok(day_dirs) = fs::read_dir(&dir) {
        for day_entry in day_dirs.flatten() {
            if day_entry.path().is_dir() {
                if let Ok(txt_files) = fs::read_dir(day_entry.path()) {
                    for txt_entry in txt_files.flatten() {
                        let path = txt_entry.path();
                        if path.extension().is_some_and(|ext| ext == "txt") {
                            files.push(path);
                        }
                    }
                }
            }
        }
    }

    // Sort by modification time (newest first)
    files.sort_by(|a, b| {
        let a_time = fs::metadata(a).and_then(|m| m.modified()).ok();
        let b_time = fs::metadata(b).and_then(|m| m.modified()).ok();
        b_time.cmp(&a_time)
    });

    // Take the requested limit and create entries
    for path in files.into_iter().take(limit) {
        let timestamp = fs::metadata(&path)
            .and_then(|m| m.modified())
            .map(DateTime::<Local>::from)
            .unwrap_or_else(|_| Local::now());

        let preview = fs::read_to_string(&path)
            .unwrap_or_default()
            .trim()
            .lines()
            .next()
            .unwrap_or("")
            .chars()
            .take(60)
            .collect();

        entries.push(HistoryEntry {
            path,
            timestamp,
            preview,
        });
    }

    entries
}

/// Get the latest history entry, if any
pub fn latest_entry() -> Option<HistoryEntry> {
    recent_entries(1).into_iter().next()
}

/// Open the history folder in Finder
pub fn open_history_folder() {
    let dir = history_dir();
    if let Err(e) = Command::new("open").arg(&dir).spawn() {
        error!("Failed to open history folder: {}", e);
    }
}

/// Clear all history entries
#[allow(dead_code)] // Prepared for future "Clear History" menu option
pub fn clear_history() {
    let dir = history_dir();
    if let Ok(day_dirs) = fs::read_dir(&dir) {
        for day_entry in day_dirs.flatten() {
            if day_entry.path().is_dir() {
                if let Ok(txt_files) = fs::read_dir(day_entry.path()) {
                    for txt_entry in txt_files.flatten() {
                        let path = txt_entry.path();
                        if path.extension().is_some_and(|ext| ext == "txt") {
                            if let Err(e) = fs::remove_file(&path) {
                                warn!("Failed to delete history entry '{}': {}", path.display(), e);
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_history_dir() {
        let dir = history_dir();
        assert!(dir.to_string_lossy().contains("Transcripts"));
    }

    #[test]
    fn test_save_and_retrieve() {
        let text = "Test transcript content";
        let entry = save_entry(text);

        assert!(entry.path.exists());
        assert_eq!(entry.preview, text);

        // Clean up
        let _ = fs::remove_file(&entry.path);
    }

    #[test]
    fn test_entry_label() {
        let entry = HistoryEntry {
            path: PathBuf::from("/tmp/test.txt"),
            timestamp: Local::now(),
            preview: "Hello world".to_string(),
        };

        let label = entry.label();
        assert!(label.contains("Hello world"));
    }
}
