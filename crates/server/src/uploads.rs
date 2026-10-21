//! User-uploaded fonts: the only font files the server stores. Bytes go to
//! `<data_dir>/uploads/<sha256>`; metadata goes in the `uploads` table.

use std::{io::SeekFrom, path::PathBuf};

use axum::{
    Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, State},
    http::{HeaderValue, StatusCode, header},
    response::Response,
    routing::{get, post},
};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;
use typeset_core::{FontSource, Upload, family_key};

use crate::{
    auth::AuthUser,
    config::Config,
    db::{next_rev, now},
    error::{AppError, AppResult},
    state::AppState,
    sync::{UPLOAD_COLS, UploadRow},
};

pub fn router(config: &Config) -> Router<AppState> {
    // POST   /api/uploads              multipart field `file` -> Upload
    //        (also adds/undeletes the `upload:<family>` library entry)
    // GET    /api/uploads/{id}/file    -> font bytes
    // DELETE /api/uploads/{id}         -> Upload (tombstone)
    Router::new()
        .route("/api/uploads", post(create_upload))
        .route("/api/uploads/{id}", axum::routing::delete(delete_upload))
        .route("/api/uploads/{id}/file", get(get_upload_file))
        .layer(DefaultBodyLimit::max(config.max_upload_bytes))
}

/// What we read out of a font file.
#[derive(Debug, PartialEq)]
struct FontInfo {
    family: String,
    style: String,
    weight: u16,
    italic: bool,
}

/// Content type for a font by its magic bytes.
fn font_content_type(head: &[u8]) -> &'static str {
    if head.starts_with(b"OTTO") {
        "font/otf"
    } else {
        "font/ttf"
    }
}

/// Parses TTF/OTF bytes. WOFF, WOFF2 and collections are rejected.
fn parse_font(data: &[u8]) -> AppResult<FontInfo> {
    let bad = |m: &str| AppError::BadRequest(m.to_owned());
    match data.get(..4) {
        Some(b"wOFF") | Some(b"wOF2") => {
            return Err(bad(
                "WOFF and WOFF2 fonts are not supported; upload a TTF or OTF file",
            ));
        }
        Some(b"ttcf") => {
            return Err(bad(
                "font collections (.ttc) are not supported; upload a TTF or OTF file",
            ));
        }
        Some([0, 1, 0, 0]) | Some(b"OTTO") | Some(b"true") => {}
        _ => return Err(bad("file is not a TrueType or OpenType font")),
    }
    let face = ttf_parser::Face::parse(data, 0)
        .map_err(|_| bad("file is not a valid TrueType or OpenType font"))?;

    let name = |ids: [u16; 2]| {
        ids.iter().find_map(|id| {
            face.names()
                .into_iter()
                .filter(|n| n.name_id == *id)
                .find_map(|n| n.to_string())
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty())
        })
    };
    let family = name([16, 1]).ok_or_else(|| bad("font has no family name"))?;
    let style = name([17, 2]).unwrap_or_else(|| "Regular".to_owned());
    Ok(FontInfo {
        family,
        style,
        weight: face.weight().to_number(),
        italic: face.is_italic(),
    })
}

/// Strips path components and control characters and caps the length.
fn sanitize_filename(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or("");
    let cleaned: String = base.chars().filter(|c| !c.is_control()).collect();
    let cleaned = cleaned.trim().trim_start_matches('.');
    let capped: String = cleaned.chars().take(120).collect();
    let capped = capped.trim().to_owned();
    if capped.is_empty() {
        "font.ttf".to_owned()
    } else {
        capped
    }
}

/// Makes a filename safe inside a quoted `Content-Disposition` header.
fn header_filename(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii() && !matches!(c, '"' | '\\' | ';' | '%') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn blob_path(config: &Config, sha256: &str) -> PathBuf {
    config.uploads_dir().join(sha256)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Writes `data` to `path` atomically (temp file in the same dir, then rename).
async fn write_blob(path: &std::path::Path, data: &[u8]) -> AppResult<()> {
    if tokio::fs::try_exists(path).await? {
        return Ok(());
    }
    let dir = path
        .parent()
        .ok_or_else(|| AppError::Internal("bad uploads path".into()))?;
    tokio::fs::create_dir_all(dir).await?;
    let tmp = dir.join(format!(".tmp-{}", uuid::Uuid::new_v4()));
    if let Err(e) = tokio::fs::write(&tmp, data).await {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(e.into());
    }
    if let Err(e) = tokio::fs::rename(&tmp, path).await {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(e.into());
    }
    Ok(())
}

async fn create_upload(
    user: AuthUser,
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> AppResult<Json<Upload>> {
    let map_mp = |e: axum::extract::multipart::MultipartError| {
        if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
            AppError::PayloadTooLarge("upload is too large".into())
        } else {
            AppError::BadRequest(format!("invalid multipart body: {}", e.body_text()))
        }
    };

    let mut file = None;
    while let Some(field) = multipart.next_field().await.map_err(map_mp)? {
        if field.name() == Some("file") {
            let filename = sanitize_filename(field.file_name().unwrap_or(""));
            let bytes = field.bytes().await.map_err(map_mp)?;
            file = Some((filename, bytes));
            break;
        }
    }
    let (filename, bytes) =
        file.ok_or_else(|| AppError::BadRequest("missing multipart field `file`".into()))?;

    let info = parse_font(&bytes)?;
    let sha256 = hex(&Sha256::digest(&bytes));
    write_blob(&blob_path(&state.config, &sha256), &bytes).await?;

    let mut tx = state.db.begin().await?;
    let existing = sqlx::query_as::<_, UploadRow>(&format!(
        "SELECT {UPLOAD_COLS} FROM uploads WHERE user_id = ? AND sha256 = ? AND deleted = 0"
    ))
    .bind(&user.user_id)
    .bind(&sha256)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(row) = existing {
        return Ok(Json(row.into_upload()));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let rev = next_rev(&mut tx).await?;
    let ts = now();
    sqlx::query(
        "INSERT INTO uploads (id, user_id, family, style, weight, italic, filename, sha256, size, deleted, rev, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?, ?)",
    )
    .bind(&id)
    .bind(&user.user_id)
    .bind(&info.family)
    .bind(&info.style)
    .bind(info.weight as i64)
    .bind(info.italic)
    .bind(&filename)
    .bind(&sha256)
    .bind(bytes.len() as i64)
    .bind(rev)
    .bind(ts)
    .execute(&mut *tx)
    .await?;

    // Make sure the family shows up in the library, keeping any favorite.
    sqlx::query(
        "INSERT INTO library_entries (user_id, key, source, family, variants, favorite, deleted, rev, updated_at)
         VALUES (?, ?, 'upload', ?, '[]', 0, 0, ?, ?)
         ON CONFLICT(user_id, key) DO UPDATE SET
            deleted = 0, rev = excluded.rev, updated_at = excluded.updated_at",
    )
    .bind(&user.user_id)
    .bind(family_key(FontSource::Upload, &info.family))
    .bind(&info.family)
    .bind(rev)
    .bind(ts)
    .execute(&mut *tx)
    .await?;

    let row = fetch_upload(&mut tx, &user.user_id, &id).await?;
    tx.commit().await?;
    Ok(Json(row.ok_or(AppError::NotFound)?.into_upload()))
}

async fn fetch_upload(
    conn: &mut sqlx::SqliteConnection,
    user_id: &str,
    id: &str,
) -> sqlx::Result<Option<UploadRow>> {
    sqlx::query_as::<_, UploadRow>(&format!(
        "SELECT {UPLOAD_COLS} FROM uploads WHERE id = ? AND user_id = ?"
    ))
    .bind(id)
    .bind(user_id)
    .fetch_optional(conn)
    .await
}

async fn delete_upload(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<Upload>> {
    let mut tx = state.db.begin().await?;
    let current = fetch_upload(&mut tx, &user.user_id, &id)
        .await?
        .filter(|r| !r.deleted)
        .ok_or(AppError::NotFound)?;
    let (family, sha256) = (current.family.clone(), current.sha256.clone());

    let rev = next_rev(&mut tx).await?;
    let ts = now();
    sqlx::query("UPDATE uploads SET deleted = 1, rev = ?, updated_at = ? WHERE id = ?")
        .bind(rev)
        .bind(ts)
        .bind(&id)
        .execute(&mut *tx)
        .await?;

    // Tombstone the library entry once the family has no files left.
    let remaining: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM uploads WHERE user_id = ? AND family = ? AND deleted = 0",
    )
    .bind(&user.user_id)
    .bind(&family)
    .fetch_one(&mut *tx)
    .await?;
    if remaining == 0 {
        sqlx::query(
            "UPDATE library_entries SET deleted = 1, rev = ?, updated_at = ?
             WHERE user_id = ? AND key = ? AND deleted = 0",
        )
        .bind(rev)
        .bind(ts)
        .bind(&user.user_id)
        .bind(family_key(FontSource::Upload, &family))
        .execute(&mut *tx)
        .await?;
    }

    let shared: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM uploads WHERE sha256 = ? AND deleted = 0")
            .bind(&sha256)
            .fetch_one(&mut *tx)
            .await?;
    let row = fetch_upload(&mut tx, &user.user_id, &id).await?;
    tx.commit().await?;

    if shared == 0
        && let Err(e) = tokio::fs::remove_file(blob_path(&state.config, &sha256)).await
        && e.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!("failed to remove upload blob {sha256}: {e}");
    }
    Ok(Json(row.ok_or(AppError::NotFound)?.into_upload()))
}

async fn get_upload_file(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Response> {
    let mut conn = state.db.acquire().await?;
    let row = fetch_upload(&mut conn, &user.user_id, &id)
        .await?
        .filter(|r| !r.deleted)
        .ok_or(AppError::NotFound)?;
    drop(conn);

    let mut file = tokio::fs::File::open(blob_path(&state.config, &row.sha256))
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                AppError::NotFound
            } else {
                e.into()
            }
        })?;
    let mut head = [0u8; 4];
    let n = file.read(&mut head).await?;
    file.seek(SeekFrom::Start(0)).await?;
    let content_type = font_content_type(&head[..n]);
    let disposition = format!(
        "attachment; filename=\"{}\"",
        header_filename(&sanitize_filename(&row.filename))
    );

    let mut resp = Response::new(Body::from_stream(ReaderStream::new(file)));
    let headers = resp.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    headers.insert(header::CONTENT_LENGTH, HeaderValue::from(row.size as u64));
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&disposition)
            .map_err(|e| AppError::Internal(format!("content-disposition: {e}")))?,
    );
    Ok(resp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_filenames() {
        assert_eq!(sanitize_filename("../../etc/passwd"), "passwd");
        assert_eq!(sanitize_filename(r"C:\fonts\My Font.ttf"), "My Font.ttf");
        assert_eq!(sanitize_filename("a\u{0}b\nc.otf"), "abc.otf");
        assert_eq!(sanitize_filename(""), "font.ttf");
        assert_eq!(sanitize_filename("..."), "font.ttf");
        assert_eq!(sanitize_filename(&"x".repeat(500)).chars().count(), 120);
    }

    #[test]
    fn header_filename_is_header_safe() {
        assert_eq!(header_filename("a\"b;é.ttf"), "a_b__.ttf");
    }

    #[test]
    fn detects_content_type() {
        assert_eq!(font_content_type(b"OTTO"), "font/otf");
        assert_eq!(font_content_type(&[0, 1, 0, 0]), "font/ttf");
        assert_eq!(font_content_type(b""), "font/ttf");
    }

    #[test]
    fn rejects_non_fonts() {
        assert!(matches!(
            parse_font(b"hello world"),
            Err(AppError::BadRequest(_))
        ));
        assert!(
            matches!(parse_font(b"wOF2xxxx"), Err(AppError::BadRequest(m)) if m.contains("WOFF"))
        );
        assert!(
            matches!(parse_font(b"ttcfxxxx"), Err(AppError::BadRequest(m)) if m.contains(".ttc"))
        );
        assert!(matches!(
            parse_font(&[0, 1, 0, 0, 0, 0]),
            Err(AppError::BadRequest(_))
        ));
    }
}
