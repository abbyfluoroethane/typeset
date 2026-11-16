//! Settings, catalog, library, collections and uploads: talks to the server
//! and keeps the local mirror tables up to date.

use reqwest::{Method, multipart};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sqlx::SqliteConnection;
use tauri::State;
use typeset_core::{
    CatalogResponse, Collection, CollectionInput, LibraryEntry, LibraryEntryInput, SyncResponse,
    Upload,
};

use crate::{
    api::{self, Api, encode_segment},
    error::{AppError, AppResult},
    fonts::ReconcileReport,
    state::{AppState, Settings, now},
};

/// A cached catalog younger than this is served without asking the server.
const CATALOG_MAX_AGE_SECS: i64 = 6 * 60 * 60;

/// Everything the UI needs to render the library, read from the local
/// mirror so it works offline.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LocalState {
    /// Live (non-deleted) entries only.
    pub library: Vec<LibraryEntry>,
    pub collections: Vec<Collection>,
    pub uploads: Vec<Upload>,
    pub last_rev: i64,
    pub last_synced_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncSummary {
    pub state: LocalState,
    /// Present when `auto_install` is on.
    pub reconcile: Option<ReconcileReport>,
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> AppResult<Settings> {
    state.settings().await
}

#[tauri::command]
pub async fn update_settings(
    state: State<'_, AppState>,
    mut settings: Settings,
) -> AppResult<Settings> {
    settings.server_url = match settings.server_url.as_deref() {
        Some(url) => normalize_server_url(url)?,
        None => None,
    };
    state.save_settings(&settings).await?;
    Ok(settings)
}

/// Trims and validates a server URL. Empty input means "no server".
fn normalize_server_url(input: &str) -> AppResult<Option<String>> {
    let url = input.trim().trim_end_matches('/');
    if url.is_empty() {
        return Ok(None);
    }
    let host = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .ok_or_else(|| AppError::Other("server URL must start with http:// or https://".into()))?;
    if host.is_empty() {
        return Err(AppError::Other("server URL has no host".into()));
    }
    Ok(Some(url.to_string()))
}

#[tauri::command]
pub async fn get_catalog(state: State<'_, AppState>, refresh: bool) -> AppResult<CatalogResponse> {
    let cached: Option<(i64, String)> =
        sqlx::query_as("SELECT fetched_at, body FROM catalog_cache WHERE id = 1")
            .fetch_optional(&state.db)
            .await?;
    if !refresh
        && let Some((fetched_at, body)) = &cached
        && now() - fetched_at < CATALOG_MAX_AGE_SECS
        && let Ok(catalog) = serde_json::from_str(body)
    {
        return Ok(catalog);
    }

    let api = Api::new(&state).await?;
    let path = if refresh {
        "/api/catalog?refresh=1"
    } else {
        "/api/catalog"
    };
    match api.get::<CatalogResponse>(path).await {
        Ok(catalog) => {
            sqlx::query(
                "INSERT INTO catalog_cache (id, fetched_at, body) VALUES (1, ?, ?)
                 ON CONFLICT (id) DO UPDATE SET fetched_at = excluded.fetched_at, body = excluded.body",
            )
            .bind(now())
            .bind(serde_json::to_string(&catalog)?)
            .execute(&state.db)
            .await?;
            Ok(catalog)
        }
        // Offline: a stale catalog beats none.
        Err(AppError::Http(e)) => match cached.and_then(|(_, b)| serde_json::from_str(&b).ok()) {
            Some(catalog) => Ok(catalog),
            None => Err(AppError::Http(e)),
        },
        Err(e) => Err(e),
    }
}

#[tauri::command]
pub async fn get_local_state(state: State<'_, AppState>) -> AppResult<LocalState> {
    read_local_state(&state).await
}

async fn read_bodies<T: DeserializeOwned>(state: &AppState, table: &str) -> AppResult<Vec<T>> {
    let rows: Vec<String> = sqlx::query_scalar(&format!("SELECT body FROM {table}"))
        .fetch_all(&state.db)
        .await?;
    rows.iter()
        .map(|body| serde_json::from_str(body).map_err(AppError::from))
        .collect()
}

async fn read_local_state(state: &AppState) -> AppResult<LocalState> {
    let mut library: Vec<LibraryEntry> = read_bodies(state, "library").await?;
    library.retain(|e| !e.deleted);
    library.sort_by(|a, b| a.family.cmp(&b.family).then_with(|| a.key.cmp(&b.key)));

    let mut collections: Vec<Collection> = read_bodies(state, "collections").await?;
    collections.retain(|c| !c.deleted);
    collections.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.id.cmp(&b.id)));

    let mut uploads: Vec<Upload> = read_bodies(state, "uploads").await?;
    uploads.retain(|u| !u.deleted);
    uploads.sort_by(|a, b| (&a.family, a.weight, a.italic).cmp(&(&b.family, b.weight, b.italic)));

    Ok(LocalState {
        library,
        collections,
        uploads,
        last_rev: state.get_setting("last_rev").await?.unwrap_or(0),
        last_synced_at: state.get_setting("last_synced_at").await?,
    })
}

/// Empties the mirror and forgets the sync position. Used on logout and
/// when the configured server changes.
pub(crate) async fn reset_mirror(state: &AppState) -> AppResult<()> {
    let mut tx = state.db.begin().await?;
    clear_mirror(&mut tx).await?;
    tx.commit().await?;
    Ok(())
}

async fn clear_mirror(conn: &mut SqliteConnection) -> AppResult<()> {
    for table in ["library", "collections", "uploads"] {
        sqlx::query(&format!("DELETE FROM {table}"))
            .execute(&mut *conn)
            .await?;
    }
    sqlx::query(
        "DELETE FROM settings WHERE key IN ('last_rev', 'last_synced_at', 'synced_server')",
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

async fn put_setting(
    conn: &mut SqliteConnection,
    key: &str,
    value: &impl Serialize,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO settings (key, value) VALUES (?, ?)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
    )
    .bind(key)
    .bind(serde_json::to_string(value)?)
    .execute(conn)
    .await?;
    Ok(())
}

// Mirror upserts: one row per server row, stored as its JSON wire form.

async fn upsert_library(conn: &mut SqliteConnection, entry: &LibraryEntry) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO library (key, body) VALUES (?, ?)
         ON CONFLICT (key) DO UPDATE SET body = excluded.body",
    )
    .bind(&entry.key)
    .bind(serde_json::to_string(entry)?)
    .execute(conn)
    .await?;
    Ok(())
}

async fn upsert_collection(conn: &mut SqliteConnection, collection: &Collection) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO collections (id, body) VALUES (?, ?)
         ON CONFLICT (id) DO UPDATE SET body = excluded.body",
    )
    .bind(&collection.id)
    .bind(serde_json::to_string(collection)?)
    .execute(conn)
    .await?;
    Ok(())
}

async fn upsert_upload(conn: &mut SqliteConnection, upload: &Upload) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO uploads (id, body) VALUES (?, ?)
         ON CONFLICT (id) DO UPDATE SET body = excluded.body",
    )
    .bind(&upload.id)
    .bind(serde_json::to_string(upload)?)
    .execute(conn)
    .await?;
    Ok(())
}

/// Pulls changes since `last_rev` into the mirror in one transaction. Resets
/// the mirror first if the configured server differs from the last one
/// synced. Does not touch installed fonts.
pub async fn pull(state: &AppState) -> AppResult<()> {
    let api = Api::new(state).await?;
    let synced_server: Option<String> = state.get_setting("synced_server").await?;
    let server_changed = synced_server.as_deref() != Some(api.base_url.as_str());
    let since: i64 = if server_changed {
        0
    } else {
        state.get_setting("last_rev").await?.unwrap_or(0)
    };

    let response: SyncResponse = api.get(&format!("/api/sync?since={since}")).await?;

    let mut tx = state.db.begin().await?;
    if server_changed {
        clear_mirror(&mut tx).await?;
    }
    for entry in &response.library {
        upsert_library(&mut tx, entry).await?;
    }
    for collection in &response.collections {
        upsert_collection(&mut tx, collection).await?;
    }
    for upload in &response.uploads {
        upsert_upload(&mut tx, upload).await?;
    }
    put_setting(&mut tx, "last_rev", &response.rev.max(since)).await?;
    put_setting(&mut tx, "last_synced_at", &now()).await?;
    put_setting(&mut tx, "synced_server", &api.base_url).await?;
    tx.commit().await?;
    Ok(())
}

#[tauri::command]
pub async fn sync_now(state: State<'_, AppState>) -> AppResult<SyncSummary> {
    pull(&state).await?;
    let reconcile = if state.settings().await?.auto_install {
        match crate::fonts::reconcile(&state).await {
            Ok(report) => Some(report),
            Err(e) => {
                tracing::warn!("font reconcile failed after sync: {e}");
                None
            }
        }
    } else {
        None
    };
    Ok(SyncSummary {
        state: read_local_state(&state).await?,
        reconcile,
    })
}

#[tauri::command]
pub async fn library_put(
    state: State<'_, AppState>,
    key: String,
    input: LibraryEntryInput,
) -> AppResult<LibraryEntry> {
    let api = Api::new(&state).await?;
    let entry: LibraryEntry = api
        .put(&format!("/api/library/{}", encode_segment(&key)), &input)
        .await?;
    upsert_library(&mut *state.db.acquire().await?, &entry).await?;
    Ok(entry)
}

#[tauri::command]
pub async fn library_delete(state: State<'_, AppState>, key: String) -> AppResult<LibraryEntry> {
    let api = Api::new(&state).await?;
    let entry: LibraryEntry = api
        .delete(&format!("/api/library/{}", encode_segment(&key)))
        .await?;
    upsert_library(&mut *state.db.acquire().await?, &entry).await?;
    Ok(entry)
}

#[tauri::command]
pub async fn collection_put(
    state: State<'_, AppState>,
    id: String,
    input: CollectionInput,
) -> AppResult<Collection> {
    let api = Api::new(&state).await?;
    let collection: Collection = api
        .put(&format!("/api/collections/{}", encode_segment(&id)), &input)
        .await?;
    upsert_collection(&mut *state.db.acquire().await?, &collection).await?;
    Ok(collection)
}

#[tauri::command]
pub async fn collection_delete(state: State<'_, AppState>, id: String) -> AppResult<Collection> {
    let api = Api::new(&state).await?;
    let collection: Collection = api
        .delete(&format!("/api/collections/{}", encode_segment(&id)))
        .await?;
    upsert_collection(&mut *state.db.acquire().await?, &collection).await?;
    Ok(collection)
}

/// Uploads font files one by one, stopping at the first failure. Files
/// already uploaded stay in the mirror, and a pull afterwards brings in the
/// `upload:<family>` library entries the server created.
#[tauri::command]
pub async fn upload_fonts(
    state: State<'_, AppState>,
    paths: Vec<String>,
) -> AppResult<Vec<Upload>> {
    let api = Api::new(&state).await?;
    let mut uploaded = Vec::new();
    let mut failure = None;
    for path in &paths {
        match upload_one(&state, &api, path).await {
            Ok(upload) => uploaded.push(upload),
            Err(e) => {
                failure = Some(e);
                break;
            }
        }
    }
    if !uploaded.is_empty()
        && let Err(e) = pull(&state).await
    {
        // Don't hide an upload error behind a pull error.
        if failure.is_none() {
            failure = Some(e);
        } else {
            tracing::warn!("sync after upload failed: {e}");
        }
    }
    match failure {
        Some(e) => Err(e),
        None => Ok(uploaded),
    }
}

async fn upload_one(state: &AppState, api: &Api, path: &str) -> AppResult<Upload> {
    let filename = std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| AppError::Other(format!("not a file path: {path}")))?;
    let bytes = tokio::fs::read(path).await?;
    let form =
        multipart::Form::new().part("file", multipart::Part::bytes(bytes).file_name(filename));
    let upload: Upload =
        api::send_json(api.request(Method::POST, "/api/uploads")?.multipart(form)).await?;
    upsert_upload(&mut *state.db.acquire().await?, &upload).await?;
    Ok(upload)
}

#[tauri::command]
pub async fn delete_upload(state: State<'_, AppState>, id: String) -> AppResult<Upload> {
    let api = Api::new(&state).await?;
    let upload: Upload = api
        .delete(&format!("/api/uploads/{}", encode_segment(&id)))
        .await?;
    upsert_upload(&mut *state.db.acquire().await?, &upload).await?;
    Ok(upload)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_server_url() {
        let n = |s| normalize_server_url(s).unwrap();
        assert_eq!(
            n("  https://fonts.example.com/// "),
            Some("https://fonts.example.com".into())
        );
        assert_eq!(
            n("http://localhost:8080"),
            Some("http://localhost:8080".into())
        );
        assert_eq!(n("   "), None);
        assert!(normalize_server_url("fonts.example.com").is_err());
        assert!(normalize_server_url("https://").is_err());
    }
}
