//! Project lifecycle: create (folder + index + vhost + hosts), start (php-cgi +
//! web server + open browser), stop, delete, open helpers.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use super::services::{self, ProcessRegistry};
use crate::models::{Project, ProjectConfig};
use crate::state::{AppState, StateStore};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallProgress {
    step: String,
    percent: u32,
}

fn emit_progress(app: &AppHandle, step: &str, percent: u32) {
    let _ = app.emit(
        "project-install-progress",
        InstallProgress { step: step.into(), percent },
    );
}

/// Pick the PHP install dir for scaffolding: the requested version, else the
/// default, else any installed PHP. Returns `(php_dir, version)`.
fn resolve_php(st: &AppState, requested: &str) -> Result<(PathBuf, String), String> {
    let pick = st
        .installed
        .iter()
        .find(|c| c.component == "php" && c.version == requested)
        .or_else(|| {
            st.default_php
                .as_deref()
                .and_then(|v| st.installed.iter().find(|c| c.component == "php" && c.version == v))
        })
        .or_else(|| st.installed.iter().find(|c| c.component == "php"))
        .ok_or("no PHP runtime installed — install PHP on the PHP tab first")?;
    Ok((PathBuf::from(&pick.path), pick.version.clone()))
}

fn server_component(web_server: &str) -> String {
    web_server.to_ascii_lowercase()
}

/// Map a database choice to its engine family, or `None` for choices that need
/// no server-side database (SQLite, "None").
fn db_engine_kind(database: &str) -> Option<&'static str> {
    match database.trim().to_ascii_lowercase().as_str() {
        "mysql" | "mariadb" => Some("mysql"),
        "postgresql" | "postgres" => Some("postgresql"),
        _ => None,
    }
}

/// Numeric version compare ("8.4.0" > "8.0.36", "11.4.2" > "10.11.8").
fn db_version_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let parts = |s: &str| {
        s.split('.').map(|p| p.trim().parse::<u32>().unwrap_or(0)).collect::<Vec<u32>>()
    };
    parts(a).cmp(&parts(b))
}

/// Pick the installed engine version to use for a database family. Database
/// engines share a fixed port (3306/5432), so only one runs at a time — prefer
/// the **currently running** version (what the user started on the Databases tab);
/// otherwise the **newest installed**, with MariaDB preferred over MySQL for the
/// MySQL family. Returns `(component, version, install_dir)`.
fn resolve_db(st: &AppState, reg: &ProcessRegistry, kind: &str) -> Option<(String, String, PathBuf)> {
    let products: &[&str] = match kind {
        "mysql" => &["mariadb", "mysql"],
        "postgresql" => &["postgresql"],
        _ => return None,
    };
    let mut candidates: Vec<(String, String, PathBuf)> = st
        .installed
        .iter()
        .filter(|c| products.contains(&c.component.as_str()))
        .map(|c| (c.component.clone(), c.version.clone(), PathBuf::from(&c.path)))
        .collect();
    if candidates.is_empty() {
        return None;
    }

    // 1) The running version wins — it's the one the user started on the port.
    if let Ok(registry) = reg.0.lock() {
        if let Some(found) = candidates
            .iter()
            .find(|(comp, ver, _)| registry.is_running(&format!("{comp}-{ver}")))
        {
            return Some(found.clone());
        }
    }

    // 2) Otherwise newest installed, honoring product preference (MariaDB first).
    candidates.sort_by(|a, b| {
        let rank = |comp: &str| products.iter().position(|p| *p == comp).unwrap_or(usize::MAX);
        rank(&a.0).cmp(&rank(&b.0)).then_with(|| db_version_cmp(&b.1, &a.1))
    });
    candidates.into_iter().next()
}

/// Installed web servers as `(component, version)` (only one can run at a time).
pub(crate) fn installed_web_servers(st: &AppState) -> Vec<(String, String)> {
    st.installed
        .iter()
        .filter(|c| c.component == "nginx" || c.component == "apache")
        .map(|c| (c.component.clone(), c.version.clone()))
        .collect()
}

/// The web server service id currently in the process registry, if any.
fn running_server_id(installed: &[(String, String)], reg: &ProcessRegistry) -> Option<String> {
    let registry = reg.0.lock().ok()?;
    installed
        .iter()
        .map(|(c, v)| format!("{c}-{v}"))
        .find(|id| registry.is_running(id))
}

/// The server projects should use: the running one, else Nginx, else whichever
/// is installed. Web server is global now, not chosen per-project.
pub(crate) fn resolve_active_server_id(installed: &[(String, String)], reg: &ProcessRegistry) -> Option<String> {
    if installed.is_empty() {
        return None;
    }
    running_server_id(installed, reg).or_else(|| {
        installed
            .iter()
            .find(|(c, _)| c == "nginx")
            .or_else(|| installed.first())
            .map(|(c, v)| format!("{c}-{v}"))
    })
}

/// Remove a domain's vhost from both server config dirs (only one will exist).
fn remove_vhost_all(domain: &str) {
    let _ = crate::commands::config::remove_vhost("nginx".into(), domain.to_string());
    let _ = crate::commands::config::remove_vhost("apache".into(), domain.to_string());
}

fn now_ts() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default()
}

/// Minimal HTML-escape for the project name before it's interpolated into the
/// landing page (names are slug-ish but treat them as untrusted anyway).
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// `@font-face` rules embedding the welcome page's fonts (Geist + JetBrains Mono)
/// as base64 woff2, so the page renders the exact design **fully offline** — no
/// Google Fonts / CDN request. Fonts are baked into the binary via `include_bytes!`.
fn welcome_fontface_css() -> String {
    use base64::Engine;
    // (CSS family, weight, woff2 bytes) — only the weights the design actually uses.
    const FONTS: &[(&str, u16, &[u8])] = &[
        ("Geist", 400, include_bytes!("../../assets/fonts/geist-sans-latin-400-normal.woff2")),
        ("Geist", 600, include_bytes!("../../assets/fonts/geist-sans-latin-600-normal.woff2")),
        ("Geist", 800, include_bytes!("../../assets/fonts/geist-sans-latin-800-normal.woff2")),
        ("JetBrains Mono", 400, include_bytes!("../../assets/fonts/jetbrains-mono-latin-400-normal.woff2")),
        ("JetBrains Mono", 500, include_bytes!("../../assets/fonts/jetbrains-mono-latin-500-normal.woff2")),
        ("JetBrains Mono", 600, include_bytes!("../../assets/fonts/jetbrains-mono-latin-600-normal.woff2")),
    ];
    let mut css = String::new();
    for (family, weight, bytes) in FONTS {
        let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
        css.push_str(&format!(
            "@font-face{{font-family:'{family}';font-style:normal;font-weight:{weight};font-display:swap;src:url(data:font/woff2;base64,{b64}) format('woff2');}}\n"
        ));
    }
    css
}

/// Welcome page for a fresh blank project — matches the DESIGN/ "Project Welcome
/// v2" mock 1:1 (Stackr hex mark, status badge, PHP/server/doc-root stat cards).
/// Live values come from PHP at request time; the project name is interpolated.
/// Fonts are embedded (base64) so it renders offline — no external requests.
fn default_index(name: &str) -> String {
    LANDING_TEMPLATE
        .replace("__FONTS__", &welcome_fontface_css())
        .replace("__NAME__", &html_escape(name))
}

const LANDING_TEMPLATE: &str = r##"<?php
$php    = PHP_VERSION;
$server = $_SERVER['SERVER_SOFTWARE'] ?? 'PHP';
$root   = $_SERVER['DOCUMENT_ROOT'] ?? __DIR__;
?>
<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>__NAME__ · Stackr</title>
<style>
__FONTS__</style>
<style>
  * { box-sizing:border-box; }
  body { margin:0; min-height:100vh; background:#0a0c11; color:#e6e8ee; font-family:'Geist',-apple-system,BlinkMacSystemFont,sans-serif; -webkit-font-smoothing:antialiased; text-rendering:optimizeLegibility; }
  h1,h2,h3,p { margin:0; }
  a { text-decoration:none; color:inherit; }
  ::selection { background:rgba(79,127,255,.32); color:#fff; }
  @keyframes sw-pulse { 0%,100% { box-shadow:0 0 0 0 rgba(63,185,80,.45); } 55% { box-shadow:0 0 0 6px rgba(63,185,80,0); } }
  @keyframes sw-glow { 0%,100% { opacity:.5; transform:translate(-50%,-50%) scale(1); } 50% { opacity:.8; transform:translate(-50%,-50%) scale(1.08); } }
  @keyframes sw-rise { from { opacity:0; transform:translateY(18px); } to { opacity:1; transform:none; } }
  @keyframes sw-mark { from { opacity:0; transform:translateY(10px) scale(.9); } to { opacity:1; transform:none; } }
  @media (prefers-reduced-motion: no-preference){
    .sw-r1 { animation:sw-rise .6s cubic-bezier(.22,.7,.3,1) .05s both; }
    .sw-r2 { animation:sw-rise .6s cubic-bezier(.22,.7,.3,1) .14s both; }
    .sw-r3 { animation:sw-rise .6s cubic-bezier(.22,.7,.3,1) .24s both; }
    .sw-markel { animation:sw-mark .7s cubic-bezier(.22,.7,.3,1) both; }
  }
  @media (max-width:680px){ .sw-stats { grid-template-columns:1fr !important; } }
</style>
</head>
<body>
<div style="position:relative; min-height:100vh; display:flex; align-items:center; justify-content:center; padding:56px 24px; overflow:hidden;">

  <!-- background -->
  <div style="position:absolute; inset:0; background:
      radial-gradient(105% 75% at 50% -12%, rgba(79,127,255,.18), transparent 55%),
      #0a0c11; pointer-events:none;"></div>
  <div style="position:absolute; inset:0; opacity:.5; pointer-events:none;
      background-image:linear-gradient(rgba(255,255,255,.022) 1px,transparent 1px),linear-gradient(90deg,rgba(255,255,255,.022) 1px,transparent 1px);
      background-size:56px 56px;
      -webkit-mask-image:radial-gradient(115% 90% at 50% 28%, #000 32%, transparent 80%);
      mask-image:radial-gradient(115% 90% at 50% 28%, #000 32%, transparent 80%);"></div>

  <div style="position:relative; width:100%; max-width:640px; text-align:center;">

    <!-- logo mark -->
    <div class="sw-r1" style="position:relative; display:flex; justify-content:center; margin-bottom:30px;">
      <div style="position:absolute; top:50%; left:50%; width:150px; height:150px; border-radius:50%; background:radial-gradient(circle, rgba(79,127,255,.42), transparent 68%); filter:blur(10px); animation:sw-glow 4.5s ease-in-out infinite; pointer-events:none;"></div>
      <div class="sw-markel" style="position:relative; width:84px; height:84px; border-radius:22px; background:linear-gradient(155deg,#1a1f2b,#0e1016); border:1px solid #2a3142; display:flex; align-items:center; justify-content:center; box-shadow:0 16px 40px rgba(0,0,0,.55), 0 0 0 1px rgba(79,127,255,.08) inset;">
        <svg width="46" height="46" viewBox="0 0 48 48" fill="none" stroke-linecap="round" stroke-linejoin="round">
          <polygon points="24,5 40,14.5 40,33.5 24,43 8,33.5 8,14.5" fill="#4f7fff"></polygon>
          <polyline points="20.5,17 28.5,24 20.5,31" stroke="#0e1016" stroke-width="3.8"></polyline>
        </svg>
      </div>
    </div>

    <!-- status -->
    <div class="sw-r2" style="display:flex; justify-content:center; margin-bottom:22px;">
      <span style="display:inline-flex; align-items:center; gap:8px; background:rgba(63,185,80,.1); border:1px solid rgba(63,185,80,.28); color:#4cc763; border-radius:30px; padding:6px 14px 6px 11px; font:600 12px 'Geist';">
        <span style="width:7px; height:7px; border-radius:50%; background:#3fb950; animation:sw-pulse 2.2s infinite;"></span>Running
      </span>
    </div>

    <!-- title -->
    <h1 class="sw-r2" style="font:800 clamp(46px,9vw,72px)/1 'Geist'; letter-spacing:-.045em; color:#f3f5f9;">__NAME__</h1>
    <p class="sw-r2" style="font:400 16px/1.6 'Geist'; color:#8b91a0; margin:16px auto 0; max-width:400px; text-wrap:pretty;">Your PHP project is live and served by Stackr.</p>

    <!-- stats -->
    <div class="sw-stats sw-r3" style="display:grid; grid-template-columns:repeat(2,1fr); gap:10px; margin:38px 0 0; text-align:left;">
      <div style="background:#0d1016; border:1px solid #20242f; border-radius:13px; padding:16px 16px 15px;">
        <div style="display:flex; align-items:center; gap:8px; color:#7a9bff; margin-bottom:11px;">
          <span style="display:flex; align-items:center; justify-content:center; width:26px; height:26px; border-radius:7px; background:rgba(79,127,255,.1);"><svg width="14" height="14" viewBox="0 0 24 24" style="fill:none;stroke:currentColor;stroke-width:2;stroke-linecap:round;stroke-linejoin:round;"><polyline points="16 18 22 12 16 6"/><polyline points="8 6 2 12 8 18"/></svg></span>
          <span style="font:600 10.5px 'JetBrains Mono'; letter-spacing:.08em; text-transform:uppercase; color:#6f7686;">PHP version</span>
        </div>
        <div style="font:500 14px 'JetBrains Mono'; color:#e6e8ee; overflow-wrap:anywhere; line-height:1.4;"><?= htmlspecialchars($php) ?></div>
      </div>
      <div style="background:#0d1016; border:1px solid #20242f; border-radius:13px; padding:16px 16px 15px;">
        <div style="display:flex; align-items:center; gap:8px; color:#7a9bff; margin-bottom:11px;">
          <span style="display:flex; align-items:center; justify-content:center; width:26px; height:26px; border-radius:7px; background:rgba(79,127,255,.1);"><svg width="14" height="14" viewBox="0 0 24 24" style="fill:none;stroke:currentColor;stroke-width:2;stroke-linecap:round;stroke-linejoin:round;"><rect width="20" height="8" x="2" y="2" rx="2"/><rect width="20" height="8" x="2" y="14" rx="2"/><line x1="6" x2="6.01" y1="6" y2="6"/><line x1="6" x2="6.01" y1="18" y2="18"/></svg></span>
          <span style="font:600 10.5px 'JetBrains Mono'; letter-spacing:.08em; text-transform:uppercase; color:#6f7686;">Web server</span>
        </div>
        <div style="font:500 14px 'JetBrains Mono'; color:#e6e8ee; overflow-wrap:anywhere; line-height:1.4;"><?= htmlspecialchars($server) ?></div>
      </div>
      <div style="background:#0d1016; border:1px solid #20242f; border-radius:13px; padding:16px 16px 15px; grid-column:span 2;">
        <div style="display:flex; align-items:center; gap:8px; color:#7a9bff; margin-bottom:11px;">
          <span style="display:flex; align-items:center; justify-content:center; width:26px; height:26px; border-radius:7px; background:rgba(79,127,255,.1);"><svg width="14" height="14" viewBox="0 0 24 24" style="fill:none;stroke:currentColor;stroke-width:2;stroke-linecap:round;stroke-linejoin:round;"><path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"/></svg></span>
          <span style="font:600 10.5px 'JetBrains Mono'; letter-spacing:.08em; text-transform:uppercase; color:#6f7686;">Document root</span>
        </div>
        <div style="font:500 14px 'JetBrains Mono'; color:#e6e8ee; overflow-wrap:anywhere; line-height:1.4;"><?= htmlspecialchars($root) ?></div>
      </div>
    </div>

    <!-- footer hint -->
    <div class="sw-r3" style="display:inline-flex; align-items:center; gap:9px; margin-top:30px; font:400 12.5px 'Geist'; color:#6f7686;">
      <span>Edit <span style="font-family:'JetBrains Mono'; color:#9aa1ae; font-size:12px;">index.php</span> to get started</span>
      <span style="width:3px; height:3px; border-radius:50%; background:#3a4150;"></span>
      <span style="display:inline-flex; align-items:center; gap:6px;">
        <svg width="13" height="13" viewBox="0 0 48 48" fill="none" stroke-linecap="round" stroke-linejoin="round"><polygon points="24,5 40,14.5 40,33.5 24,43 8,33.5 8,14.5" fill="#4f7fff"></polygon><polyline points="20.5,17 28.5,24 20.5,31" stroke="#0a0c11" stroke-width="4.4"></polyline></svg>
        Powered by Stackr
      </span>
    </div>

  </div>
</div>
</body>
</html>
"##;

/// Block until the web server actually serves `domain` on :80, or `timeout`
/// elapses. After a fresh spawn (and especially after `nginx -s reload`, which
/// swaps workers asynchronously) there's a brief window where the request still
/// hits the catch-all (the vhost isn't loaded yet) or php-cgi hasn't bound (502).
/// We must NOT treat an *application's own* 404 as "not ready" — a fresh
/// framework (e.g. an empty Symfony skeleton) legitimately returns 404 for `/`,
/// and polling that for the full timeout wastes seconds and floods the error log.
/// So the only "not ready" signals are: connection refused, a 502/503 (php-cgi
/// warming up), or nginx's *own* catch-all 404 page (identified by its
/// "<center>nginx</center>" footer). Any real upstream response — including an
/// app 404 — means the site is live. Best-effort: returns after `timeout`
/// regardless. Pure std (no extra deps) — a minimal HTTP/1.0 probe.
fn wait_until_served(domain: &str, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    let req = format!("GET / HTTP/1.0\r\nHost: {domain}\r\nConnection: close\r\n\r\n");
    while Instant::now() < deadline {
        if let Ok(mut s) = TcpStream::connect(("127.0.0.1", 80)) {
            let _ = s.set_read_timeout(Some(Duration::from_millis(1500)));
            let _ = s.set_write_timeout(Some(Duration::from_millis(1000)));
            if s.write_all(req.as_bytes()).is_ok() {
                // HTTP/1.0 + Connection: close → server closes after the (small)
                // response, so read to EOF, capped so a big page can't stall us.
                let mut buf = Vec::new();
                let mut chunk = [0u8; 2048];
                loop {
                    match s.read(&mut chunk) {
                        Ok(0) => break,
                        Ok(n) => {
                            buf.extend_from_slice(&chunk[..n]);
                            if buf.len() >= 8192 {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                if !buf.is_empty() {
                    let resp = String::from_utf8_lossy(&buf);
                    let status = resp.lines().next().unwrap_or("");
                    let warming = status.contains(" 502") || status.contains(" 503");
                    // nginx's catch-all `return 404` renders nginx's default error
                    // page, whose footer is "<center>nginx…</center>"; an app 404
                    // has no such marker.
                    let nginx_catchall = status.contains(" 404") && resp.contains("<center>nginx");
                    if !warming && !nginx_catchall {
                        return;
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_millis(120));
    }
}

pub(crate) fn open_url(url: &str) {
    let mut cmd = Command::new("cmd");
    cmd.args(["/c", "start", "", url]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let _ = cmd.spawn();
}

fn open_path(program: &str, path: &str) -> Result<(), String> {
    let mut cmd = Command::new(program);
    cmd.arg(path);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Detect a project's document root inside `base` by looking for the conventional
/// public directory of common PHP frameworks; empty string means the root itself.
pub(crate) fn detect_doc_root_dir(base: &std::path::Path) -> String {
    for sub in ["public", "web", "webroot"] {
        if base.join(sub).is_dir() {
            return sub.to_string();
        }
    }
    String::new()
}

/// Suggested document-root subdirectory for a folder the user is about to open
/// (so the import UI can prefill it). Empty string = serve the folder root.
#[tauri::command]
pub fn detect_doc_root(path: String) -> String {
    detect_doc_root_dir(&PathBuf::from(&path))
}

/// Resolve a project's document root from its stored path + framework/type.
fn project_public_dir(p: &Project) -> PathBuf {
    let base = PathBuf::from(&p.path);
    // An explicit doc root (set when opening an existing folder) always wins.
    if let Some(sub) = p.doc_root.as_deref() {
        let sub = sub.trim().trim_matches(|c| c == '\\' || c == '/');
        return if sub.is_empty() { base } else { base.join(sub) };
    }
    match p.r#type.as_str() {
        "Clone from Git" => {
            if base.join("public").is_dir() {
                base.join("public")
            } else {
                base
            }
        }
        _ => {
            let sub = crate::scaffold::doc_root_subdir(p.framework.as_deref(), &p.r#type);
            if sub.is_empty() {
                base
            } else {
                base.join(sub)
            }
        }
    }
}

pub(crate) fn write_vhost_file(
    server: &str,
    domain: &str,
    public: &PathBuf,
    fcgi_port: u16,
    https: bool,
) -> Result<(), String> {
    // When HTTPS is on, mint the per-domain cert and hand its paths to the vhost.
    let tls_paths = if https {
        crate::tls::ensure_domain_cert(domain)?;
        Some((crate::paths::domain_cert(domain), crate::paths::domain_key(domain)))
    } else {
        None
    };
    let tls = tls_paths.as_ref().map(|(c, k)| (c.as_path(), k.as_path()));

    let (dir, content) = match server {
        "nginx" => (
            crate::paths::nginx_sites_dir(),
            crate::config_gen::nginx_vhost(domain, public, 80, fcgi_port, tls),
        ),
        "apache" => {
            let dir = crate::paths::apache_sites_dir();
            // Apache needs mod_ssl + Listen 443 loaded before any :443 vhost. Drop
            // (or remove) the managed bootstrap alongside the vhosts.
            crate::paths::ensure_dir(&dir).map_err(|e| e.to_string())?;
            let bootstrap = dir.join("_ssl.conf");
            if https {
                std::fs::write(&bootstrap, crate::config_gen::apache_ssl_bootstrap())
                    .map_err(|e| e.to_string())?;
            } else {
                let _ = std::fs::remove_file(&bootstrap);
            }
            (dir, crate::config_gen::apache_vhost(domain, public, 80, fcgi_port, tls))
        }
        other => return Err(format!("unknown web server '{other}'")),
    };
    crate::paths::ensure_dir(&dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(format!("{domain}.conf")), content).map_err(|e| e.to_string())
}

/// Remove vhost files for domains that are no longer projects, so leftovers from
/// old/failed deletes or manual edits don't linger and serve a stale FastCGI port
/// (the source of the cross-host 502s). The Adminer tool domain is preserved — it
/// regenerates its own vhost on demand. Run at startup before any server starts,
/// so no reload is needed. Hosts entries are left alone (harmless: an unmatched
/// host now hits the catch-all 404, and removing them needs elevation).
pub(crate) fn prune_orphan_sites(projects: &[Project]) {
    use std::collections::HashSet;
    let mut keep: HashSet<String> = projects.iter().map(|p| p.domain.to_ascii_lowercase()).collect();
    keep.insert("adminer.test".into());

    for dir in [crate::paths::nginx_sites_dir(), crate::paths::apache_sites_dir()] {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("conf") {
                continue;
            }
            let orphan = path
                .file_stem()
                .and_then(|s| s.to_str())
                // Files starting with `_` are Stackr-managed includes (e.g.
                // `_ssl.conf`), not project vhosts — never prune them.
                .map(|stem| !stem.starts_with('_') && !keep.contains(&stem.to_ascii_lowercase()))
                .unwrap_or(false);
            if orphan {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
}

#[tauri::command]
pub fn get_projects(state: State<'_, StateStore>) -> Result<Vec<Project>, String> {
    let st = state.0.lock().map_err(|e| e.to_string())?;
    Ok(st.projects.clone())
}

#[tauri::command]
pub async fn create_project(
    app: AppHandle,
    state: State<'_, StateStore>,
    reg: State<'_, ProcessRegistry>,
    config: ProjectConfig,
) -> Result<Project, String> {
    let name = config.name.trim().to_string();
    if name.is_empty() {
        return Err("project name is required".into());
    }

    // If a server-backed database is requested, its engine must be installed —
    // fail early (before scaffolding) with a clear, actionable message.
    let db_choice = config
        .database
        .clone()
        .filter(|d| !d.trim().is_empty() && !d.eq_ignore_ascii_case("none"));
    if let Some(ref d) = db_choice {
        if let Some(kind) = db_engine_kind(d) {
            let st = state.0.lock().map_err(|e| e.to_string())?;
            if resolve_db(&st, &reg, kind).is_none() {
                return Err(format!(
                    "{d} is selected but no matching database engine is installed — install it on the Databases tab, or choose None"
                ));
            }
        }
    }
    let path_str = if config.path.trim().is_empty() {
        let base = {
            let st = state.0.lock().map_err(|e| e.to_string())?;
            st.settings.sites_dir.clone()
        };
        PathBuf::from(&base).join(&name).to_string_lossy().to_string()
    } else {
        config.path.clone()
    };
    let path = PathBuf::from(&path_str);
    let ptype = config.r#type.clone();
    let framework = config.framework.clone();

    emit_progress(&app, "Creating project folder", 8);
    // Ensure the parent sites directory exists (configurable, may differ from www_root).
    if let Some(parent) = path.parent() {
        crate::paths::ensure_dir(parent).map_err(|e| e.to_string())?;
    }

    // Framework/Git scaffolding needs a clean target — Composer's create-project
    // and `git clone` both refuse a non-empty directory. A previous *failed*
    // attempt can leave a partial folder (Composer writes the skeleton before it
    // fails resolving deps), and since the project is only recorded *after* a
    // successful scaffold, that leftover has no project entry to delete. Clear it
    // on retry — but never wipe an existing, recorded project of the same name.
    if matches!(ptype.as_str(), "Framework" | "Clone from Git") && path.exists() {
        let recorded = {
            let st = state.0.lock().map_err(|e| e.to_string())?;
            st.projects.iter().any(|p| p.id == name)
        };
        if recorded {
            return Err(format!(
                "A project named \"{name}\" already exists — delete it first, or choose another name."
            ));
        }
        std::fs::remove_dir_all(&path)
            .map_err(|e| format!("could not clear the leftover folder at {}: {e}", path.display()))?;
    }

    // --- scaffold by project type ---
    match ptype.as_str() {
        "Framework" => match framework.as_deref() {
            Some("WordPress") => {
                emit_progress(&app, "Downloading WordPress", 30);
                crate::download::download_and_extract(
                    "https://wordpress.org/latest.zip",
                    &path,
                    |_, _| {},
                )
                .await?;
            }
            Some(fw) => {
                let base = crate::scaffold::composer_package(fw)
                    .ok_or_else(|| format!("{fw} can't be scaffolded via Composer yet"))?;
                // Pin the requested major via a Composer version constraint (e.g.
                // "laravel/laravel:^11"); empty/None installs the latest stable.
                let package = match config.framework_version.as_deref().map(str::trim) {
                    Some(c) if !c.is_empty() => format!("{base}:{c}"),
                    _ => base.to_string(),
                };
                let package = package.as_str();
                let (php_dir, _v) = {
                    let st = state.0.lock().map_err(|e| e.to_string())?;
                    resolve_php(&st, &config.php_version)?
                };
                let php_exe = php_dir.join("php.exe");

                emit_progress(&app, "Installing Composer", 22);
                let phar = crate::scaffold::ensure_composer().await?;

                emit_progress(&app, &format!("Setting up {fw}"), 45);
                let (dest, pkg) = (path.clone(), package.to_string());
                tokio::task::spawn_blocking(move || {
                    crate::scaffold::run_composer_create(&php_exe, &php_dir, &phar, &pkg, &dest)
                })
                .await
                .map_err(|e| e.to_string())??;

                // Laravel ships an .env via post-create scripts; ensure it exists.
                if fw == "Laravel" {
                    let (env, example) = (path.join(".env"), path.join(".env.example"));
                    if !env.exists() && example.exists() {
                        let _ = std::fs::copy(&example, &env);
                    }
                }
            }
            None => return Err("framework not specified".into()),
        },
        "Clone from Git" => {
            let url = config
                .git_url
                .clone()
                .filter(|u| !u.trim().is_empty())
                .ok_or("a Git repository URL is required")?;
            // Prefer system git; fall back to a portable MinGit downloaded on
            // demand, so cloning works with no git installed.
            emit_progress(&app, "Preparing Git", 18);
            let git = crate::scaffold::ensure_git().await?;
            emit_progress(&app, "Cloning repository", 30);
            let dest = path.clone();
            let url_cl = url.clone();
            let git_cl = git.clone();
            tokio::task::spawn_blocking(move || crate::scaffold::clone_git(&git_cl, &url_cl, &dest))
                .await
                .map_err(|e| e.to_string())??;

            if path.join("composer.json").exists() {
                let (php_dir, _v) = {
                    let st = state.0.lock().map_err(|e| e.to_string())?;
                    resolve_php(&st, &config.php_version)?
                };
                let php_exe = php_dir.join("php.exe");
                emit_progress(&app, "Installing Composer", 55);
                let phar = crate::scaffold::ensure_composer().await?;
                emit_progress(&app, "Installing dependencies", 68);
                let dest = path.clone();
                tokio::task::spawn_blocking(move || {
                    crate::scaffold::run_composer_install(&php_exe, &php_dir, &phar, &dest)
                })
                .await
                .map_err(|e| e.to_string())??;
            }
        }
        "Open existing" => {
            // Import a project that already lives on disk — never scaffold or write
            // into it. Just validate the folder, and if it's a Composer project
            // missing its deps, install them so it can actually run.
            if !path.is_dir() {
                return Err(format!("folder does not exist: {}", path.display()));
            }
            {
                let st = state.0.lock().map_err(|e| e.to_string())?;
                if st.projects.iter().any(|p| p.id == name) {
                    return Err(format!(
                        "A project named \"{name}\" already exists — choose another name."
                    ));
                }
            }
            if path.join("composer.json").exists() && !path.join("vendor").exists() {
                let (php_dir, _v) = {
                    let st = state.0.lock().map_err(|e| e.to_string())?;
                    resolve_php(&st, &config.php_version)?
                };
                let php_exe = php_dir.join("php.exe");
                emit_progress(&app, "Installing Composer", 40);
                let phar = crate::scaffold::ensure_composer().await?;
                emit_progress(&app, "Installing dependencies", 62);
                let dest = path.clone();
                tokio::task::spawn_blocking(move || {
                    crate::scaffold::run_composer_install(&php_exe, &php_dir, &phar, &dest)
                })
                .await
                .map_err(|e| e.to_string())??;
            }
        }
        _ => {
            // Blank PHP starter.
            let public = path.join("public");
            crate::paths::ensure_dir(&public).map_err(|e| e.to_string())?;
            let index = public.join("index.php");
            if !index.exists() {
                std::fs::write(&index, default_index(&name)).map_err(|e| e.to_string())?;
            }
        }
    }

    // Server is no longer chosen per-project — record the installed web server
    // (Nginx preferred). The actual server is resolved live at start time.
    let web_server = {
        let st = state.0.lock().map_err(|e| e.to_string())?;
        if st.installed.iter().any(|c| c.component == "nginx") {
            "Nginx".to_string()
        } else if st.installed.iter().any(|c| c.component == "apache") {
            "Apache".to_string()
        } else {
            config.web_server.clone()
        }
    };

    // The vhost is written when the project is *started* (vhost present ⟺ the
    // site is being served), so a freshly-created project stays "stopped" until
    // the user starts it.
    emit_progress(&app, &format!("Configuring {}", server_component(&web_server)), 88);

    let git_url = config.git_url.clone().filter(|u| !u.trim().is_empty());
    let project = Project {
        id: name.clone(),
        name,
        r#type: config.r#type,
        framework: config.framework,
        php_version: config.php_version,
        web_server,
        database: config.database,
        domain: config.domain.clone(),
        path: path_str,
        status: "stopped".into(),
        git_url,
        created_at: now_ts(),
        doc_root: config.doc_root.clone(),
    };

    {
        let mut st = state.0.lock().map_err(|e| e.to_string())?;
        st.projects.retain(|p| p.id != project.id);
        st.projects.push(project.clone());
        st.save()?;
    }

    // --- per-project database: create a schema named after the project ---
    if let Some(ref d) = db_choice {
        if let Some(kind) = db_engine_kind(d) {
            let (comp, ver, dir) = {
                let st = state.0.lock().map_err(|e| e.to_string())?;
                resolve_db(&st, &reg, kind).ok_or_else(|| format!("{d} engine is no longer installed"))?
            };
            let db_name = crate::db::sanitize_db_name(&project.name);
            emit_progress(&app, &format!("Creating database {db_name}"), 92);

            // Bring the engine up (it must accept connections to create the DB).
            let id = format!("{comp}-{ver}");
            let spawned = services::ensure_started(&app, &reg, &id)?;

            let dir2 = dir.clone();
            let db_name2 = db_name.clone();
            let create = tokio::task::spawn_blocking(move || match kind {
                "postgresql" => crate::db::create_postgres_database(&dir2, 5432, &db_name2),
                _ => crate::db::create_mysql_database(&dir2, 3306, &db_name2),
            })
            .await
            .map_err(|e| e.to_string())?;

            // Leave the engine as we found it: if we started it just for this,
            // stop it again so creating a project doesn't silently run a service.
            if spawned {
                let _ = services::ensure_stopped(&app, &reg, &id);
            }
            create?;

            // Point a Laravel project's .env at the schema we just created —
            // Laravel defaults to sqlite, so otherwise the new database goes unused.
            if framework.as_deref() == Some("Laravel") {
                let _ = crate::scaffold::configure_laravel_env_db(&path, kind, &db_name);
            }
        }
    }

    // Best-effort: register the .test domain (may prompt for elevation).
    emit_progress(&app, "Registering domain", 95);
    let _ = crate::hosts::add_host(&config.domain);
    emit_progress(&app, "Done", 100);

    Ok(project)
}

#[tauri::command]
pub fn start_project(
    app: AppHandle,
    state: State<'_, StateStore>,
    reg: State<'_, ProcessRegistry>,
    id: String,
) -> Result<(), String> {
    let (project, installed_servers, php_version, db_id, https) = {
        let st = state.0.lock().map_err(|e| e.to_string())?;
        let p = st.projects.iter().find(|p| p.id == id).ok_or("project not found")?.clone();
        let servers = installed_web_servers(&st);
        // Serve with the project's OWN PHP version (falling back to the default,
        // then any installed) — so each project runs on its chosen version.
        let (_, php) = resolve_php(&st, &p.php_version)?;
        // The project's database engine, if it uses one (so its DB is up too).
        let db_id = p
            .database
            .as_deref()
            .and_then(db_engine_kind)
            .and_then(|kind| resolve_db(&st, &reg, kind))
            .map(|(comp, ver, _)| format!("{comp}-{ver}"));
        (p, servers, php, db_id, st.settings.https)
    };

    // Active web server = running one, else Nginx, else whichever is installed.
    // (Server is global now, not chosen per-project; only one can own :80.)
    let server_id = resolve_active_server_id(&installed_servers, &reg)
        .ok_or("no web server installed — install Nginx or Apache first")?;

    // Bring up this project's PHP runtime on its own FastCGI port, then (re)write
    // its vhost pointing at that port — so it's served by its own PHP version.
    // The web server's master config (which includes the vhosts) is regenerated
    // by spawn on a fresh start, or by reload_server when it's already running.
    let fcgi_port = services::ensure_php_runtime(&reg, &php_version)?;
    let public = project_public_dir(&project);
    // Write the vhost for EVERY installed web server (not only the active one), so
    // switching the global server later never leaves this project serving a 404.
    for (comp, _ver) in &installed_servers {
        write_vhost_file(comp, &project.domain, &public, fcgi_port, https)?;
    }
    let spawned = services::ensure_started(&app, &reg, &server_id)?;
    if !spawned {
        // Server was already running — reload so THIS project's vhost is applied
        // (otherwise the server keeps serving the default/first site).
        services::reload_server(&app, &reg, &server_id)?;
    }

    // Bring the project's database engine up too (best-effort — the schema was
    // created at project-creation time; here we just ensure the server is running).
    if let Some(db_id) = &db_id {
        let _ = services::ensure_started(&app, &reg, db_id);
    }

    {
        let mut st = state.0.lock().map_err(|e| e.to_string())?;
        if let Some(p) = st.projects.iter_mut().find(|p| p.id == id) {
            p.status = "running".into();
        }
        st.save()?;
    }

    // Wait for the server to actually serve this vhost before opening the browser,
    // so a fresh start / reload doesn't briefly show the catch-all 404 (or a 502
    // while php-cgi is still binding). Bounded — opens anyway after the timeout.
    wait_until_served(&project.domain, Duration::from_secs(6));
    let scheme = if https { "https" } else { "http" };
    open_url(&format!("{scheme}://{}", project.domain));
    Ok(())
}

/// Change a project's PHP version. Persists immediately; if the project is
/// running, brings up the new version's php-cgi, re-points the vhost at its port
/// and reloads the web server so the switch takes effect live.
#[tauri::command]
pub fn set_project_php(
    app: AppHandle,
    state: State<'_, StateStore>,
    reg: State<'_, ProcessRegistry>,
    id: String,
    version: String,
) -> Result<(), String> {
    let (running, project, installed_servers, https) = {
        let mut st = state.0.lock().map_err(|e| e.to_string())?;
        if !st.installed.iter().any(|c| c.component == "php" && c.version == version) {
            return Err(format!("PHP {version} is not installed"));
        }
        let servers = installed_web_servers(&st);
        let https = st.settings.https;
        let p = st.projects.iter_mut().find(|p| p.id == id).ok_or("project not found")?;
        p.php_version = version.clone();
        let running = p.status == "running";
        let project = p.clone();
        st.save()?;
        (running, project, servers, https)
    };

    if running {
        let fcgi_port = services::ensure_php_runtime(&reg, &version)?;
        if let Some(server_id) = resolve_active_server_id(&installed_servers, &reg) {
            let server = server_id.split('-').next().unwrap_or("nginx").to_string();
            let public = project_public_dir(&project);
            write_vhost_file(&server, &project.domain, &public, fcgi_port, https)?;
            services::reload_server(&app, &reg, &server_id)?;
        }
    }
    Ok(())
}

/// Change a project's database engine. Persists immediately; if a server-backed
/// engine is chosen it must be installed, and a schema named after the project is
/// (idempotently) created. The previous database is left intact — not dropped.
#[tauri::command]
pub async fn set_project_db(
    app: AppHandle,
    state: State<'_, StateStore>,
    reg: State<'_, ProcessRegistry>,
    id: String,
    database: Option<String>,
) -> Result<(), String> {
    let choice = database
        .clone()
        .filter(|d| !d.trim().is_empty() && !d.eq_ignore_ascii_case("none"));
    let kind = choice.as_deref().and_then(db_engine_kind);

    let (project, engine) = {
        let mut st = state.0.lock().map_err(|e| e.to_string())?;
        // A server-backed engine must be installed before we record/use it.
        let engine = match kind {
            Some(k) => Some(resolve_db(&st, &reg, k).ok_or_else(|| {
                format!(
                    "{} is selected but no matching engine is installed — install it on the Databases tab",
                    choice.as_deref().unwrap_or("That database")
                )
            })?),
            None => None,
        };
        let p = st.projects.iter_mut().find(|p| p.id == id).ok_or("project not found")?;
        p.database = choice.clone();
        let project = p.clone();
        st.save()?;
        (project, engine)
    };

    if let (Some(kind), Some((comp, ver, dir))) = (kind, engine) {
        let db_name = crate::db::sanitize_db_name(&project.name);
        let svc_id = format!("{comp}-{ver}");
        let spawned = services::ensure_started(&app, &reg, &svc_id)?;
        let (dir2, db2) = (dir.clone(), db_name.clone());
        let create = tokio::task::spawn_blocking(move || match kind {
            "postgresql" => crate::db::create_postgres_database(&dir2, 5432, &db2),
            _ => crate::db::create_mysql_database(&dir2, 3306, &db2),
        })
        .await
        .map_err(|e| e.to_string())?;
        // Leave run-state as we found it: if we only started the engine to create
        // the schema (project not running), stop it again.
        if spawned && project.status != "running" {
            let _ = services::ensure_stopped(&app, &reg, &svc_id);
        }
        create?;

        // Keep a Laravel project's .env in sync with the newly-selected database.
        if project.framework.as_deref() == Some("Laravel") {
            let _ = crate::scaffold::configure_laravel_env_db(
                std::path::Path::new(&project.path),
                kind,
                &db_name,
            );
        }
    }
    Ok(())
}

#[tauri::command]
pub fn stop_project(
    app: AppHandle,
    state: State<'_, StateStore>,
    reg: State<'_, ProcessRegistry>,
    id: String,
) -> Result<(), String> {
    let (domain, installed_servers) = {
        let st = state.0.lock().map_err(|e| e.to_string())?;
        let p = st.projects.iter().find(|p| p.id == id).ok_or("project not found")?;
        (p.domain.clone(), installed_web_servers(&st))
    };

    // Drop this project's vhost so its domain stops being served.
    remove_vhost_all(&domain);

    let any_running = {
        let mut st = state.0.lock().map_err(|e| e.to_string())?;
        if let Some(p) = st.projects.iter_mut().find(|p| p.id == id) {
            p.status = "stopped".into();
        }
        st.save()?;
        st.projects.iter().any(|p| p.status == "running")
    };

    let server_id = running_server_id(&installed_servers, &reg);
    if any_running {
        // Other sites still up — reload the running server so the vhost drops out.
        if let Some(server_id) = &server_id {
            let _ = services::reload_server(&app, &reg, server_id);
        }
    } else {
        // Nothing left running — stop the web server + all php-cgi runtimes.
        if let Some(server_id) = &server_id {
            let _ = services::ensure_stopped(&app, &reg, server_id);
        }
        let _ = services::stop_all_php_runtimes(&reg);
    }
    Ok(())
}

#[tauri::command]
pub fn open_project_folder(state: State<'_, StateStore>, id: String) -> Result<(), String> {
    let path = {
        let st = state.0.lock().map_err(|e| e.to_string())?;
        st.projects.iter().find(|p| p.id == id).map(|p| p.path.clone()).ok_or("project not found")?
    };
    open_path("explorer", &path)
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdeInfo {
    id: String,
    name: String,
}

/// IDEs Stackr knows how to detect + launch, in display order.
const IDES: &[(&str, &str)] = &[
    ("vscode", "VS Code"),
    ("vscode-insiders", "VS Code Insiders"),
    ("cursor", "Cursor"),
    ("phpstorm", "PhpStorm"),
    ("sublime", "Sublime Text"),
];

/// First non-empty line of `where <cmd>`, i.e. the command's path if on PATH.
fn on_path(cmd: &str) -> Option<PathBuf> {
    let mut c = Command::new("where");
    c.arg(cmd);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    let out = c.output().ok()?;
    if !out.status.success() {
        return None;
    }
    pick_launcher(&String::from_utf8_lossy(&out.stdout))
}

/// From `where` output, prefer a directly-runnable launcher (.exe/.cmd/.bat) over
/// the extensionless shell shim it often lists first (e.g. VS Code's `bin\code`).
fn pick_launcher(stdout: &str) -> Option<PathBuf> {
    let lines: Vec<&str> = stdout.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    lines
        .iter()
        .find(|l| {
            let lower = l.to_ascii_lowercase();
            lower.ends_with(".exe") || lower.ends_with(".cmd") || lower.ends_with(".bat")
        })
        .or_else(|| lines.first())
        .map(PathBuf::from)
}

/// Resolve an IDE id to a launchable exe/script: PATH first, then known install
/// locations. `None` means it isn't installed.
fn resolve_ide(id: &str) -> Option<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let pf = std::env::var_os("ProgramFiles").map(PathBuf::from);
    let at = |base: &Option<PathBuf>, rest: &str| base.as_ref().map(|b| b.join(rest));

    let (cmds, paths): (&[&str], Vec<Option<PathBuf>>) = match id {
        "vscode" => (
            &["code"],
            vec![
                at(&local, "Programs/Microsoft VS Code/Code.exe"),
                at(&pf, "Microsoft VS Code/Code.exe"),
            ],
        ),
        "vscode-insiders" => (
            &["code-insiders"],
            vec![at(&local, "Programs/Microsoft VS Code Insiders/Code - Insiders.exe")],
        ),
        "cursor" => (&["cursor"], vec![at(&local, "Programs/cursor/Cursor.exe")]),
        "phpstorm" => (
            &["phpstorm64", "phpstorm"],
            vec![at(&local, "JetBrains/Toolbox/scripts/phpstorm.cmd")],
        ),
        "sublime" => (&["subl"], vec![at(&pf, "Sublime Text/sublime_text.exe")]),
        _ => (&[], vec![]),
    };
    for cmd in cmds {
        if let Some(p) = on_path(cmd) {
            return Some(p);
        }
    }
    paths.into_iter().flatten().find(|p| p.exists())
}

/// IDEs detected as installed on this machine (for the "Open in IDE" picker).
/// Async + off-thread: it spawns several `where` lookups, which must not block
/// the UI event loop.
#[tauri::command]
pub async fn detect_ides() -> Vec<IdeInfo> {
    tauri::async_runtime::spawn_blocking(|| {
        IDES.iter()
            .filter(|(id, _)| resolve_ide(id).is_some())
            .map(|(id, name)| IdeInfo { id: id.to_string(), name: name.to_string() })
            .collect()
    })
    .await
    .unwrap_or_default()
}

/// Open a project's folder in the chosen IDE.
#[tauri::command]
pub fn open_in_ide(state: State<'_, StateStore>, id: String, ide: String) -> Result<(), String> {
    let path = {
        let st = state.0.lock().map_err(|e| e.to_string())?;
        st.projects.iter().find(|p| p.id == id).map(|p| p.path.clone()).ok_or("project not found")?
    };
    let exe = resolve_ide(&ide).ok_or_else(|| format!("{ide} is not installed"))?;

    // JetBrains Toolbox launchers are .cmd scripts → run through cmd.
    let ext = exe.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let mut cmd = if ext == "cmd" || ext == "bat" {
        let mut c = Command::new("cmd");
        c.args(["/c", &exe.to_string_lossy(), &path]);
        c
    } else {
        let mut c = Command::new(&exe);
        c.arg(&path);
        c
    };
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Open a terminal in the project's folder with `php`, `composer`, and `git` on
/// PATH (project's PHP first) without touching the system PATH. Not bound to the
/// kill-on-close job, so the terminal outlives Stackr.
#[tauri::command]
pub async fn open_terminal(state: State<'_, StateStore>, id: String) -> Result<(), String> {
    let (path, php_dir) = {
        let st = state.0.lock().map_err(|e| e.to_string())?;
        let p = st.projects.iter().find(|p| p.id == id).ok_or("project not found")?;
        (p.path.clone(), resolve_php(&st, &p.php_version)?.0)
    };

    // Best-effort so `composer` works in the shell — never fail terminal-open if
    // the download is unavailable (offline); the shim is harmless without it.
    let _ = crate::scaffold::ensure_composer().await;
    crate::scaffold::ensure_composer_shim();

    // Augmented PATH: project PHP + Composer shim (+ portable git if present),
    // then the inherited PATH.
    let mut parts = vec![
        php_dir.to_string_lossy().to_string(),
        crate::scaffold::composer_dir().to_string_lossy().to_string(),
    ];
    if let Some(git) = crate::scaffold::portable_git_cmd_dir() {
        parts.push(git.to_string_lossy().to_string());
    }
    if let Ok(existing) = std::env::var("PATH") {
        parts.push(existing);
    }

    // Launch via a hidden `cmd /c start "" cmd /k …` rather than spawning cmd with
    // CREATE_NEW_CONSOLE directly: as a GUI (console-less) process, Stackr's std
    // handles are null, and Rust's default inherit passes them with
    // STARTF_USESTDHANDLES — so a directly-spawned cmd gets a null stdin, `/k` hits
    // EOF and the window flashes shut. `start` gives the new cmd a real console.
    // The window (from `start`) isn't a child of the job, so it outlives Stackr.
    let mut cmd = Command::new("cmd");
    cmd.current_dir(&path)
        .args([
            "/c",
            "start",
            "", // empty window title (start treats the first quoted arg as the title)
            "cmd",
            "/k",
            "title Stackr - php, composer, git on PATH",
        ])
        .env("PATH", parts.join(";"))
        .env("COMPOSER_HOME", crate::scaffold::composer_dir().join("home"));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Hide the transient outer cmd; `start` opens the real, visible terminal.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_project(
    app: AppHandle,
    state: State<'_, StateStore>,
    reg: State<'_, ProcessRegistry>,
    id: String,
    delete_files: bool,
) -> Result<(), String> {
    let (domain, path, installed_servers) = {
        let st = state.0.lock().map_err(|e| e.to_string())?;
        let p = st.projects.iter().find(|p| p.id == id).ok_or("project not found")?;
        (p.domain.clone(), p.path.clone(), installed_web_servers(&st))
    };

    remove_vhost_all(&domain);
    let _ = crate::hosts::remove_host(&domain);

    let any_running = {
        let mut st = state.0.lock().map_err(|e| e.to_string())?;
        st.projects.retain(|p| p.id != id);
        st.save()?;
        st.projects.iter().any(|p| p.status == "running")
    };

    // Same teardown as stopping: reload the server if other sites remain, else
    // shut the (now idle) web server + php-cgi runtimes down.
    let server_id = running_server_id(&installed_servers, &reg);
    if any_running {
        if let Some(server_id) = &server_id {
            let _ = services::reload_server(&app, &reg, server_id);
        }
    } else if let Some(server_id) = &server_id {
        let _ = services::ensure_stopped(&app, &reg, server_id);
        let _ = services::stop_all_php_runtimes(&reg);
    } else {
        let _ = services::stop_all_php_runtimes(&reg);
    }

    // Optionally wipe the project folder — done last, after the server has
    // released the files (vhost removed + reloaded/stopped above).
    if delete_files && !path.trim().is_empty() {
        let dir = std::path::PathBuf::from(&path);
        if dir.exists() {
            std::fs::remove_dir_all(&dir)
                .map_err(|e| format!("project removed, but deleting its files failed: {e}"))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::pick_launcher;

    #[test]
    fn prefers_runnable_launcher_over_shim() {
        // `where code` lists the extensionless bash shim first; we must pick .cmd.
        let out = "C:\\Users\\x\\AppData\\Local\\Programs\\Microsoft VS Code\\bin\\code\nC:\\Users\\x\\AppData\\Local\\Programs\\Microsoft VS Code\\bin\\code.cmd\n";
        assert!(pick_launcher(out).unwrap().to_string_lossy().ends_with("code.cmd"));
    }

    #[test]
    fn falls_back_to_first_when_no_known_ext() {
        let out = "C:\\tools\\thing.exe\n";
        assert!(pick_launcher(out).unwrap().to_string_lossy().ends_with("thing.exe"));
        assert!(pick_launcher("").is_none());
    }
}
