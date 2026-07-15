//! Persisted application state — `C:\Stackr\stackr.json`.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledComponent {
    pub component: String, // canonical id, e.g. "nginx"
    pub name: String,      // display name, e.g. "Nginx"
    pub version: String,
    pub path: String, // install directory
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub startup: bool,   // launch Stackr at Windows login
    pub autostart: bool, // start last-running services on open
    pub notify: bool,    // desktop notifications
    #[serde(default = "default_sites_dir")]
    pub sites_dir: String, // base directory new projects are created under
    #[serde(default = "default_tld")]
    pub tld: String, // local TLD for project domains, e.g. ".test"
    #[serde(default)]
    pub https: bool, // serve projects over HTTPS with the local CA
}

fn default_sites_dir() -> String {
    crate::paths::www_root().to_string_lossy().to_string()
}

fn default_tld() -> String {
    ".test".to_string()
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            startup: false,
            autostart: false,
            notify: true,
            sites_dir: default_sites_dir(),
            tld: default_tld(),
            https: false,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppState {
    pub installed: Vec<InstalledComponent>,
    #[serde(default)]
    pub default_php: Option<String>,
    #[serde(default)]
    pub projects: Vec<crate::models::Project>,
    #[serde(default)]
    pub settings: AppSettings,
    /// Transient (never persisted): set when `load()` had to fall back to the
    /// `.bak` because the live file was missing/corrupt, so the UI can inform the
    /// user their state was recovered rather than silently changed.
    #[serde(skip)]
    pub restored_from_backup: bool,
}

impl AppState {
    pub fn load() -> AppState {
        let path = crate::paths::state_file();
        // Prefer the live file; if it's missing or corrupt (e.g. a crash truncated
        // a non-atomic write in an older build), fall back to the last good .bak
        // rather than silently starting empty and dropping every project.
        let live = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<AppState>(&s).ok());
        let (mut state, restored) = match live {
            Some(s) => (s, false),
            None => {
                let bak = path.with_extension("json.bak");
                match std::fs::read_to_string(&bak)
                    .ok()
                    .and_then(|s| serde_json::from_str::<AppState>(&s).ok())
                {
                    Some(s) => (s, true),
                    None => (AppState::default(), false),
                }
            }
        };
        state.restored_from_backup = restored;
        // The process registry is empty on startup, so any project persisted as
        // "running" from a previous session is stale — its nginx/php-cgi died
        // with the old process. Reset to "stopped" so the UI is truthful.
        for p in state.projects.iter_mut() {
            p.status = "stopped".into();
        }
        state
    }

    pub fn save(&self) -> Result<(), String> {
        crate::paths::ensure_dir(&crate::paths::root()).map_err(|e| e.to_string())?;
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        let dest = crate::paths::state_file();
        // Write to a sibling temp file then atomically swap it in, so a crash
        // mid-write can never truncate stackr.json and lose every project +
        // installed component. Keep the previous good copy as .bak first.
        let tmp = dest.with_extension("json.tmp");
        std::fs::write(&tmp, &json).map_err(|e| e.to_string())?;
        if dest.exists() {
            let _ = std::fs::copy(&dest, dest.with_extension("json.bak"));
        }
        // rename can transiently fail while an AV/indexer holds a handle on the
        // destination; retry a few times with a short backoff before giving up.
        let mut attempt: u64 = 0;
        loop {
            match std::fs::rename(&tmp, &dest) {
                Ok(()) => return Ok(()),
                Err(_) if attempt < 5 => {
                    attempt += 1;
                    std::thread::sleep(std::time::Duration::from_millis(50 * attempt));
                }
                Err(e) => return Err(e.to_string()),
            }
        }
    }

    pub fn upsert(&mut self, c: InstalledComponent) {
        if let Some(existing) = self
            .installed
            .iter_mut()
            .find(|x| x.component == c.component && x.version == c.version)
        {
            *existing = c;
        } else {
            self.installed.push(c);
        }
    }

    pub fn remove(&mut self, component: &str, version: &str) {
        self.installed
            .retain(|x| !(x.component == component && x.version == version));
    }
}

/// Tauri-managed wrapper.
pub struct StateStore(pub Mutex<AppState>);

#[tauri::command]
pub fn get_installed(state: State<'_, StateStore>) -> Result<Vec<InstalledComponent>, String> {
    let st = state.0.lock().map_err(|e| e.to_string())?;
    Ok(st.installed.clone())
}

#[tauri::command]
pub fn set_default_php(state: State<'_, StateStore>, version: String) -> Result<(), String> {
    let mut st = state.0.lock().map_err(|e| e.to_string())?;
    st.default_php = Some(version);
    st.save()
}

#[tauri::command]
pub fn get_settings(state: State<'_, StateStore>) -> Result<AppSettings, String> {
    let st = state.0.lock().map_err(|e| e.to_string())?;
    Ok(st.settings.clone())
}

/// One-shot: returns whether the last load recovered state from the `.bak`, then
/// clears the flag so the frontend shows the "restored" notice only once.
#[tauri::command]
pub fn take_restore_notice(state: State<'_, StateStore>) -> Result<bool, String> {
    let mut st = state.0.lock().map_err(|e| e.to_string())?;
    let was = st.restored_from_backup;
    st.restored_from_backup = false;
    Ok(was)
}

/// Where Stackr keeps everything, plus whether this looks like a first run.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RootInfo {
    pub root: String,
    pub default_root: String,
    pub is_first_run: bool,
}

#[tauri::command]
pub fn get_root_info() -> RootInfo {
    RootInfo {
        root: crate::paths::root().to_string_lossy().to_string(),
        default_root: crate::paths::default_root().to_string_lossy().to_string(),
        is_first_run: crate::paths::is_first_run(),
    }
}

/// Choose the data root (first-run picker). Writes the `%APPDATA%` pointer, then
/// commits the current (empty, on first run) state at the new location so it's
/// materialized immediately.
#[tauri::command]
pub fn set_root(state: State<'_, StateStore>, path: String) -> Result<(), String> {
    let old_default_www = crate::paths::default_root().join("www").to_string_lossy().to_string();
    crate::paths::set_root(std::path::Path::new(&path))?;
    let mut st = state.0.lock().map_err(|e| e.to_string())?;
    // If the sites dir is still the untouched default, retarget it under the new
    // root so first projects land beside everything else.
    if st.settings.sites_dir == old_default_www {
        st.settings.sites_dir = crate::paths::www_root().to_string_lossy().to_string();
    }
    st.save()
}

#[tauri::command]
pub fn save_settings(state: State<'_, StateStore>, settings: AppSettings) -> Result<(), String> {
    // Apply the "launch at login" registry entry to match the new setting.
    crate::autostart::set_autostart(settings.startup)?;
    let mut st = state.0.lock().map_err(|e| e.to_string())?;
    st.settings = settings;
    st.save()
}
