//! Binary downloader: fetch a component ZIP, unzip into
//! `C:\Stackr\bin\{component}\{version}\`, streaming `download-progress` events
//! and recording the result in `stackr.json`.

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::state::{InstalledComponent, StateStore};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadProgress {
    component: String,
    percent: u32,
    bytes_downloaded: u64,
    total_bytes: u64,
}

#[tauri::command]
pub async fn install_component(
    app: AppHandle,
    state: State<'_, StateStore>,
    component_type: String,
    version: String,
) -> Result<(), String> {
    // Web servers install the newest available build ("latest" sentinel from the
    // UI); everything else uses the exact version requested.
    let version = if version == "latest" || version.is_empty() {
        resolve_latest(&component_type).await?
    } else {
        version
    };

    let dest = crate::paths::component_dir(&component_type, &version);

    // Prefer the manifest's authoritative (URL, SHA-256) for this exact build; a
    // matching sha256 makes the download verified. Fall back to the scraped/pinned
    // sources (PHP + MySQL each have current+archive URLs) as unverified `None`.
    let manifest_entry = crate::manifest::lookup(&component_type, &version).await;

    let fallbacks: Vec<String> = match component_type.as_str() {
        "php" => php_zip_urls(&version),
        "mysql" => mysql_zip_urls(&version),
        _ => match resolve_url(&component_type, &version) {
            Ok(u) => vec![u],
            // Only fatal if the manifest didn't supply a URL either.
            Err(e) if manifest_entry.is_none() => return Err(e),
            Err(_) => vec![],
        },
    };

    // (url, expected_sha256) candidates in priority order.
    let mut candidates: Vec<(String, Option<String>)> = Vec::new();
    if let Some(ref e) = manifest_entry {
        candidates.push((e.url.clone(), e.sha256.clone()));
    }
    for u in fallbacks {
        if !candidates.iter().any(|(cu, _)| cu == &u) {
            candidates.push((u, None));
        }
    }
    if candidates.is_empty() {
        return Err(format!("no download source for {component_type} {version}"));
    }

    // Stream the download, emitting throttled progress (only on percent change).
    let mut last_err = String::new();
    let mut ok = false;
    let mut digest = String::new();
    let mut used_url = String::new();
    for (url, expected_sha) in &candidates {
        let app_progress = app.clone();
        let comp = component_type.clone();
        let mut last_percent = u32::MAX;
        match crate::download::download_and_extract_checked(url, &dest, expected_sha.as_deref(), move |downloaded, total| {
            let percent = if total > 0 {
                ((downloaded as f64 / total as f64) * 100.0) as u32
            } else {
                0
            };
            if percent != last_percent {
                last_percent = percent;
                let _ = app_progress.emit(
                    "download-progress",
                    DownloadProgress {
                        component: comp.clone(),
                        percent,
                        bytes_downloaded: downloaded,
                        total_bytes: total,
                    },
                );
            }
        })
        .await
        {
            Ok(dg) => {
                digest = dg;
                used_url = url.clone();
                ok = true;
                break;
            }
            Err(e) => last_err = e,
        }
    }
    if !ok {
        return Err(format!("could not download {component_type} {version}: {last_err}"));
    }

    // Record into persisted state.
    {
        let mut st = state.0.lock().map_err(|e| e.to_string())?;
        st.upsert(InstalledComponent {
            component: component_type.clone(),
            name: display_name(&component_type),
            version: version.clone(),
            path: dest.to_string_lossy().to_string(),
        });
        st.save()?;
    }

    // Mark the install complete (final write). The startup sweep treats a version
    // dir with no marker as a broken/partial install. Record the source + digest
    // so it can seed the version manifest later.
    let _ = std::fs::write(
        crate::paths::install_marker(&component_type, &version),
        format!("sha256={digest}\nurl={used_url}\n"),
    );

    // Final 100% tick.
    let _ = app.emit(
        "download-progress",
        DownloadProgress {
            component: component_type,
            percent: 100,
            bytes_downloaded: 0,
            total_bytes: 0,
        },
    );
    Ok(())
}

#[tauri::command]
pub fn uninstall_component(
    state: State<'_, StateStore>,
    component_type: String,
    version: String,
) -> Result<(), String> {
    let dir = crate::paths::component_dir(&component_type, &version);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    // Drop the generated master config so a reinstall regenerates it against the
    // fresh install dir (otherwise a stale path to the old version lingers).
    match component_type.as_str() {
        "nginx" => {
            let _ = std::fs::remove_file(crate::paths::nginx_conf());
        }
        "apache" => {
            let _ = std::fs::remove_file(crate::paths::apache_conf());
        }
        _ => {}
    }
    let mut st = state.0.lock().map_err(|e| e.to_string())?;
    st.remove(&component_type, &version);
    // If the default PHP was removed, fall back to another installed one (or none).
    if component_type == "php" && st.default_php.as_deref() == Some(version.as_str()) {
        st.default_php = st
            .installed
            .iter()
            .find(|c| c.component == "php")
            .map(|c| c.version.clone());
    }
    st.save()
}

/// Startup cleanup: clear the download scratch dir, and remove any
/// `bin/{component}/{version}` dir that is neither recorded in state nor carries
/// an `.installed` marker — i.e. leftover from a crashed/aborted install.
/// Recorded components are never touched (older installs predate the marker).
pub fn prune_broken_installs(state: &crate::state::AppState) {
    // Wipe leftover in-flight download archives from a previous session.
    let downloads = crate::paths::downloads_dir();
    if let Ok(rd) = std::fs::read_dir(&downloads) {
        for e in rd.flatten() {
            let _ = std::fs::remove_file(e.path());
        }
    }

    let recorded: std::collections::HashSet<(String, String)> = state
        .installed
        .iter()
        .map(|c| (c.component.clone(), c.version.clone()))
        .collect();

    let Ok(comps) = std::fs::read_dir(crate::paths::bin_root()) else {
        return;
    };
    for comp in comps.flatten() {
        let comp_name = comp.file_name().to_string_lossy().to_string();
        // Skip dotfiles (e.g. .downloads) and stray files.
        if comp_name.starts_with('.') || !comp.path().is_dir() {
            continue;
        }
        let Ok(vers) = std::fs::read_dir(comp.path()) else {
            continue;
        };
        for ver in vers.flatten() {
            if !ver.path().is_dir() {
                continue;
            }
            let ver_name = ver.file_name().to_string_lossy().to_string();
            let recorded_here = recorded.contains(&(comp_name.clone(), ver_name.clone()));
            let has_marker = ver.path().join(".installed").exists();
            if !recorded_here && !has_marker {
                let _ = std::fs::remove_dir_all(ver.path());
            }
        }
    }
}

fn display_name(component: &str) -> String {
    match component {
        "nginx" => "Nginx",
        "apache" => "Apache",
        "php" => "PHP",
        "mysql" => "MySQL",
        "mariadb" => "MariaDB",
        "postgresql" => "PostgreSQL",
        "redis" => "Redis",
        "memcached" => "Memcached",
        "mailpit" => "Mailpit",
        other => other,
    }
    .to_string()
}

/// Resolve the newest installable version for components that support
/// "install latest". nginx is detected live from its download index; Apache is
/// pinned (Apache Lounge has no derivable "latest").
async fn resolve_latest(component: &str) -> Result<String, String> {
    match component {
        "nginx" => resolve_latest_nginx().await,
        "apache" => Ok("2.4.68".to_string()),
        other => Err(format!("'{other}' does not support installing 'latest'")),
    }
}

/// Scrape nginx.org's download directory and return the highest `.zip` version.
async fn resolve_latest_nginx() -> Result<String, String> {
    let body = reqwest::Client::new()
        .get("https://nginx.org/download/")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())?;
    highest_nginx_zip(&body)
        .map(|(a, b, c)| format!("{a}.{b}.{c}"))
        .ok_or_else(|| "could not determine the latest nginx version".to_string())
}

/// Highest `nginx-X.Y.Z.zip` version mentioned in an nginx download listing.
fn highest_nginx_zip(body: &str) -> Option<(u32, u32, u32)> {
    let mut best: Option<(u32, u32, u32)> = None;
    for part in body.split("nginx-").skip(1) {
        if let Some((ver, consumed)) = parse_leading_semver(part) {
            if part[consumed..].starts_with(".zip") {
                best = Some(best.map_or(ver, |b| b.max(ver)));
            }
        }
    }
    best
}

/// Parse a leading `X.Y.Z` from `s`, returning the version and bytes consumed.
pub(crate) fn parse_leading_semver(s: &str) -> Option<((u32, u32, u32), usize)> {
    let bytes = s.as_bytes();
    let mut nums = [0u32; 3];
    let mut i = 0;
    for (n, slot) in nums.iter_mut().enumerate() {
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i == start {
            return None;
        }
        *slot = s[start..i].parse().ok()?;
        if n < 2 {
            if i < bytes.len() && bytes[i] == b'.' {
                i += 1;
            } else {
                return None;
            }
        }
    }
    Some(((nums[0], nums[1], nums[2]), i))
}

/// The MSVC build tag in PHP's Windows zip names, which varies by branch:
/// 7.x → vc15, 8.0–8.3 → vs16, 8.4+ → vs17.
pub(crate) fn php_compiler_tag(version: &str) -> &'static str {
    let mut it = version.split('.');
    let maj: u32 = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let min: u32 = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    match (maj, min) {
        (7, _) => "vc15",
        (8, 0..=3) => "vs16",
        _ => "vs17",
    }
}

/// Candidate URLs for a PHP build (Thread-Safe x64): current builds live under
/// `releases/`, EOL ones under `releases/archives/`. Tried in order.
fn php_zip_urls(version: &str) -> Vec<String> {
    let file = format!("php-{version}-Win32-{}-x64.zip", php_compiler_tag(version));
    vec![
        format!("https://windows.php.net/downloads/releases/{file}"),
        format!("https://windows.php.net/downloads/releases/archives/{file}"),
    ]
}

/// Candidate URLs for a MySQL community zip: the current GA build of a series sits
/// under the CDN's `Downloads/`, superseded ones under `archives/`. Tried in order.
/// (Hit the CDN directly — the dev.mysql.com `/get/` redirect WAF-blocks our agent.)
fn mysql_zip_urls(version: &str) -> Vec<String> {
    let mm = major_minor(version);
    vec![
        format!("https://cdn.mysql.com/Downloads/MySQL-{mm}/mysql-{version}-winx64.zip"),
        format!("https://cdn.mysql.com/archives/mysql-{mm}/mysql-{version}-winx64.zip"),
    ]
}

/// Installable PHP versions (latest patch per minor, 7.4 → newest), scraped live
/// from windows.php.net's current + archive listings. Newest first.
#[tauri::command]
pub async fn get_php_available() -> Result<Vec<String>, String> {
    use std::collections::BTreeMap;

    let client = reqwest::Client::new();
    let mut html = String::new();
    for url in [
        "https://windows.php.net/downloads/releases/",
        "https://windows.php.net/downloads/releases/archives/",
    ] {
        if let Ok(resp) = client.get(url).send().await {
            if let Ok(body) = resp.text().await {
                html.push_str(&body);
            }
        }
    }
    if html.is_empty() {
        return Err("could not reach windows.php.net".to_string());
    }

    // Keep the highest patch per (major, minor) among Thread-Safe x64 zips.
    let mut best: BTreeMap<(u32, u32), (u32, u32, u32)> = BTreeMap::new();
    for part in html.split("php-").skip(1) {
        let Some(((a, b, c), consumed)) = parse_leading_semver(part) else {
            continue;
        };
        let rest = &part[consumed..];
        if !rest.starts_with("-Win32-") {
            continue; // excludes the -nts- (non-thread-safe) variants
        }
        let Some(zip) = rest.find(".zip") else { continue };
        if !rest[..zip].ends_with("-x64") {
            continue; // excludes x86
        }
        if a < 7 || (a == 7 && b < 4) {
            continue; // 7.4 and up only
        }
        let slot = best.entry((a, b)).or_insert((a, b, c));
        if (a, b, c) > *slot {
            *slot = (a, b, c);
        }
    }

    let mut versions: Vec<(u32, u32, u32)> = best.into_values().collect();
    versions.sort_unstable();
    versions.reverse();
    Ok(versions.into_iter().map(|(a, b, c)| format!("{a}.{b}.{c}")).collect())
}

/// Resolve a download URL for a component+version.
/// nginx is fully reliable; PHP is best-effort (Windows.php.net layout).
/// Other engines return an explicit error until their resolvers are added.
fn resolve_url(component: &str, version: &str) -> Result<String, String> {
    match component {
        // Reliable.
        "nginx" => Ok(format!("https://nginx.org/download/nginx-{version}.zip")),
        // MariaDB keeps every release in its archive — fully reliable.
        "mariadb" => Ok(format!(
            "https://archive.mariadb.org/mariadb-{version}/winx64-packages/mariadb-{version}-winx64.zip"
        )),
        // Apache Lounge uses date-stamped, VS-tagged filenames that can't be
        // derived from the version, so known builds are pinned explicitly.
        "apache" => match version {
            "2.4.68" => Ok(
                "https://www.apachelounge.com/download/VS18/binaries/httpd-2.4.68-260617-Win64-VS18.zip"
                    .to_string(),
            ),
            other => Err(format!(
                "Apache {other}: no pinned Windows build URL (Apache Lounge filenames are date-stamped). Add it to resolve_url()."
            )),
        },
        // PHP: the compiler tag depends on the version (see php_zip_urls).
        "php" => php_zip_urls(version)
            .into_iter()
            .next()
            .ok_or_else(|| "could not build PHP download URL".to_string()),
        // The dev.mysql.com /get/ redirect WAF-blocks non-curl agents on GET, so
        // go straight to the CDN (see mysql_zip_urls for the current/archived split).
        "mysql" => mysql_zip_urls(version)
            .into_iter()
            .next()
            .ok_or_else(|| "could not build MySQL download URL".to_string()),
        "postgresql" => Ok(format!(
            "https://get.enterprisedb.com/postgresql/postgresql-{version}-1-windows-x64-binaries.zip"
        )),
        // Windows ports live on GitHub with non-derivable asset names — pin them.
        "redis" => match version {
            "5.0.14.1" => Ok(
                "https://github.com/tporadowski/redis/releases/download/v5.0.14.1/Redis-x64-5.0.14.1.zip"
                    .to_string(),
            ),
            other => Err(format!(
                "Redis {other}: no pinned Windows build URL. Add it to resolve_url()."
            )),
        },
        "memcached" => match version {
            "1.6.8" => Ok(
                "https://github.com/jefyt/memcached-windows/releases/download/1.6.8_mingw_libressl/memcached-1.6.8-win64-mingw.zip"
                    .to_string(),
            ),
            other => Err(format!(
                "Memcached {other}: no pinned Windows build URL. Add it to resolve_url()."
            )),
        },
        // Mailpit ships one Windows x64 zip per release with a derivable name.
        "mailpit" => Ok(format!(
            "https://github.com/axllent/mailpit/releases/download/v{version}/mailpit-windows-amd64.zip"
        )),
        other => Err(format!(
            "no download source configured yet for '{other}' — add it to resolve_url()"
        )),
    }
}

/// "8.0.36" -> "8.0" (used for vendor download paths).
fn major_minor(version: &str) -> String {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() >= 2 {
        format!("{}.{}", parts[0], parts[1])
    } else {
        version.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{highest_nginx_zip, mysql_zip_urls, php_compiler_tag, php_zip_urls};

    #[test]
    fn mysql_urls_try_downloads_then_archives() {
        let urls = mysql_zip_urls("8.0.36");
        assert_eq!(urls.len(), 2);
        assert!(urls[0].ends_with("Downloads/MySQL-8.0/mysql-8.0.36-winx64.zip"));
        assert!(urls[1].contains("/archives/mysql-8.0/mysql-8.0.36-winx64.zip"));
    }

    #[test]
    fn php_compiler_tag_by_branch() {
        assert_eq!(php_compiler_tag("7.4.33"), "vc15");
        assert_eq!(php_compiler_tag("8.1.31"), "vs16");
        assert_eq!(php_compiler_tag("8.3.4"), "vs16");
        assert_eq!(php_compiler_tag("8.4.3"), "vs17");
        assert_eq!(php_compiler_tag("9.0.0"), "vs17");
    }

    #[test]
    fn php_urls_try_releases_then_archives() {
        let urls = php_zip_urls("7.4.33");
        assert_eq!(urls.len(), 2);
        assert!(urls[0].ends_with("releases/php-7.4.33-Win32-vc15-x64.zip"));
        assert!(urls[1].contains("/archives/php-7.4.33-Win32-vc15-x64.zip"));
    }

    #[test]
    fn picks_highest_nginx_zip_ignoring_tarballs() {
        let body = r#"
            <a href="nginx-1.24.0.zip">nginx-1.24.0.zip</a>
            <a href="nginx-1.27.3.zip">nginx-1.27.3.zip</a>
            <a href="nginx-1.28.0.tar.gz">nginx-1.28.0.tar.gz</a>
            <a href="nginx-1.26.2.zip">nginx-1.26.2.zip</a>
        "#;
        // 1.28.0 is a tarball (excluded); highest .zip is 1.27.3.
        assert_eq!(highest_nginx_zip(body), Some((1, 27, 3)));
    }

    /// Live proof of the PHP version support: the scraped list offers 7.4 + 8.x,
    /// and a legacy build (7.4.33 — only in archives/, vc15 tag) downloads via the
    /// releases→archives fallback and yields a real php.exe.
    ///   cargo test installs_legacy_php_from_archives -- --ignored --nocapture
    #[tokio::test]
    #[ignore = "hits windows.php.net and downloads PHP 7.4"]
    async fn installs_legacy_php_from_archives() {
        let avail = super::get_php_available().await.expect("version list");
        assert!(avail.iter().any(|v| v.starts_with("7.4")), "should offer 7.4: {avail:?}");
        assert!(avail.iter().any(|v| v.starts_with("8.")), "should offer 8.x: {avail:?}");

        let dest = std::env::temp_dir().join("stackr-php74-test");
        let _ = std::fs::remove_dir_all(&dest);
        let mut ok = false;
        let mut last = String::new();
        for url in &php_zip_urls("7.4.33") {
            match crate::download::download_and_extract(url, &dest, |_, _| {}).await {
                Ok(()) => {
                    ok = true;
                    break;
                }
                Err(e) => last = e,
            }
        }
        assert!(ok, "7.4.33 download failed: {last}");
        assert!(dest.join("php.exe").exists(), "php.exe should exist after extract");
        let _ = std::fs::remove_dir_all(&dest);
    }
}
