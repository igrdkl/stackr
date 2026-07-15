//! Filesystem layout for Stackr — everything lives under `C:\Stackr`.
#![allow(dead_code)] // some path helpers are used by not-yet-built steps

use std::io;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

/// Resolved data root, cached after the first lookup so `root()` stays cheap.
static ROOT_CACHE: RwLock<Option<PathBuf>> = RwLock::new(None);

/// Built-in default when the user hasn't chosen a data root.
pub fn default_root() -> PathBuf {
    PathBuf::from("C:\\Stackr")
}

/// `%APPDATA%\Stackr\root.txt` — a pointer to the chosen data root. It lives
/// OUTSIDE the root itself (otherwise we'd need the root to find the root).
fn pointer_file() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("Stackr").join("root.txt"))
}

/// Parse the pointer file's contents: a trimmed non-empty line is a path.
fn parse_pointer(raw: &str) -> Option<PathBuf> {
    let trimmed = raw.trim();
    (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
}

/// The chosen root from the pointer file, if one has been set and is non-empty.
fn read_pointer() -> Option<PathBuf> {
    parse_pointer(&std::fs::read_to_string(pointer_file()?).ok()?)
}

/// A root dir counts as fresh (safe to relocate from) only when it holds neither
/// persisted state nor any installed binaries.
fn is_fresh_root(has_state: bool, has_bin: bool) -> bool {
    !has_state && !has_bin
}

/// Root data directory. Resolves once from the `%APPDATA%` pointer (falling back
/// to `C:\Stackr`) and caches the result.
pub fn root() -> PathBuf {
    if let Some(p) = ROOT_CACHE.read().ok().and_then(|g| g.clone()) {
        return p;
    }
    let resolved = read_pointer().unwrap_or_else(default_root);
    if let Ok(mut g) = ROOT_CACHE.write() {
        if g.is_none() {
            *g = Some(resolved.clone());
        }
    }
    resolved
}

/// A genuinely fresh install: no pointer set yet AND the default root has no
/// state or binaries. The second check keeps an existing `C:\Stackr` user (from
/// before this feature) from ever being prompted to relocate.
pub fn is_first_run() -> bool {
    if read_pointer().is_some() {
        return false;
    }
    let d = default_root();
    is_fresh_root(d.join("stackr.json").exists(), d.join("bin").exists())
}

/// Persist the chosen data root to the pointer file and update the live cache so
/// it takes effect without a restart. Only meaningful during first-run setup —
/// once components exist under a root, moving them is the user's job.
pub fn set_root(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("data folder must be an absolute path".into());
    }
    std::fs::create_dir_all(path).map_err(|e| format!("cannot create {}: {e}", path.display()))?;
    let ptr = pointer_file().ok_or("APPDATA is not set")?;
    if let Some(parent) = ptr.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&ptr, path.to_string_lossy().as_bytes()).map_err(|e| e.to_string())?;
    if let Ok(mut g) = ROOT_CACHE.write() {
        *g = Some(path.to_path_buf());
    }
    Ok(())
}

/// `C:\Stackr\bin`
pub fn bin_root() -> PathBuf {
    root().join("bin")
}

/// `C:\Stackr\bin\{component}\{version}`
pub fn component_dir(component: &str, version: &str) -> PathBuf {
    bin_root().join(component).join(version)
}

/// `C:\Stackr\bin\.downloads` — scratch dir for in-flight downloads, kept out of
/// the component dirs so an aborted/corrupt download never looks installed.
pub fn downloads_dir() -> PathBuf {
    bin_root().join(".downloads")
}

/// Marker written as the final step of a successful install
/// (`bin/{component}/{version}/.installed`). Its absence on a version dir means a
/// broken/partial install that the startup sweep may remove.
pub fn install_marker(component: &str, version: &str) -> PathBuf {
    component_dir(component, version).join(".installed")
}

/// `C:\Stackr\config`
pub fn config_root() -> PathBuf {
    root().join("config")
}

/// `C:\Stackr\logs`
pub fn logs_root() -> PathBuf {
    root().join("logs")
}

/// `C:\Stackr\data` — persistent database data dirs, kept OUT of the per-version
/// `bin` dir so uninstalling or upgrading an engine binary never deletes the
/// databases living in it.
pub fn data_root() -> PathBuf {
    root().join("data")
}

/// Data dir for a MySQL-family engine, keyed by family (`mysql` / `mariadb`) so
/// an in-place version upgrade reuses the same databases, while the two
/// wire-incompatible families stay separate: `C:\Stackr\data\{component}`.
pub fn mysql_data_dir(component: &str) -> PathBuf {
    data_root().join(component)
}

/// PostgreSQL data dir, keyed by MAJOR version — PostgreSQL refuses to start on a
/// data dir created by a different major: `C:\Stackr\data\postgresql\{major}`.
pub fn postgres_data_dir(major: &str) -> PathBuf {
    data_root().join("postgresql").join(major)
}

/// `C:\Stackr\config\nginx\nginx.conf` — generated master config.
pub fn nginx_conf() -> PathBuf {
    config_root().join("nginx").join("nginx.conf")
}

/// `C:\Stackr\config\nginx\sites` — one generated vhost file per project.
pub fn nginx_sites_dir() -> PathBuf {
    config_root().join("nginx").join("sites")
}

/// `C:\Stackr\config\apache\sites`
pub fn apache_sites_dir() -> PathBuf {
    config_root().join("apache").join("sites")
}

/// `C:\Stackr\config\apache\httpd.conf` — generated master config.
pub fn apache_conf() -> PathBuf {
    config_root().join("apache").join("httpd.conf")
}

/// `C:\Stackr\logs\nginx`
pub fn nginx_log_dir() -> PathBuf {
    logs_root().join("nginx")
}

/// `C:\Stackr\logs\apache` — generated Apache logs directory.
pub fn apache_log_dir() -> PathBuf {
    logs_root().join("apache")
}

/// `C:\Stackr\logs\mysql` — MySQL/MariaDB error log directory.
pub fn mysql_log_dir() -> PathBuf {
    logs_root().join("mysql")
}

/// The log file shown for a service in the Logs tab. Servers/DBs write their
/// own logs to these paths; redis/memcached/postgres/php have their stdout+stderr
/// captured to `logs/<component>.log`.
pub fn service_log_file(component: &str) -> PathBuf {
    match component {
        "nginx" => nginx_log_dir().join("error.log"),
        "apache" => apache_log_dir().join("error.log"),
        "mysql" | "mariadb" => mysql_log_dir().join("error.log"),
        other => logs_root().join(format!("{other}.log")),
    }
}

/// `C:\Stackr\www` — project sites.
pub fn www_root() -> PathBuf {
    root().join("www")
}

/// `C:\Stackr\tools\adminer` — Adminer web root (served as `index.php`).
pub fn adminer_dir() -> PathBuf {
    root().join("tools").join("adminer")
}

/// `C:\Stackr\backups` — SQL dumps exported before uninstalling a DB engine.
pub fn backups_dir() -> PathBuf {
    root().join("backups")
}

/// `config\ca` — the local root CA (cert + key) that signs per-domain certs.
pub fn ca_dir() -> PathBuf {
    config_root().join("ca")
}
/// The local root CA certificate (PEM) — imported into the Windows trust store.
pub fn ca_cert() -> PathBuf {
    ca_dir().join("stackr-root-ca.pem")
}
/// The local root CA private key (PEM) — never leaves the machine.
pub fn ca_key() -> PathBuf {
    ca_dir().join("stackr-root-ca-key.pem")
}
/// `config\certs` — per-domain leaf certs signed by the local CA.
pub fn certs_dir() -> PathBuf {
    config_root().join("certs")
}
/// Leaf certificate (PEM) for `domain`.
pub fn domain_cert(domain: &str) -> PathBuf {
    certs_dir().join(format!("{domain}.crt"))
}
/// Leaf private key (PEM) for `domain`.
pub fn domain_key(domain: &str) -> PathBuf {
    certs_dir().join(format!("{domain}.key"))
}

/// Main state file: `C:\Stackr\stackr.json`.
pub fn state_file() -> PathBuf {
    root().join("stackr.json")
}

/// Create a directory (and parents) if missing.
pub fn ensure_dir(path: &std::path::Path) -> io::Result<()> {
    std::fs::create_dir_all(path)
}

#[cfg(test)]
mod tests {
    use super::{is_fresh_root, parse_pointer};

    #[test]
    fn pointer_parsing_ignores_blank_and_whitespace() {
        assert_eq!(parse_pointer(""), None);
        assert_eq!(parse_pointer("   \r\n"), None);
        assert_eq!(parse_pointer("  D:\\Dev\\Stackr \n").unwrap().to_string_lossy(), "D:\\Dev\\Stackr");
    }

    #[test]
    fn fresh_root_only_when_empty() {
        assert!(is_fresh_root(false, false)); // nothing there → fresh → prompt
        assert!(!is_fresh_root(true, false)); // has stackr.json → existing user
        assert!(!is_fresh_root(false, true)); // has bin\ → existing user
        assert!(!is_fresh_root(true, true));
    }
}
