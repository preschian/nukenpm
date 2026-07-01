//! Filesystem helpers: directory size calculation and human-friendly formatting.

use std::fs;
use std::path::Path;
use std::time::SystemTime;

/// Aggregate size and file count of a directory.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct DirStats {
    pub size: u64,
    pub files: u64,
}

/// Compute the total size (in bytes) and file count of a directory by walking
/// it iteratively.
///
/// Symbolic links are never followed, which keeps us safe from cycles and avoids
/// counting files that live outside the tree.
pub fn dir_stats(path: &Path) -> DirStats {
    let mut stats = DirStats::default();
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
                stats.size += meta.len();
                stats.files += 1;
            }
        }
    }

    stats
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

/// Insert thousands separators into a number (e.g. `41208` → `41,208`).
pub fn format_thousands(n: u64) -> String {
    let digits = n.to_string();
    let len = digits.len();
    let mut out = String::with_capacity(len + len / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Whether a directory hasn't been touched in more than roughly six months.
///
/// Used to gently highlight stale directories that are the safest to reclaim.
pub fn is_stale(modified: Option<SystemTime>) -> bool {
    let Some(time) = modified else {
        return false;
    };
    let Ok(elapsed) = SystemTime::now().duration_since(time) else {
        return false;
    };
    elapsed.as_secs() / 86_400 > 180
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
    fn dir_stats_sums_files_recursively() {
        let base = std::env::temp_dir().join(format!("nukenpm-size-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(base.join("a/b")).unwrap();
        fs::write(base.join("a/one.txt"), b"1234").unwrap(); // 4 bytes
        fs::write(base.join("a/b/two.txt"), b"567").unwrap(); // 3 bytes
        let stats = dir_stats(&base);
        assert_eq!(stats.size, 7);
        assert_eq!(stats.files, 2);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn format_thousands_groups_digits() {
        assert_eq!(format_thousands(0), "0");
        assert_eq!(format_thousands(42), "42");
        assert_eq!(format_thousands(1_000), "1,000");
        assert_eq!(format_thousands(41_208), "41,208");
        assert_eq!(format_thousands(1_234_567), "1,234,567");
    }
}
