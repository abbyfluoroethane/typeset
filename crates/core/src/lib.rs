//! Wire types shared by the Typeset server and desktop client.
//!
//! Timestamps are Unix seconds. `rev` values come from a single server-wide
//! counter; every write bumps it, and `GET /api/sync?since=N` returns every
//! row (tombstones included) with `rev > N`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A family from the Google Fonts Developer API (`webfonts/v1`), trimmed to
/// the fields Typeset uses. File URLs always point at Google; Typeset never
/// stores Google font files.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogFamily {
    pub family: String,
    pub category: String,
    pub variants: Vec<String>,
    pub subsets: Vec<String>,
    pub version: String,
    pub last_modified: String,
    /// Variant (e.g. `regular`, `700italic`) to TTF URL.
    pub files: BTreeMap<String, String>,
    /// URL of a tiny subset containing only the glyphs of the family name,
    /// for rendering previews in lists.
    #[serde(default)]
    pub menu: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogResponse {
    pub fetched_at: i64,
    pub families: Vec<CatalogFamily>,
}

/// Where a family's files come from.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum FontSource {
    Google,
    Upload,
}

/// Identifies a family in a user's library: `google:<Family Name>` or
/// `upload:<Family Name>`.
pub fn family_key(source: FontSource, family: &str) -> String {
    match source {
        FontSource::Google => format!("google:{family}"),
        FontSource::Upload => format!("upload:{family}"),
    }
}

/// Splits a family key back into its source and family name.
pub fn parse_family_key(key: &str) -> Option<(FontSource, &str)> {
    let (prefix, family) = key.split_once(':')?;
    if family.is_empty() {
        return None;
    }
    match prefix {
        "google" => Some((FontSource::Google, family)),
        "upload" => Some((FontSource::Upload, family)),
        _ => None,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: String,
    pub email: Option<String>,
    pub name: Option<String>,
}

/// A family the user wants available on every signed-in device.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LibraryEntry {
    pub key: String,
    pub source: FontSource,
    pub family: String,
    /// Google variants to install. Empty means all variants. Ignored for
    /// uploads, where every uploaded file of the family is installed.
    pub variants: Vec<String>,
    pub favorite: bool,
    pub deleted: bool,
    pub rev: i64,
    pub updated_at: i64,
}

/// Body of `PUT /api/library/{key}`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LibraryEntryInput {
    #[serde(default)]
    pub variants: Vec<String>,
    #[serde(default)]
    pub favorite: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub family_keys: Vec<String>,
    pub deleted: bool,
    pub rev: i64,
    pub updated_at: i64,
}

/// Body of `PUT /api/collections/{id}`. The client picks the id (a UUID).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionInput {
    pub name: String,
    #[serde(default)]
    pub family_keys: Vec<String>,
}

/// One font file a user uploaded. These are the only font files the server
/// stores.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Upload {
    pub id: String,
    pub family: String,
    /// Subfamily name from the font, e.g. `Bold Italic`.
    pub style: String,
    pub weight: u16,
    pub italic: bool,
    pub filename: String,
    pub sha256: String,
    pub size: i64,
    pub deleted: bool,
    pub rev: i64,
    pub updated_at: i64,
}

/// Response of `GET /api/sync?since=N`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SyncResponse {
    /// Highest rev included; pass it as `since` next time.
    pub rev: i64,
    pub library: Vec<LibraryEntry>,
    pub collections: Vec<Collection>,
    pub uploads: Vec<Upload>,
}

/// Body of `POST /auth/token`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenRequest {
    pub code: String,
    pub code_verifier: String,
    #[serde(default)]
    pub device_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenResponse {
    pub token: String,
    pub user: User,
}

/// JSON error body for every non-2xx API response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub error: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn family_key_round_trips() {
        let key = family_key(FontSource::Google, "Noto Sans JP");
        assert_eq!(key, "google:Noto Sans JP");
        assert_eq!(
            parse_family_key(&key),
            Some((FontSource::Google, "Noto Sans JP"))
        );
        assert_eq!(parse_family_key("upload:"), None);
        assert_eq!(parse_family_key("other:Foo"), None);
    }
}
