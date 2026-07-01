//! Filesystem helpers: directory size calculation and human-friendly formatting.

use std::fs;
use std::path::Path;
use std::time::SystemTime;

/// Compute the total size (in bytes) of a directory by walking it iteratively.
///
/// Symbolic links are never followed, which keeps us safe from cycles and avoids
/// counting files that live outside the tree.
pub fn dir_size(path: &Path) -> u64 {
    let mut total: u64 = 0;
    let mut stack = vec![path.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let Ok(read_dir) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in read_dir.flatten() {
            // `file_type()` from `read_dir` does not traverse symlinks, so a
            // symlink to a directory reports as neither dir nor file below.
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                stack.push(entry.path());
            } else if file_type.is_file()
                && let Ok(meta) = entry.metadata()
            {
                total += meta.len();
            }
        }
    }

    total
}

/// Format a byte count as a compact, human-readable string (e.g. `1.4 GB`).
pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}

/// Format a modification time as a short relative age (e.g. `3d`, `2mo`, `1y`).
pub fn format_age(modified: Option<SystemTime>) -> String {
    let Some(time) = modified else {
        return "-".to_string();
    };
    let Ok(elapsed) = SystemTime::now().duration_since(time) else {
        return "future".to_string();
    };
    let days = elapsed.as_secs() / 86_400;
    if days < 1 {
        "today".to_string()
    } else if days < 30 {
        format!("{days}d")
    } else if days < 365 {
        format!("{}mo", days / 30)
    } else {
        format!("{}y", days / 365)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn human_size_formats_units() {
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(1024), "1.0 KB");
        assert_eq!(human_size(1536), "1.5 KB");
        assert_eq!(human_size(1024 * 1024), "1.0 MB");
        assert_eq!(human_size(3 * 1024 * 1024 * 1024), "3.0 GB");
    }

    #[test]
    fn dir_size_sums_files_recursively() {
        let base = std::env::temp_dir().join(format!("nukenpm-size-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(base.join("a/b")).unwrap();
        fs::write(base.join("a/one.txt"), b"1234").unwrap(); // 4 bytes
        fs::write(base.join("a/b/two.txt"), b"567").unwrap(); // 3 bytes
        assert_eq!(dir_size(&base), 7);
        let _ = fs::remove_dir_all(&base);
    }
}
