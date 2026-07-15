mod autostart;
mod commands;
mod config_gen;
mod db;
mod defender;
mod download;
mod hosts;
mod job;
mod manifest;
mod models;
mod paths;
mod php_ini;
mod scaffold;
mod state;
mod sysreq;
mod tls;
mod tray;

use commands::services::ProcessRegistry;
use commands::{config, downloader, logs, php, projects, services, tools};
use state::{AppState, StateStore};

/// Strip the WebView2 browser chrome so the window behaves natively: no default
/// right-click menu, no Ctrl+wheel zoom. Browser accelerator keys (F5/Ctrl+R
/// reload, F12 devtools, Ctrl+P …) are disabled only in release builds so
/// `tauri dev` keeps devtools + reload for debugging.
#[cfg(target_os = "windows")]
fn lockdown_webview(window: &tauri::WebviewWindow) {
    let _ = window.with_webview(|webview| unsafe {
        let core = match webview.controller().CoreWebView2() {
            Ok(c) => c,
            Err(_) => return,
        };
        if let Ok(settings) = core.Settings() {
            let _ = settings.SetAreDefaultContextMenusEnabled(false);
            let _ = settings.SetIsZoomControlEnabled(false);
            #[cfg(not(debug_assertions))]
            {
                use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings3;
                use windows_core::Interface;
                if let Ok(s3) = settings.cast::<ICoreWebView2Settings3>() {
                    let _ = s3.SetAreBrowserAcceleratorKeysEnabled(false);
                }
            }
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Kill-on-close job so spawned services never outlive Stackr.
    job::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(StateStore(std::sync::Mutex::new(AppState::load())))
        .manage(ProcessRegistry::default())
        .setup(|app| {
            tray::build(app.handle())?;
            // Trim oversized logs before any service can open them for appending.
            logs::rotate_on_startup();
            // Watchdog: keep service status truthful and revive dead php-cgi/servers.
            services::start_watchdog(app.handle().clone());
            // Drop vhost files for domains that are no longer projects (stale
            // leftovers cause cross-host 502s). Server isn't running yet.
            if let Ok(st) = tauri::Manager::state::<StateStore>(app).0.lock() {
                projects::prune_orphan_sites(&st.projects);
                // Remove leftover partial installs + download scratch from a crash.
                downloader::prune_broken_installs(&st);
            }
            #[cfg(target_os = "windows")]
            if let Some(win) = tauri::Manager::get_webview_window(app, "main") {
                lockdown_webview(&win);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Hide to tray instead of exiting so spawned services keep running;
            // "Quit" in the tray menu actually terminates the app.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            // state
            state::get_installed,
            state::set_default_php,
            state::get_settings,
            state::save_settings,
            state::take_restore_notice,
            state::get_root_info,
            state::set_root,
            sysreq::system_report,
            defender::defender_status,
            defender::add_defender_exclusion,
            db::export_databases,
            tls::https_status,
            tls::enable_https,
            tls::disable_https,
            // services (web servers, databases, caches share start/stop/restart)
            services::get_servers,
            services::get_databases,
            services::get_caches,
            services::get_mail,
            services::start_service,
            services::stop_service,
            services::restart_service,
            services::start_php_runtime,
            services::stop_php_runtime,
            // downloader
            downloader::install_component,
            downloader::uninstall_component,
            downloader::get_php_available,
            // php
            php::get_php_versions,
            php::get_php_extensions,
            php::list_php_extensions,
            php::install_php_extension,
            php::toggle_extension,
            php::read_php_ini,
            php::save_php_ini,
            php::xdebug_status,
            php::set_xdebug,
            // projects
            projects::get_projects,
            projects::create_project,
            projects::start_project,
            projects::stop_project,
            projects::set_project_php,
            projects::set_project_db,
            projects::open_project_folder,
            projects::open_in_ide,
            projects::open_terminal,
            projects::detect_ides,
            projects::detect_doc_root,
            projects::delete_project,
            // config
            config::read_config,
            config::save_config,
            config::get_config_path,
            config::write_vhost,
            config::remove_vhost,
            config::regenerate_nginx_conf,
            config::read_service_config,
            config::save_service_config,
            config::reset_service_config,
            // tools
            tools::open_adminer,
            // logs
            logs::read_log,
            logs::read_all_logs,
            logs::clear_log,
            logs::clear_all_logs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
