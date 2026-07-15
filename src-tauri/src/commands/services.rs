//! Process management for all long-running engines (web servers, databases,
//! caches). Real start/stop/restart via `std::process::Command`, tracked in an
//! in-memory registry. Windows processes spawn with CREATE_NO_WINDOW and are
//! stopped by killing the whole process tree.

use std::collections::HashMap;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::state::StateStore;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

const SERVER_KINDS: &[&str] = &["nginx", "apache"];
const DATABASE_KINDS: &[&str] = &["mysql", "mariadb", "postgresql"];
const CACHE_KINDS: &[&str] = &["redis", "memcached"];
const MAIL_KINDS: &[&str] = &["mailpit"];

/// Mailpit's loopback ports: SMTP trap + web UI (the UI port is also the health probe).
const MAILPIT_SMTP_PORT: u16 = 1025;
const MAILPIT_UI_PORT: u16 = 8025;

/// A running child plus when it started (used for restart backoff).
struct Managed {
    child: Child,
    started: Instant,
}

/// Everything needed to relaunch a service the watchdog finds dead.
#[derive(Clone)]
struct SpawnPlan {
    exe: PathBuf,
    cwd: PathBuf,
    args: Vec<String>,
    envs: Vec<(String, String)>,
    log: Option<PathBuf>,
}

/// A respawnable service: its launch plan and how many consecutive revives.
struct Respawn {
    plan: SpawnPlan,
    restarts: u32,
}

/// Tracked processes keyed by id (`"{component}-{version}"`, or `php-cgi-{minor}`).
/// `specs` holds a launch plan only for ids the watchdog is allowed to revive.
#[derive(Default)]
pub struct Registry {
    procs: HashMap<String, Managed>,
    specs: HashMap<String, Respawn>,
}

impl Registry {
    /// True if a process with `id` is currently tracked as running.
    pub(crate) fn is_running(&self, id: &str) -> bool {
        self.procs.contains_key(id)
    }

    /// How long the tracked process for `id` has been running, if any.
    fn started_elapsed(&self, id: &str) -> Option<Duration> {
        self.procs.get(id).map(|m| m.started.elapsed())
    }
}

pub struct ProcessRegistry(pub Mutex<Registry>);

impl Default for ProcessRegistry {
    fn default() -> Self {
        ProcessRegistry(Mutex::new(Registry::default()))
    }
}

// Watchdog tuning.
const WATCHDOG_INTERVAL: Duration = Duration::from_secs(3);
const STABLE_RUN: Duration = Duration::from_secs(30);
const MAX_RESTARTS: u32 = 5;

/// Grace window after a service is spawned during which "process up but port not
/// answering yet" reads as `starting` rather than `unhealthy` (mysqld/postgres
/// cold-start takes several seconds).
const STARTUP_GRACE: Duration = Duration::from_secs(20);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceInfo {
    pub id: String,
    pub component: String,
    pub name: String,
    pub version: String,
    pub status: String, // "running" | "stopped"
    pub port: u16,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusChanged {
    id: String,
    status: String,
}

fn split_id(id: &str) -> Result<(String, String), String> {
    id.split_once('-')
        .map(|(c, v)| (c.to_string(), v.to_string()))
        .ok_or_else(|| format!("invalid service id '{id}' (expected 'component-version')"))
}

fn display_name(component: &str) -> &str {
    match component {
        "nginx" => "Nginx",
        "apache" => "Apache",
        "mysql" => "MySQL",
        "mariadb" => "MariaDB",
        "postgresql" => "PostgreSQL",
        "redis" => "Redis",
        "memcached" => "Memcached",
        "mailpit" => "Mailpit",
        other => other,
    }
}

fn default_port(component: &str) -> u16 {
    match component {
        "nginx" | "apache" => 80,
        "mysql" | "mariadb" => 3306,
        "postgresql" => 5432,
        "redis" => 6379,
        "memcached" => 11211,
        "mailpit" => MAILPIT_UI_PORT,
        _ => 0,
    }
}

// ---- process spawning ----

/// Find `name` directly under `dir`, else one directory level deep (some
/// Windows ports nest the binary in a subfolder).
fn locate_exe(dir: &Path, name: &str) -> Option<std::path::PathBuf> {
    let direct = dir.join(name);
    if direct.exists() {
        return Some(direct);
    }
    std::fs::read_dir(dir).ok()?.flatten().find_map(|e| {
        let p = e.path().join(name);
        p.exists().then_some(p)
    })
}

fn spawn_plan(plan: &SpawnPlan) -> Result<Child, String> {
    if !plan.exe.exists() {
        return Err(format!("executable not found: {}", plan.exe.display()));
    }
    let mut cmd = Command::new(&plan.exe);
    cmd.current_dir(&plan.cwd).args(&plan.args);
    for (k, v) in &plan.envs {
        cmd.env(k, v);
    }
    // Capture stdout+stderr to a fresh log file (for engines that log there).
    if let Some(log) = &plan.log {
        if let Some(parent) = log.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(f) = std::fs::File::create(log) {
            if let Ok(f2) = f.try_clone() {
                cmd.stdout(Stdio::from(f)).stderr(Stdio::from(f2));
            }
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let child = cmd.spawn().map_err(|e| e.to_string())?;
    // Bind to the kill-on-close job so it can't outlive Stackr on its port.
    crate::job::assign(&child);
    Ok(child)
}

/// Services the watchdog may revive on unexpected death: request servers and
/// caches (stateless). Databases are left alone — a crashed DB warrants
/// attention, not a blind restart on a possibly mid-write data dir.
fn is_respawnable(id: &str) -> bool {
    if id.starts_with(PHP_CGI_PREFIX) {
        return true;
    }
    matches!(
        id.split_once('-').map(|(c, _)| c),
        Some("nginx") | Some("apache") | Some("redis") | Some("memcached") | Some("mailpit")
    )
}

fn run_blocking(exe: &Path, cwd: &Path, args: &[String]) -> Result<(), String> {
    if !exe.exists() {
        return Err(format!("executable not found: {}", exe.display()));
    }
    let mut cmd = Command::new(exe);
    cmd.current_dir(cwd).args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd.output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(())
}

/// First-run initialization for a PostgreSQL data directory (best-effort). `data`
/// lives outside the version dir; migrate a legacy in-version datadir if present.
fn init_postgres(bin_dir: &Path, data: &Path) -> Result<(), String> {
    if data.join("PG_VERSION").exists() {
        return Ok(());
    }
    if let Some(parent) = data.parent() {
        crate::paths::ensure_dir(parent).map_err(|e| e.to_string())?;
    }
    let legacy = bin_dir.join("data");
    if legacy.join("PG_VERSION").exists() && !data.exists() {
        return std::fs::rename(&legacy, data)
            .or_else(|_| crate::db::copy_dir_all(&legacy, data))
            .map_err(|e| format!("migrating PostgreSQL data dir: {e}"));
    }
    let exe = bin_dir.join("bin").join("initdb.exe");
    run_blocking(
        &exe,
        bin_dir,
        &["-D".into(), data.display().to_string(), "-U".into(), "postgres".into(), "-A".into(), "trust".into()],
    )
}

/// Build the launch plan for a component (running config-generation side effects
/// as needed). Every engine binds to 127.0.0.1 only — a dev stack must never be
/// reachable from the LAN.
fn component_plan(dir: &Path, component: &str, version: &str) -> Result<SpawnPlan, String> {
    let plan = |exe: PathBuf, cwd: PathBuf, args: Vec<String>, log: Option<PathBuf>| SpawnPlan {
        exe,
        cwd,
        args,
        envs: Vec::new(),
        log,
    };
    match component {
        "nginx" => {
            // Ensure the master config exists (preserving in-app edits) and run with it.
            crate::config_gen::ensure_nginx_master(dir)?;
            let conf = crate::paths::nginx_conf();
            Ok(plan(
                dir.join("nginx.exe"),
                dir.to_path_buf(),
                vec![
                    "-p".into(),
                    dir.to_string_lossy().to_string(),
                    "-c".into(),
                    conf.to_string_lossy().replace('\\', "/"),
                ],
                None, // nginx writes its own error.log/access.log
            ))
        }
        "apache" => {
            // Ensure a Stackr-managed config exists (correct ServerRoot + PHP via
            // mod_proxy_fcgi + per-site vhosts), preserving in-app edits, then run it.
            crate::config_gen::ensure_apache_master(dir)?;
            let conf = crate::paths::apache_conf();
            Ok(plan(
                dir.join("bin").join("httpd.exe"),
                dir.to_path_buf(),
                vec!["-f".into(), conf.to_string_lossy().replace('\\', "/")],
                None, // Apache logs to logs/apache/error.log (set in the conf)
            ))
        }
        "redis" => {
            let exe = locate_exe(dir, "redis-server.exe")
                .ok_or("redis-server.exe not found in the Redis install")?;
            let cwd = exe.parent().unwrap_or(dir).to_path_buf();
            Ok(plan(
                exe,
                cwd,
                vec!["--port".into(), "6379".into(), "--bind".into(), "127.0.0.1".into()],
                Some(crate::paths::service_log_file("redis")),
            ))
        }
        "memcached" => {
            let exe = locate_exe(dir, "memcached.exe")
                .ok_or("memcached.exe not found in the Memcached install")?;
            let cwd = exe.parent().unwrap_or(dir).to_path_buf();
            Ok(plan(
                exe,
                cwd,
                vec!["-p".into(), "11211".into(), "-l".into(), "127.0.0.1".into(), "-v".into()],
                Some(crate::paths::service_log_file("memcached")),
            ))
        }
        "mailpit" => {
            let exe = locate_exe(dir, "mailpit.exe")
                .ok_or("mailpit.exe not found in the Mailpit install")?;
            let cwd = exe.parent().unwrap_or(dir).to_path_buf();
            Ok(plan(
                exe,
                cwd,
                vec![
                    "--smtp".into(),
                    format!("127.0.0.1:{MAILPIT_SMTP_PORT}"),
                    "--listen".into(),
                    format!("127.0.0.1:{MAILPIT_UI_PORT}"),
                ],
                Some(crate::paths::service_log_file("mailpit")),
            ))
        }
        "mysql" | "mariadb" => {
            let data = crate::paths::mysql_data_dir(component);
            crate::db::ensure_mysql_data(dir, &data)?;
            let _ = crate::paths::ensure_dir(&crate::paths::mysql_log_dir());
            let daemon = crate::db::mysql_daemon(dir)
                .ok_or("MySQL/MariaDB server binary not found (mariadbd.exe / mysqld.exe)")?;
            Ok(plan(daemon, dir.to_path_buf(), crate::db::mysql_serve_args(&data, 3306), None))
        }
        "postgresql" => {
            let major = version.split('.').next().unwrap_or(version);
            let data = crate::paths::postgres_data_dir(major);
            init_postgres(dir, &data)?;
            Ok(plan(
                dir.join("bin").join("postgres.exe"),
                dir.to_path_buf(),
                vec![
                    "-D".into(),
                    data.display().to_string(),
                    "-p".into(),
                    "5432".into(),
                    "-c".into(),
                    "listen_addresses=127.0.0.1".into(),
                ],
                Some(crate::paths::service_log_file("postgresql")),
            ))
        }
        other => Err(format!("don't know how to start '{other}'")),
    }
}

fn kill_tree(pid: u32) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
    }
    #[cfg(not(windows))]
    {
        let _ = Command::new("kill").arg(pid.to_string()).output();
    }
}

fn prune(reg: &mut Registry) {
    reg.procs
        .retain(|_, m| !matches!(m.child.try_wait(), Ok(Some(_))));
}

/// True if `port` can be bound right now (nothing else is listening on it).
fn port_available(port: u16) -> bool {
    TcpListener::bind(("0.0.0.0", port)).is_ok()
}

/// True if something is accepting connections on the loopback `port` — a
/// lightweight health probe. Raw TCP, so it leaves nothing in access logs.
fn port_responds(port: u16) -> bool {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    TcpStream::connect_timeout(&addr, Duration::from_millis(300)).is_ok()
}

// ---- listing ----

fn list_services(
    state: &State<'_, StateStore>,
    reg: &State<'_, ProcessRegistry>,
    kinds: &[&str],
) -> Result<Vec<ServiceInfo>, String> {
    let installed = {
        let st = state.0.lock().map_err(|e| e.to_string())?;
        st.installed.clone()
    };
    // Snapshot liveness under the registry lock, then probe ports AFTER releasing
    // it — a health probe can wait up to 300ms and must not block other callers.
    let mut rows: Vec<(crate::state::InstalledComponent, String, u16, Option<Duration>)> = Vec::new();
    {
        let mut registry = reg.0.lock().map_err(|e| e.to_string())?;
        prune(&mut registry);
        for c in installed.into_iter().filter(|c| kinds.contains(&c.component.as_str())) {
            let id = format!("{}-{}", c.component, c.version);
            let elapsed = registry.started_elapsed(&id);
            let port = default_port(&c.component);
            rows.push((c, id, port, elapsed));
        }
    }

    Ok(rows
        .into_iter()
        .map(|(c, id, port, elapsed)| {
            // stopped: no process. Otherwise probe the port: answering = running;
            // silent but inside the startup grace = starting; silent after = unhealthy.
            let status = match elapsed {
                None => "stopped",
                Some(_) if port == 0 => "running",
                Some(e) => {
                    if port_responds(port) {
                        "running"
                    } else if e < STARTUP_GRACE {
                        "starting"
                    } else {
                        "unhealthy"
                    }
                }
            };
            ServiceInfo {
                id,
                name: display_name(&c.component).to_string(),
                port,
                status: status.to_string(),
                component: c.component,
                version: c.version,
            }
        })
        .collect())
}

#[tauri::command]
pub fn get_servers(
    state: State<'_, StateStore>,
    reg: State<'_, ProcessRegistry>,
) -> Result<Vec<ServiceInfo>, String> {
    list_services(&state, &reg, SERVER_KINDS)
}

#[tauri::command]
pub fn get_databases(
    state: State<'_, StateStore>,
    reg: State<'_, ProcessRegistry>,
) -> Result<Vec<ServiceInfo>, String> {
    list_services(&state, &reg, DATABASE_KINDS)
}

#[tauri::command]
pub fn get_caches(
    state: State<'_, StateStore>,
    reg: State<'_, ProcessRegistry>,
) -> Result<Vec<ServiceInfo>, String> {
    list_services(&state, &reg, CACHE_KINDS)
}

#[tauri::command]
pub fn get_mail(
    state: State<'_, StateStore>,
    reg: State<'_, ProcessRegistry>,
) -> Result<Vec<ServiceInfo>, String> {
    list_services(&state, &reg, MAIL_KINDS)
}

// ---- start / stop / restart (generic for any engine) ----

type Reg = Mutex<Registry>;

/// Start a service if not already running. Returns `true` if it was actually
/// spawned, `false` if it was already running.
fn do_start(app: &AppHandle, reg: &Reg, id: &str) -> Result<bool, String> {
    let (component, version) = split_id(id)?;
    let dir = crate::paths::component_dir(&component, &version);
    if !dir.exists() {
        return Err(format!("{component} {version} is not installed"));
    }

    let mut registry = reg.lock().map_err(|e| e.to_string())?;
    prune(&mut registry);
    if registry.procs.contains_key(id) {
        return Ok(false);
    }

    // Fail fast on a port clash instead of letting the engine bind-fail and die
    // (e.g. nginx and Apache both default to :80).
    let port = default_port(&component);
    if port != 0 && !port_available(port) {
        let holder = registry
            .procs
            .keys()
            .filter_map(|k| k.split_once('-').map(|(c, _)| c.to_string()))
            .find(|c| c != &component && default_port(c) == port);
        return Err(match holder {
            Some(other) => format!(
                "{} can't start: port {port} is in use by {} — stop it first.",
                display_name(&component),
                display_name(&other)
            ),
            None => format!(
                "{} can't start: port {port} is already in use by another application.",
                display_name(&component)
            ),
        });
    }

    let plan = component_plan(&dir, &component, &version)?;
    let child = spawn_plan(&plan)?;
    registry
        .procs
        .insert(id.to_string(), Managed { child, started: Instant::now() });
    if is_respawnable(id) {
        registry
            .specs
            .insert(id.to_string(), Respawn { plan, restarts: 0 });
    }
    drop(registry);

    let _ = app.emit(
        "service-status-changed",
        StatusChanged { id: id.to_string(), status: "running".into() },
    );
    Ok(true)
}

fn do_stop(app: &AppHandle, reg: &Reg, id: &str) -> Result<(), String> {
    let mut registry = reg.lock().map_err(|e| e.to_string())?;
    // Drop the respawn intent first so the watchdog won't revive a user stop.
    registry.specs.remove(id);
    if let Some(mut m) = registry.procs.remove(id) {
        kill_tree(m.child.id());
        let _ = m.child.kill();
        let _ = m.child.wait();
    }
    drop(registry);

    let _ = app.emit(
        "service-status-changed",
        StatusChanged { id: id.to_string(), status: "stopped".into() },
    );
    Ok(())
}

#[tauri::command]
pub fn start_service(app: AppHandle, reg: State<'_, ProcessRegistry>, id: String) -> Result<(), String> {
    do_start(&app, &reg.0, &id).map(|_| ())
}

#[tauri::command]
pub fn stop_service(app: AppHandle, reg: State<'_, ProcessRegistry>, id: String) -> Result<(), String> {
    do_stop(&app, &reg.0, &id)
}

#[tauri::command]
pub fn restart_service(app: AppHandle, reg: State<'_, ProcessRegistry>, id: String) -> Result<(), String> {
    do_stop(&app, &reg.0, &id)?;
    do_start(&app, &reg.0, &id).map(|_| ())
}

/// Apply newly-written vhosts to a running web server. nginx reloads gracefully;
/// Apache (no reliable console reload) is hard-restarted. No-op if not running.
pub(crate) fn reload_server(app: &AppHandle, reg: &ProcessRegistry, id: &str) -> Result<(), String> {
    let (component, version) = split_id(id)?;
    {
        let mut registry = reg.0.lock().map_err(|e| e.to_string())?;
        prune(&mut registry);
        if !registry.procs.contains_key(id) {
            return Ok(());
        }
    }
    match component.as_str() {
        "nginx" => {
            let dir = crate::paths::component_dir(&component, &version);
            crate::config_gen::ensure_nginx_master(&dir)?;
            let conf = crate::paths::nginx_conf();
            run_blocking(
                &dir.join("nginx.exe"),
                &dir,
                &[
                    "-p".into(),
                    dir.to_string_lossy().to_string(),
                    "-c".into(),
                    conf.to_string_lossy().replace('\\', "/"),
                    "-s".into(),
                    "reload".into(),
                ],
            )
        }
        // Console httpd has no graceful reload — regenerate config via a restart.
        "apache" => {
            do_stop(app, &reg.0, id)?;
            do_start(app, &reg.0, id).map(|_| ())
        }
        _ => Ok(()),
    }
}

// ---- PHP FastCGI runtime (one php-cgi per PHP version) ----
//
// Each installed PHP minor series runs its own php-cgi on a distinct port, so
// projects can be served by different PHP versions at the same time. The web
// server's per-project vhost points at the port for that project's version.

const PHP_CGI_PREFIX: &str = "php-cgi-";

/// `major.minor` of a version ("8.2.14" → "8.2"). All patch releases of a minor
/// share one php-cgi runtime.
fn php_minor_series(version: &str) -> String {
    let mut it = version.split('.');
    let major = it.next().unwrap_or("0");
    let minor = it.next().unwrap_or("0");
    format!("{major}.{minor}")
}

/// Deterministic FastCGI port for a PHP version, derived from its minor series:
/// `9000 + major*10 + minor` (8.2.x → 9082, 8.3.x → 9083, 7.4.x → 9074). Distinct
/// per minor series and clear of the other engines' ports.
pub(crate) fn php_fcgi_port(version: &str) -> u16 {
    let mut it = version.split('.');
    let major: u16 = it.next().and_then(|s| s.parse().ok()).unwrap_or(8);
    let minor: u16 = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    9000 + major.saturating_mul(10).saturating_add(minor)
}

/// Registry id for a version's php-cgi process (keyed by minor series).
fn php_cgi_id(version: &str) -> String {
    format!("{PHP_CGI_PREFIX}{}", php_minor_series(version))
}

/// Idempotently start a service by id (used by Projects). Returns `true` if it
/// was actually spawned, `false` if it was already running.
pub(crate) fn ensure_started(app: &AppHandle, reg: &ProcessRegistry, id: &str) -> Result<bool, String> {
    do_start(app, &reg.0, id)
}

/// Stop a service by id (used by Projects when the last project shuts down).
pub(crate) fn ensure_stopped(app: &AppHandle, reg: &ProcessRegistry, id: &str) -> Result<(), String> {
    do_stop(app, &reg.0, id)
}

/// Idempotently start php-cgi for `version` on its own FastCGI port. Returns the
/// port the runtime listens on (so the caller can wire the matching vhost).
pub(crate) fn ensure_php_runtime(reg: &ProcessRegistry, version: &str) -> Result<u16, String> {
    let port = php_fcgi_port(version);
    let id = php_cgi_id(version);
    let dir = crate::paths::component_dir("php", version);
    let exe = dir.join("php-cgi.exe");
    let mut registry = reg.0.lock().map_err(|e| e.to_string())?;
    prune(&mut registry);
    if registry.procs.contains_key(&id) {
        return Ok(port);
    }
    // Make sure the served runtime has the common extensions enabled (DB drivers,
    // openssl, mbstring, …) before php-cgi starts.
    let _ = crate::scaffold::ensure_php_runtime_ini(&dir);
    let plan = SpawnPlan {
        exe,
        cwd: dir.clone(),
        args: vec!["-b".into(), format!("127.0.0.1:{port}")],
        // php-cgi under FastCGI self-terminates after PHP_FCGI_MAX_REQUESTS
        // (default 500), returning a 502 mid-session. 0 = never exit on request
        // count; the watchdog handles genuine crashes.
        //
        // PHP_FCGI_CHILDREN spawns a pool of worker processes (verified on
        // Windows: 1 master + N children). A single worker serializes every
        // request and DEADLOCKS on loopback HTTP (a PHP page requesting another
        // URL on the same runtime — WordPress cron/REST, Laravel SPA→own API).
        envs: vec![
            ("PHP_FCGI_MAX_REQUESTS".into(), "0".into()),
            ("PHP_FCGI_CHILDREN".into(), "4".into()),
        ],
        log: Some(crate::paths::service_log_file("php")),
    };
    let child = spawn_plan(&plan)?;
    registry
        .procs
        .insert(id.clone(), Managed { child, started: Instant::now() });
    registry.specs.insert(id, Respawn { plan, restarts: 0 });
    Ok(port)
}

#[tauri::command]
pub fn start_php_runtime(reg: State<'_, ProcessRegistry>, version: String) -> Result<(), String> {
    ensure_php_runtime(&reg, &version).map(|_| ())
}

/// Stop the php-cgi runtime for a single PHP version (e.g. before uninstalling it).
pub(crate) fn stop_php_runtime_version(reg: &ProcessRegistry, version: &str) -> Result<(), String> {
    let id = php_cgi_id(version);
    let mut registry = reg.0.lock().map_err(|e| e.to_string())?;
    registry.specs.remove(&id);
    if let Some(mut m) = registry.procs.remove(&id) {
        kill_tree(m.child.id());
        let _ = m.child.kill();
        let _ = m.child.wait();
    }
    Ok(())
}

/// Restart the php-cgi runtime for `version` only if it's currently running, so
/// a php.ini change (e.g. toggling Xdebug) takes effect for live projects without
/// spinning up a runtime that nothing is using.
pub(crate) fn restart_php_runtime_if_running(reg: &ProcessRegistry, version: &str) -> Result<(), String> {
    let id = php_cgi_id(version);
    let was_running = {
        let registry = reg.0.lock().map_err(|e| e.to_string())?;
        registry.is_running(&id)
    };
    if was_running {
        stop_php_runtime_version(reg, version)?;
        ensure_php_runtime(reg, version)?;
    }
    Ok(())
}

/// Stop every php-cgi runtime (used by Projects when no project is left running).
pub(crate) fn stop_all_php_runtimes(reg: &ProcessRegistry) -> Result<(), String> {
    let mut registry = reg.0.lock().map_err(|e| e.to_string())?;
    let ids: Vec<String> = registry
        .procs
        .keys()
        .filter(|k| k.starts_with(PHP_CGI_PREFIX))
        .cloned()
        .collect();
    for id in ids {
        registry.specs.remove(&id);
        if let Some(mut m) = registry.procs.remove(&id) {
            kill_tree(m.child.id());
            let _ = m.child.kill();
            let _ = m.child.wait();
        }
    }
    Ok(())
}

#[tauri::command]
pub fn stop_php_runtime(reg: State<'_, ProcessRegistry>, version: String) -> Result<(), String> {
    stop_php_runtime_version(&reg, &version)
}

// ---- watchdog ----

/// Background monitor: notices services that exited unexpectedly, keeps the UI
/// status truthful by emitting `service-status-changed`, and revives the
/// respawnable ones (php-cgi, web servers, caches). A run that stayed up past
/// `STABLE_RUN` resets its restart counter; a service that dies faster than that
/// more than `MAX_RESTARTS` times is left stopped so we never crash-loop.
pub fn start_watchdog(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(WATCHDOG_INTERVAL);
        let reg = tauri::Manager::state::<ProcessRegistry>(&app);
        let mut events: Vec<(String, String)> = Vec::new();
        {
            let mut registry = match reg.0.lock() {
                Ok(r) => r,
                Err(_) => continue,
            };
            // Ids whose child exited since the last tick, with how long it ran.
            let dead: Vec<(String, Duration)> = registry
                .procs
                .iter_mut()
                .filter_map(|(id, m)| match m.child.try_wait() {
                    Ok(Some(_)) => Some((id.clone(), m.started.elapsed())),
                    _ => None,
                })
                .collect();

            for (id, uptime) in dead {
                registry.procs.remove(&id);
                // No spec => not respawnable (e.g. a database): just report stopped.
                let restarts = match registry.specs.get(&id) {
                    Some(_) if uptime > STABLE_RUN => 0,
                    Some(s) => s.restarts,
                    None => {
                        events.push((id, "stopped".into()));
                        continue;
                    }
                };
                if restarts >= MAX_RESTARTS {
                    registry.specs.remove(&id);
                    events.push((id, "stopped".into()));
                    continue;
                }
                let plan = registry.specs.get(&id).map(|s| s.plan.clone());
                match plan.map(|p| spawn_plan(&p)) {
                    Some(Ok(child)) => {
                        registry
                            .procs
                            .insert(id.clone(), Managed { child, started: Instant::now() });
                        if let Some(s) = registry.specs.get_mut(&id) {
                            s.restarts = restarts + 1;
                        }
                        // Still running — no status change to report.
                    }
                    _ => {
                        registry.specs.remove(&id);
                        events.push((id, "stopped".into()));
                    }
                }
            }
        }
        for (id, status) in events {
            let _ = app.emit("service-status-changed", StatusChanged { id, status });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{locate_exe, php_cgi_id, php_fcgi_port, port_available};
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::process::Command;
    use std::time::Duration;

    #[test]
    fn php_runtime_port_and_id_per_minor_series() {
        // Distinct port per minor series; patch releases collapse to one runtime.
        assert_eq!(php_fcgi_port("8.2.14"), 9082);
        assert_eq!(php_fcgi_port("8.3.0"), 9083);
        assert_eq!(php_fcgi_port("7.4.33"), 9074);
        assert_eq!(php_fcgi_port("8.2.10"), php_fcgi_port("8.2.27"));
        assert_ne!(php_fcgi_port("8.2.0"), php_fcgi_port("8.3.0"));
        assert_eq!(php_cgi_id("8.2.14"), "php-cgi-8.2");
        assert_eq!(php_cgi_id("8.2.10"), php_cgi_id("8.2.27"));
    }

    #[test]
    fn detects_port_in_use() {
        let l = TcpListener::bind(("0.0.0.0", 0)).expect("bind ephemeral");
        let port = l.local_addr().unwrap().port();
        assert!(!port_available(port), "a held port must read as unavailable");
        drop(l);
        assert!(port_available(port), "a freed port must read as available");
    }

    /// Live proof: download the Redis Windows port, start it, and get a PONG.
    ///   cargo test redis_responds_to_ping -- --ignored --nocapture
    #[tokio::test]
    #[ignore = "downloads Redis (Windows port) and runs a live PING"]
    async fn redis_responds_to_ping() {
        let dir = std::env::temp_dir().join("stackr-redis-test");
        if locate_exe(&dir, "redis-server.exe").is_none() {
            crate::download::download_and_extract(
                "https://github.com/tporadowski/redis/releases/download/v5.0.14.1/Redis-x64-5.0.14.1.zip",
                &dir,
                |_, _| {},
            )
            .await
            .expect("redis download");
        }
        let server = locate_exe(&dir, "redis-server.exe").expect("redis-server.exe");
        let cli = locate_exe(&dir, "redis-cli.exe").expect("redis-cli.exe");

        let mut child = Command::new(&server)
            .current_dir(server.parent().unwrap())
            .args(["--port", "6390"])
            .spawn()
            .expect("spawn redis-server");

        let mut ok = false;
        let mut last = String::new();
        for _ in 0..12 {
            tokio::time::sleep(Duration::from_millis(500)).await;
            if let Ok(out) = Command::new(&cli).args(["-p", "6390", "ping"]).output() {
                last = String::from_utf8_lossy(&out.stdout).to_string();
                if last.to_uppercase().contains("PONG") {
                    ok = true;
                    break;
                }
            }
        }
        let _ = child.kill();
        let _ = child.wait();
        assert!(ok, "Redis never returned PONG; last: {last}");
    }

    /// Proves stdout+stderr capture to a log file works (Redis logs to stdout),
    /// which is how the Logs tab gets redis/memcached/postgres/php output.
    ///   cargo test redis_writes_to_log_file -- --ignored --nocapture
    #[tokio::test]
    #[ignore = "downloads Redis and verifies stdout capture"]
    async fn redis_writes_to_log_file() {
        use std::process::Stdio;

        let dir = std::env::temp_dir().join("stackr-redis-test");
        if locate_exe(&dir, "redis-server.exe").is_none() {
            crate::download::download_and_extract(
                "https://github.com/tporadowski/redis/releases/download/v5.0.14.1/Redis-x64-5.0.14.1.zip",
                &dir,
                |_, _| {},
            )
            .await
            .expect("redis download");
        }
        let server = locate_exe(&dir, "redis-server.exe").expect("redis-server.exe");
        let log = dir.join("redis-capture.log");
        let f = std::fs::File::create(&log).unwrap();
        let f2 = f.try_clone().unwrap();

        let mut child = Command::new(&server)
            .current_dir(server.parent().unwrap())
            .args(["--port", "6391"])
            .stdout(Stdio::from(f))
            .stderr(Stdio::from(f2))
            .spawn()
            .expect("spawn redis-server");

        tokio::time::sleep(Duration::from_millis(1500)).await;
        let _ = child.kill();
        let _ = child.wait();

        let content = std::fs::read_to_string(&log).unwrap_or_default();
        assert!(
            content.to_lowercase().contains("redis") || content.contains("Ready to accept"),
            "captured log should contain Redis startup output, got: {content}"
        );
    }

    /// Live proof: download the Memcached Windows port, start it, and read its
    /// version over a raw TCP connection.
    ///   cargo test memcached_reports_version -- --ignored --nocapture
    #[tokio::test]
    #[ignore = "downloads Memcached (Windows port) and runs a live check"]
    async fn memcached_reports_version() {
        let dir = std::env::temp_dir().join("stackr-memcached-test");
        if locate_exe(&dir, "memcached.exe").is_none() {
            crate::download::download_and_extract(
                "https://github.com/jefyt/memcached-windows/releases/download/1.6.8_mingw_libressl/memcached-1.6.8-win64-mingw.zip",
                &dir,
                |_, _| {},
            )
            .await
            .expect("memcached download");
        }
        let exe = locate_exe(&dir, "memcached.exe").expect("memcached.exe");

        let mut child = Command::new(&exe)
            .current_dir(exe.parent().unwrap())
            .args(["-p", "11290"])
            .spawn()
            .expect("spawn memcached");

        let mut ok = false;
        let mut last = String::new();
        for _ in 0..12 {
            tokio::time::sleep(Duration::from_millis(500)).await;
            if let Ok(mut s) = TcpStream::connect("127.0.0.1:11290") {
                let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
                let _ = s.write_all(b"version\r\n");
                let mut buf = [0u8; 64];
                if let Ok(n) = s.read(&mut buf) {
                    last = String::from_utf8_lossy(&buf[..n]).to_string();
                    if last.to_uppercase().contains("VERSION") {
                        ok = true;
                        break;
                    }
                }
            }
        }
        let _ = child.kill();
        let _ = child.wait();
        assert!(ok, "Memcached never reported a version; last: {last}");
    }
}
