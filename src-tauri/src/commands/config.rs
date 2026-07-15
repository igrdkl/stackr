//! Config-file read/write for the in-app editors, plus per-site vhost
//! generation used when creating/starting projects.

use std::fs;
use std::path::PathBuf;

use serde::Serialize;
use tauri::State;

use crate::state::StateStore;

#[tauri::command]
pub fn read_config(path: String) -> Result<String, String> {
    fs::read_to_string(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_config(path: String, content: String) -> Result<(), String> {
    fs::write(&path, content).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_config_path(service_id: String) -> String {
    format!("C:\\Stackr\\config\\{service_id}")
}

// ---- In-app config editor (nginx.conf / Apache httpd.conf / php.ini) ----

/// A config file surfaced in the in-app editor.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigDoc {
    pub component: String,
    pub label: String,    // file name shown in the editor header
    pub path: String,     // absolute path on disk
    pub content: String,  // current file contents
    pub generated: bool,  // true ⟹ Stackr can regenerate defaults ("Reset")
    pub hint: String,     // one-line note on when edits take effect
}

/// Install directory for a component, from persisted state.
fn installed_dir(state: &State<'_, StateStore>, component: &str) -> Result<PathBuf, String> {
    let st = state.0.lock().map_err(|e| e.to_string())?;
    st.installed
        .iter()
        .find(|c| c.component == component)
        .map(|c| PathBuf::from(&c.path))
        .ok_or_else(|| format!("{component} is not installed"))
}

/// Resolve a component's editable config file, generating defaults when needed,
/// and return its current contents.
#[tauri::command]
pub fn read_service_config(
    state: State<'_, StateStore>,
    component: String,
    version: String,
) -> Result<ConfigDoc, String> {
    let (path, label, generated, hint) = match component.as_str() {
        "nginx" => {
            let dir = installed_dir(&state, "nginx")?;
            crate::config_gen::ensure_nginx_master(&dir)?;
            (
                crate::paths::nginx_conf(),
                "nginx.conf".to_string(),
                true,
                "Edits apply when Nginx is restarted.".to_string(),
            )
        }
        "apache" => {
            let dir = installed_dir(&state, "apache")?;
            crate::config_gen::ensure_apache_master(&dir)?;
            (
                crate::paths::apache_conf(),
                "httpd.conf".to_string(),
                true,
                "Edits apply when Apache is restarted.".to_string(),
            )
        }
        "php" => (
            crate::commands::php::ensure_ini(&version)?,
            "php.ini".to_string(),
            false,
            "Edits apply when the PHP runtime is restarted.".to_string(),
        ),
        other => return Err(format!("no editable config for '{other}'")),
    };

    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    Ok(ConfigDoc {
        component,
        label,
        path: path.to_string_lossy().to_string(),
        content,
        generated,
        hint,
    })
}

/// Persist edited contents to a component's config file.
#[tauri::command]
pub fn save_service_config(
    component: String,
    version: String,
    content: String,
) -> Result<(), String> {
    let path = match component.as_str() {
        "nginx" => crate::paths::nginx_conf(),
        "apache" => crate::paths::apache_conf(),
        "php" => crate::commands::php::ini_path(&version),
        other => return Err(format!("no editable config for '{other}'")),
    };
    if let Some(parent) = path.parent() {
        crate::paths::ensure_dir(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&path, content).map_err(|e| e.to_string())
}

/// Regenerate a component's config from Stackr defaults (nginx/apache only) and
/// return the fresh contents.
#[tauri::command]
pub fn reset_service_config(
    state: State<'_, StateStore>,
    component: String,
    version: String,
) -> Result<ConfigDoc, String> {
    match component.as_str() {
        "nginx" => crate::config_gen::write_nginx_master(&installed_dir(&state, "nginx")?)?,
        "apache" => crate::config_gen::write_apache_master(&installed_dir(&state, "apache")?)?,
        other => return Err(format!("'{other}' config cannot be reset")),
    }
    read_service_config(state, component, version)
}

/// Generate (or overwrite) a per-project vhost for the given web server.
/// `fcgi_port` selects which php-cgi runtime (PHP version) serves the site.
#[tauri::command]
pub fn write_vhost(
    server: String,
    domain: String,
    root: String,
    port: u16,
    fcgi_port: u16,
) -> Result<(), String> {
    let root = PathBuf::from(root);
    let (dir, content) = match server.as_str() {
        "nginx" => (
            crate::paths::nginx_sites_dir(),
            crate::config_gen::nginx_vhost(&domain, &root, port, fcgi_port, None),
        ),
        "apache" => (
            crate::paths::apache_sites_dir(),
            crate::config_gen::apache_vhost(&domain, &root, port, fcgi_port, None),
        ),
        other => return Err(format!("unknown web server '{other}'")),
    };
    crate::paths::ensure_dir(&dir).map_err(|e| e.to_string())?;
    fs::write(dir.join(format!("{domain}.conf")), content).map_err(|e| e.to_string())
}

/// Remove a project's vhost file.
#[tauri::command]
pub fn remove_vhost(server: String, domain: String) -> Result<(), String> {
    let dir = match server.as_str() {
        "nginx" => crate::paths::nginx_sites_dir(),
        "apache" => crate::paths::apache_sites_dir(),
        other => return Err(format!("unknown web server '{other}'")),
    };
    let file = dir.join(format!("{domain}.conf"));
    if file.exists() {
        fs::remove_file(&file).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Regenerate the nginx master config against the installed nginx.
#[tauri::command]
pub fn regenerate_nginx_conf(state: State<'_, StateStore>) -> Result<(), String> {
    let dir = {
        let st = state.0.lock().map_err(|e| e.to_string())?;
        st.installed
            .iter()
            .find(|c| c.component == "nginx")
            .map(|c| PathBuf::from(&c.path))
            .ok_or_else(|| "nginx is not installed".to_string())?
    };
    crate::config_gen::write_nginx_master(&dir)
}
