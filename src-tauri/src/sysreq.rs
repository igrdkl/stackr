//! Host system requirement checks (Windows runtime prerequisites). The bundled
//! engines (PHP, nginx, MySQL/MariaDB, …) are MSVC builds, so a clean Windows that
//! lacks the Visual C++ runtime will fail to start them. We also surface the
//! Windows build + WebView2 runtime version so compatibility is visible in-app and
//! in bug reports.

use serde::Serialize;

/// A snapshot of the host's relevant runtime prerequisites.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemReport {
    /// MSVC x64 redistributable present (bundled engines need it).
    pub vcredist: bool,
    /// Human-readable Windows version, e.g. "Windows 11 23H2 (build 22631)".
    pub windows: String,
    /// Installed WebView2 Runtime version, if detectable.
    pub webview2: Option<String>,
    /// Whether the host meets Stackr's minimum: Windows 10 1803+ with WebView2.
    pub supported: bool,
}

/// True if the Microsoft Visual C++ 2015–2022 x64 Redistributable is present.
/// Checks the redistributable's registry marker first, then falls back to the
/// actual runtime DLLs the bundled binaries link against.
#[cfg(windows)]
pub fn vcredist_x64_installed() -> bool {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    // The 2015–2022 redist writes Installed=1 here (64-bit view for an x64 build).
    let via_registry = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(r"SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\X64")
        .and_then(|k| k.get_value::<u32, _>("Installed"))
        .map(|installed| installed == 1)
        .unwrap_or(false);
    if via_registry {
        return true;
    }

    // Fallback: the runtime DLLs PHP 8.x (VS2019+) needs in System32.
    let system32 = std::env::var_os("SystemRoot")
        .map(|root| std::path::PathBuf::from(root).join("System32"))
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Windows\System32"));
    system32.join("vcruntime140.dll").exists() && system32.join("vcruntime140_1.dll").exists()
}

/// Windows build number from the registry (e.g. 22631), or 0 if unknown.
#[cfg(windows)]
fn windows_build() -> u32 {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;
    RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion")
        .ok()
        .and_then(|k| k.get_value::<String, _>("CurrentBuild").ok())
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

/// Human-readable Windows version. (ProductName still says "Windows 10" on 11, so
/// the name is derived from the build: 22000+ → Windows 11.)
#[cfg(windows)]
pub fn windows_version() -> String {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;
    let build = windows_build();
    let name = if build >= 22000 { "Windows 11" } else { "Windows 10" };
    let display = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion")
        .ok()
        .and_then(|k| k.get_value::<String, _>("DisplayVersion").ok())
        .unwrap_or_default();
    if display.is_empty() {
        format!("{name} (build {build})")
    } else {
        format!("{name} {display} (build {build})")
    }
}

/// Installed WebView2 Runtime version (the fixed GUID is the Evergreen runtime).
#[cfg(windows)]
pub fn webview2_version() -> Option<String> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;
    const GUID: &str = "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
    let hklm = format!(r"SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{GUID}");
    let hkcu = format!(r"Software\Microsoft\EdgeUpdate\Clients\{GUID}");
    let pv = |root, path: &str| -> Option<String> {
        RegKey::predef(root)
            .open_subkey(path)
            .ok()
            .and_then(|k| k.get_value::<String, _>("pv").ok())
            .filter(|v| !v.is_empty() && v != "0.0.0.0")
    };
    pv(HKEY_LOCAL_MACHINE, &hklm).or_else(|| pv(HKEY_CURRENT_USER, &hkcu))
}

#[cfg(windows)]
fn report() -> SystemReport {
    let webview2 = webview2_version();
    // WebView2 requires Windows 10 v1803 (build 17134); that's also Stackr's floor.
    let supported = windows_build() >= 17134 && webview2.is_some();
    SystemReport { vcredist: vcredist_x64_installed(), windows: windows_version(), webview2, supported }
}

#[cfg(not(windows))]
pub fn vcredist_x64_installed() -> bool {
    true
}

#[cfg(not(windows))]
fn report() -> SystemReport {
    SystemReport { vcredist: true, windows: "non-Windows host".into(), webview2: None, supported: true }
}

/// Host runtime prerequisites: VC++ redist, Windows version, WebView2 version.
#[tauri::command]
pub fn system_report() -> SystemReport {
    report()
}
