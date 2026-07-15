//! Best-effort Windows Defender exclusion helper for the Stackr root.
//!
//! Real-time scanning of `C:\Stackr` scans every file PHP/Composer/Git/Node
//! touch, which noticeably slows composer installs, artisan commands and the
//! FastCGI file churn. Excluding the root removes that tax.
//!
//! Two deliberate constraints:
//! - **Detection is read-only and non-elevated** (`Get-MpPreference`). It can
//!   legitimately return `unknown` — Defender may be off, replaced by a
//!   third-party AV, or managed by group policy — so the UI never asserts a
//!   state it can't see.
//! - **Adding the exclusion is an explicit, user-initiated UAC action**
//!   (`Add-MpPreference` relaunched with `-Verb RunAs`). Never silent, never
//!   auto-run on startup.

use serde::Serialize;
#[cfg(windows)]
use std::process::Command;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Current exclusion state for the Stackr root.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefenderStatus {
    /// `Some(true)` excluded, `Some(false)` scanned, `None` couldn't determine
    /// (Defender absent/managed, another AV, or query blocked).
    pub excluded: Option<bool>,
    /// The path Stackr manages — always its root, shown for the manual fallback.
    pub path: String,
}

/// The path we manage — Stackr's root, without a trailing separator.
fn managed_path() -> String {
    crate::paths::root()
        .to_string_lossy()
        .trim_end_matches('\\')
        .to_string()
}

/// Read the current Defender exclusion paths and check whether `path` is among
/// them. Returns `None` if the query can't run (no Defender, blocked, etc.).
#[cfg(windows)]
fn query_excluded(path: &str) -> Option<bool> {
    let script =
        "$ErrorActionPreference='Stop'; (Get-MpPreference).ExclusionPath -join [Environment]::NewLine";
    let mut cmd = Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", script]);
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }
    let target = path.trim_end_matches('\\').to_ascii_lowercase();
    let found = String::from_utf8_lossy(&out.stdout).lines().any(|line| {
        line.trim().trim_end_matches('\\').to_ascii_lowercase() == target
    });
    Some(found)
}

/// Add `path` to Defender's exclusions by relaunching PowerShell elevated. The
/// outer (non-elevated) shell only fires the UAC prompt; the elevated child runs
/// `Add-MpPreference`. A declined prompt makes `Start-Process` throw → `Err`.
#[cfg(windows)]
fn add_exclusion(path: &str) -> Result<(), String> {
    // Staged as a .ps1 so the elevated invocation needs no in-line quoting gymnastics.
    let script = format!(
        "Add-MpPreference -ExclusionPath '{}'\n",
        path.replace('\'', "''")
    );
    let tmp = std::env::temp_dir().join("stackr-defender-add.ps1");
    std::fs::write(&tmp, script).map_err(|e| e.to_string())?;

    let ps = format!(
        "Start-Process powershell -Verb RunAs -Wait -ArgumentList \
         '-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File \"{}\"'",
        tmp.display()
    );
    let mut cmd = Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", &ps]);
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd.output().map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&tmp);
    if out.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        // Declined UAC surfaces as a cancel; give the user a clear message.
        if err.contains("canceled") || err.contains("cancelled") {
            Err("Elevation was cancelled — the exclusion was not added.".into())
        } else {
            Err(format!("Could not add the exclusion: {}", err.trim()))
        }
    }
}

#[cfg(not(windows))]
fn query_excluded(_path: &str) -> Option<bool> {
    None
}

#[cfg(not(windows))]
fn add_exclusion(_path: &str) -> Result<(), String> {
    Err("Windows Defender exclusions are only available on Windows.".into())
}

/// Whether `C:\Stackr` is currently excluded from Defender scanning.
#[tauri::command]
pub async fn defender_status() -> DefenderStatus {
    let path = managed_path();
    let probe = path.clone();
    let excluded = tokio::task::spawn_blocking(move || query_excluded(&probe))
        .await
        .unwrap_or(None);
    DefenderStatus { excluded, path }
}

/// Add `C:\Stackr` to Defender's exclusions (fires one UAC prompt).
#[tauri::command]
pub async fn add_defender_exclusion() -> Result<(), String> {
    let path = managed_path();
    tokio::task::spawn_blocking(move || add_exclusion(&path))
        .await
        .map_err(|e| e.to_string())?
}
