//! Core download + unzip pipeline (Tauri-independent so it can be unit-tested).

use std::path::Path;

use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

/// Browser-like UA — some hosts (e.g. Apache Lounge) refuse non-browser agents.
const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Stackr/0.1";

/// Download `url` (a ZIP) and extract it into `dest_dir`, reporting progress as
/// `(downloaded_bytes, total_bytes)`. `total` is 0 when the server omits
/// Content-Length. The temporary archive is removed on success.
pub async fn download_and_extract<F>(url: &str, dest_dir: &Path, on_progress: F) -> Result<(), String>
where
    F: FnMut(u64, u64),
{
    download_and_extract_checked(url, dest_dir, None, on_progress)
        .await
        .map(|_| ())
}

/// Like [`download_and_extract`], but streams a SHA-256 of the archive and — when
/// `expected_sha256` is `Some` — verifies it BEFORE extracting (a mismatch
/// deletes the download and errors, so nothing is unpacked). The download lands
/// in a scratch dir (`bin/.downloads`) and is only extracted once verified, so an
/// aborted download never leaves a half-unpacked dir that looks installed.
/// Returns the lowercase hex digest (so callers can log it / seed a manifest).
pub async fn download_and_extract_checked<F>(
    url: &str,
    dest_dir: &Path,
    expected_sha256: Option<&str>,
    mut on_progress: F,
) -> Result<String, String>
where
    F: FnMut(u64, u64),
{
    let downloads = crate::paths::downloads_dir();
    crate::paths::ensure_dir(&downloads).map_err(|e| e.to_string())?;
    // Name the scratch file per component+version so parallel installs don't clash.
    let comp = dest_dir
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .unwrap_or("component");
    let ver = dest_dir.file_name().and_then(|s| s.to_str()).unwrap_or("download");
    let tmp_zip = downloads.join(format!("{comp}-{ver}.stackr-dl.zip"));

    // --- download (streamed, hashed on the fly) ---
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("download failed: HTTP {}", resp.status()));
    }
    let total = resp.content_length().unwrap_or(0);

    let mut file = tokio::fs::File::create(&tmp_zip)
        .await
        .map_err(|e| e.to_string())?;
    let mut downloaded: u64 = 0;
    let mut hasher = Sha256::new();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        hasher.update(&chunk);
        file.write_all(&chunk).await.map_err(|e| e.to_string())?;
        downloaded += chunk.len() as u64;
        on_progress(downloaded, total);
    }
    file.flush().await.map_err(|e| e.to_string())?;
    drop(file);

    let digest = hex::encode(hasher.finalize());
    if let Some(expected) = expected_sha256 {
        if !expected.eq_ignore_ascii_case(&digest) {
            let _ = std::fs::remove_file(&tmp_zip);
            return Err(format!(
                "checksum mismatch for {url}\n  expected {expected}\n  got      {digest}\n\
                 refusing to install a corrupt or tampered download"
            ));
        }
    }

    // --- extract (blocking work off the async runtime), only after verifying ---
    let tmp_zip_cl = tmp_zip.clone();
    let dest_cl = dest_dir.to_path_buf();
    let result = tokio::task::spawn_blocking(move || extract_zip(&tmp_zip_cl, &dest_cl))
        .await
        .map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&tmp_zip);
    result.map(|_| digest)
}

/// Download `url` to a single file at `dest` (no extraction). Used for
/// non-archive artifacts such as `composer.phar`.
pub async fn download_file(url: &str, dest: &Path) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        crate::paths::ensure_dir(parent).map_err(|e| e.to_string())?;
    }
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("download failed: HTTP {}", resp.status()));
    }
    let mut file = tokio::fs::File::create(dest).await.map_err(|e| e.to_string())?;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        file.write_all(&chunk).await.map_err(|e| e.to_string())?;
    }
    file.flush().await.map_err(|e| e.to_string())?;
    Ok(())
}

/// Extract `zip_path` into `dest`, then flatten a single wrapping root folder
/// (e.g. `nginx-1.27.3/`) so binaries land directly under `dest`.
fn extract_zip(zip_path: &Path, dest: &Path) -> Result<(), String> {
    // Clean any prior install so re-installs don't leave stale files (or a second
    // wrapping dir that would defeat flattening).
    let _ = std::fs::remove_dir_all(dest);
    std::fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    let f = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(f).map_err(|e| e.to_string())?;
    archive.extract(dest).map_err(|e| e.to_string())?;
    flatten_single_root(dest)
}

fn flatten_single_root(dest: &Path) -> Result<(), String> {
    let entries: Vec<_> = std::fs::read_dir(dest)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .collect();
    // Flatten when there's exactly one wrapping directory at the top level, even
    // if a few loose files sit beside it (e.g. Apache's `Apache24/` + readmes).
    let dirs: Vec<_> = entries.iter().filter(|e| e.path().is_dir()).collect();
    if dirs.len() != 1 {
        return Ok(());
    }
    let only = dirs[0].path();
    // Move each child of the wrapping dir up into `dest`.
    for child in std::fs::read_dir(&only).map_err(|e| e.to_string())? {
        let child = child.map_err(|e| e.to_string())?;
        let target = dest.join(child.file_name());
        if target.exists() {
            continue;
        }
        std::fs::rename(child.path(), target).map_err(|e| e.to_string())?;
    }
    std::fs::remove_dir_all(&only).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn downloads_and_flattens_nginx() {
        let dest = std::env::temp_dir()
            .join("stackr-dl-test")
            .join("nginx")
            .join("1.27.3");
        let _ = std::fs::remove_dir_all(dest.parent().unwrap());

        let mut last = (0u64, 0u64);
        download_and_extract(
            "https://nginx.org/download/nginx-1.27.3.zip",
            &dest,
            |d, t| last = (d, t),
        )
        .await
        .expect("download+extract should succeed");

        assert!(last.0 > 0, "should have downloaded some bytes");
        assert!(
            dest.join("nginx.exe").exists(),
            "nginx.exe must exist directly under the version dir after flatten"
        );

        let _ = std::fs::remove_dir_all(dest.parent().unwrap().parent().unwrap());
    }

    /// A wrong expected checksum must abort BEFORE extracting — nothing installed.
    ///   cargo test rejects_bad_checksum -- --ignored --nocapture
    #[tokio::test]
    #[ignore = "downloads nginx to prove checksum verification rejects a mismatch"]
    async fn rejects_bad_checksum() {
        let dest = std::env::temp_dir()
            .join("stackr-checksum-test")
            .join("nginx")
            .join("1.27.3");
        let _ = std::fs::remove_dir_all(dest.parent().unwrap());

        let res = download_and_extract_checked(
            "https://nginx.org/download/nginx-1.27.3.zip",
            &dest,
            Some("0000000000000000000000000000000000000000000000000000000000000000"),
            |_, _| {},
        )
        .await;

        assert!(res.is_err(), "a bad checksum must error");
        assert!(
            !dest.join("nginx.exe").exists(),
            "nothing must be extracted when the checksum fails"
        );
        let _ = std::fs::remove_dir_all(dest.parent().unwrap().parent().unwrap());
    }
}
