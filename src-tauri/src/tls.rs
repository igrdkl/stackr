//! Local certificate authority + per-domain leaf certs for browser-trusted HTTPS.
//!
//! When the user enables HTTPS, Stackr generates a self-signed root CA under
//! `config\ca`, imports it into the CURRENT USER's Windows trust store (via
//! `certutil -user`, no admin), and thereafter signs a leaf cert per project
//! domain under `config\certs`. Browsers then serve `https://{domain}` with no
//! warning. The CA private key never leaves the machine.

use rcgen::{
    BasicConstraints, CertificateParams, DistinguishedName, DnType, ExtendedKeyUsagePurpose, IsCa,
    KeyPair, KeyUsagePurpose,
};
use serde::Serialize;
use std::process::Command;
use tauri::State;

use crate::state::StateStore;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Friendly CN used both in the cert and to look it up in the trust store.
const CA_COMMON_NAME: &str = "Stackr Local CA";

/// Generate the root CA (cert + key) if it isn't on disk yet. Idempotent.
pub fn ensure_ca() -> Result<(), String> {
    if crate::paths::ca_cert().exists() && crate::paths::ca_key().exists() {
        return Ok(());
    }
    crate::paths::ensure_dir(&crate::paths::ca_dir()).map_err(|e| e.to_string())?;

    let key = KeyPair::generate().map_err(|e| e.to_string())?;
    let mut params = CertificateParams::new(Vec::new()).map_err(|e| e.to_string())?;
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, CA_COMMON_NAME);
    dn.push(DnType::OrganizationName, "Stackr");
    params.distinguished_name = dn;
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
    // Long validity — a privately-trusted local CA isn't subject to the public
    // 398-day leaf cap, and this avoids re-issuing during normal use.
    params.not_before = rcgen::date_time_ymd(2024, 1, 1);
    params.not_after = rcgen::date_time_ymd(2035, 1, 1);

    let cert = params.self_signed(&key).map_err(|e| e.to_string())?;
    std::fs::write(crate::paths::ca_cert(), cert.pem()).map_err(|e| e.to_string())?;
    std::fs::write(crate::paths::ca_key(), key.serialize_pem()).map_err(|e| e.to_string())?;
    Ok(())
}

/// Reload the CA cert+key from disk so we can sign leaves with it.
fn load_ca() -> Result<(rcgen::Certificate, KeyPair), String> {
    let key_pem = std::fs::read_to_string(crate::paths::ca_key()).map_err(|e| e.to_string())?;
    let cert_pem = std::fs::read_to_string(crate::paths::ca_cert()).map_err(|e| e.to_string())?;
    let key = KeyPair::from_pem(&key_pem).map_err(|e| e.to_string())?;
    let params = CertificateParams::from_ca_cert_pem(&cert_pem).map_err(|e| e.to_string())?;
    let cert = params.self_signed(&key).map_err(|e| e.to_string())?;
    Ok((cert, key))
}

/// Generate a leaf cert for `domain` (and `*.{domain}`), signed by the CA, unless
/// it already exists. Ensures the CA exists first.
pub fn ensure_domain_cert(domain: &str) -> Result<(), String> {
    ensure_ca()?;
    if crate::paths::domain_cert(domain).exists() && crate::paths::domain_key(domain).exists() {
        return Ok(());
    }
    crate::paths::ensure_dir(&crate::paths::certs_dir()).map_err(|e| e.to_string())?;
    let (ca_cert, ca_key) = load_ca()?;

    let leaf_key = KeyPair::generate().map_err(|e| e.to_string())?;
    let sans = vec![domain.to_string(), format!("*.{domain}")];
    let mut params = CertificateParams::new(sans).map_err(|e| e.to_string())?;
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, domain);
    params.distinguished_name = dn;
    params.is_ca = IsCa::NoCa;
    params.use_authority_key_identifier_extension = true;
    params.key_usages = vec![KeyUsagePurpose::DigitalSignature, KeyUsagePurpose::KeyEncipherment];
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    params.not_before = rcgen::date_time_ymd(2024, 1, 1);
    params.not_after = rcgen::date_time_ymd(2035, 1, 1);

    let leaf = params
        .signed_by(&leaf_key, &ca_cert, &ca_key)
        .map_err(|e| e.to_string())?;
    std::fs::write(crate::paths::domain_cert(domain), leaf.pem()).map_err(|e| e.to_string())?;
    std::fs::write(crate::paths::domain_key(domain), leaf_key.serialize_pem())
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Whether the CA is already imported into the current user's Root store.
#[cfg(windows)]
pub fn is_ca_trusted() -> bool {
    let mut cmd = Command::new("certutil");
    cmd.args(["-user", "-store", "Root", CA_COMMON_NAME]);
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.output().map(|o| o.status.success()).unwrap_or(false)
}

/// Import the CA into the current user's trust store. The first import shows a
/// one-time Windows security prompt (no admin needed with `-user`).
#[cfg(windows)]
pub fn trust_ca() -> Result<(), String> {
    ensure_ca()?;
    let mut cmd = Command::new("certutil");
    cmd.args(["-user", "-addstore", "-f", "Root"]);
    cmd.arg(crate::paths::ca_cert());
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd.output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "certutil could not import the CA: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

#[cfg(not(windows))]
pub fn is_ca_trusted() -> bool {
    false
}

#[cfg(not(windows))]
pub fn trust_ca() -> Result<(), String> {
    Err("trusting the CA is only supported on Windows".into())
}

/// HTTPS feature state for the UI: whether it's enabled and whether the CA is
/// trusted by the OS (so browsers won't warn).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpsStatus {
    pub enabled: bool,
    pub trusted: bool,
}

#[tauri::command]
pub async fn https_status(state: State<'_, StateStore>) -> Result<HttpsStatus, String> {
    let enabled = { state.0.lock().map_err(|e| e.to_string())?.settings.https };
    let trusted = tokio::task::spawn_blocking(is_ca_trusted)
        .await
        .map_err(|e| e.to_string())?;
    Ok(HttpsStatus { enabled, trusted })
}

/// Enable HTTPS: generate the CA if needed, import it into the trust store (one
/// Windows prompt), and flip the setting. Projects then serve over HTTPS on
/// their next start/restart.
#[tauri::command]
pub async fn enable_https(state: State<'_, StateStore>) -> Result<HttpsStatus, String> {
    tokio::task::spawn_blocking(|| {
        ensure_ca()?;
        trust_ca()
    })
    .await
    .map_err(|e| e.to_string())??;
    {
        let mut st = state.0.lock().map_err(|e| e.to_string())?;
        st.settings.https = true;
        st.save()?;
    }
    let trusted = tokio::task::spawn_blocking(is_ca_trusted)
        .await
        .map_err(|e| e.to_string())?;
    Ok(HttpsStatus { enabled: true, trusted })
}

/// Disable HTTPS. Leaves the CA + certs in place (harmless); projects revert to
/// HTTP on their next start/restart.
#[tauri::command]
pub fn disable_https(state: State<'_, StateStore>) -> Result<HttpsStatus, String> {
    let mut st = state.0.lock().map_err(|e| e.to_string())?;
    st.settings.https = false;
    st.save()?;
    Ok(HttpsStatus { enabled: false, trusted: is_ca_trusted() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ca_and_leaf_generate_and_chain_names() {
        // Generate a CA and a leaf in isolation (no disk / trust store).
        let ca_key = KeyPair::generate().unwrap();
        let mut ca_params = CertificateParams::new(Vec::new()).unwrap();
        ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, CA_COMMON_NAME);
        ca_params.distinguished_name = dn;
        ca_params.not_before = rcgen::date_time_ymd(2024, 1, 1);
        ca_params.not_after = rcgen::date_time_ymd(2035, 1, 1);
        let ca_cert = ca_params.self_signed(&ca_key).unwrap();

        let leaf_key = KeyPair::generate().unwrap();
        let mut leaf_params =
            CertificateParams::new(vec!["blog.test".to_string(), "*.blog.test".to_string()]).unwrap();
        leaf_params.not_before = rcgen::date_time_ymd(2024, 1, 1);
        leaf_params.not_after = rcgen::date_time_ymd(2035, 1, 1);
        let leaf = leaf_params.signed_by(&leaf_key, &ca_cert, &ca_key).unwrap();

        let pem = leaf.pem();
        assert!(pem.contains("BEGIN CERTIFICATE"));
        // The leaf PEM is a single cert (issuer is the CA, not embedded).
        assert_eq!(pem.matches("BEGIN CERTIFICATE").count(), 1);
    }
}
