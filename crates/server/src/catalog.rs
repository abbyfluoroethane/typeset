//! Google Fonts catalog proxy. The server holds the API key, caches the
//! catalog metadata in SQLite for `catalog_ttl_secs`, and serves it to
//! clients. Font files are never fetched or stored here.

use std::{collections::BTreeMap, time::Duration};

use axum::{
    Router,
    extract::{Query, State},
    http::{HeaderValue, header},
    response::{IntoResponse, Response},
    routing::get,
};
use serde::Deserialize;
use tokio::sync::Mutex;
use tower_http::compression::CompressionLayer;
use typeset_core::{CatalogFamily, CatalogResponse};

use crate::{
    auth::AuthUser,
    db::now,
    error::{AppError, AppResult},
    state::AppState,
};

const GOOGLE_URL: &str = "https://www.googleapis.com/webfonts/v1/webfonts";

/// Held while refreshing so concurrent requests don't all hit Google.
static REFRESH_LOCK: Mutex<()> = Mutex::const_new(());

pub fn router() -> Router<AppState> {
    // GET /api/catalog            -> CatalogResponse (cached)
    // GET /api/catalog?refresh=1  -> forces a refetch from Google
    Router::new()
        .route("/api/catalog", get(get_catalog))
        .layer(CompressionLayer::new())
}

#[derive(Debug, Deserialize)]
struct CatalogQuery {
    #[serde(default)]
    refresh: Option<String>,
}

/// The cached row: when it was fetched and the JSON body.
#[derive(sqlx::FromRow)]
struct CacheRow {
    fetched_at: i64,
    body: String,
}

async fn get_catalog(
    _user: AuthUser,
    State(state): State<AppState>,
    Query(q): Query<CatalogQuery>,
) -> AppResult<Response> {
    let started = now();
    let refresh = matches!(q.refresh.as_deref(), Some("1" | "true"));
    let ttl = state.config.catalog_ttl_secs;
    let is_fresh = |row: &CacheRow| now() - row.fetched_at < ttl;

    let cached = read_cache(&state).await?;
    if let Some(row) = &cached
        && !refresh
        && is_fresh(row)
    {
        return Ok(json_response(cached.unwrap().body));
    }

    let _guard = REFRESH_LOCK.lock().await;
    // Another request may have refreshed while we waited for the lock.
    let cached = read_cache(&state).await?;
    if let Some(row) = &cached {
        let reusable = if refresh {
            row.fetched_at >= started
        } else {
            is_fresh(row)
        };
        if reusable {
            return Ok(json_response(cached.unwrap().body));
        }
    }

    match fetch_catalog(&state.config.google_fonts_api_key).await {
        Ok(families) => {
            let resp = CatalogResponse {
                fetched_at: now(),
                families,
            };
            let body = serde_json::to_string(&resp)
                .map_err(|e| AppError::Internal(format!("serialize catalog: {e}")))?;
            sqlx::query(
                "INSERT INTO catalog_cache (id, fetched_at, body) VALUES (1, ?, ?)
                 ON CONFLICT(id) DO UPDATE SET fetched_at = excluded.fetched_at, body = excluded.body",
            )
            .bind(resp.fetched_at)
            .bind(&body)
            .execute(&state.db)
            .await?;
            Ok(json_response(body))
        }
        Err(e) => match cached {
            Some(row) => {
                tracing::warn!("google fonts fetch failed, serving stale catalog: {e}");
                Ok(json_response(row.body))
            }
            None => Err(AppError::Upstream(e)),
        },
    }
}

async fn read_cache(state: &AppState) -> AppResult<Option<CacheRow>> {
    Ok(
        sqlx::query_as::<_, CacheRow>("SELECT fetched_at, body FROM catalog_cache WHERE id = 1")
            .fetch_optional(&state.db)
            .await?,
    )
}

fn json_response(body: String) -> Response {
    (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        )],
        body,
    )
        .into_response()
}

/// Subset of a `webfonts/v1` item that Typeset uses.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoogleItem {
    family: String,
    #[serde(default)]
    category: String,
    #[serde(default)]
    variants: Vec<String>,
    #[serde(default)]
    subsets: Vec<String>,
    #[serde(default)]
    version: String,
    #[serde(default)]
    last_modified: String,
    #[serde(default)]
    files: BTreeMap<String, String>,
    #[serde(default)]
    menu: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GoogleResponse {
    #[serde(default)]
    items: Vec<GoogleItem>,
}

fn https(url: String) -> String {
    match url.strip_prefix("http://") {
        Some(rest) => format!("https://{rest}"),
        None => url,
    }
}

fn map_item(item: GoogleItem) -> CatalogFamily {
    CatalogFamily {
        family: item.family,
        category: item.category,
        variants: item.variants,
        subsets: item.subsets,
        version: item.version,
        last_modified: item.last_modified,
        files: item.files.into_iter().map(|(k, v)| (k, https(v))).collect(),
        menu: item.menu.map(https),
    }
}

/// Fetches the catalog in popularity order. Errors are plain strings that
/// never contain the API key.
async fn fetch_catalog(api_key: &str) -> Result<Vec<CatalogFamily>, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| e.without_url().to_string())?;
    let resp = client
        .get(GOOGLE_URL)
        .query(&[("key", api_key), ("sort", "popularity")])
        .send()
        .await
        .map_err(|e| e.without_url().to_string())?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("google fonts API returned {status}"));
    }
    let parsed: GoogleResponse = resp.json().await.map_err(|e| e.without_url().to_string())?;
    Ok(parsed.items.into_iter().map(map_item).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_google_items_and_rewrites_http() {
        let json = r#"{"kind":"webfonts#webfontList","items":[
            {"family":"Roboto","variants":["regular","700italic"],"subsets":["latin"],
             "version":"v30","lastModified":"2022-09-22","category":"sans-serif",
             "files":{"regular":"http://fonts.gstatic.com/a.ttf","700italic":"https://fonts.gstatic.com/b.ttf"},
             "menu":"http://fonts.gstatic.com/m.ttf"},
            {"family":"Bare"}]}"#;
        let parsed: GoogleResponse = serde_json::from_str(json).unwrap();
        let fams: Vec<_> = parsed.items.into_iter().map(map_item).collect();
        assert_eq!(fams.len(), 2);
        assert_eq!(fams[0].family, "Roboto");
        assert_eq!(fams[0].files["regular"], "https://fonts.gstatic.com/a.ttf");
        assert_eq!(
            fams[0].files["700italic"],
            "https://fonts.gstatic.com/b.ttf"
        );
        assert_eq!(
            fams[0].menu.as_deref(),
            Some("https://fonts.gstatic.com/m.ttf")
        );
        assert!(fams[1].files.is_empty());
    }
}
