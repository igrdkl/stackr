//! Database engine specifics shared by the service layer. MariaDB and MySQL
//! differ in both binary names (`mariadbd.exe` vs `mysqld.exe`) and first-run
//! init tooling (`mariadb-install-db.exe` vs `mysqld --initialize-insecure`).

use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// First existing `bin/<name>` among the candidates.
pub fn find_bin(dir: &Path, names: &[&str]) -> Option<PathBuf> {
    let bin = dir.join("bin");
    names.iter().map(|n| bin.join(n)).find(|p| p.exists())
}

/// Server daemon for a MySQL-family install (MariaDB first, then MySQL).
pub fn mysql_daemon(dir: &Path) -> Option<PathBuf> {
    find_bin(dir, &["mariadbd.exe", "mysqld.exe"])
}

/// CLI client for a MySQL-family install (used to create per-project databases).
pub fn mysql_client(dir: &Path) -> Option<PathBuf> {
    find_bin(dir, &["mariadb.exe", "mysql.exe"])
}

fn run(exe: &Path, cwd: &Path, args: &[String]) -> Result<(), String> {
    let mut cmd = Command::new(exe);
    cmd.current_dir(cwd).args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd.output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let outp = String::from_utf8_lossy(&out.stdout);
        let msg = if !err.trim().is_empty() { err.trim() } else { outp.trim() };
        return Err(if msg.is_empty() {
            format!("init exited with status {}", out.status)
        } else {
            msg.lines().rev().take(6).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n")
        });
    }
    Ok(())
}

/// Ensure the MySQL/MariaDB data dir at `data` is initialized. `bin_dir` is the
/// engine's install dir (source of the init tool). `data` lives OUTSIDE the
/// version dir (see [`crate::paths::mysql_data_dir`]) so upgrading or uninstalling
/// the binary never touches databases. On first run this migrates any data an
/// older build left inside the version dir — and reuses the pre-built `data/`
/// that MariaDB Windows zips ship — instead of a fresh (and previously untested)
/// `mariadb-install-db`.
pub fn ensure_mysql_data(bin_dir: &Path, data: &Path) -> Result<(), String> {
    // A populated system schema means the data dir is already initialized.
    if data.join("mysql").is_dir() {
        return Ok(());
    }
    if let Some(parent) = data.parent() {
        crate::paths::ensure_dir(parent).map_err(|e| e.to_string())?;
    }

    // Migrate a legacy/bundled datadir from inside the version dir, if present.
    let legacy = bin_dir.join("data");
    if legacy.join("mysql").is_dir() && !data.exists() {
        return std::fs::rename(&legacy, data)
            .or_else(|_| copy_dir_all(&legacy, data))
            .map_err(|e| format!("migrating MySQL/MariaDB data dir: {e}"));
    }

    // Fresh init. MariaDB: dedicated installer.
    if let Some(installer) = find_bin(bin_dir, &["mariadb-install-db.exe", "mysql_install_db.exe"]) {
        return run(&installer, bin_dir, &[format!("--datadir={}", data.display())]);
    }
    // MySQL: bootstrap an insecure (passwordless root) data dir.
    if let Some(daemon) = find_bin(bin_dir, &["mysqld.exe"]) {
        return run(
            &daemon,
            bin_dir,
            &["--initialize-insecure".into(), format!("--datadir={}", data.display())],
        );
    }
    Err("no MySQL/MariaDB init tool found (mariadb-install-db / mysqld)".into())
}

/// Recursively copy `src` into `dst` (fallback for a cross-volume data-dir move).
pub fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_all(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

/// Turn a project name into a safe SQL identifier: lowercase, `[a-z0-9_]`, never
/// empty, never leading with a digit.
pub fn sanitize_db_name(name: &str) -> String {
    let mut s: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' })
        .collect();
    s = s.trim_matches('_').to_string();
    if s.is_empty() {
        return "app".to_string();
    }
    if s.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
        format!("db_{s}")
    } else {
        s
    }
}

/// Create a MySQL/MariaDB database (idempotent) as the passwordless `root` over
/// TCP. Retries to ride out the server's cold start. `db` must already be a safe
/// identifier (see [`sanitize_db_name`]).
pub fn create_mysql_database(dir: &Path, port: u16, db: &str) -> Result<(), String> {
    let client = mysql_client(dir).ok_or("MySQL/MariaDB client (mariadb.exe/mysql.exe) not found")?;
    let sql = format!(
        "CREATE DATABASE IF NOT EXISTS `{db}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;"
    );
    let args = vec![
        "--host=127.0.0.1".to_string(),
        format!("--port={port}"),
        "--protocol=TCP".to_string(),
        "-u".to_string(),
        "root".to_string(),
        "-e".to_string(),
        sql,
    ];
    let mut last = String::new();
    for _ in 0..25 {
        match run(&client, dir, &args) {
            Ok(()) => return Ok(()),
            Err(e) => {
                last = e;
                std::thread::sleep(std::time::Duration::from_millis(700));
            }
        }
    }
    Err(format!("could not create database `{db}`: {last}"))
}

/// Create a PostgreSQL database (idempotent) as the `postgres` superuser. Treats
/// an "already exists" failure as success. Retries for cold start.
pub fn create_postgres_database(dir: &Path, port: u16, db: &str) -> Result<(), String> {
    let createdb = dir.join("bin").join("createdb.exe");
    if !createdb.exists() {
        return Err("createdb.exe not found in the PostgreSQL install".to_string());
    }
    let args = vec![
        "-h".to_string(),
        "127.0.0.1".to_string(),
        "-p".to_string(),
        port.to_string(),
        "-U".to_string(),
        "postgres".to_string(),
        db.to_string(),
    ];
    let mut last = String::new();
    for _ in 0..25 {
        match run(&createdb, dir, &args) {
            Ok(()) => return Ok(()),
            Err(e) => {
                if e.to_lowercase().contains("already exists") {
                    return Ok(());
                }
                last = e;
                std::thread::sleep(std::time::Duration::from_millis(700));
            }
        }
    }
    Err(format!("could not create database \"{db}\": {last}"))
}

/// Daemon args to serve the given `data` dir on a port, logging errors to a
/// Stackr path.
pub fn mysql_serve_args(data: &Path, port: u16) -> Vec<String> {
    let log_dir = crate::paths::mysql_log_dir();
    let _ = crate::paths::ensure_dir(&log_dir);
    let log = log_dir.join("error.log");
    vec![
        format!("--datadir={}", data.display()),
        format!("--port={port}"),
        // Loopback only — the DB must never be reachable from the LAN.
        "--bind-address=127.0.0.1".to_string(),
        format!("--log-error={}", log.display()),
    ]
}

/// Seconds since the Unix epoch, for a unique dump filename (0 if the clock is
/// before the epoch, which never happens in practice).
fn unix_time() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Dump every database from a **running** engine to a timestamped `.sql` file
/// under `C:\Stackr\backups`. The dump tools connect over TCP, so the engine must
/// be up — a connection failure returns a clear "start it first" message rather
/// than a raw driver error. Returns the created dump's path.
///
/// Used by the export-before-uninstall flow so a user can't lose their data by
/// removing the only engine that can read it.
fn dump_all(component: &str, version: &str) -> Result<String, String> {
    let dir = crate::paths::component_dir(component, version);
    let backups = crate::paths::backups_dir();
    crate::paths::ensure_dir(&backups).map_err(|e| e.to_string())?;
    let out_path = backups.join(format!("{component}-{version}-{}.sql", unix_time()));

    let (exe, args) = match component {
        "mysql" | "mariadb" => {
            let dump = find_bin(&dir, &["mariadb-dump.exe", "mysqldump.exe"])
                .ok_or("dump tool not found (mariadb-dump.exe / mysqldump.exe)")?;
            (
                dump,
                vec![
                    "--host=127.0.0.1".to_string(),
                    "--port=3306".to_string(),
                    "--protocol=TCP".to_string(),
                    "-u".to_string(),
                    "root".to_string(),
                    "--all-databases".to_string(),
                ],
            )
        }
        "postgresql" => {
            let dump = dir.join("bin").join("pg_dumpall.exe");
            if !dump.exists() {
                return Err("pg_dumpall.exe not found in the PostgreSQL install".to_string());
            }
            (
                dump,
                vec![
                    "-h".to_string(),
                    "127.0.0.1".to_string(),
                    "-p".to_string(),
                    "5432".to_string(),
                    "-U".to_string(),
                    "postgres".to_string(),
                ],
            )
        }
        other => return Err(format!("'{other}' is not an exportable database engine")),
    };

    // Stream the dump straight to the file; capture only stderr for diagnostics.
    let file = std::fs::File::create(&out_path).map_err(|e| e.to_string())?;
    let mut cmd = Command::new(&exe);
    cmd.current_dir(&dir)
        .args(&args)
        .stdout(std::process::Stdio::from(file))
        .stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd
        .spawn()
        .and_then(|c| c.wait_with_output())
        .map_err(|e| e.to_string())?;

    if out.status.success() {
        return Ok(out_path.to_string_lossy().to_string());
    }

    // Failed — drop the partial/empty dump and explain.
    let _ = std::fs::remove_file(&out_path);
    let err = String::from_utf8_lossy(&out.stderr).to_lowercase();
    if err.contains("connect") || err.contains("refused") || err.contains("can't connect") {
        Err("Could not connect — start the database before exporting.".to_string())
    } else {
        Err(format!(
            "Export failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// Export all databases from a running DB engine to `C:\Stackr\backups`.
#[tauri::command]
pub async fn export_databases(component: String, version: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || dump_all(&component, &version))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_db_names() {
        assert_eq!(sanitize_db_name("my-shop"), "my_shop");
        assert_eq!(sanitize_db_name("My Shop!"), "my_shop");
        assert_eq!(sanitize_db_name("blog.test"), "blog_test");
        assert_eq!(sanitize_db_name("123app"), "db_123app");
        assert_eq!(sanitize_db_name("---"), "app");
        assert_eq!(sanitize_db_name("café"), "caf"); // non-ascii dropped to _ then trimmed
    }

    #[test]
    fn prefers_mariadb_binaries() {
        let base = std::env::temp_dir().join("stackr-db-discovery");
        let bin = base.join("bin");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("mariadbd.exe"), b"x").unwrap();
        std::fs::write(bin.join("mysqld.exe"), b"x").unwrap();

        assert!(mysql_daemon(&base).unwrap().ends_with("mariadbd.exe"));
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Live proof of the per-project DB feature: start MariaDB, create a database
    /// from a (sanitized) project name, and confirm it exists. Reuses the cached
    /// download from `mariadb_serves_a_query`.
    ///   cargo test creates_a_project_database -- --ignored --nocapture
    #[tokio::test]
    #[ignore = "downloads MariaDB and creates a real database"]
    async fn creates_a_project_database() {
        use std::time::Duration;

        let dir = std::env::temp_dir().join("stackr-mariadb-test").join("mariadb");
        if mysql_daemon(&dir).is_none() {
            crate::download::download_and_extract(
                "https://archive.mariadb.org/mariadb-11.4.2/winx64-packages/mariadb-11.4.2-winx64.zip",
                &dir,
                |_, _| {},
            )
            .await
            .expect("mariadb download");
        }
        ensure_mysql_data(&dir, &dir.join("data")).expect("init data dir");

        let daemon = mysql_daemon(&dir).expect("server binary");
        let client = mysql_client(&dir).expect("client binary");
        let mut server = {
            let mut cmd = Command::new(&daemon);
            cmd.current_dir(&dir).args(mysql_serve_args(&dir.join("data"), 3308));
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(CREATE_NO_WINDOW);
            }
            cmd.spawn().expect("spawn mariadbd")
        };

        // Project name with spaces/punctuation → sanitized identifier.
        let db = sanitize_db_name("My Shop!");
        assert_eq!(db, "my_shop");
        let create = {
            let (dir, db) = (dir.clone(), db.clone());
            tokio::task::spawn_blocking(move || create_mysql_database(&dir, 3308, &db))
                .await
                .expect("join")
        };

        // Verify the schema is actually there.
        let mut listed = String::new();
        if create.is_ok() {
            for _ in 0..10 {
                let out = Command::new(&client)
                    .current_dir(&dir)
                    .args([
                        "--host=127.0.0.1",
                        "--port=3308",
                        "--protocol=TCP",
                        "-u",
                        "root",
                        "-e",
                        "SHOW DATABASES LIKE 'my_shop';",
                    ])
                    .output();
                if let Ok(out) = out {
                    listed = String::from_utf8_lossy(&out.stdout).to_string();
                    if listed.contains("my_shop") {
                        break;
                    }
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }

        let _ = server.kill();
        let _ = server.wait();

        create.expect("create_mysql_database failed");
        assert!(listed.contains("my_shop"), "database not found; SHOW returned: {listed}");
    }

    /// Live proof: download MariaDB, start the server, and run a real query via
    /// the bundled client. Heavy + networked, so `#[ignore]`d:
    ///   cargo test mariadb_serves_a_query -- --ignored --nocapture
    #[tokio::test]
    #[ignore = "downloads MariaDB (~100MB) and runs a live server"]
    async fn mariadb_serves_a_query() {
        use std::time::Duration;

        let base = std::env::temp_dir().join("stackr-mariadb-test");
        let _ = std::fs::remove_dir_all(&base);
        let dir = base.join("mariadb");

        crate::download::download_and_extract(
            "https://archive.mariadb.org/mariadb-11.4.2/winx64-packages/mariadb-11.4.2-winx64.zip",
            &dir,
            |_, _| {},
        )
        .await
        .expect("mariadb download");

        ensure_mysql_data(&dir, &dir.join("data")).expect("init data dir");

        let daemon = mysql_daemon(&dir).expect("server binary");
        let client = mysql_client(&dir).expect("client binary");

        let mut server = {
            let mut cmd = Command::new(&daemon);
            cmd.current_dir(&dir).args(mysql_serve_args(&dir.join("data"), 3307));
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(CREATE_NO_WINDOW);
            }
            cmd.spawn().expect("spawn mariadbd")
        };

        // Poll until it accepts a query (cold InnoDB start is a few seconds).
        let mut last = String::new();
        let mut ok = false;
        for _ in 0..20 {
            tokio::time::sleep(Duration::from_millis(1000)).await;
            let out = Command::new(&client)
                .current_dir(&dir)
                .args([
                    "--host=127.0.0.1",
                    "--port=3307",
                    "--protocol=TCP",
                    "-u",
                    "root",
                    "-e",
                    "SELECT VERSION();",
                ])
                .output();
            if let Ok(out) = out {
                if out.status.success() {
                    last = String::from_utf8_lossy(&out.stdout).to_string();
                    ok = true;
                    break;
                }
                last = String::from_utf8_lossy(&out.stderr).to_string();
            }
        }

        let _ = server.kill();
        let _ = server.wait();

        assert!(ok, "server never accepted a query; last: {last}");
        assert!(
            last.to_lowercase().contains("maria") || last.chars().any(|c| c.is_ascii_digit()),
            "unexpected VERSION() output: {last}"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    /// Live proof that MySQL (not just MariaDB) installs, initializes via
    /// `mysqld --initialize-insecure`, serves, and accepts a per-project database.
    ///   cargo test mysql_serves_and_creates_db -- --ignored --nocapture
    #[tokio::test]
    #[ignore = "downloads MySQL (~180MB), initializes, serves and creates a database"]
    async fn mysql_serves_and_creates_db() {
        use std::time::Duration;

        let dir = std::env::temp_dir().join("stackr-mysql-test").join("mysql");
        if mysql_daemon(&dir).is_none() {
            crate::download::download_and_extract(
                "https://cdn.mysql.com/archives/mysql-8.0/mysql-8.0.36-winx64.zip",
                &dir,
                |_, _| {},
            )
            .await
            .expect("mysql download");
        }
        // Sanity: the MySQL family resolved the right binaries (mysqld/mysql, no MariaDB).
        assert!(mysql_daemon(&dir).unwrap().ends_with("mysqld.exe"));
        ensure_mysql_data(&dir, &dir.join("data")).expect("init data dir");

        let daemon = mysql_daemon(&dir).expect("server binary");
        let client = mysql_client(&dir).expect("client binary");
        let mut server = {
            let mut cmd = Command::new(&daemon);
            cmd.current_dir(&dir).args(mysql_serve_args(&dir.join("data"), 3309));
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(CREATE_NO_WINDOW);
            }
            cmd.spawn().expect("spawn mysqld")
        };

        let db = sanitize_db_name("My Shop!");
        assert_eq!(db, "my_shop");
        let create = {
            let (dir, db) = (dir.clone(), db.clone());
            tokio::task::spawn_blocking(move || create_mysql_database(&dir, 3309, &db))
                .await
                .expect("join")
        };

        let mut listed = String::new();
        if create.is_ok() {
            for _ in 0..10 {
                let out = Command::new(&client)
                    .current_dir(&dir)
                    .args([
                        "--host=127.0.0.1",
                        "--port=3309",
                        "--protocol=TCP",
                        "-u",
                        "root",
                        "-e",
                        "SHOW DATABASES LIKE 'my_shop';",
                    ])
                    .output();
                if let Ok(out) = out {
                    listed = String::from_utf8_lossy(&out.stdout).to_string();
                    if listed.contains("my_shop") {
                        break;
                    }
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }

        // Graceful shutdown so the data dir isn't left mid-write.
        let _ = Command::new("taskkill")
            .args(["/PID", &server.id().to_string(), "/T", "/F"])
            .output();
        let _ = server.kill();
        let _ = server.wait();

        create.expect("create_mysql_database failed");
        assert!(listed.contains("my_shop"), "database not found; SHOW returned: {listed}");
    }

    /// Live proof that PostgreSQL installs (EnterpriseDB zip), initializes via
    /// `initdb`, serves, and accepts a per-project database via `createdb`.
    ///   cargo test postgres_serves_and_creates_db -- --ignored --nocapture
    #[tokio::test]
    #[ignore = "downloads PostgreSQL, initdb, serves and creates a database"]
    async fn postgres_serves_and_creates_db() {
        use std::time::Duration;

        let dir = std::env::temp_dir().join("stackr-postgres-test").join("pg");
        let initdb = dir.join("bin").join("initdb.exe");
        if !initdb.exists() {
            crate::download::download_and_extract(
                "https://get.enterprisedb.com/postgresql/postgresql-16.2-1-windows-x64-binaries.zip",
                &dir,
                |_, _| {},
            )
            .await
            .expect("postgres download");
        }
        assert!(initdb.exists(), "initdb.exe missing after extract (flatten failed?)");

        let data = dir.join("data");
        if !data.join("PG_VERSION").exists() {
            run(
                &initdb,
                &dir,
                &[
                    "-D".into(),
                    data.display().to_string(),
                    "-U".into(),
                    "postgres".into(),
                    "-A".into(),
                    "trust".into(),
                    "-E".into(),
                    "UTF8".into(),
                ],
            )
            .expect("initdb");
        }

        let postgres = dir.join("bin").join("postgres.exe");
        let mut server = {
            let mut cmd = Command::new(&postgres);
            cmd.current_dir(&dir)
                .args(["-D", &data.display().to_string(), "-p", "5434"]);
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(CREATE_NO_WINDOW);
            }
            cmd.spawn().expect("spawn postgres")
        };

        let db = sanitize_db_name("blog.test"); // -> "blog_test"
        assert_eq!(db, "blog_test");
        let create = {
            let (dir, db) = (dir.clone(), db.clone());
            tokio::task::spawn_blocking(move || create_postgres_database(&dir, 5434, &db))
                .await
                .expect("join")
        };

        // Verify the database is really there via psql.
        let psql = dir.join("bin").join("psql.exe");
        let mut listed = String::new();
        if create.is_ok() {
            for _ in 0..10 {
                let out = Command::new(&psql)
                    .current_dir(&dir)
                    .args([
                        "-h", "127.0.0.1", "-p", "5434", "-U", "postgres", "-tAc",
                        "SELECT datname FROM pg_database WHERE datname='blog_test';",
                    ])
                    .output();
                if let Ok(out) = out {
                    listed = String::from_utf8_lossy(&out.stdout).to_string();
                    if listed.contains("blog_test") {
                        break;
                    }
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }

        let _ = Command::new("taskkill")
            .args(["/PID", &server.id().to_string(), "/T", "/F"])
            .output();
        let _ = server.kill();
        let _ = server.wait();

        create.expect("create_postgres_database failed");
        assert!(listed.contains("blog_test"), "database not found; psql returned: {listed}");
    }
}
