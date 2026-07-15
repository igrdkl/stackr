//! Version manifest client.
//!
//! The public `stackr-manifest` repo is the authoritative catalog of installable
//! component versions: each entry carries the official download URL and (once the
//! nightly CI fills them in) a SHA-256. The client fetches it network-first,
//! caches it under `C:\Stackr\config\manifest.json`, and falls back to that cache
//! when offline. When the manifest has no entry for a requested (component,
//! version), the installer falls back to its existing scraped/pinned URLs — so a
//! stale or unreachable manifest never blocks an install.

use serde::Deserialize;
use std::sync::Mutex;

const MANIFEST_URL: &str =
    "https://raw.githubusercontent.com/igrdkl/stackr-manifest/main/manifest.json";

/// One installable build of a component.
#[derive(Clone, Deserialize)]
pub struct ManifestEntry {
    pub version: String,
    pub url: String,
    /// `None` until the CI computes it; the installer then skips verification.
    #[serde(default)]
    pub sha256: Option<String>,
}

/// The parsed catalog: component id → builds (newest first).
#[derive(Clone, Deserialize)]
pub struct Manifest {
    #[allow(dead_code)]
    pub schema: u32,
    #[serde(default)]
    pub components: std::collections::HashMap<String, Vec<ManifestEntry>>,
}

/// Loaded-once-per-session copy so a wizard installing several prerequisites
/// doesn't refetch for each one.
static SESSION: Mutex<Option<Manifest>> = Mutex::new(None);

fn cache_path() -> std::path::PathBuf {
    crate::paths::config_root().join("manifest.json")
}

/// Fetch the manifest from the network and cache it to disk. Short timeout so an
/// offline/slow network falls through to the cache quickly rather than hanging.
async fn fetch_and_cache() -> Option<Manifest> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(6))
        .build()
        .ok()?;
    let body = client
        .get(MANIFEST_URL)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .text()
        .await
        .ok()?;
    let manifest: Manifest = serde_json::from_str(&body).ok()?;
    let path = cache_path();
    if let Some(parent) = path.parent() {
        let _ = crate::paths::ensure_dir(parent);
    }
    let _ = std::fs::write(&path, &body); // cache is best-effort
    Some(manifest)
}

/// The last successfully cached manifest, if any.
fn load_cached() -> Option<Manifest> {
    let body = std::fs::read_to_string(cache_path()).ok()?;
    serde_json::from_str(&body).ok()
}

/// The catalog: session copy → fresh network fetch (cached) → on-disk cache.
/// Returns `None` only when we've never reached the network and have no cache.
async fn get() -> Option<Manifest> {
    if let Some(m) = SESSION.lock().ok().and_then(|g| g.clone()) {
        return Some(m);
    }
    let m = match fetch_and_cache().await {
        Some(m) => Some(m),
        None => load_cached(),
    };
    if let Some(ref man) = m {
        if let Ok(mut g) = SESSION.lock() {
            *g = Some(man.clone());
        }
    }
    m
}

/// The manifest's entry for an exact (component, version), if present.
pub async fn lookup(component: &str, version: &str) -> Option<ManifestEntry> {
    get()
        .await?
        .components
        .get(component)?
        .iter()
        .find(|e| e.version == version)
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_manifest_with_null_and_real_sha() {
        let json = r#"{
            "schema": 1,
            "components": {
                "nginx": [
                    { "version": "1.27.3", "url": "https://x/nginx-1.27.3.zip", "sha256": null },
                    { "version": "1.26.2", "url": "https://x/nginx-1.26.2.zip", "sha256": "abc123" }
                ]
            }
        }"#;
        let m: Manifest = serde_json::from_str(json).unwrap();
        let nginx = m.components.get("nginx").unwrap();
        assert_eq!(nginx.len(), 2);
        assert_eq!(nginx[0].sha256, None);
        assert_eq!(nginx[1].sha256.as_deref(), Some("abc123"));
    }

    /// Live proof the client reaches the public repo, parses it, and resolves an
    /// entry.  cargo test manifest -- --ignored --nocapture
    #[tokio::test]
    #[ignore = "hits the live stackr-manifest repo"]
    async fn fetches_and_looks_up_nginx() {
        let e = lookup("nginx", "1.27.3").await.expect("nginx 1.27.3 in manifest");
        assert!(e.url.contains("nginx-1.27.3.zip"), "unexpected url: {}", e.url);
    }
}
