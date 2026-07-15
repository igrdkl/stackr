//! PHP versions, extensions and php.ini management.

use std::path::PathBuf;

use serde::Serialize;
use tauri::State;

use crate::models::PhpVersion;
use crate::state::StateStore;

pub(crate) fn ini_path(version: &str) -> PathBuf {
    crate::paths::component_dir("php", version).join("php.ini")
}

/// Ensure a `php.ini` exists (seeded from `php.ini-development` with the common
/// runtime extensions on first creation, then left to the user). Shared with the
/// php-cgi runtime so the toggle UI and the served runtime read the same file.
pub(crate) fn ensure_ini(version: &str) -> Result<PathBuf, String> {
    let dir = crate::paths::component_dir("php", version);
    crate::paths::ensure_dir(&dir).map_err(|e| e.to_string())?;
    crate::scaffold::ensure_php_runtime_ini(&dir)?;
    Ok(dir.join("php.ini"))
}

fn major_minor(v: &str) -> String {
    let parts: Vec<&str> = v.split('.').collect();
    if parts.len() >= 2 {
        format!("{}.{}", parts[0], parts[1])
    } else {
        v.to_string()
    }
}

#[tauri::command]
pub fn get_php_versions(state: State<'_, StateStore>) -> Result<Vec<PhpVersion>, String> {
    let (installed, default) = {
        let st = state.0.lock().map_err(|e| e.to_string())?;
        (st.installed.clone(), st.default_php.clone())
    };

    let mut out = Vec::new();
    for c in installed.into_iter().filter(|c| c.component == "php") {
        let is_default = default.as_deref() == Some(c.version.as_str());
        let dir = crate::paths::component_dir("php", &c.version);
        out.push(PhpVersion {
            major_minor: major_minor(&c.version),
            status: if is_default { "active" } else { "installed" }.to_string(),
            is_default,
            bin_path: dir.to_string_lossy().to_string(),
            ini_path: dir.join("php.ini").to_string_lossy().to_string(),
            extensions: Vec::new(),
            version: c.version,
        });
    }
    Ok(out)
}

/// Names of currently-enabled extensions for a version (parsed from php.ini).
#[tauri::command]
pub fn get_php_extensions(php_version: String) -> Result<Vec<String>, String> {
    let ini = ensure_ini(&php_version)?;
    let content = std::fs::read_to_string(&ini).map_err(|e| e.to_string())?;
    Ok(crate::php_ini::enabled_extensions(&content))
}

#[tauri::command]
pub fn toggle_extension(
    php_version: String,
    extension: String,
    enabled: bool,
) -> Result<(), String> {
    let ini = ensure_ini(&php_version)?;
    let content = std::fs::read_to_string(&ini).map_err(|e| e.to_string())?;
    let updated = crate::php_ini::set_extension(&content, &extension, enabled);
    std::fs::write(&ini, updated).map_err(|e| e.to_string())
}

/// One extension as shown in the Extensions panel.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhpExt {
    pub name: String,
    pub enabled: bool,
    pub installed: bool, // DLL present in the build's ext/ dir
    pub pecl: bool,      // installable via PECL (not bundled)
    pub description: String,
}

/// PECL extensions we can install cleanly on Windows (no extra native libs).
/// imagick/memcached are intentionally excluded — they need ImageMagick /
/// libmemcached DLLs that aren't part of a simple drop-in.
const PECL_EXTENSIONS: &[&str] = &["xdebug", "redis", "apcu", "igbinary", "mongodb"];

fn ext_description(name: &str) -> &'static str {
    match name {
        "opcache" => "Zend OPcache (bytecode cache)",
        "pdo_mysql" => "MySQL PDO driver",
        "mysqli" => "MySQL improved driver",
        "pdo_sqlite" => "SQLite PDO driver",
        "sqlite3" => "SQLite 3 driver",
        "pdo_pgsql" => "PostgreSQL PDO driver",
        "pgsql" => "PostgreSQL driver",
        "gd" => "Image processing",
        "mbstring" => "Multibyte strings",
        "curl" => "HTTP client",
        "openssl" => "TLS & cryptography",
        "zip" => "Zip archives",
        "fileinfo" => "File type detection",
        "intl" => "Internationalization",
        "bcmath" => "Arbitrary-precision math",
        "gmp" => "GNU multiple precision",
        "sodium" => "Modern cryptography",
        "exif" => "Image metadata",
        "soap" => "SOAP protocol client",
        "sockets" => "Low-level sockets",
        "ftp" => "FTP client",
        "ldap" => "LDAP directory access",
        "xsl" => "XSLT transforms",
        "gettext" => "i18n message catalogs",
        "calendar" => "Calendar conversions",
        "redis" => "Redis client (PECL)",
        "xdebug" => "Debugger & profiler (PECL)",
        "apcu" => "User-data cache (PECL)",
        "igbinary" => "Fast serializer (PECL)",
        "mongodb" => "MongoDB driver (PECL)",
        _ => "",
    }
}

/// Bundled extension names from the build's `ext/php_*.dll`.
fn scan_ext_dir(version: &str) -> Vec<String> {
    let ext = crate::paths::component_dir("php", version).join("ext");
    let mut out: Vec<String> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&ext) {
        for e in rd.flatten() {
            let p = e.path();
            let is_dll = p
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.eq_ignore_ascii_case("dll"))
                .unwrap_or(false);
            if !is_dll {
                continue;
            }
            if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                if let Some(name) = stem.strip_prefix("php_") {
                    out.push(name.to_ascii_lowercase());
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Full extension list for a version: the build's real `ext/` DLLs (toggleable)
/// plus a curated set of installable PECL extras not already present.
#[tauri::command]
pub fn list_php_extensions(php_version: String) -> Result<Vec<PhpExt>, String> {
    use std::collections::HashSet;

    let ini = ensure_ini(&php_version)?;
    let content = std::fs::read_to_string(&ini).map_err(|e| e.to_string())?;
    let enabled: HashSet<String> = crate::php_ini::enabled_extensions(&content).into_iter().collect();

    let bundled = scan_ext_dir(&php_version);
    let bundled_set: HashSet<&str> = bundled.iter().map(|s| s.as_str()).collect();

    let mut out: Vec<PhpExt> = bundled
        .iter()
        .map(|n| PhpExt {
            name: n.clone(),
            enabled: enabled.contains(n),
            installed: true,
            pecl: false,
            description: ext_description(n).to_string(),
        })
        .collect();

    for &p in PECL_EXTENSIONS {
        if !bundled_set.contains(p) {
            out.push(PhpExt {
                name: p.to_string(),
                enabled: enabled.contains(p),
                installed: false,
                pecl: true,
                description: ext_description(p).to_string(),
            });
        }
    }

    // Installed first, then alphabetical.
    out.sort_by(|a, b| b.installed.cmp(&a.installed).then(a.name.cmp(&b.name)));
    Ok(out)
}

/// Candidate Windows PECL zip URLs for `ext` (Thread-Safe x64), newest first.
async fn pecl_zip_candidates(ext: &str, mm: &str, tag: &str) -> Result<Vec<String>, String> {
    let base = format!("https://downloads.php.net/~windows/pecl/releases/{ext}/");
    let body = reqwest::Client::new()
        .get(&base)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())?;

    let mut vers: Vec<(u32, u32, u32)> = Vec::new();
    for part in body.split("href=\"").skip(1) {
        if let Some((v, consumed)) = crate::commands::downloader::parse_leading_semver(part) {
            // Version directory links look like `1.2.3/` (skip RCs/betas: `1.2.3RC1/`).
            if part[consumed..].starts_with('/') {
                vers.push(v);
            }
        }
    }
    vers.sort_unstable();
    vers.dedup();
    vers.reverse();

    Ok(vers
        .into_iter()
        .take(8)
        .map(|(a, b, c)| {
            let v = format!("{a}.{b}.{c}");
            format!("{base}{v}/php_{ext}-{v}-{mm}-ts-{tag}-x64.zip")
        })
        .collect())
}

/// Download a PECL extension's DLL(s) into the build's `ext/` and enable it.
#[tauri::command]
pub async fn install_php_extension(php_version: String, name: String) -> Result<(), String> {
    let dir = crate::paths::component_dir("php", &php_version);
    let ext_dir = dir.join("ext");
    if !ext_dir.exists() {
        return Err("PHP 'ext' directory not found — is this version installed?".into());
    }
    let mm = major_minor(&php_version);
    let tag = crate::commands::downloader::php_compiler_tag(&php_version);

    let candidates = pecl_zip_candidates(&name, &mm, tag).await?;
    if candidates.is_empty() {
        return Err(format!("no Windows build of '{name}' found for PHP {mm}"));
    }

    let tmp = std::env::temp_dir().join(format!("stackr-pecl-{name}-{mm}"));
    let _ = std::fs::remove_dir_all(&tmp);
    let mut ok = false;
    let mut last = String::new();
    for url in &candidates {
        match crate::download::download_and_extract(url, &tmp, |_, _| {}).await {
            Ok(()) => {
                ok = true;
                break;
            }
            Err(e) => last = e,
        }
    }
    if !ok {
        return Err(format!("could not download '{name}' for PHP {mm}: {last}"));
    }

    // Copy every DLL from the package into ext/ (covers any bundled deps).
    let mut copied = 0;
    for e in std::fs::read_dir(&tmp).map_err(|e| e.to_string())?.flatten() {
        let p = e.path();
        let is_dll = p
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.eq_ignore_ascii_case("dll"))
            .unwrap_or(false);
        if is_dll {
            if let Some(fname) = p.file_name() {
                std::fs::copy(&p, ext_dir.join(fname)).map_err(|e| e.to_string())?;
                copied += 1;
            }
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
    if copied == 0 {
        return Err(format!("the '{name}' package contained no DLL"));
    }

    // Enable it in php.ini.
    let ini = ensure_ini(&php_version)?;
    let content = std::fs::read_to_string(&ini).map_err(|e| e.to_string())?;
    std::fs::write(&ini, crate::php_ini::set_extension(&content, &name, true))
        .map_err(|e| e.to_string())
}

/// Xdebug state for a PHP version, for the one-click Debug toggle.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XdebugStatus {
    pub installed: bool, // php_xdebug.dll present in ext/
    pub enabled: bool,   // zend_extension=xdebug active in php.ini
    pub port: u16,       // xdebug.client_port the debugger should listen on
}

const XDEBUG_PORT: u16 = 9003;

#[tauri::command]
pub fn xdebug_status(php_version: String) -> Result<XdebugStatus, String> {
    let installed = scan_ext_dir(&php_version).iter().any(|n| n == "xdebug");
    let ini = ensure_ini(&php_version)?;
    let content = std::fs::read_to_string(&ini).map_err(|e| e.to_string())?;
    let enabled = crate::php_ini::enabled_extensions(&content).iter().any(|n| n == "xdebug");
    Ok(XdebugStatus { installed, enabled, port: XDEBUG_PORT })
}

/// One-click debug: enable Xdebug for a PHP version (downloading its DLL on first
/// use), write a sane step-debug config, and restart the runtime if it's live;
/// or disable it. `xdebug.start_with_request=trigger` keeps every request fast —
/// the debugger only attaches when the IDE/browser sends the trigger.
#[tauri::command]
pub async fn set_xdebug(
    reg: tauri::State<'_, crate::commands::services::ProcessRegistry>,
    php_version: String,
    enabled: bool,
) -> Result<(), String> {
    if enabled {
        // Fetch + register the DLL on first enable (installer loads it as a
        // zend_extension since php_ini treats xdebug as Zend).
        let have_dll = scan_ext_dir(&php_version).iter().any(|n| n == "xdebug");
        if !have_dll {
            install_php_extension(php_version.clone(), "xdebug".to_string()).await?;
        }
        let ini = ensure_ini(&php_version)?;
        let mut content = std::fs::read_to_string(&ini).map_err(|e| e.to_string())?;
        content = crate::php_ini::set_extension(&content, "xdebug", true);
        content = crate::php_ini::set_kv(&content, "xdebug.mode", "debug");
        content = crate::php_ini::set_kv(&content, "xdebug.start_with_request", "trigger");
        content = crate::php_ini::set_kv(&content, "xdebug.client_host", "127.0.0.1");
        content = crate::php_ini::set_kv(&content, "xdebug.client_port", &XDEBUG_PORT.to_string());
        std::fs::write(&ini, content).map_err(|e| e.to_string())?;
    } else {
        let ini = ensure_ini(&php_version)?;
        let content = std::fs::read_to_string(&ini).map_err(|e| e.to_string())?;
        // Only unload the extension; leave the xdebug.* keys (inert when disabled).
        std::fs::write(&ini, crate::php_ini::set_extension(&content, "xdebug", false))
            .map_err(|e| e.to_string())?;
    }
    crate::commands::services::restart_php_runtime_if_running(&reg, &php_version)
}

#[tauri::command]
pub fn read_php_ini(php_version: String) -> Result<String, String> {
    let ini = ensure_ini(&php_version)?;
    std::fs::read_to_string(&ini).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_php_ini(php_version: String, content: String) -> Result<(), String> {
    let ini = ini_path(&php_version);
    if let Some(parent) = ini.parent() {
        crate::paths::ensure_dir(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&ini, content).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Live proof the Windows PECL channel resolver works: find redis builds for
    /// PHP 8.3 (vs16, TS) and download one — the DLL the installer copies to ext/.
    ///   cargo test installs_redis_pecl_for_php83 -- --ignored --nocapture
    #[tokio::test]
    #[ignore = "hits downloads.php.net and downloads the redis PECL DLL"]
    async fn installs_redis_pecl_for_php83() {
        let urls = pecl_zip_candidates("redis", "8.3", "vs16").await.expect("candidates");
        assert!(!urls.is_empty(), "should find redis versions on the PECL channel");

        let tmp = std::env::temp_dir().join("stackr-pecl-redis-test");
        let _ = std::fs::remove_dir_all(&tmp);
        let mut ok = false;
        let mut last = String::new();
        for url in &urls {
            match crate::download::download_and_extract(url, &tmp, |_, _| {}).await {
                Ok(()) => {
                    ok = true;
                    break;
                }
                Err(e) => last = e,
            }
        }
        assert!(ok, "redis PECL download failed (tried {urls:?}): {last}");
        assert!(tmp.join("php_redis.dll").exists(), "php_redis.dll should be extracted");
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
