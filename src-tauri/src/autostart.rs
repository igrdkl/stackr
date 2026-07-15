//! "Launch at Windows login" via the per-user Run registry key
//! (`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`). No admin needed.

#[cfg(windows)]
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
#[cfg(windows)]
const VALUE_NAME: &str = "Stackr";

/// Register or unregister Stackr to launch when the current user logs in.
/// Enabling points the Run entry at the current executable.
#[cfg(windows)]
pub fn set_autostart(enable: bool) -> Result<(), String> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (run, _) = hkcu
        .create_subkey(RUN_KEY)
        .map_err(|e| format!("opening Run key: {e}"))?;

    if enable {
        let exe = std::env::current_exe().map_err(|e| format!("resolving exe path: {e}"))?;
        // Quote the path so spaces are handled by the loader.
        let cmd = format!("\"{}\"", exe.display());
        run.set_value(VALUE_NAME, &cmd)
            .map_err(|e| format!("writing Run value: {e}"))
    } else {
        match run.delete_value(VALUE_NAME) {
            Ok(()) => Ok(()),
            // Absent value = already disabled.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("deleting Run value: {e}")),
        }
    }
}

/// No-op on non-Windows so the rest of the app stays portable.
#[cfg(not(windows))]
pub fn set_autostart(_enable: bool) -> Result<(), String> {
    Ok(())
}
