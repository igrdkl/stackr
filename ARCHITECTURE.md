# Stackr — Архітектура

Детальний опис того, що робить проект, з яких вузлів складається, як вони
взаємодіють і навіщо кожен потрібен. Документ описує **фактичну** реалізацію
(не первісну специфікацію з `CLAUDE.md`).

- Користувацький огляд можливостей → [`README.md`](README.md)
- Поточний стан / журнал робіт → [`STATUS.md`](STATUS.md)

---

## 1. Що це таке

**Stackr** — десктопний менеджер локального середовища веб-розробки для Windows
(нативна альтернатива Laragon/XAMPP). Запускається «чистим»: користувач сам через
UI встановлює потрібні компоненти (PHP, веб-сервер, БД, кеш), створює проекти й
запускає їх на локальних доменах `*.test`.

Головні принципи:

- **Нічого не ставиться в систему.** Усі бінарники — портативні, лежать у
  `C:\Stackr\bin\...`. Системний PATH не змінюється; усе запускається за явними
  шляхами.
- **Без Docker і без WSL** — тільки нативні Windows-бінарники.
- **Програма ставить усе сама** — завантажує ZIP-архіви з офіційних джерел,
  розпаковує, генерує конфіги, керує процесами.

---

## 2. Технологічний стек

| Шар | Технологія |
|-----|-----------|
| Оболонка | **Tauri 2** (нативне вікно + WebView2, фронтенд у вебв'ю, бекенд на Rust) |
| Фронтенд | **React 19 + TypeScript + Vite**, стилі **Tailwind v3**, іконки **lucide-react** |
| Стан фронтенду | **Zustand** (один глобальний store) |
| Бекенд | **Rust** — `tokio`, `serde`, `reqwest` (rustls), `zip`, `winreg`, `webview2-com`/`windows`, `sha2`+`hex` (цілісність завантажень) |
| Збірка/інсталятор | Vite → `tauri build` → **NSIS** (`Stackr_x64-setup.exe`, per-user) |
| Оновлення | **Tauri updater** (підписаний; `latest.json` у GitHub Releases) + `tauri-plugin-process` для релончу |

Роутингу немає — активна вкладка тримається в стані (`nav`).

---

## 3. Архітектура згори

```
┌─────────────────────────────────────────────────────────────┐
│  Нативне вікно Tauri (frameless + власний titlebar)           │
│  ┌───────────────────────────────────────────────────────┐  │
│  │  WebView2  ── React UI (сторінки, модалки, Zustand)    │  │
│  └───────────────────────────────────────────────────────┘  │
│            │  invoke(cmd)            ▲  events                │
│            ▼  (Tauri IPC)            │  (download-progress …)  │
│  ┌───────────────────────────────────────────────────────┐  │
│  │  Rust backend (#[tauri::command])                      │  │
│  │   ├─ керує процесами (nginx / php-cgi / mysql …)       │  │
│  │   ├─ завантажує+розпаковує бінарники                   │  │
│  │   ├─ генерує конфіги (vhost / nginx.conf / php.ini)    │  │
│  │   └─ читає/пише стан у C:\Stackr\stackr.json           │  │
│  └───────────────────────────────────────────────────────┘  │
└──────────────┬──────────────────────────┬───────────────────┘
               ▼                          ▼
   Дочірні процеси (job object)   Файлова система  C:\Stackr\
   nginx.exe, php-cgi.exe,        bin / www / config / logs …
   mariadbd.exe, redis-server …
```

- Фронтенд **ніколи** не чіпає файли/процеси напряму — лише через `invoke(...)`
  до Rust-команд.
- Rust спавнить довгоживучі процеси й тримає їх у реєстрі в пам'яті
  (`ProcessRegistry`) разом зі **специфікаціями рестарту** (`SpawnPlan`).
  **Сервіси** (nginx / php-cgi / БД / кеш) прив'язані до **job object**
  (kill-on-close) і не переживають Stackr; **термінал проекту** навмисно
  **не** в job-об'єкті — тож він живе далі після виходу зі Stackr (сам процес
  Stackr також не в job).
- Фоновий **watchdog** (окремий потік, опитування раз на 3 с) стежить за
  сервісами й **автоматично перепіднімає** ті, що впали, з обмеженням на
  crash-loop; статуси течуть у UI подією `service-status-changed`.
- Прогрес завантажень/встановлень тече назад у UI **подіями** Tauri.

---

## 4. Розкладка на диску (`C:\Stackr\`)

```
C:\Stackr\
├── bin\               портативні бінарники, по версіях
│   ├── php\8.2.32\        php.exe, php-cgi.exe, ext\, php.ini
│   ├── nginx\1.27.3\
│   ├── apache\2.4.68\
│   ├── mariadb\… , mysql\… , postgresql\…
│   ├── redis\… , memcached\…
│   ├── composer\          composer.phar + composer.bat (шим) + home\config.json
│   ├── git\2.55.0.2\      portable MinGit (лише якщо системного git нема)
│   └── .downloads\        скретч для завантажень (ZIP тут перевіряється, тоді розпаковка)
│       + .installed маркер у кожній версії (sha256 + url)
├── data\              дані БД, ОКРЕМО від бінарників (переживають реінсталл рушія)
│   ├── mysql\ , mariadb\
│   └── postgresql\<major>\
├── www\               папки проектів (для створених у Stackr)
│   └── my-app\public\index.php
├── config\            згенеровані конфіги (переживають рестарти)
│   ├── nginx\nginx.conf  +  sites\<domain>.conf  (по одному на проект)
│   └── apache\httpd.conf +  sites\<domain>.conf
├── logs\              nginx\ apache\ mysql\ + php.log, redis.log …
├── tools\adminer\     веб-корінь Adminer (index.php)
├── backups\           SQL-дампи (export-before-uninstall для БД)
├── stackr.json        головний стан: installed / defaultPhp / projects / settings
└── stackr.json.bak    резервна копія (atomic save: tmp → .bak → rename)
```

**Інваріант:** vhost-файл у `config\{server}\sites\` існує ⟺ проект зараз
обслуговується. Створення проекту vhost НЕ пише — він з'являється при
`start_project` і зникає при `stop`/`delete`. Осиротілі vhost прибираються на
старті (`prune_orphan_sites`).

---

## 5. Бекенд (Rust) — вузли

Точка входу: `main.rs` → `stackr_lib::run()` у **`lib.rs`**, який будує
`tauri::Builder`, реєструє всі команди (`generate_handler!`), піднімає трей,
чистить осиротілі vhost, застосовує WebView-lockdown і задає поведінку
«ховатись у трей при закритті».

### 5.1 Інфраструктура / ядро

| Модуль | Навіщо |
|--------|--------|
| `lib.rs` | Складання застосунку, реєстрація команд, setup-хуки, lockdown WebView2, hide-to-tray. |
| `paths.rs` | Єдине джерело правди про розкладку даних + `data_root`, `mysql_data_dir`/`postgres_data_dir` (дані БД поза версіями), `downloads_dir`, `install_marker`, `backups_dir`. **Конфігурований корінь**: `root()` резолвиться з вказівника `%APPDATA%\Stackr\root.txt` (кеш у `RwLock`), дефолт `C:\Stackr`; `is_first_run`/`set_root` для first-run пікера (свіжа інсталяція = нема вказівника + порожній дефолт-корінь). |
| `state.rs` | `AppState` (installed / default_php / projects / settings), `StateStore(Mutex)`. **Атомний save**: tmp → copy у `.bak` → rename з ретраями; **load** падає на `.bak` при відсутності/пошкодженні й піднімає прапор `restored_from_backup` (одноразово читає команда `take_restore_notice` → тост у UI). |
| `download.rs` | `download_and_extract_checked` — стрім ZIP у `bin\.downloads`, рахує **SHA-256**, **звіряє до розпаковки** (mismatch = видалити + помилка), повертає дайджест; `download_and_extract` — тонка обгортка (без очікуваного хешу). |
| `manifest.rs` | Клієнт **version-маніфесту** (`github.com/igrdkl/stackr-manifest`): `lookup(component, version)` дає авторитетні `(url, sha256)`. Network-first → кеш `config\manifest.json` → офлайн-кеш → (в інсталері) фолбек на скрейпінг. Session-кеш, короткий таймаут — стейл/офлайн-маніфест не блокує встановлення. |
| `job.rs` | Windows **JobObject** (kill-on-close) — прив'язує лише **сервіси**; термінал і сам Stackr свідомо поза job. |
| `tray.rs` | Системний трей (Show / Quit). |
| `autostart.rs` | Автозапуск з Windows через реєстр (`winreg`, ключ Run). |
| `sysreq.rs` | Звіт про передумови хоста: VC++ Redistributable, версія Windows, версія WebView2. |
| `hosts.rs` | Додавання/видалення записів у `hosts` (потребує елевації); **no-op для `localhost`/`*.localhost`** (RFC 6761 — резолвляться без hosts, без UAC). |
| `tls.rs` | **HTTPS**: локальний root CA + leaf-сертифікати на домен (`rcgen`, бекенд `ring`). `ensure_ca`/`ensure_domain_cert`, довіра через `certutil -user` (без адміна), команди `https_status`/`enable_https`/`disable_https`. Ключ CA не покидає машину. |

### 5.2 Доменна логіка (без IPC)

| Модуль | Навіщо |
|--------|--------|
| `models.rs` | Типи домену: `Project`, `ProjectConfig`, `PhpVersion`, `PhpExtension` (serde camelCase — спільні з фронтендом). |
| `config_gen.rs` | Генератори конфігів: майстер-`nginx.conf` (з catch-all `return 404`), per-site nginx vhost, Apache vhost + `httpd.conf` (правка `SRVROOT`, mod_proxy_fcgi). **Dual-stack loopback**: усі vhost + catch-all + Apache `Listen` слухають і `127.0.0.1`, і `[::1]` (браузери резолвлять `*.localhost` у `::1`), нічого не на 0.0.0.0. **HTTPS**: коли ввімкнено, vhost отримує додатковий `listen 443 ssl` / `<VirtualHost :443>` з сертифікатом; Apache — окремий `_ssl.conf` бутстрап (`apache_ssl_bootstrap`). `ensure_*_master` пише лише якщо файлу нема (не затирає ручні правки). |
| `php_ini.rs` | Парсинг/редагування `php.ini`: `set_kv`, `set_extension` (`extension=...`), `is_zend` (Xdebug вантажиться як `zend_extension`). |
| `db.rs` | Ініціалізація даних MySQL/MariaDB/PostgreSQL у `data\` (з міграцією легасі-датадіру), запуск демона з `--bind-address=127.0.0.1` / `listen_addresses=127.0.0.1`, `create_*_database`, `sanitize_db_name`, `copy_dir_all`. **`export_databases`** (`#[tauri::command]`): дамп усіх БД запущеного рушія (`mariadb-dump`/`mysqldump --all-databases` / `pg_dumpall`) стрімом у `backups\` — для export-before-uninstall. |
| `scaffold.rs` | Скафолдинг: `composer.phar` (`ensure_composer` + керований `config.json`, що знімає блок security-advisories) + **`composer.bat` шим** (`ensure_composer_shim`), `run_composer_create`/`install`, Composer-пакети фреймворків, `clone_git`, **portable MinGit** (`ensure_git`, `portable_git_cmd_dir`), `ensure_php_runtime_ini`. |

### 5.3 Команди (`#[tauri::command]`, шар IPC у `commands/`)

| Модуль | Команди / відповідальність |
|--------|----------------------------|
| `services.rs` | **Реєстр процесів** (`ProcessRegistry`: `Managed` + `SpawnPlan`/`Respawn` специфікації) + життєвий цикл усіх рушіїв: `start/stop/restart_service`, перевірка зайнятості порту, `reload_server`. **Watchdog** (`start_watchdog`): фоновий потік перепіднімає впалі сервіси (cap на crash-loop) + шле `service-status-changed`. **Level-2 health**: `port_responds` → статус running / starting (у межах startup-grace) / unhealthy / stopped. Усі рушії біндяться на loopback (redis `--bind`, memcached `-l`, postgres `listen_addresses`). **php-cgi на версію**: `ensure_php_runtime` піднімає `php-cgi.exe` на порту `9000+major*10+minor` з `PHP_FCGI_MAX_REQUESTS=0` (без самозавершення) + `PHP_FCGI_CHILDREN=4`; `restart_php_runtime_if_running`. **Mailpit**: single-binary mail catcher (SMTP `1025` / UI `8025`, loopback), watchdog-respawnable. `get_servers/databases/caches/mail`. |
| `downloader.rs` | `install_component` / `uninstall_component`, резолвери URL для кожного компонента, `get_php_available` (скрейпить windows.php.net). Пише `.installed` маркер (sha256 + url) і `prune_broken_installs` на старті (чистить `.downloads`, зносить версії без запису/маркера). Шле `download-progress`. |
| `php.rs` | `get_php_versions`, розширення (`list_php_extensions`, `toggle_extension`, `install_php_extension` через PECL), `read_php_ini`/`save_php_ini`; **Xdebug**: `xdebug_status` / `set_xdebug` (тягне DLL з PECL, вантажить `zend_extension`, пише step-debug конфіг, рестартить рушій лише якщо живий). |
| `projects.rs` | Життєвий цикл проектів: `create_project` (Blank / Framework / Clone from Git / **Open existing**), `start_project` (піднімає php-cgi + сервер, пише vhost у **кожен** встановлений сервер, чекає готовності, відкриває браузер), `stop`/`delete_project`, `set_project_php`/`set_project_db`, `detect_doc_root`, IDE (`detect_ides`/`open_in_ide`), **`open_terminal`** (cmd у папці проекту з php/composer/git на PATH, поза job — переживає Stackr), `prune_orphan_sites`, `wait_until_served`. |
| `config.rs` | Редактор конфігів: `read/save_config`, `write_vhost`/`remove_vhost`, `regenerate_nginx_conf`, `read/save/reset_service_config` (nginx.conf / httpd.conf / php.ini). |
| `tools.rs` | `open_adminer` — піднімає vhost для `adminer.test` на активному сервері й відкриває браузер. |
| `logs.rs` | `read_log` / `read_all_logs` (об'єднаний потік з тегами) / `clear_log` / `clear_all_logs`. |

---

## 6. Фронтенд (React) — вузли

| Шлях | Навіщо |
|------|--------|
| `main.tsx` | Точка входу; `installDesktopShell()` (глушить браузерну поведінку). |
| `App.tsx` | Каркас: `<Titlebar>` + `<Sidebar>` + активна сторінка (`SCREENS[nav]`); на старті вантажить увесь стан і робить `checkSystem()`. Модалки монтуються тут глобально. |
| `store/useStore.ts` | **Єдиний Zustand-store** — увесь стан UI + усі дії (обгортки над `api`, оркестрація майстра/встановлень, тости, confirm). |
| `lib/api.ts` | Тонкі обгортки над `invoke(...)` для кожної команди (+ `takeRestoreNotice`, `openTerminal`, `xdebugStatus`/`setXdebug`) + підписки на події. Єдиний місток фронтенд→бекенд. |
| `lib/serviceStatus.ts` | Мапінг стану сервісу у візуал: `statusVisual` (колір/лейбл для running/starting/unhealthy/stopped), `hasProcess`. |
| `lib/updater.ts` | Обгортки над `@tauri-apps/plugin-updater` / `-process`: `checkForUpdate`, `installPendingUpdate` (progress → relaunch), `appVersion`. |
| `lib/desktopShell.ts` | Блокує контекстне меню/перезавантаження/зум/drop, щоб було схоже на програму, а не браузер. |
| `lib/styles.ts`, `lib/cn.ts`, `lib/projectVisual.ts` | Спільні класи, склейка className, візуал проекту (монограма/кольори за фреймворком). |
| `types/index.ts` | Усі TypeScript-типи (дзеркало моделей бекенду + стан UI). |
| `data/catalog.ts` | Статичні каталоги: рушії (`SERVER_/DB_/CACHE_ENGINES`), **фреймворки з версіями + рекомендованим PHP**, демо-дані для прев'ю в браузері. |
| `pages/*` | По сторінці на вкладку: `Servers`, `PHP` (+ `XdebugControl` per version), `Databases`, `Cache`, `Mail` (Mailpit: «Open inbox» + `.env` сніпет), `Projects` (+ кнопка терміналу), `Logs`, `Settings` (+ TLD-пресети, HTTPS-тумблер, Defender, **Updates**, data-folder). |
| `components/Layout/*` | `Titlebar` (перетяг вікна, min/max/close, бренд+версія), `Sidebar` (навігація + нижній статус активного сервера). |
| `components/*` (модалки) | `NewProjectWizard` (+ режим **Open existing**), `InstallModal`, `ConfigEditorModal` (редактор з пошуком+підсвіткою), `ConfirmDialog`, `AllExtensionsModal`, `FirstRunModal` (вибір кореня на першому запуску). |
| `components/ui/*` | Примітиви дизайн-системи: `Select`, `Toggle`, `Spinner`, `ProgressBar`, `Monogram`, `ScreenHeader`, `Toaster`, `ModalBackdrop`, `ServiceCards` тощо. |

> У чистому браузері (`npm run dev`, без Tauri) store підставляє демо-дані —
> UI можна дивитися без бекенду. У зібраному застосунку працює реальний бекенд.

---

## 7. Ключові потоки (як вузли працюють разом)

**Встановлення компонента**
`InstallModal`/сторінка → `installComponent(type, ver)` → `downloader::install_component`
резолвить джерело: спершу `manifest::lookup` (авторитетні URL + sha256), інакше
скрейп/пін → `download::download_and_extract_checked` (качає у `.downloads`, рахує
й звіряє SHA-256 якщо він є, шле `download-progress`) → розпаковка → `.installed` маркер →
запис у `stackr.json` → фронтенд перечитує списки. На старті `prune_broken_installs`
зносить недокачані/биті версії.

**Нагляд за сервісами**
`start_watchdog` (потік із setup) раз на 3 с звіряє реєстр: якщо процес сервісу впав —
перепіднімає за збереженим `SpawnPlan` (до ліміту рестартів), емітить
`service-status-changed`; UI оновлює бейджі через `serviceStatus.statusVisual`.

**Створення проекту (майстер)**
`NewProjectWizard` → `runWizInstall` у store: спершу **доставляє відсутні
передумови** (PHP/сервер/БД через `installComponent`, встановлене пропускає) →
`createProject(config)` → `projects::create_project` скафолдить за типом (порожній
index.php / Composer / `git clone` / нічого для «Open existing») → за потреби
створює БД → додає запис у hosts → повертає `Project`.

**Запуск проекту**
`start_project`: `ensure_php_runtime(ver)` (php-cgi на своєму порту) → `write_vhost_file`
(root = doc-root проекту, fastcgi_pass на цей порт) → `ensure_started(server)` або
`reload_server` → `wait_until_served` (чекає, поки vhost реально віддає сайт, а не
catch-all 404 nginx чи 502 поки php-cgi піднімається) → відкриває `http://{domain}`.

**Редактор конфігів**
Сторінка → `openConfig` → `read_service_config` → редагування у `ConfigEditorModal`
(власний пошук: backdrop-шар із `<mark>` під прозорим textarea) → `save_service_config`;
для згенерованих — «Reset to default» (`reset_service_config`).

**Логи**
`Logs` опитує `read_log`/`read_all_logs`; об'єднаний потік тегується джерелом і
сортується за часом.

---

## 8. Ключові рішення та інваріанти

- **Один активний веб-сервер** володіє портом 80. Сервер глобальний (не на
  проект); версія обирається лише коли жодного не встановлено. Тому `start_project`
  пише vhost у **кожен** встановлений сервер — перемикання сервера не дає 404.
- **PHP-FPM на Windows нема** → використовуємо `php-cgi.exe` у режимі FastCGI,
  **по одному процесу на minor-серію** на детермінованому порту `9000+major*10+minor`.
  Це дає різні проекти на різних версіях PHP одночасно. `PHP_FCGI_MAX_REQUESTS=0`
  прибирає самозавершення (інакше 502), `PHP_FCGI_CHILDREN=4` дає конкурентність.
- **Тільки loopback, dual-stack.** Жоден рушій не слухає 0.0.0.0 — усе на
  `127.0.0.1` **і** `[::1]` (браузери резолвлять `*.localhost` у IPv6). Це і є
  бар'єр безпеки (тому root БД лишається без пароля — Laragon-style).
- **vhost присутній ⟺ сайт обслуговується.** Це робить стан детермінованим і
  дозволяє прибирати «сирітські» конфіги на старті.
- **Майстер-конфіги не затираються** (`ensure_*_master` пише лише якщо відсутні) —
  ручні правки в редакторі переживають рестарти; «Reset» повертає дефолт.
- **Watchdog + respawn.** Фоновий потік раз на 3 с перепіднімає впалі сервіси
  (з cap на crash-loop); статуси — рівня 2 (running / starting / unhealthy / stopped
  за реальним відгуком порту, а не лише «процес живий»).
- **Цілісність завантажень.** ZIP качається у скретч, звіряється по SHA-256 **до**
  розпаковки; кожна версія має `.installed` маркер; биті інсталяції зносяться на старті.
- **Дані БД — поза бінарниками** (`data\...`) → реінсталл/апгрейд рушія не чіпає бази;
  легасі-датадір мігрується автоматично.
- **Атомний стан.** `stackr.json` пишеться tmp → `.bak` → rename; load падає на `.bak`
  при пошкодженні (з тостом у UI).
- **Kill-on-close** через JobObject — але лише для **сервісів**; **термінал проекту**
  поза job і переживає Stackr. Вікно **ховається в трей** при закритті (сервіси
  живуть далі), повний вихід — «Quit» у треї.
- **Composer 2.9+** блокує версії з security-advisories → у керованому
  `COMPOSER_HOME/config.json` вимкнено `policy.advisories.block` (щоб ставились
  старіші мажори фреймворків).
- **Git необов'язковий**: `Clone from Git` бере системний git, а якщо його нема —
  на вимогу тягне portable **MinGit** у `bin\git`.
- **WebView-lockdown**: вимкнено контекстне меню/зум завжди; браузерні
  «прискорювачі» (F5/Ctrl+R/F12) — лише в релізі (у dev лишаються для налагодження).

---

## 9. Розробка

```bash
npm install
npm run tauri dev     # dev-режим (гаряче перезавантаження UI + бекенд)
npm run tauri build   # реліз + NSIS-інсталятор

# перевірки
npx tsc --noEmit                          # типи фронтенду
cargo check                               # бекенд (debug)
cargo check --release                     # ВАЖЛИВО для коду під #[cfg(not(debug_assertions))]
cargo test --lib                          # юніт-тести (важкі мережеві — #[ignore])
```

> Нюанс: код під `#[cfg(not(debug_assertions))]` **не** компілюється звичайним
> `cargo check` — перевіряй його через `cargo check --release`.
