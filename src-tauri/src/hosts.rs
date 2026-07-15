//! Windows hosts-file management. Tries a direct write first; on permission
//! denial, retries through an elevated copy (triggers a UAC prompt).

use std::path::PathBuf;
use std::process::Command;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn hosts_path() -> PathBuf {
    PathBuf::from(r"C:\Windows\System32\drivers\etc\hosts")
}

/// True if a non-comment hosts line maps `domain`.
fn line_maps(line: &str, domain: &str) -> bool {
    let l = line.trim();
    if l.starts_with('#') {
        return false;
    }
    let before_comment = l.split('#').next().unwrap_or("");
    before_comment
        .split_whitespace()
        .skip(1) // skip the IP
        .any(|t| t.eq_ignore_ascii_case(domain))
}

/// `localhost` and any `*.localhost` name resolve to 127.0.0.1 in browsers per
/// RFC 6761, so they need no hosts entry — and thus no admin/UAC prompt.
fn resolves_without_hosts(domain: &str) -> bool {
    let d = domain.trim().to_ascii_lowercase();
    d == "localhost" || d.ends_with(".localhost")
}

pub fn add_host(domain: &str) -> Result<(), String> {
    if resolves_without_hosts(domain) {
        return Ok(());
    }
    let current = std::fs::read_to_string(hosts_path()).unwrap_or_default();
    if current.lines().any(|l| line_maps(l, domain)) {
        return Ok(());
    }
    let mut updated = current;
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push_str("\r\n");
    }
    updated.push_str(&format!("127.0.0.1\t{domain}\t# Stackr\r\n"));
    write_hosts(&updated)
}

pub fn remove_host(domain: &str) -> Result<(), String> {
    if resolves_without_hosts(domain) {
        return Ok(());
    }
    let current = match std::fs::read_to_string(hosts_path()) {
        Ok(c) => c,
        Err(_) => return Ok(()),
    };
    let kept: Vec<&str> = current.lines().filter(|l| !line_maps(l, domain)).collect();
    let mut updated = kept.join("\r\n");
    updated.push_str("\r\n");
    write_hosts(&updated)
}

fn write_hosts(content: &str) -> Result<(), String> {
    if std::fs::write(hosts_path(), content).is_ok() {
        return Ok(());
    }
    write_hosts_elevated(content)
}

/// Copy a staged hosts file into place with elevation (UAC).
fn write_hosts_elevated(content: &str) -> Result<(), String> {
    let tmp = std::env::temp_dir().join("stackr-hosts.tmp");
    std::fs::write(&tmp, content).map_err(|e| e.to_string())?;
    let ps = format!(
        "Start-Process -FilePath cmd.exe -ArgumentList '/c copy /Y \"{}\" \"{}\"' -Verb RunAs -Wait",
        tmp.display(),
        hosts_path().display()
    );
    let mut cmd = Command::new("powershell");
    cmd.args(["-NoProfile", "-Command", &ps]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd.output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "could not update hosts file (admin required): {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}
