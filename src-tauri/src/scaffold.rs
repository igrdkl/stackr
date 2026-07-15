//! Project scaffolding: Composer-based frameworks, WordPress, and Git clones.
//! Network + child-process work lives here; `commands::projects` orchestrates it
//! and emits progress events.

use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Relative document-root subdirectory for a project, by framework/type.
/// An empty string means the project root itself (e.g. WordPress).
pub fn doc_root_subdir(framework: Option<&str>, ptype: &str) -> &'static str {
    match framework {
        Some("Yii2") => "web",
        Some("CakePHP") => "webroot",
        Some("WordPress") => "",
        // Laravel, Symfony, CodeIgniter, Slim, Phalcon, …
        Some(_) => "public",
        None => {
            if ptype == "Clone from Git" {
                ""
            } else {
                "public"
            }
        }
    }
}

/// Composer package for a framework, or `None` if it isn't Composer-installable
/// (WordPress is handled separately; Phalcon is not supported).
pub fn composer_package(framework: &str) -> Option<&'static str> {
    match framework {
        "Laravel" => Some("laravel/laravel"),
        "Symfony" => Some("symfony/skeleton"),
        "CodeIgniter" => Some("codeigniter4/appstarter"),
        "Yii2" => Some("yiisoft/yii2-app-basic"),
        "CakePHP" => Some("cakephp/app"),
        "Slim" => Some("slim/slim-skeleton"),
        _ => None,
    }
}

/// `C:\Stackr\bin\composer`
pub fn composer_dir() -> PathBuf {
    crate::paths::bin_root().join("composer")
}

/// `C:\Stackr\bin\composer\composer.phar`
pub fn composer_phar() -> PathBuf {
    composer_dir().join("composer.phar")
}

/// Download `composer.phar` if it isn't already present; returns its path.
pub async fn ensure_composer() -> Result<PathBuf, String> {
    let phar = composer_phar();
    if phar.exists() {
        return Ok(phar);
    }
    crate::paths::ensure_dir(&composer_dir()).map_err(|e| e.to_string())?;
    crate::download::download_file("https://getcomposer.org/composer-stable.phar", &phar).await?;
    Ok(phar)
}

/// Write a `composer` shim (`composer.bat`) next to `composer.phar` so `composer`
/// resolves on a project terminal's PATH — the phar isn't runnable on its own.
/// It calls whatever `php` is first on PATH (i.e. the project's PHP).
pub fn ensure_composer_shim() {
    let dir = composer_dir();
    let _ = crate::paths::ensure_dir(&dir);
    let _ = std::fs::write(dir.join("composer.bat"), "@php \"%~dp0composer.phar\" %*\r\n");
}

/// Path to the portable MinGit `cmd` dir (holding `git.exe`), if it's installed —
/// so a project terminal can get `git` on PATH when there's no system git.
pub fn portable_git_cmd_dir() -> Option<PathBuf> {
    let dir = crate::paths::component_dir("git", MINGIT_VERSION).join("cmd");
    dir.exists().then_some(dir)
}

/// Extensions Composer, framework post-install scripts, and DB tooling (Adminer)
/// rely on. Only those whose DLL is present get enabled.
const RUNTIME_EXTENSIONS: &[&str] = &[
    "openssl", "mbstring", "curl", "zip", "fileinfo", "pdo_sqlite", "pdo_mysql",
    "mysqli", "pgsql", "pdo_pgsql", "gd", "intl", "exif", "sodium", "bcmath",
];

/// Ensure `<php_dir>\php.ini` exists with a correct absolute `extension_dir`.
/// On first creation it also enables the common runtime extensions (DB drivers,
/// openssl, mbstring, …) whose DLL is present — php.exe auto-loads php.ini from
/// its own directory, so Composer and the children it spawns inherit them.
/// On later calls it only keeps `extension_dir` correct and **never** re-enables,
/// so the user's choices in the Extensions panel are respected.
pub fn ensure_php_runtime_ini(php_dir: &Path) -> Result<(), String> {
    let ini = php_dir.join("php.ini");
    let existed = ini.exists();
    let mut content = if existed {
        std::fs::read_to_string(&ini).map_err(|e| e.to_string())?
    } else {
        ["php.ini-development", "php.ini-production"]
            .iter()
            .map(|c| php_dir.join(c))
            .find(|p| p.exists())
            .map(|p| std::fs::read_to_string(p))
            .transpose()
            .map_err(|e| e.to_string())?
            .unwrap_or_default()
    };

    let ext_dir = php_dir.join("ext");
    let ext_dir_val = format!("\"{}\"", ext_dir.to_string_lossy().replace('\\', "/"));
    content = crate::php_ini::set_kv(&content, "extension_dir", &ext_dir_val);

    if !existed {
        for ext in RUNTIME_EXTENSIONS {
            if ext_dir.join(format!("php_{ext}.dll")).exists() {
                content = crate::php_ini::set_extension(&content, ext, true);
            }
        }
    }
    std::fs::write(&ini, content).map_err(|e| e.to_string())
}

/// Run a program to completion, returning a trimmed error on non-zero exit.
/// `program` may be an absolute path or a name resolved via PATH (e.g. `git`).
fn run(program: &str, cwd: &Path, args: &[String], env: &[(&str, String)]) -> Result<(), String> {
    let mut cmd = Command::new(program);
    cmd.current_dir(cwd).args(args);
    for (k, v) in env {
        cmd.env(k, v);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = match cmd.output() {
        Ok(o) => o,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(format!("'{program}' was not found — is it installed and on PATH?"))
        }
        Err(e) => return Err(e.to_string()),
    };
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let stdout = String::from_utf8_lossy(&out.stdout);
        let raw = if !stderr.trim().is_empty() { stderr } else { stdout };
        // Keep the last few lines — that's where the actionable error usually is.
        let tail: Vec<&str> = raw.lines().filter(|l| !l.trim().is_empty()).collect();
        let msg = tail[tail.len().saturating_sub(8)..].join("\n");
        return Err(if msg.is_empty() {
            format!("'{program}' exited with status {}", out.status)
        } else {
            msg
        });
    }
    Ok(())
}

/// `php composer.phar create-project <package> <dest>` with a private
/// COMPOSER_HOME. Runs from `dest`'s parent so the target may be created fresh.
pub fn run_composer_create(
    php_exe: &Path,
    php_dir: &Path,
    composer_phar: &Path,
    package: &str,
    dest: &Path,
) -> Result<(), String> {
    ensure_php_runtime_ini(php_dir)?;
    let cwd = dest.parent().unwrap_or(dest);
    crate::paths::ensure_dir(cwd).map_err(|e| e.to_string())?;
    let args = vec![
        composer_phar.to_string_lossy().to_string(),
        "create-project".into(),
        package.to_string(),
        dest.to_string_lossy().to_string(),
        "--no-interaction".into(),
        "--prefer-dist".into(),
        "--no-progress".into(),
    ];
    run(&php_exe.to_string_lossy(), cwd, &args, &composer_env())
}

/// `php composer.phar install` inside an existing project (used after a clone).
pub fn run_composer_install(
    php_exe: &Path,
    php_dir: &Path,
    composer_phar: &Path,
    dest: &Path,
) -> Result<(), String> {
    ensure_php_runtime_ini(php_dir)?;
    let args = vec![
        composer_phar.to_string_lossy().to_string(),
        "install".into(),
        "--no-interaction".into(),
        "--prefer-dist".into(),
        "--no-progress".into(),
    ];
    run(&php_exe.to_string_lossy(), dest, &args, &composer_env())
}

fn composer_env() -> Vec<(&'static str, String)> {
    let home = composer_dir().join("home");
    let _ = crate::paths::ensure_dir(&home);
    // Composer 2.9+ refuses to install package versions flagged by Packagist
    // security advisories, which blocks scaffolding older framework majors
    // (e.g. Laravel 10). Stackr is a local dev tool where the user deliberately
    // picks the version, so relax the policy in our managed global config. Both
    // the 2.10+ key (`policy.advisories.block`) and the 2.9 legacy key
    // (`audit.block-insecure`) are set for cross-version compatibility.
    // `--no-audit` / COMPOSER_NO_AUDIT do NOT bypass the block (composer#12607).
    let cfg = r#"{
    "config": {
        "audit": { "block-insecure": false },
        "policy": { "advisories": { "block": false } }
    }
}
"#;
    let _ = std::fs::write(home.join("config.json"), cfg);
    vec![("COMPOSER_HOME", home.to_string_lossy().to_string())]
}

// Portable MinGit (Git for Windows, minimal build) — downloaded on demand into
// bin/git when no system git is on PATH, so "Clone from Git" works with zero
// preinstalled prerequisites (Stackr's whole model). git.exe lives at cmd/git.exe.
const MINGIT_VERSION: &str = "2.55.0.2";
const MINGIT_URL: &str =
    "https://github.com/git-for-windows/git/releases/download/v2.55.0.windows.2/MinGit-2.55.0.2-64-bit.zip";

/// Whether a usable `git` is on the system PATH.
fn system_git_available() -> bool {
    let mut cmd = Command::new("git");
    cmd.arg("--version");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    matches!(cmd.output(), Ok(o) if o.status.success())
}

/// Resolve a git executable to clone with: prefer system git; otherwise download
/// a portable MinGit into `bin/git` and return the path to its git.exe. Lets
/// "Clone from Git" work on a machine with no git installed.
pub async fn ensure_git() -> Result<String, String> {
    if system_git_available() {
        return Ok("git".to_string());
    }
    let dir = crate::paths::component_dir("git", MINGIT_VERSION);
    let exe = dir.join("cmd").join("git.exe");
    if !exe.exists() {
        crate::download::download_and_extract(MINGIT_URL, &dir, |_, _| {}).await?;
    }
    if !exe.exists() {
        return Err("portable Git was downloaded but git.exe is missing".to_string());
    }
    Ok(exe.to_string_lossy().to_string())
}

/// `git clone <url> <dest>` using the resolved git executable (system or portable).
pub fn clone_git(git: &str, url: &str, dest: &Path) -> Result<(), String> {
    let parent = dest.parent().unwrap_or(dest);
    crate::paths::ensure_dir(parent).map_err(|e| e.to_string())?;
    run(
        git,
        parent,
        &[
            "clone".into(),
            url.to_string(),
            dest.to_string_lossy().to_string(),
        ],
        &[],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doc_roots_match_frameworks() {
        assert_eq!(doc_root_subdir(Some("Laravel"), "Framework"), "public");
        assert_eq!(doc_root_subdir(Some("Yii2"), "Framework"), "web");
        assert_eq!(doc_root_subdir(Some("CakePHP"), "Framework"), "webroot");
        assert_eq!(doc_root_subdir(Some("WordPress"), "Framework"), "");
        assert_eq!(doc_root_subdir(None, "Blank PHP"), "public");
        assert_eq!(doc_root_subdir(None, "Clone from Git"), "");
    }

    #[test]
    fn composer_packages_resolve() {
        assert_eq!(composer_package("Laravel"), Some("laravel/laravel"));
        assert_eq!(composer_package("Slim"), Some("slim/slim-skeleton"));
        assert_eq!(composer_package("WordPress"), None);
        assert_eq!(composer_package("Phalcon"), None);
    }

    /// Full live proof of framework scaffolding: download PHP + nginx + Composer,
    /// `composer create-project laravel/laravel`, then serve the app over
    /// nginx + php-cgi and assert the Laravel welcome page renders. Exercises the
    /// php.ini fix (Laravel's `@php artisan key:generate` post-script needs
    /// extensions in a child process). Heavy + networked, so `#[ignore]`d:
    ///   cargo test scaffolds_and_serves_laravel -- --ignored --nocapture
    #[tokio::test]
    #[ignore = "downloads php+nginx+composer, scaffolds Laravel and serves it"]
    async fn scaffolds_and_serves_laravel() {
        use std::process::Command;
        use std::time::Duration;

        let fwd = |p: &Path| p.to_string_lossy().replace('\\', "/");

        let base = std::env::temp_dir().join("stackr-laravel-test");
        let _ = std::fs::remove_dir_all(&base);
        let php_dir = base.join("php");
        let nginx_dir = base.join("nginx");
        let app_dir = base.join("www").join("lara");
        let sites = base.join("sites");
        let logs = base.join("logs");
        std::fs::create_dir_all(&sites).unwrap();
        std::fs::create_dir_all(&logs).unwrap();

        crate::download::download_and_extract(
            "https://windows.php.net/downloads/releases/archives/php-8.3.4-Win32-vs16-x64.zip",
            &php_dir,
            |_, _| {},
        )
        .await
        .expect("php download");
        crate::download::download_and_extract(
            "https://nginx.org/download/nginx-1.27.3.zip",
            &nginx_dir,
            |_, _| {},
        )
        .await
        .expect("nginx download");

        let phar = base.join("composer.phar");
        crate::download::download_file("https://getcomposer.org/composer-stable.phar", &phar)
            .await
            .expect("composer download");

        // The feature under test: scaffold Laravel via Composer.
        let php_exe = php_dir.join("php.exe");
        run_composer_create(&php_exe, &php_dir, &phar, "laravel/laravel", &app_dir)
            .expect("composer create-project laravel/laravel");

        assert!(app_dir.join("vendor").is_dir(), "vendor/ must exist");
        assert!(app_dir.join("artisan").exists(), "artisan must exist");
        assert!(
            app_dir.join("public").join("index.php").exists(),
            "public/index.php must exist"
        );
        assert!(app_dir.join(".env").exists(), ".env must be created");

        // Serve it.
        let public = app_dir.join("public");
        std::fs::write(sites.join("lara.conf"), crate::config_gen::nginx_vhost("lara.test", &public, 8089, 9000, None)).unwrap();
        let glob = format!("{}/*.conf", fwd(&sites));
        let conf_path = base.join("nginx.conf");
        std::fs::write(
            &conf_path,
            crate::config_gen::nginx_master_conf(&nginx_dir, &glob, &logs),
        )
        .unwrap();

        let mut php = Command::new(php_dir.join("php-cgi.exe"))
            .args(["-b", "127.0.0.1:9000"])
            .current_dir(&php_dir)
            .spawn()
            .expect("spawn php-cgi");
        let mut ng = Command::new(nginx_dir.join("nginx.exe"))
            .args(["-p", &fwd(&nginx_dir), "-c", &fwd(&conf_path)])
            .current_dir(&nginx_dir)
            .spawn()
            .expect("spawn nginx");

        tokio::time::sleep(Duration::from_millis(2500)).await;

        let res = reqwest::Client::new()
            .get("http://127.0.0.1:8089/")
            .header("Host", "lara.test")
            .send()
            .await;

        let _ = Command::new(nginx_dir.join("nginx.exe"))
            .args(["-p", &fwd(&nginx_dir), "-s", "stop"])
            .current_dir(&nginx_dir)
            .output();
        let _ = ng.kill();
        let _ = ng.wait();
        let _ = php.kill();
        let _ = php.wait();

        let resp = res.expect("HTTP request failed");
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        assert!(status.is_success(), "expected 2xx, got {status}; body: {body}");
        assert!(
            body.contains("Laravel"),
            "expected the Laravel welcome page, got: {}",
            &body.chars().take(400).collect::<String>()
        );

        let _ = std::fs::remove_dir_all(&base);
    }
}
