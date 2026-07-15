# Stackr

**Stackr** is a native Windows desktop app for managing a local PHP development
environment — a from-scratch alternative to Laragon / XAMPP / MAMP with a modern,
IDE-grade dark UI. No Docker, no WSL: it downloads and runs **native Windows
binaries** of the web server, PHP, databases and cache engines, and wires them
together for you.

It starts **empty** — you install only the components your projects need, from
the UI. Everything lives under `C:\Stackr\` and nothing is installed globally
into the system.

Built on **Tauri 2 (Rust)** + **React 19 + TypeScript + Vite** + **Tailwind CSS v3**
+ **Zustand**.

---

## What it's for

A fast, offline, native control panel for PHP development on Windows. Stackr lets
you:

- Spin up a local web stack (Nginx **or** Apache + PHP via FastCGI) without
  hand-editing config files.
- Run **multiple PHP versions side by side**, each project served on its own
  version simultaneously.
- Install and manage **MySQL / MariaDB / PostgreSQL** and **Redis / Memcached**.
- Create projects from scratch, from a **framework** (Laravel, Symfony, WordPress,
  CodeIgniter, Yii2, CakePHP, Slim) or by **cloning a Git repo**, with the domain,
  vhost, database and `hosts` entry set up automatically.
- Open each project in the browser, in your IDE, or its folder — and tail logs
  from every service in one place.

### Who it's for

Windows-based PHP developers (agencies, freelancers, WordPress/Laravel shops) who
want Laragon-style convenience with a cleaner UI, true multi-version PHP, and a
fully offline, native footprint — without containers or virtualization.

### Typical use cases

- Maintain several client sites that each require a different PHP version, all
  running at once on `*.test` domains.
- Scaffold a new Laravel/WordPress site and have it live in the browser in under
  a minute.
- Clone an existing repository, pick its PHP version and database, and run it.
- Keep a single Redis/MySQL instance for local work, started on demand.

---

## Feature overview

### Servers (Nginx / Apache)
- Install **Nginx** or **Apache** (downloaded from official sources, latest build).
- One global active web server owns port 80; Start / Stop / Restart.
- **Port-conflict check** before starting — refuses to launch if the port is
  already bound and names the conflicting service.
- Built-in **config editor** for the generated master config (edits survive
  restarts; "Reset to default" regenerates).
- A catch-all `default_server` returns a clean 404 for unknown hosts instead of
  leaking another site's backend.

### PHP
- Install **multiple versions** (7.4 → latest), scraped live from
  `windows.php.net` releases + archives.
- **True multi-version runtime:** one `php-cgi` per minor series on its own
  FastCGI port (`9000 + major*10 + minor`), so every project runs on the PHP
  version it asks for — at the same time.
- A **default** version is the pre-selection for new projects (and the fallback
  when a project's chosen version is gone).
- **Extensions:** read from the real `ext/` directory, toggled on/off by editing
  `php.ini`, installed via PECL, searchable per version, with a "Show all
  extensions" browser.
- **One-click Xdebug** per version — downloads the matching DLL from the PECL
  Windows channel, loads it as a `zend_extension` and writes a sane step-debug
  config (`mode=debug`, `start_with_request=trigger` so normal requests stay
  fast, client `127.0.0.1:9003`). Works with any DBGp IDE (VS Code, PhpStorm, …);
  a copyable VS Code `launch.json` is provided.
- Built-in **`php.ini` editor**.

### Databases (MySQL / MariaDB / PostgreSQL)
- Curated version lists per engine; install **several versions**, with
  **one running at a time** per engine (they share the fixed port 3306 / 5432).
- Start / Stop / Restart / **Uninstall**.
- **Export before uninstall** — uninstalling a running engine offers to dump all
  databases first (`mysqldump --all-databases` / `pg_dumpall`) to
  `C:\Stackr\backups\`, so removing the only engine that can read your data never
  loses it.
- **Data lives outside the binaries** (`C:\Stackr\data\...`), so reinstalling or
  upgrading an engine keeps your databases; a legacy in-version data directory is
  migrated automatically on first run.
- First-run initialization handled automatically (`mysqld --initialize`,
  `initdb`), and daemons bind to `127.0.0.1` only.
- **Adminer** (one-click) — a single-file DB admin UI served at
  `http://adminer.test` through the active web server (Nginx **or** Apache) + PHP.
- **Per-project databases:** selecting an engine creates a schema named after the
  project. The version used is resolved as **running → newest installed**
  (MariaDB preferred over MySQL for the MySQL family).
- SQLite is supported by PHP directly (no service to run).

### Cache (Redis / Memcached)
- Install / Start / Stop / **Uninstall**, same unified card UI as Databases.
- Not project-scoped (one viable Windows build per engine).

### HTTPS (local CA)
- One toggle in Settings serves every project over **`https://`** with a
  locally-trusted certificate — no browser warnings.
- Stackr generates its own root CA, imports it into your **current-user** trust
  store (one Windows prompt, no admin), and signs a short leaf cert per project
  domain. Certs and the CA live under `config\ca` / `config\certs`; the CA key
  never leaves the machine.
- Works on both Nginx and Apache (loopback-only, dual-stack). Start or restart a
  project after enabling to serve it over HTTPS.

### Mail (Mailpit)
- One-click install of **Mailpit** — a single-binary mail catcher that traps every
  email your projects send (nothing leaves your machine).
- **SMTP on `127.0.0.1:1025`**, web **inbox on `127.0.0.1:8025`** (loopback only);
  Start / Stop / Restart / Uninstall like any engine, and auto-respawned by the
  watchdog if it dies.
- **Open inbox** button plus a copyable `.env` SMTP block to point Laravel/Symfony/
  any framework at it.

### Projects
- **New Project wizard** — three types:
  - **Blank PHP** → a Stackr-branded welcome page (offline, fonts embedded).
  - **Framework** → `composer create-project` (Laravel, Symfony, CodeIgniter,
    Yii2, CakePHP, Slim) or WordPress (`latest.zip`).
  - **Clone from Git** → clones a repo URL.
- Per project: name, auto domain `{name}.{tld}`, PHP version (from installed),
  optional database, and target folder (under the configurable sites directory).
- On each project row: **switch PHP version**, **switch database**, Open in
  browser, **Open in IDE** (auto-detects VS Code, VS Code Insiders, Cursor,
  PhpStorm, Sublime Text), **open a terminal**, open folder, Start / Stop, and
  Delete (with an opt-in "also delete files from disk" checkbox).
- **Open terminal** launches a shell in the project folder with the project's
  **PHP, Composer and Git already on `PATH`** (nothing changed globally). It runs
  outside the kill-on-close job, so it keeps working after Stackr exits.
- **Start** brings up the project's own PHP runtime, (re)writes its vhost **to
  every installed web server** (so switching the active server never 404s),
  starts the web server + its database, registers the host, and opens the browser.

### Logs
- Per-service tabs plus an **"All"** merged stream, color-coded by source.
- Parsed timestamps, level-colored lines, single-line truncated with
  **click-to-expand**, auto-scroll, and Clear.
- Reads are async + tail-only (last 256 KB) so the UI never blocks.

### Settings
- Toggles: launch Stackr at Windows login, auto-start services, desktop
  notifications.
- **Sites directory** (native folder picker) — where new projects are created.
- **Local TLD** — presets for `.test` (default) and **`.localhost`**; the latter
  is resolved by browsers with no `hosts` edit and **no UAC prompt** (RFC 6761).
  Still editable to any custom suffix.
- **System** diagnostics card: Windows version, WebView2 runtime version, and
  Visual C++ runtime status.

### Updates
- **In-app auto-update** (signed) — Stackr checks for a newer release on launch
  and offers a one-click **Download & install** in Settings → Updates, then
  relaunches. Updates are cryptographically verified against a bundled public key.
- See [`RELEASING.md`](RELEASING.md) for how releases are built and signed.

### Desktop integration
- Custom **frameless title bar** with native min / maximize / close controls
  (no white OS chrome).
- Behaves like a native app, not a web page: no right-click context menu, no zoom,
  and reload/devtools disabled in release builds.
- **System tray** — closing the window hides to tray (services keep running);
  Quit truly exits.
- **Fully offline** — all fonts and icons are bundled; no CDN/Google Fonts
  requests anywhere, including generated project pages.

---

## How it works

- **Configurable data folder** — on first run Stackr asks where to keep everything
  and remembers it via a pointer in `%APPDATA%\Stackr\root.txt`; the default is
  `C:\Stackr\`. Existing installs are never prompted. The current folder is shown
  in Settings.
- **Filesystem layout** — everything under the chosen data folder (default `C:\Stackr\`):
  ```
  C:\Stackr\
  ├── bin\        # installed engines:  php\8.2.31\, nginx\1.31.2\, mysql\…, redis\…
  │               #   + .downloads\ (verified scratch) and per-version .installed markers
  ├── data\       # database data dirs, separate from the binaries (mysql\, postgresql\…)
  ├── www\        # project sites (configurable)
  ├── config\     # generated nginx/apache master + per-site vhosts, php.ini
  ├── logs\       # per-service logs
  ├── tools\      # Adminer
  ├── backups\    # SQL dumps exported before a DB engine uninstall
  ├── stackr.json # persisted state: installed components, projects, settings
  └── stackr.json.bak  # atomic-save backup
  ```
- **Process management** — a registry tracks running services and their restart
  specs. **Services** (web server, php-cgi, DB, cache) are tied to a Windows **job
  object** so they die with Stackr; the **project terminal** is intentionally
  outside it and survives. A background **watchdog** (3 s) auto-respawns crashed
  services with a crash-loop cap. Console windows are suppressed (`CREATE_NO_WINDOW`).
- **Loopback only, dual-stack** — no engine listens on `0.0.0.0`; everything binds
  `127.0.0.1` **and** `[::1]` (browsers resolve `*.localhost` to IPv6). That
  loopback boundary is the security barrier (so the DB root user stays passwordless,
  Laragon-style).
- **PHP on Windows** uses `php-cgi.exe` in FastCGI mode (Windows has no
  `php-fpm`); the web server proxies `.php` to the matching port. `PHP_FCGI_CHILDREN`
  gives concurrency and `PHP_FCGI_MAX_REQUESTS=0` prevents worker self-termination.
- **Virtual hosts** are generated per project (and per tool, e.g. Adminer); the
  master config simply `include`s the `sites/` directory.
- **`hosts` file** entries are added/removed with a UAC elevation step when needed
  — but `*.localhost` domains skip this entirely (no edit, no prompt).
- **Version manifest** — a public catalog
  ([stackr-manifest](https://github.com/igrdkl/stackr-manifest)) is the authoritative
  source of component download URLs and their SHA-256. Stackr fetches it
  network-first, caches it to `config\manifest.json` (used offline), and falls back
  to scraping official sites when a build isn't listed.
- **Download integrity** — archives download to a scratch dir and, when the manifest
  provides a checksum, are verified by **SHA-256 before extraction**; each version
  gets an `.installed` marker and broken/partial installs are swept on startup.
- **Resilience** — `stackr.json` is written atomically (temp → `.bak` → rename) and
  falls back to the backup if corrupt. On startup Stackr resets stale "running"
  flags and prunes orphan vhosts so leftovers can't cause stale-backend errors.

---

## System requirements

- **Windows 10 (v1803 / build 17134) or Windows 11**, x64.
- **WebView2 Runtime** — auto-installed by the app installer if missing.
- **Microsoft Visual C++ 2015–2022 x64 Redistributable** — required by the
  bundled engines (PHP, MySQL, …). Stackr detects it and warns if it's missing
  (Settings → System).
- Administrator rights are requested only when editing the `hosts` file.

---

## Development

```bash
npm install
npm run dev          # Vite-only UI in the browser (http://localhost:1420)
npm run tauri dev    # full desktop app (requires the Rust toolchain)
npm run build        # type-check (tsc) + production frontend build
npm run tauri build  # package the Windows app (.msi / .exe)

cargo test --manifest-path src-tauri/Cargo.toml --lib   # Rust unit tests
```

**CI** ([`.github/workflows/ci.yml`](.github/workflows/ci.yml)) builds the
frontend, runs the Rust unit tests and compiles a release binary on both
`windows-2019` and `windows-2022`, covering the Windows 10 and 11 toolchains.

### Tech stack

| Layer        | Choice                                                        |
|--------------|---------------------------------------------------------------|
| Shell        | Tauri 2 (Rust backend + WebView2 frontend)                    |
| Frontend     | React 19, TypeScript, Vite 7                                  |
| Styling      | Tailwind CSS v3 (tokens in `tailwind.config.js`)              |
| State        | Zustand (`src/store/useStore.ts`)                             |
| Icons        | lucide-react (inline SVG)                                     |
| Fonts        | Geist + JetBrains Mono via `@fontsource` (bundled, offline)   |
| Rust crates  | tauri, tokio, reqwest, zip, winreg, webview2-com, base64, sha2, hex, rcgen |

### Project layout

```
src/
├── components/
│   ├── Layout/        # Titlebar, Sidebar
│   ├── ui/            # Toggle, Monogram, Select, Spinner, ProgressBar,
│   │                  #   ServiceCards, ModalBackdrop, Logo, Toaster…
│   ├── InstallModal.tsx
│   ├── NewProjectWizard.tsx
│   ├── ConfigEditorModal.tsx
│   ├── AllExtensionsModal.tsx
│   └── ConfirmDialog.tsx
├── pages/             # Servers, PHP, Databases, Cache, Projects, Logs, Settings
├── store/useStore.ts  # global Zustand state + all Tauri calls
├── data/catalog.ts    # static catalogs (engines, extensions, frameworks)
├── lib/               # api.ts (invoke wrappers), cn(), styles, desktopShell
└── types/index.ts

src-tauri/
├── src/
│   ├── lib.rs         # Tauri builder, command registration, WebView2 lockdown
│   ├── commands/      # services, php, projects, downloader, config, logs, tools
│   ├── config_gen.rs  # nginx/apache master + per-site vhost generation
│   ├── db.rs          # MySQL/MariaDB/PostgreSQL init + database creation
│   ├── scaffold.rs    # framework scaffolding, runtime php.ini
│   ├── php_ini.rs     # php.ini parsing / extension toggling
│   ├── hosts.rs       # hosts-file management (with UAC)
│   ├── job.rs         # kill-on-close job object
│   ├── autostart.rs   # launch-at-login registry entry
│   ├── tray.rs        # system tray
│   ├── sysreq.rs      # VC++/Windows/WebView2 system report
│   ├── paths.rs       # C:\Stackr layout
│   └── state.rs       # stackr.json persistence + settings
├── assets/fonts/      # embedded woff2 for the offline welcome page
└── tauri.conf.json
```

The high-fidelity UI matches the designs in [`DESIGN/`](DESIGN/) 1:1 (dark theme).
See [`STATUS.md`](STATUS.md) for the detailed build status and design notes.
