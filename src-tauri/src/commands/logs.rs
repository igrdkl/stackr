//! Reading per-service log files for the Logs tab.

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use serde::Serialize;

/// All services that write a log file Stackr can surface. (mysql/mariadb share
/// one error.log, so "mysql" covers both.)
const LOG_SERVICES: &[&str] = &["nginx", "apache", "php", "mysql", "postgresql", "redis", "memcached"];

/// One raw log line tagged with the service it came from (parsed on the frontend).
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogRaw {
    pub service: String,
    pub line: String,
}

/// Last `max_lines` lines of a file, reading only the trailing bytes so a huge log
/// stays cheap (empty if the file doesn't exist yet).
fn tail(path: &Path, max_lines: usize) -> Vec<String> {
    const CAP: u64 = 256 * 1024; // last 256 KB covers far more than max_lines
    let Ok(mut f) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let start = len.saturating_sub(CAP);
    if start > 0 && f.seek(SeekFrom::Start(start)).is_err() {
        return Vec::new();
    }
    let mut buf = Vec::new();
    if f.read_to_end(&mut buf).is_err() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&buf);
    let mut lines: Vec<&str> = text.lines().collect();
    // Reading mid-file likely sliced a line in half — drop the partial first one.
    if start > 0 && !lines.is_empty() {
        lines.remove(0);
    }
    let begin = lines.len().saturating_sub(max_lines.max(1));
    // Cap line length: a single pathologically long line (e.g. a minified blob)
    // rendered with `nowrap` can stall the webview's layout. Truncate (char-safe).
    const MAX_LINE: usize = 2000;
    lines[begin..]
        .iter()
        .map(|s| {
            if s.len() > MAX_LINE {
                let cut: String = s.chars().take(MAX_LINE).collect();
                format!("{cut}… [truncated]")
            } else {
                s.to_string()
            }
        })
        .collect()
}

/// Last `max_lines` lines of a service's log file. Async + off-thread so reading
/// never blocks the UI event loop.
#[tauri::command]
pub async fn read_log(component: String, max_lines: usize) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || tail(&crate::paths::service_log_file(&component), max_lines))
        .await
        .map_err(|e| e.to_string())
}

/// Merged tail of every service's log, each line tagged with its source. The
/// frontend parses the timestamp/level and orders the combined stream. Async +
/// off-thread (it reads up to 7 files) so it never blocks the UI.
#[tauri::command]
pub async fn read_all_logs(max_lines: usize) -> Result<Vec<LogRaw>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut out = Vec::new();
        for &svc in LOG_SERVICES {
            for line in tail(&crate::paths::service_log_file(svc), max_lines) {
                out.push(LogRaw { service: svc.to_string(), line });
            }
        }
        out
    })
    .await
    .map_err(|e| e.to_string())
}

/// Truncate a service's log file.
#[tauri::command]
pub fn clear_log(component: String) -> Result<(), String> {
    let path = crate::paths::service_log_file(&component);
    if path.exists() {
        std::fs::write(&path, b"").map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Truncate every service's log file (the "All" view's Clear).
#[tauri::command]
pub fn clear_all_logs() -> Result<(), String> {
    for &svc in LOG_SERVICES {
        let path = crate::paths::service_log_file(svc);
        if path.exists() {
            let _ = std::fs::write(&path, b"");
        }
    }
    Ok(())
}

/// Above this size a log is trimmed to its recent tail on startup so a long-lived
/// service can't grow its log without bound between sessions.
const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024; // 5 MB
/// How much of the tail to keep when a log is rotated.
const KEEP_LOG_BYTES: u64 = 1024 * 1024; // 1 MB

/// Trim every oversized service log to its most recent tail, keeping whole lines.
/// Best-effort and meant to run at startup — before any service is spawned, so no
/// writer holds the file open. Files that can't be read/rewritten are skipped.
pub fn rotate_on_startup() {
    for &svc in LOG_SERVICES {
        rotate_file(&crate::paths::service_log_file(svc), MAX_LOG_BYTES, KEEP_LOG_BYTES);
    }
}

/// Rewrite `path` with only its last `keep` bytes if it exceeds `max`.
fn rotate_file(path: &Path, max: u64, keep: u64) {
    let Ok(meta) = std::fs::metadata(path) else {
        return; // missing file — nothing to rotate
    };
    if meta.len() <= max {
        return;
    }
    let Ok(mut f) = std::fs::File::open(path) else {
        return;
    };
    let start = meta.len().saturating_sub(keep);
    if start > 0 && f.seek(SeekFrom::Start(start)).is_err() {
        return;
    }
    let mut buf = Vec::new();
    if f.read_to_end(&mut buf).is_err() {
        return;
    }
    drop(f); // release the read handle before rewriting
    // Seeking mid-file likely sliced a line — drop the partial leading fragment.
    let text = String::from_utf8_lossy(&buf);
    let body = if start > 0 {
        match text.find('\n') {
            Some(i) => &text[i + 1..],
            None => text.as_ref(),
        }
    } else {
        text.as_ref()
    };
    let header = format!(
        "-- older entries trimmed by Stackr on startup (log exceeded {} MB) --\n",
        max / (1024 * 1024)
    );
    let _ = std::fs::write(path, format!("{header}{body}"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotate_trims_oversized_log_keeping_recent_lines() {
        let dir = std::env::temp_dir().join("stackr-logtest");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("rotate.log");

        // 2000 numbered lines — comfortably over the tiny test cap below.
        let mut content = String::new();
        for i in 0..2000 {
            content.push_str(&format!("line {i}\n"));
        }
        std::fs::write(&path, &content).unwrap();
        let original = std::fs::metadata(&path).unwrap().len();

        // Trim to ~1 KB when over 4 KB.
        rotate_file(&path, 4 * 1024, 1024);

        let after = std::fs::read_to_string(&path).unwrap();
        assert!((after.len() as u64) < original, "file should shrink");
        assert!(after.starts_with("-- older entries trimmed"), "keeps a trim header");
        assert!(after.trim_end().ends_with("line 1999"), "keeps the newest line");
        assert!(!after.contains("line 0\n"), "drops the oldest lines");

        // Under the cap → untouched.
        std::fs::write(&path, b"small\n").unwrap();
        rotate_file(&path, 4 * 1024, 1024);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "small\n");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
