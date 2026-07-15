use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhpExtension {
    pub name: String,
    pub enabled: bool,
    pub installed: bool,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhpVersion {
    pub version: String,
    pub major_minor: String,
    pub status: String,
    pub is_default: bool,
    pub bin_path: String,
    pub ini_path: String,
    pub extensions: Vec<PhpExtension>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub r#type: String,
    pub framework: Option<String>,
    pub php_version: String,
    pub web_server: String,
    pub database: Option<String>,
    pub domain: String,
    pub path: String,
    pub status: String,
    pub git_url: Option<String>,
    pub created_at: String,
    /// Explicit document-root subdirectory (relative to `path`). Set when opening
    /// an existing folder; empty/None falls back to per-type detection.
    #[serde(default)]
    pub doc_root: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectConfig {
    pub name: String,
    pub r#type: String,
    pub framework: Option<String>,
    pub php_version: String,
    pub web_server: String,
    pub database: Option<String>,
    pub domain: String,
    pub path: String,
    #[serde(default)]
    pub git_url: Option<String>,
    /// Optional Composer version constraint for the framework (e.g. "^11").
    /// Empty/None installs the framework's latest stable.
    #[serde(default)]
    pub framework_version: Option<String>,
    /// Document-root subdirectory for "Open existing" projects (relative to path).
    #[serde(default)]
    pub doc_root: Option<String>,
}
