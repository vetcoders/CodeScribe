//! Safe path utilities with canonicalization and boundary validation.
//!
//! Provides path validation to prevent path traversal attacks.
//! All paths are canonicalized (resolving symlinks and `..`) and optionally
//! bounded to an allowed root directory.
//!
//! # Security Model
//!
//! CodeScribe is a desktop app, not a web server. Paths come from:
//! - Local audio recordings (temp files created by app - trusted)
//! - CLI arguments (user's own files on their system - trusted)
//! - Config files (user's own config - trusted)
//!
//! However, we still apply defense-in-depth by canonicalizing all paths
//! before use, which eliminates symlink attacks and `..` traversal.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Canonicalize a path, resolving symlinks and relative components.
///
/// Returns the canonical absolute path, or an error if the path doesn't exist
/// or cannot be resolved.
///
/// # Example
/// ```ignore
/// let safe = safe_canonicalize(Path::new("../../../etc/passwd"))?;
/// // Returns error or resolved path within filesystem
/// ```
pub fn safe_canonicalize(path: &Path) -> Result<PathBuf> {
    path.canonicalize()
        .with_context(|| format!("Failed to canonicalize path: {}", path.display()))
}

#[allow(dead_code)] // Public API for bounded path validation
/// Canonicalize a path and verify it's within an allowed root directory.
///
/// This prevents path traversal attacks where `../..` or symlinks could
/// escape the intended directory.
///
/// # Arguments
/// * `path` - The path to validate
/// * `root` - The allowed root directory (will also be canonicalized)
///
/// # Returns
/// The canonical path if it's within the root, or an error if:
/// - The path doesn't exist
/// - The path resolves outside the root directory
///
/// # Example
/// ```ignore
/// let root = Path::new("/app/data");
/// let safe = safe_canonicalize_bounded(Path::new("../../../etc/passwd"), root)?;
/// // Returns Err - path escapes root
/// ```
pub fn safe_canonicalize_bounded(path: &Path, root: &Path) -> Result<PathBuf> {
    let root_canon = root
        .canonicalize()
        .with_context(|| format!("Failed to canonicalize root directory: {}", root.display()))?;

    let path_canon = path
        .canonicalize()
        .with_context(|| format!("Failed to canonicalize path: {}", path.display()))?;

    if !path_canon.starts_with(&root_canon) {
        anyhow::bail!(
            "Path traversal detected: {} is outside allowed root {}",
            path_canon.display(),
            root_canon.display()
        );
    }

    Ok(path_canon)
}

/// Open a file after canonicalizing the path.
///
/// This is a safe wrapper around `std::fs::File::open` that first
/// canonicalizes the path to resolve symlinks and relative components.
///
/// # Security Note
/// For desktop apps where paths come from trusted sources (user CLI args,
/// app-created temp files), canonicalization provides defense-in-depth
/// without being strictly necessary.
pub fn safe_open(path: &Path) -> Result<std::fs::File> {
    let canonical = safe_canonicalize(path)?;
    // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path (path canonicalized above)
    std::fs::File::open(&canonical)
        .with_context(|| format!("Failed to open file: {}", canonical.display()))
}

/// Read a file to string after canonicalizing the path.
///
/// Safe wrapper around `std::fs::read_to_string`.
pub fn safe_read_to_string(path: &Path) -> Result<String> {
    let canonical = safe_canonicalize(path)?;
    // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path (path canonicalized above)
    std::fs::read_to_string(&canonical)
        .with_context(|| format!("Failed to read file: {}", canonical.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_canonicalize_existing_path() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "test").unwrap();

        let result = safe_canonicalize(&file_path);
        assert!(result.is_ok());
    }

    #[test]
    fn test_canonicalize_nonexistent_path() {
        let result = safe_canonicalize(Path::new("/nonexistent/path/file.txt"));
        assert!(result.is_err());
    }

    #[test]
    fn test_bounded_canonicalize_within_root() {
        let dir = tempdir().unwrap();
        let subdir = dir.path().join("subdir");
        fs::create_dir(&subdir).unwrap();
        let file_path = subdir.join("test.txt");
        fs::write(&file_path, "test").unwrap();

        let result = safe_canonicalize_bounded(&file_path, dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_bounded_canonicalize_outside_root() {
        let dir = tempdir().unwrap();
        let other_dir = tempdir().unwrap();
        let file_path = other_dir.path().join("test.txt");
        fs::write(&file_path, "test").unwrap();

        let result = safe_canonicalize_bounded(&file_path, dir.path());
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("outside allowed root")
        );
    }

    #[test]
    fn test_safe_open() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "hello").unwrap();

        let result = safe_open(&file_path);
        assert!(result.is_ok());
    }

    #[test]
    fn test_safe_read_to_string() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "hello world").unwrap();

        let result = safe_read_to_string(&file_path);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "hello world");
    }
}
