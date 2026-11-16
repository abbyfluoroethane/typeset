//! Web setup and admin page, served at `/admin`.
//!
//! Who may use it:
//! - While setup is open (the server is not configured, or has no admin
//!   yet), anyone holding the setup token. The server prints the token to
//!   its log at startup; the browser keeps it in a cookie.
//! - Afterwards, admins signed in through the identity provider.
//!
//! The first admin is the user who signs in from the setup page. The page
//! itself is three static files; everything else is JSON under
//! `/admin/api`.

use std::collections::HashMap;

use axum::{
    Json, Router,
    extract::{FromRequestParts, Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header, request::Parts},
    middleware,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{delete, get, post, put},
};
use openidconnect::url::Url;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    auth::{
        self, KIND_WEB, KIND_WEB_CLAIM, SESSION_COOKIE, constant_time_eq, cookie_value, set_cookie,
        sha256_hex,
    },
    catalog, db,
    error::{AppError, AppResult},
    settings::{self, Settings},
    state::AppState,
};

const SETUP_COOKIE: &str = "typeset_setup";
/// The setup cookie only needs to outlive one setup sitting.
const SETUP_COOKIE_TTL_SECS: i64 = 24 * 60 * 60;

const PAGE: &str = include_str!("../web/admin.html");
const SCRIPT: &str = include_str!("../web/admin.js");
const STYLE: &str = include_str!("../web/admin.css");

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(|| async { Redirect::to("/admin") }))
        .route("/setup", get(setup_link))
        .route("/admin", get(|| async { Html(PAGE) }))
        .route("/admin/admin.js", get(script))
        .route("/admin/admin.css", get(style))
        .route("/admin/login", get(login))
        .route("/admin/api/state", get(get_state))
        .route("/admin/api/unlock", post(unlock))
        .route("/admin/api/logout", post(logout))
        .route("/admin/api/settings", get(get_settings).put(put_settings))
        .route("/admin/api/test/oidc", post(test_oidc))
        .route("/admin/api/test/fonts", post(test_fonts))
        .route("/admin/api/overview", get(overview))
        .route(
            "/admin/api/users/{id}",
            put(update_user).delete(delete_user),
        )
        .route("/admin/api/sessions/{id}", delete(delete_session))
        .route("/admin/api/catalog/refresh", post(refresh_catalog))
        .layer(middleware::map_response(security_headers))
}

async fn security_headers(mut response: Response) -> Response {
    let headers = response.headers_mut();
    for (name, value) in [
        (
            header::CONTENT_SECURITY_POLICY,
            "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; \
             form-action 'self'; frame-ancestors 'none'; base-uri 'none'",
        ),
        (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        (header::REFERRER_POLICY, "no-referrer"),
        (header::CACHE_CONTROL, "no-store"),
    ] {
        headers.insert(name, HeaderValue::from_static(value));
    }
    response
}

async fn script() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        SCRIPT,
    )
}

async fn style() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/css; charset=utf-8")], STYLE)
}

// ---------------------------------------------------------------------------
// Who is asking

async fn admin_exists(state: &AppState) -> AppResult<bool> {
    Ok(
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM users WHERE is_admin = 1)")
            .fetch_one(&state.db)
            .await?,
    )
}

/// Setup is open until the server is configured and has an admin.
async fn setup_open(state: &AppState) -> AppResult<bool> {
    Ok(state.setup_token_hash.is_some()
        && (!state.settings().is_configured() || !admin_exists(state).await?))
}

fn token_matches(state: &AppState, token: &str) -> bool {
    state
        .setup_token_hash
        .as_deref()
        .is_some_and(|hash| constant_time_eq(&sha256_hex(token.as_bytes()), hash))
}

/// True if the request carries the setup token and setup is still open.
async fn has_setup_access(state: &AppState, headers: &HeaderMap) -> AppResult<bool> {
    Ok(
        cookie_value(headers, SETUP_COOKIE).is_some_and(|t| token_matches(state, t))
            && setup_open(state).await?,
    )
}

struct SessionUser {
    id: String,
    email: Option<String>,
    name: Option<String>,
    is_admin: bool,
    token_hash: String,
}

/// The user behind the session cookie, if it is valid.
async fn session_user(state: &AppState, headers: &HeaderMap) -> AppResult<Option<SessionUser>> {
    let Some(token) = cookie_value(headers, SESSION_COOKIE) else {
        return Ok(None);
    };
    let token_hash = sha256_hex(token.as_bytes());
    let row: Option<(String, Option<String>, Option<String>, bool)> = sqlx::query_as(
        "SELECT u.id, u.email, u.name, u.is_admin FROM sessions s \
         JOIN users u ON u.id = s.user_id WHERE s.token_hash = ? AND s.expires_at > ?",
    )
    .bind(&token_hash)
    .bind(db::now())
    .fetch_optional(&state.db)
    .await?;
    Ok(row.map(|(id, email, name, is_admin)| SessionUser {
        id,
        email,
        name,
        is_admin,
        token_hash,
    }))
}

/// A signed-in admin.
struct Admin {
    user_id: String,
}

impl FromRequestParts<AppState> for Admin {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> AppResult<Self> {
        match session_user(state, &parts.headers).await? {
            Some(user) if user.is_admin => Ok(Admin { user_id: user.id }),
            Some(_) => Err(AppError::Forbidden),
            None => Err(AppError::Unauthorized),
        }
    }
}

/// Someone allowed to change settings: an admin, or the holder of the setup
/// token while setup is open.
struct Operator;

impl FromRequestParts<AppState> for Operator {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> AppResult<Self> {
        if has_setup_access(state, &parts.headers).await? {
            return Ok(Operator);
        }
        Admin::from_request_parts(parts, state)
            .await
            .map(|_| Operator)
    }
}

/// Whether the browser reached us over HTTPS, so cookies can be `Secure`.
fn is_https(state: &AppState, headers: &HeaderMap) -> bool {
    headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.eq_ignore_ascii_case("https"))
        || state.settings().public_url.starts_with("https://")
}

// ---------------------------------------------------------------------------
// Entry, unlock and sign-in

#[derive(Deserialize)]
struct SetupQuery {
    token: Option<String>,
}

/// `GET /setup?token=…`, the link printed in the server log. Moves the
/// token into a cookie so it does not stay in the address bar.
async fn setup_link(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<SetupQuery>,
) -> Response {
    match q.token {
        Some(token) if token_matches(&state, &token) => {
            let cookie = set_cookie(
                SETUP_COOKIE,
                &token,
                SETUP_COOKIE_TTL_SECS,
                is_https(&state, &headers),
            );
            ([(header::SET_COOKIE, cookie)], Redirect::to("/admin")).into_response()
        }
        _ => Redirect::to("/admin").into_response(),
    }
}

#[derive(Deserialize)]
struct UnlockInput {
    token: String,
}

async fn unlock(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<UnlockInput>,
) -> AppResult<Response> {
    let token = input.token.trim();
    if !token_matches(&state, token) || !setup_open(&state).await? {
        return Err(AppError::BadRequest(
            "That token is not valid. Use the one from the most recent server start.".into(),
        ));
    }
    let cookie = set_cookie(
        SETUP_COOKIE,
        token,
        SETUP_COOKIE_TTL_SECS,
        is_https(&state, &headers),
    );
    Ok(([(header::SET_COOKIE, cookie)], StatusCode::NO_CONTENT).into_response())
}

/// Starts a browser sign-in. From the setup page, with no admin yet, the
/// user who signs in becomes the admin.
async fn login(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Response> {
    if !state.settings().is_configured() {
        return Ok(Redirect::to("/admin").into_response());
    }
    let claim = has_setup_access(&state, &headers).await? && !admin_exists(&state).await?;
    let kind = if claim { KIND_WEB_CLAIM } else { KIND_WEB };
    auth::begin_login(&state, kind, "", "", "").await
}

async fn logout(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Response> {
    if let Some(user) = session_user(&state, &headers).await? {
        sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
            .bind(&user.token_hash)
            .execute(&state.db)
            .await?;
    }
    let secure = is_https(&state, &headers);
    Ok((
        [(
            header::SET_COOKIE,
            set_cookie(SESSION_COOKIE, "", 0, secure),
        )],
        StatusCode::NO_CONTENT,
    )
        .into_response())
}

/// Tells the page which screen to show. Needs no credentials.
///
/// - `locked`: setup is open; ask for the setup token.
/// - `setup`: token accepted; show the settings form.
/// - `claim`: configured, no admin yet; sign in to become the admin.
/// - `signin`: set up; ask an admin to sign in.
/// - `forbidden`: signed in, but not an admin.
/// - `ready`: signed in as an admin.
async fn get_state(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Json<Value>> {
    let configured = state.settings().is_configured();
    let user = session_user(&state, &headers).await?;
    let user_json = user
        .as_ref()
        .map(|u| json!({ "id": u.id, "email": u.email, "name": u.name }));

    let phase = if user.as_ref().is_some_and(|u| u.is_admin) {
        "ready"
    } else if has_setup_access(&state, &headers).await? {
        if configured { "claim" } else { "setup" }
    } else if setup_open(&state).await? {
        "locked"
    } else if user.is_some() {
        "forbidden"
    } else {
        "signin"
    };
    Ok(Json(json!({ "phase": phase, "user": user_json })))
}

// ---------------------------------------------------------------------------
// Settings

async fn saved_settings(state: &AppState) -> AppResult<HashMap<String, String>> {
    let rows: Vec<(String, String)> = sqlx::query_as("SELECT key, value FROM settings")
        .fetch_all(&state.db)
        .await?;
    Ok(rows.into_iter().collect())
}

fn env_name(key: &str) -> &'static str {
    settings::KEYS
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, env)| *env)
        .unwrap_or_default()
}

/// The current settings. Secrets are reported only as set or not set.
async fn get_settings(_op: Operator, State(state): State<AppState>) -> AppResult<Json<Value>> {
    let s = state.settings();
    let saved = saved_settings(&state).await?;
    let field = |key: &str, value: Value| {
        let locked = s.is_locked(key);
        json!({ "value": value, "locked": locked, "env": env_name(key) })
    };
    Ok(Json(json!({
        "configured": s.is_configured(),
        "redirectUri": s.redirect_uri(),
        // False while the public URL is only the built-in default.
        "publicUrlKnown": s.is_locked(settings::PUBLIC_URL)
            || saved.contains_key(settings::PUBLIC_URL),
        "fields": {
            "publicUrl": field(settings::PUBLIC_URL, json!(s.public_url)),
            "oidcIssuer": field(settings::OIDC_ISSUER, json!(s.oidc_issuer)),
            "oidcClientId": field(settings::OIDC_CLIENT_ID, json!(s.oidc_client_id)),
            "oidcClientSecret": field(
                settings::OIDC_CLIENT_SECRET,
                json!(s.oidc_client_secret.is_some()),
            ),
            "allowedEmails": field(settings::ALLOWED_EMAILS, json!(s.allowed_emails.join(", "))),
            "googleFontsApiKey": field(
                settings::GOOGLE_FONTS_API_KEY,
                json!(!s.google_fonts_api_key.is_empty()),
            ),
        },
    })))
}

/// A field left out (or `null`) keeps its saved value.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SettingsInput {
    public_url: Option<String>,
    oidc_issuer: Option<String>,
    oidc_client_id: Option<String>,
    /// An empty string clears the secret (public client).
    oidc_client_secret: Option<String>,
    allowed_emails: Option<String>,
    google_fonts_api_key: Option<String>,
}

/// Checks a public URL and returns it without a trailing slash.
fn normalize_public_url(input: &str) -> AppResult<String> {
    let bad = |m: &str| AppError::BadRequest(format!("Public URL {m}"));
    let url = Url::parse(input.trim()).map_err(|_| bad("is not a valid URL."))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(bad("must start with http:// or https://."));
    }
    if url.query().is_some() || url.fragment().is_some() || !url.username().is_empty() {
        return Err(bad("must not contain a query, fragment or user name."));
    }
    Ok(url.as_str().trim_end_matches('/').to_owned())
}

/// Validates and saves settings, then applies them without a restart. The
/// issuer and the Google Fonts key are tested live when they change, so a
/// saved configuration is one that worked.
async fn put_settings(
    _op: Operator,
    State(state): State<AppState>,
    Json(input): Json<SettingsInput>,
) -> AppResult<Json<Value>> {
    let current = state.settings();
    let mut saved = saved_settings(&state).await?;
    let mut changed: Vec<&str> = Vec::new();

    let inputs = [
        (settings::PUBLIC_URL, input.public_url),
        (settings::OIDC_ISSUER, input.oidc_issuer),
        (settings::OIDC_CLIENT_ID, input.oidc_client_id),
        (settings::OIDC_CLIENT_SECRET, input.oidc_client_secret),
        (settings::ALLOWED_EMAILS, input.allowed_emails),
        (settings::GOOGLE_FONTS_API_KEY, input.google_fonts_api_key),
    ];
    for (key, value) in inputs {
        let Some(value) = value else { continue };
        if current.is_locked(key) {
            continue;
        }
        let value = match key {
            settings::PUBLIC_URL => normalize_public_url(&value)?,
            settings::ALLOWED_EMAILS => settings::parse_emails(&value).join(","),
            _ => value.trim().to_owned(),
        };
        saved.insert(key.to_owned(), value);
        changed.push(key);
    }

    let next = Settings::merge(&state.config, &saved);
    for (value, label) in [
        (&next.oidc_issuer, "The OIDC issuer URL"),
        (&next.oidc_client_id, "The OIDC client ID"),
        (&next.google_fonts_api_key, "The Google Fonts API key"),
    ] {
        if value.is_empty() {
            return Err(AppError::BadRequest(format!("{label} is required.")));
        }
    }
    if let Some(bad) = next.allowed_emails.iter().find(|e| !e.contains('@')) {
        return Err(AppError::BadRequest(format!(
            "\"{bad}\" is not an email address."
        )));
    }

    if next.oidc_issuer != current.oidc_issuer {
        auth::check_issuer(&next.oidc_issuer).await?;
    }
    let fresh_catalog = if next.google_fonts_api_key != current.google_fonts_api_key {
        Some(
            catalog::fetch_catalog(&state.config.google_fonts_url, &next.google_fonts_api_key)
                .await
                .map_err(|e| AppError::Upstream(format!("Google Fonts key check failed: {e}")))?,
        )
    } else {
        None
    };

    let mut tx = state.db.begin().await?;
    for key in &changed {
        sqlx::query(
            "INSERT INTO settings (key, value) VALUES (?, ?) \
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        )
        .bind(key)
        .bind(&saved[*key])
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    state.set_settings(next);
    auth::forget_oidc_client().await;
    if let Some(families) = fresh_catalog {
        // The check already downloaded the catalog, so keep it.
        catalog::store_catalog(&state, families).await?;
    }
    tracing::info!("settings updated: {}", changed.join(", "));
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct TestOidcInput {
    /// Empty tests the issuer in effect now.
    #[serde(default)]
    issuer: String,
}

async fn test_oidc(
    _op: Operator,
    State(state): State<AppState>,
    Json(input): Json<TestOidcInput>,
) -> AppResult<Json<Value>> {
    let settings = state.settings();
    let issuer = match input.issuer.trim() {
        typed if !typed.is_empty() && !settings.is_locked(settings::OIDC_ISSUER) => typed,
        _ => settings.oidc_issuer.as_str(),
    };
    if issuer.is_empty() {
        return Err(AppError::BadRequest("Enter the issuer URL first.".into()));
    }
    let endpoint = auth::check_issuer(issuer).await?;
    Ok(Json(json!({ "authorizationEndpoint": endpoint })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestFontsInput {
    /// Empty tests the key in effect now.
    #[serde(default)]
    api_key: String,
}

async fn test_fonts(
    _op: Operator,
    State(state): State<AppState>,
    Json(input): Json<TestFontsInput>,
) -> AppResult<Json<Value>> {
    let settings = state.settings();
    let key = match input.api_key.trim() {
        typed if !typed.is_empty() && !settings.is_locked(settings::GOOGLE_FONTS_API_KEY) => typed,
        _ => settings.google_fonts_api_key.as_str(),
    };
    if key.is_empty() {
        return Err(AppError::BadRequest("Enter the API key first.".into()));
    }
    let families = catalog::fetch_catalog(&state.config.google_fonts_url, key)
        .await
        .map_err(AppError::Upstream)?;
    Ok(Json(json!({ "families": families.len() })))
}

// ---------------------------------------------------------------------------
// Admin: overview, users, devices, catalog

type UserRow = (
    String,
    Option<String>,
    Option<String>,
    bool,
    i64,
    i64,
    i64,
    i64,
);
type SessionRow = (i64, String, Option<String>, i64, i64, i64);

async fn overview(admin: Admin, State(state): State<AppState>) -> AppResult<Json<Value>> {
    let now = db::now();
    let users: Vec<UserRow> = sqlx::query_as(
        "SELECT u.id, u.email, u.name, u.is_admin, u.created_at, \
           (SELECT COUNT(*) FROM library_entries l WHERE l.user_id = u.id AND l.deleted = 0), \
           (SELECT COUNT(*) FROM uploads p WHERE p.user_id = u.id AND p.deleted = 0), \
           (SELECT COALESCE(SUM(p.size), 0) FROM uploads p \
             WHERE p.user_id = u.id AND p.deleted = 0) \
         FROM users u ORDER BY u.created_at",
    )
    .fetch_all(&state.db)
    .await?;
    let sessions: Vec<SessionRow> = sqlx::query_as(
        "SELECT rowid, user_id, device_name, created_at, last_used_at, expires_at \
         FROM sessions WHERE expires_at > ? ORDER BY last_used_at DESC",
    )
    .bind(now)
    .fetch_all(&state.db)
    .await?;
    let catalog: Option<(i64, i64)> = sqlx::query_as(
        "SELECT fetched_at, json_array_length(body, '$.families') FROM catalog_cache WHERE id = 1",
    )
    .fetch_optional(&state.db)
    .await?;

    let upload_bytes: i64 = users.iter().map(|u| u.7).sum();
    let upload_files: i64 = users.iter().map(|u| u.6).sum();
    Ok(Json(json!({
        "me": admin.user_id,
        "now": now,
        "version": env!("CARGO_PKG_VERSION"),
        "catalog": catalog.map(|(fetched_at, families)| json!({
            "fetchedAt": fetched_at,
            "families": families,
        })),
        "uploads": { "files": upload_files, "bytes": upload_bytes },
        "users": users.into_iter().map(|u| json!({
            "id": u.0, "email": u.1, "name": u.2, "isAdmin": u.3, "createdAt": u.4,
            "libraryCount": u.5, "uploadCount": u.6, "uploadBytes": u.7,
        })).collect::<Vec<_>>(),
        "sessions": sessions.into_iter().map(|s| json!({
            "id": s.0, "userId": s.1, "deviceName": s.2, "createdAt": s.3,
            "lastUsedAt": s.4, "expiresAt": s.5,
        })).collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UserInput {
    is_admin: bool,
}

async fn update_user(
    admin: Admin,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<UserInput>,
) -> AppResult<StatusCode> {
    // Admins cannot demote themselves, so there is always one left.
    if id == admin.user_id && !input.is_admin {
        return Err(AppError::BadRequest(
            "You cannot remove your own admin role.".into(),
        ));
    }
    let done = sqlx::query("UPDATE users SET is_admin = ? WHERE id = ?")
        .bind(input.is_admin)
        .bind(&id)
        .execute(&state.db)
        .await?;
    if done.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Deletes a user with their library, collections, uploads and sessions.
async fn delete_user(
    admin: Admin,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    if id == admin.user_id {
        return Err(AppError::BadRequest(
            "You cannot delete your own account.".into(),
        ));
    }
    let mut tx = state.db.begin().await?;
    let blobs: Vec<String> =
        sqlx::query_scalar("SELECT DISTINCT sha256 FROM uploads WHERE user_id = ?")
            .bind(&id)
            .fetch_all(&mut *tx)
            .await?;
    // The other tables go with the user (ON DELETE CASCADE).
    let done = sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    if done.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    // Keep files that another user's upload still points at.
    let mut orphans = Vec::new();
    for sha256 in blobs {
        let shared: bool =
            sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM uploads WHERE sha256 = ?)")
                .bind(&sha256)
                .fetch_one(&mut *tx)
                .await?;
        if !shared {
            orphans.push(sha256);
        }
    }
    tx.commit().await?;

    for sha256 in orphans {
        if let Err(e) = tokio::fs::remove_file(state.config.uploads_dir().join(&sha256)).await
            && e.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!("failed to remove upload blob {sha256}: {e}");
        }
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Signs a device out.
async fn delete_session(
    _admin: Admin,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<StatusCode> {
    let done = sqlx::query("DELETE FROM sessions WHERE rowid = ?")
        .bind(id)
        .execute(&state.db)
        .await?;
    if done.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn refresh_catalog(_admin: Admin, State(state): State<AppState>) -> AppResult<Json<Value>> {
    let families = catalog::refresh_now(&state).await?;
    Ok(Json(json!({ "families": families })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_public_urls() {
        assert_eq!(
            normalize_public_url(" https://fonts.example.com/ ").unwrap(),
            "https://fonts.example.com"
        );
        assert_eq!(
            normalize_public_url("http://localhost:8080/typeset/").unwrap(),
            "http://localhost:8080/typeset"
        );
        for bad in [
            "",
            "fonts.example.com",
            "ftp://fonts.example.com",
            "https://fonts.example.com/?x=1",
            "https://user@fonts.example.com",
        ] {
            assert!(normalize_public_url(bad).is_err(), "{bad}");
        }
    }
}
