//! Pure php.ini extension parsing/toggling (Tauri-independent, unit-tested).
//!
//! Extensions are enabled in php.ini via `extension=<name>` (or
//! `zend_extension=<name>` for opcache/xdebug). Values may be a bare name, a
//! `php_<name>.dll`, or a path — all normalize to the bare lowercase name.

/// Extensions that load through `zend_extension` rather than `extension`.
fn is_zend(name: &str) -> bool {
    matches!(name, "opcache" | "xdebug")
}

/// Normalize a directive value to its bare extension name.
/// `php_redis.dll` -> `redis`, `C:\php\ext\php_gd.dll` -> `gd`, `curl` -> `curl`.
fn normalize_name(value: &str) -> String {
    let v = value.trim().trim_matches('"').replace('\\', "/");
    let base = v.rsplit('/').next().unwrap_or(&v);
    let base = base.strip_prefix("php_").unwrap_or(base);
    let base = base
        .strip_suffix(".dll")
        .or_else(|| base.strip_suffix(".so"))
        .unwrap_or(base);
    base.to_ascii_lowercase()
}

/// If `line` (already comment-stripped) is an extension directive, return its name.
fn directive_name(line: &str) -> Option<String> {
    let eq = line.find('=')?;
    let key = line[..eq].trim();
    if key.eq_ignore_ascii_case("extension") || key.eq_ignore_ascii_case("zend_extension") {
        Some(normalize_name(&line[eq + 1..]))
    } else {
        None
    }
}

fn strip_comment(line: &str) -> &str {
    line.trim_start()
        .trim_start_matches([';', '#'])
        .trim_start()
}

fn is_commented(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with(';') || t.starts_with('#')
}

/// Names of all currently-active (uncommented) extensions in `ini`.
pub fn enabled_extensions(ini: &str) -> Vec<String> {
    ini.lines()
        .filter(|l| !is_commented(l))
        .filter_map(|l| directive_name(l.trim()))
        .collect()
}

/// Return `ini` with extension `name` enabled or disabled.
/// Enabling uncomments an existing line or appends a new directive;
/// disabling comments out every active matching directive.
pub fn set_extension(ini: &str, name: &str, enable: bool) -> String {
    let key = if is_zend(name) { "zend_extension" } else { "extension" };
    let target = name.to_ascii_lowercase();
    let mut found = false;

    let mut lines: Vec<String> = ini.lines().map(|s| s.to_string()).collect();
    for line in lines.iter_mut() {
        let Some(n) = directive_name(strip_comment(line)) else {
            continue;
        };
        if n != target {
            continue;
        }
        found = true;
        let active = !is_commented(line);
        if enable && !active {
            *line = format!("{key}={target}");
        } else if !enable && active {
            *line = format!(";{}", line.trim_start());
        }
    }

    let mut result = lines.join("\n");
    if enable && !found {
        if !result.is_empty() && !result.ends_with('\n') {
            result.push('\n');
        }
        result.push_str(&format!("{key}={target}\n"));
    }
    result
}

/// Set a simple `key = value` directive: replace the first existing line for
/// `key` (commented or not), else append it. For non-extension directives such
/// as `extension_dir`.
pub fn set_kv(ini: &str, key: &str, value: &str) -> String {
    let mut found = false;
    let mut lines: Vec<String> = ini.lines().map(|s| s.to_string()).collect();
    for line in lines.iter_mut() {
        let stripped = strip_comment(line);
        let Some(eq) = stripped.find('=') else { continue };
        if stripped[..eq].trim().eq_ignore_ascii_case(key) {
            *line = format!("{key} = {value}");
            found = true;
            break;
        }
    }
    let mut result = lines.join("\n");
    if !found {
        if !result.is_empty() && !result.ends_with('\n') {
            result.push('\n');
        }
        result.push_str(&format!("{key} = {value}\n"));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
[PHP]
extension=curl
extension=php_gd.dll
;extension=redis
zend_extension=opcache
;zend_extension=xdebug
";

    #[test]
    fn parses_enabled() {
        let mut on = enabled_extensions(SAMPLE);
        on.sort();
        assert_eq!(on, vec!["curl", "gd", "opcache"]);
    }

    #[test]
    fn enable_uncomments_existing() {
        let out = set_extension(SAMPLE, "redis", true);
        assert!(out.contains("extension=redis"));
        assert!(!out.contains(";extension=redis"));
        assert!(enabled_extensions(&out).contains(&"redis".to_string()));
    }

    #[test]
    fn enable_zend_uncomments_existing() {
        let out = set_extension(SAMPLE, "xdebug", true);
        assert!(out.contains("zend_extension=xdebug"));
        assert!(enabled_extensions(&out).contains(&"xdebug".to_string()));
    }

    #[test]
    fn disable_comments_active() {
        let out = set_extension(SAMPLE, "curl", false);
        assert!(!enabled_extensions(&out).contains(&"curl".to_string()));
        assert!(out.contains(";extension=curl"));
    }

    #[test]
    fn enable_absent_appends() {
        let out = set_extension(SAMPLE, "intl", true);
        assert!(out.contains("extension=intl"));
        assert!(enabled_extensions(&out).contains(&"intl".to_string()));
    }

    #[test]
    fn normalizes_dll_paths() {
        assert_eq!(normalize_name("php_pdo_mysql.dll"), "pdo_mysql");
        assert_eq!(normalize_name("\"C:\\\\php\\\\ext\\\\php_gd.dll\""), "gd");
    }

    #[test]
    fn set_kv_uncomments_existing() {
        let ini = "[PHP]\n;extension_dir = \"ext\"\nmemory_limit = 128M\n";
        let out = set_kv(ini, "extension_dir", "\"C:/php/ext\"");
        assert!(out.contains("extension_dir = \"C:/php/ext\""));
        assert!(!out.contains(";extension_dir"));
        assert!(out.contains("memory_limit = 128M"));
    }

    #[test]
    fn set_kv_appends_when_absent() {
        let out = set_kv("[PHP]\n", "extension_dir", "\"ext\"");
        assert!(out.contains("extension_dir = \"ext\""));
    }
}
