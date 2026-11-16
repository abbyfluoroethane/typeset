//! Library and collection sync. Last write wins; every write takes a new
//! rev from `db::next_rev`, and deletes are tombstones (`deleted = 1`).

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, put},
};
use serde::Deserialize;
use typeset_core::{
    Collection, CollectionInput, FontSource, LibraryEntry, LibraryEntryInput, SyncResponse, Upload,
    parse_family_key,
};

use crate::{
    auth::AuthUser,
    db::{next_rev, now},
    error::{AppError, AppResult},
    state::AppState,
};

pub fn router() -> Router<AppState> {
    // GET    /api/sync?since=N         -> SyncResponse
    // PUT    /api/library/{key}        LibraryEntryInput -> LibraryEntry
    // DELETE /api/library/{key}        -> LibraryEntry (tombstone)
    // PUT    /api/collections/{id}     CollectionInput -> Collection
    // DELETE /api/collections/{id}     -> Collection (tombstone)
    Router::new()
        .route("/api/sync", get(get_sync))
        .route(
            "/api/library/{key}",
            put(put_library).delete(delete_library),
        )
        .route(
            "/api/collections/{id}",
            put(put_collection).delete(delete_collection),
        )
}

#[derive(sqlx::FromRow)]
pub(crate) struct LibraryRow {
    key: String,
    source: String,
    family: String,
    variants: String,
    favorite: bool,
    deleted: bool,
    rev: i64,
    updated_at: i64,
}

impl LibraryRow {
    pub(crate) fn into_entry(self) -> LibraryEntry {
        LibraryEntry {
            key: self.key,
            source: if self.source == "upload" {
                FontSource::Upload
            } else {
                FontSource::Google
            },
            family: self.family,
            variants: serde_json::from_str(&self.variants).unwrap_or_default(),
            favorite: self.favorite,
            deleted: self.deleted,
            rev: self.rev,
            updated_at: self.updated_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct CollectionRow {
    id: String,
    name: String,
    family_keys: String,
    deleted: bool,
    rev: i64,
    updated_at: i64,
}

impl CollectionRow {
    fn into_collection(self) -> Collection {
        Collection {
            id: self.id,
            name: self.name,
            family_keys: serde_json::from_str(&self.family_keys).unwrap_or_default(),
            deleted: self.deleted,
            rev: self.rev,
            updated_at: self.updated_at,
        }
    }
}

#[derive(sqlx::FromRow)]
pub(crate) struct UploadRow {
    pub(crate) id: String,
    pub(crate) family: String,
    pub(crate) style: String,
    pub(crate) weight: i64,
    pub(crate) italic: bool,
    pub(crate) filename: String,
    pub(crate) sha256: String,
    pub(crate) size: i64,
    pub(crate) deleted: bool,
    pub(crate) rev: i64,
    pub(crate) updated_at: i64,
}

impl UploadRow {
    pub(crate) fn into_upload(self) -> Upload {
        Upload {
            id: self.id,
            family: self.family,
            style: self.style,
            weight: self.weight.clamp(0, u16::MAX as i64) as u16,
            italic: self.italic,
            filename: self.filename,
            sha256: self.sha256,
            size: self.size,
            deleted: self.deleted,
            rev: self.rev,
            updated_at: self.updated_at,
        }
    }
}

pub(crate) const UPLOAD_COLS: &str =
    "id, family, style, weight, italic, filename, sha256, size, deleted, rev, updated_at";

const LIBRARY_COLS: &str = "key, source, family, variants, favorite, deleted, rev, updated_at";

#[derive(Debug, Deserialize)]
struct SyncQuery {
    #[serde(default)]
    since: i64,
}

async fn get_sync(
    user: AuthUser,
    State(state): State<AppState>,
    Query(q): Query<SyncQuery>,
) -> AppResult<Json<SyncResponse>> {
    let since = q.since;
    let uid = &user.user_id;

    let library: Vec<LibraryEntry> = sqlx::query_as::<_, LibraryRow>(&format!(
        "SELECT {LIBRARY_COLS} FROM library_entries WHERE user_id = ? AND rev > ? ORDER BY rev"
    ))
    .bind(uid)
    .bind(since)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(LibraryRow::into_entry)
    .collect();

    let collections: Vec<Collection> = sqlx::query_as::<_, CollectionRow>(
        "SELECT id, name, family_keys, deleted, rev, updated_at FROM collections
         WHERE user_id = ? AND rev > ? ORDER BY rev",
    )
    .bind(uid)
    .bind(since)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(CollectionRow::into_collection)
    .collect();

    let uploads: Vec<Upload> = sqlx::query_as::<_, UploadRow>(&format!(
        "SELECT {UPLOAD_COLS} FROM uploads WHERE user_id = ? AND rev > ? ORDER BY rev"
    ))
    .bind(uid)
    .bind(since)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(UploadRow::into_upload)
    .collect();

    let rev = library
        .iter()
        .map(|e| e.rev)
        .chain(collections.iter().map(|c| c.rev))
        .chain(uploads.iter().map(|u| u.rev))
        .max()
        .unwrap_or(since);

    Ok(Json(SyncResponse {
        rev,
        library,
        collections,
        uploads,
    }))
}

async fn put_library(
    user: AuthUser,
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(input): Json<LibraryEntryInput>,
) -> AppResult<Json<LibraryEntry>> {
    let (source, family) = parse_family_key(&key)
        .ok_or_else(|| AppError::BadRequest(format!("invalid family key: {key}")))?;
    let source = match source {
        FontSource::Google => "google",
        FontSource::Upload => "upload",
    };
    let variants = serde_json::to_string(&input.variants)
        .map_err(|e| AppError::Internal(format!("serialize variants: {e}")))?;

    let mut tx = state.db.begin().await?;
    let rev = next_rev(&mut tx).await?;
    sqlx::query(
        "INSERT INTO library_entries (user_id, key, source, family, variants, favorite, deleted, rev, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, 0, ?, ?)
         ON CONFLICT(user_id, key) DO UPDATE SET
            variants = excluded.variants, favorite = excluded.favorite, deleted = 0,
            rev = excluded.rev, updated_at = excluded.updated_at",
    )
    .bind(&user.user_id)
    .bind(&key)
    .bind(source)
    .bind(family)
    .bind(&variants)
    .bind(input.favorite)
    .bind(rev)
    .bind(now())
    .execute(&mut *tx)
    .await?;
    let row = fetch_library(&mut tx, &user.user_id, &key).await?;
    tx.commit().await?;
    Ok(Json(row.ok_or(AppError::NotFound)?.into_entry()))
}

async fn fetch_library(
    conn: &mut sqlx::SqliteConnection,
    user_id: &str,
    key: &str,
) -> sqlx::Result<Option<LibraryRow>> {
    sqlx::query_as::<_, LibraryRow>(&format!(
        "SELECT {LIBRARY_COLS} FROM library_entries WHERE user_id = ? AND key = ?"
    ))
    .bind(user_id)
    .bind(key)
    .fetch_optional(conn)
    .await
}

async fn delete_library(
    user: AuthUser,
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> AppResult<Json<LibraryEntry>> {
    let mut tx = state.db.begin().await?;
    let rev = next_rev(&mut tx).await?;
    let done = sqlx::query(
        "UPDATE library_entries SET deleted = 1, rev = ?, updated_at = ? WHERE user_id = ? AND key = ?",
    )
    .bind(rev)
    .bind(now())
    .bind(&user.user_id)
    .bind(&key)
    .execute(&mut *tx)
    .await?;
    if done.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    let row = fetch_library(&mut tx, &user.user_id, &key).await?;
    tx.commit().await?;
    Ok(Json(row.ok_or(AppError::NotFound)?.into_entry()))
}

/// Validates a collection body, returning the trimmed name and keys JSON.
fn validate_collection(input: &CollectionInput) -> AppResult<(String, String)> {
    let name = input.name.trim();
    let len = name.chars().count();
    if len == 0 || len > 200 {
        return Err(AppError::BadRequest(
            "collection name must be 1 to 200 characters".into(),
        ));
    }
    if let Some(bad) = input
        .family_keys
        .iter()
        .find(|k| parse_family_key(k).is_none())
    {
        return Err(AppError::BadRequest(format!("invalid family key: {bad}")));
    }
    let keys = serde_json::to_string(&input.family_keys)
        .map_err(|e| AppError::Internal(format!("serialize keys: {e}")))?;
    Ok((name.to_owned(), keys))
}

fn parse_collection_id(id: &str) -> AppResult<String> {
    uuid::Uuid::parse_str(id)
        .map(|u| u.to_string())
        .map_err(|_| AppError::BadRequest("collection id must be a UUID".into()))
}

async fn fetch_collection(
    conn: &mut sqlx::SqliteConnection,
    id: &str,
) -> sqlx::Result<Option<(String, CollectionRow)>> {
    let row = sqlx::query_as::<_, CollectionRow>(
        "SELECT id, name, family_keys, deleted, rev, updated_at FROM collections WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&mut *conn)
    .await?;
    match row {
        Some(r) => {
            let owner: String = sqlx::query_scalar("SELECT user_id FROM collections WHERE id = ?")
                .bind(id)
                .fetch_one(conn)
                .await?;
            Ok(Some((owner, r)))
        }
        None => Ok(None),
    }
}

async fn put_collection(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<CollectionInput>,
) -> AppResult<Json<Collection>> {
    let id = parse_collection_id(&id)?;
    let (name, keys) = validate_collection(&input)?;

    let mut tx = state.db.begin().await?;
    if let Some((owner, _)) = fetch_collection(&mut tx, &id).await?
        && owner != user.user_id
    {
        return Err(AppError::NotFound);
    }
    let rev = next_rev(&mut tx).await?;
    sqlx::query(
        "INSERT INTO collections (id, user_id, name, family_keys, deleted, rev, updated_at)
         VALUES (?, ?, ?, ?, 0, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
            name = excluded.name, family_keys = excluded.family_keys, deleted = 0,
            rev = excluded.rev, updated_at = excluded.updated_at",
    )
    .bind(&id)
    .bind(&user.user_id)
    .bind(&name)
    .bind(&keys)
    .bind(rev)
    .bind(now())
    .execute(&mut *tx)
    .await?;
    let row = fetch_collection(&mut tx, &id).await?;
    tx.commit().await?;
    Ok(Json(row.ok_or(AppError::NotFound)?.1.into_collection()))
}

async fn delete_collection(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<Collection>> {
    let id = parse_collection_id(&id)?;
    let mut tx = state.db.begin().await?;
    let rev = next_rev(&mut tx).await?;
    let done = sqlx::query(
        "UPDATE collections SET deleted = 1, rev = ?, updated_at = ? WHERE id = ? AND user_id = ?",
    )
    .bind(rev)
    .bind(now())
    .bind(&id)
    .bind(&user.user_id)
    .execute(&mut *tx)
    .await?;
    if done.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    let row = fetch_collection(&mut tx, &id).await?;
    tx.commit().await?;
    Ok(Json(row.ok_or(AppError::NotFound)?.1.into_collection()))
}
