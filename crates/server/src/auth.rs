//! OIDC login and session tokens.
//!
//! Flow (the desktop client never talks to the identity provider directly):
//! 1. Client opens `GET /auth/login?redirect_uri=http://127.0.0.1:<port>/callback
//!    &state=<s>&code_challenge=<c>&code_challenge_method=S256` in the browser.
//! 2. Server stores a `pending_logins` row and redirects to the IdP
//!    (authorization code + PKCE + nonce).
//! 3. IdP redirects to `GET /auth/callback`; server exchanges the code,
//!    verifies the ID token, upserts the user, enforces `allowed_emails`,
//!    stores a one-time `auth_codes` row and redirects to the client's
//!    loopback `redirect_uri?code=<code>&state=<s>`.
//! 4. Client calls `POST /auth/token` with the code and its PKCE verifier
//!    and receives a long-lived bearer token.
//!
//! The web admin page signs in through the same IdP round trip (see
//! `begin_login`), but step 3 ends with a session cookie and a redirect to
//! `/admin` instead of a one-time code.

use std::{
    fmt::Write as _,
    sync::{Arc, OnceLock},
};

use axum::{
    Json, Router,
    extract::{FromRequestParts, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header, request::Parts},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD as B64};
use openidconnect::{
    AuthenticationFlow, AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointMaybeSet,
    EndpointNotSet, EndpointSet, IssuerUrl, Nonce, PkceCodeChallenge, PkceCodeVerifier,
    RedirectUrl, Scope, TokenResponse as OidcTokenResponse,
    core::{CoreClient, CoreProviderMetadata, CoreResponseType},
    reqwest,
    url::{Host, Url},
};
use rand::RngCore;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use tokio::sync::Mutex;
use typeset_core::{TokenRequest, TokenResponse, User};

use crate::{
    db,
    error::{AppError, AppResult},
    settings::Settings,
    state::AppState,
};

/// Pending logins older than this are dead.
const PENDING_TTL_SECS: i64 = 10 * 60;
/// Lifetime of the one-time code handed to the desktop client.
const AUTH_CODE_TTL_SECS: i64 = 60;
const SESSION_TTL_SECS: i64 = 90 * 24 * 60 * 60;
/// Sessions are only touched in the database this often.
const SESSION_TOUCH_SECS: i64 = 60 * 60;
/// Admin page sessions are shorter and do not renew.
const WEB_SESSION_TTL_SECS: i64 = 7 * 24 * 60 * 60;

/// Cookie that holds the admin page's session token.
pub const SESSION_COOKIE: &str = "typeset_session";

/// `pending_logins.kind` values.
pub const KIND_DESKTOP: &str = "desktop";
pub const KIND_WEB: &str = "web";
/// A web login that also makes the user the first admin.
pub const KIND_WEB_CLAIM: &str = "web-claim";

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/login", get(login))
        .route("/auth/callback", get(callback))
        .route("/auth/token", post(token))
        .route("/auth/logout", post(logout))
        .route("/api/me", get(me))
}

/// The signed-in user, resolved from `Authorization: Bearer <token>`.
/// Handlers take this as an argument to require authentication.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: String,
    pub token_hash: String,
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> AppResult<Self> {
        let token = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split_once(' '))
            .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
            .map(|(_, t)| t.trim())
            .filter(|t| !t.is_empty())
            .ok_or(AppError::Unauthorized)?;
        let token_hash = sha256_hex(token.as_bytes());

        let now = db::now();
        let (user_id, last_used_at): (String, i64) = sqlx::query_as(
            "SELECT user_id, last_used_at FROM sessions WHERE token_hash = ? AND expires_at > ?",
        )
        .bind(&token_hash)
        .bind(now)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::Unauthorized)?;

        if now - last_used_at >= SESSION_TOUCH_SECS {
            sqlx::query(
                "UPDATE sessions SET last_used_at = ?, expires_at = ? WHERE token_hash = ?",
            )
            .bind(now)
            .bind(now + SESSION_TTL_SECS)
            .bind(&token_hash)
            .execute(&state.db)
            .await?;
        }
        Ok(AuthUser {
            user_id,
            token_hash,
        })
    }
}

// ---------------------------------------------------------------------------
// OIDC client

/// Discovery gives us an auth URL and (maybe) a token URL, which is what the
/// typestate parameters below encode.
type OidcClient = CoreClient<
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointMaybeSet,
    EndpointMaybeSet,
>;

static OIDC_CLIENT: Mutex<Option<Arc<OidcClient>>> = Mutex::const_new(None);

/// Shared HTTP client for talking to the identity provider. Redirects are
/// disabled, as following them opens the server to SSRF.
fn http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::ClientBuilder::new()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("failed to build HTTP client")
    })
}

/// Returns the cached client, discovering the provider on first use.
async fn oidc_client(settings: &Settings) -> AppResult<Arc<OidcClient>> {
    if !settings.is_configured() {
        return Err(AppError::NotConfigured);
    }
    let mut slot = OIDC_CLIENT.lock().await;
    if let Some(client) = &*slot {
        return Ok(client.clone());
    }
    let metadata = discover(&settings.oidc_issuer).await?;
    let redirect = RedirectUrl::new(settings.redirect_uri())
        .map_err(|e| AppError::Internal(format!("invalid redirect URI: {e}")))?;
    let client = CoreClient::from_provider_metadata(
        metadata,
        ClientId::new(settings.oidc_client_id.clone()),
        settings.oidc_client_secret.clone().map(ClientSecret::new),
    )
    .set_redirect_uri(redirect);
    let client = Arc::new(client);
    *slot = Some(client.clone());
    Ok(client)
}

/// Fetches the provider's discovery document.
async fn discover(issuer: &str) -> AppResult<CoreProviderMetadata> {
    let issuer = IssuerUrl::new(issuer.to_owned())
        .map_err(|e| AppError::BadRequest(format!("invalid OIDC issuer URL: {e}")))?;
    CoreProviderMetadata::discover_async(issuer, http_client())
        .await
        .map_err(|e| AppError::Upstream(format!("OIDC discovery failed: {}", error_chain(&e))))
}

/// Checks that `issuer` serves a usable discovery document and returns its
/// authorization endpoint. Used by the setup page.
pub async fn check_issuer(issuer: &str) -> AppResult<String> {
    let metadata = discover(issuer).await?;
    if metadata.token_endpoint().is_none() {
        return Err(AppError::Upstream(
            "the OIDC provider has no token endpoint".into(),
        ));
    }
    Ok(metadata.authorization_endpoint().to_string())
}

/// Drops the cached client so the next request re-discovers the provider
/// (keys or endpoints may have rotated, or the settings changed).
pub async fn forget_oidc_client() {
    *OIDC_CLIENT.lock().await = None;
}

fn error_chain(e: &dyn std::error::Error) -> String {
    let mut msg = e.to_string();
    let mut cur = e.source();
    while let Some(c) = cur {
        let _ = write!(msg, ": {c}");
        cur = c.source();
    }
    msg
}

// ---------------------------------------------------------------------------
// Handlers

#[derive(Deserialize)]
struct LoginQuery {
    redirect_uri: String,
    state: String,
    code_challenge: String,
    code_challenge_method: String,
}

async fn login(State(state): State<AppState>, Query(q): Query<LoginQuery>) -> AppResult<Response> {
    if q.code_challenge_method != "S256" {
        return Err(AppError::BadRequest(
            "code_challenge_method must be S256".into(),
        ));
    }
    if !valid_code_challenge(&q.code_challenge) {
        return Err(AppError::BadRequest("invalid code_challenge".into()));
    }
    if !is_loopback_redirect(&q.redirect_uri) {
        return Err(AppError::BadRequest(
            "redirect_uri must be an http loopback URL (127.0.0.1, localhost or [::1])".into(),
        ));
    }
    if q.state.is_empty() || q.state.len() > 512 {
        return Err(AppError::BadRequest("invalid state".into()));
    }
    begin_login(
        &state,
        KIND_DESKTOP,
        &q.redirect_uri,
        &q.state,
        &q.code_challenge,
    )
    .await
}

/// Stores a pending login and redirects the browser to the identity
/// provider. The `client_*` values are only used by desktop logins.
pub async fn begin_login(
    state: &AppState,
    kind: &str,
    client_redirect_uri: &str,
    client_state: &str,
    client_code_challenge: &str,
) -> AppResult<Response> {
    // Opportunistic cleanup of abandoned logins.
    let now = db::now();
    sqlx::query("DELETE FROM pending_logins WHERE created_at < ?")
        .bind(now - PENDING_TTL_SECS)
        .execute(&state.db)
        .await?;
    sqlx::query("DELETE FROM auth_codes WHERE expires_at < ?")
        .bind(now)
        .execute(&state.db)
        .await?;

    let client = oidc_client(&state.settings()).await?;
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    let (url, csrf, nonce) = client
        .authorize_url(
            AuthenticationFlow::<CoreResponseType>::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        )
        .add_scope(Scope::new("email".into()))
        .add_scope(Scope::new("profile".into()))
        .set_pkce_challenge(pkce_challenge)
        .url();

    // The OIDC `state` doubles as the primary key of the pending row.
    sqlx::query(
        "INSERT INTO pending_logins (id, client_redirect_uri, client_state, client_code_challenge, \
         oidc_pkce_verifier, nonce, created_at, kind) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(csrf.secret())
    .bind(client_redirect_uri)
    .bind(client_state)
    .bind(client_code_challenge)
    .bind(pkce_verifier.secret())
    .bind(nonce.secret())
    .bind(now)
    .bind(kind)
    .execute(&state.db)
    .await?;

    Ok(Redirect::to(url.as_str()).into_response())
}

#[derive(Deserialize)]
struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

async fn callback(State(state): State<AppState>, Query(q): Query<CallbackQuery>) -> Response {
    match finish_login(&state, q).await {
        Ok(redirect) => redirect,
        Err(LoginError(status, msg)) => error_page(status, &msg),
    }
}

/// Failure shown to the user in the browser as a small HTML page.
struct LoginError(StatusCode, String);

impl From<AppError> for LoginError {
    fn from(e: AppError) -> Self {
        // Log the detail, show something generic.
        tracing::error!(error = %e, "login failed");
        match e {
            AppError::BadRequest(m) => LoginError(StatusCode::BAD_REQUEST, m),
            _ => LoginError(
                StatusCode::BAD_GATEWAY,
                "Sign-in failed. Please try again.".into(),
            ),
        }
    }
}

impl From<sqlx::Error> for LoginError {
    fn from(e: sqlx::Error) -> Self {
        AppError::Db(e).into()
    }
}

async fn finish_login(state: &AppState, q: CallbackQuery) -> Result<Response, LoginError> {
    let bad = |m: &str| LoginError(StatusCode::BAD_REQUEST, m.to_owned());

    if let Some(err) = q.error {
        let detail = q
            .error_description
            .map(|d| format!(": {d}"))
            .unwrap_or_default();
        // Single-use pending row is useless now; leave it to the cleanup.
        return Err(bad(&format!(
            "The identity provider reported an error ({err}){detail}"
        )));
    }
    let (Some(code), Some(oidc_state)) = (q.code, q.state) else {
        return Err(bad("Missing code or state."));
    };

    // Single use: the row is gone once looked up.
    let now = db::now();
    let pending: Option<(String, String, String, String, String, String)> = sqlx::query_as(
        "DELETE FROM pending_logins WHERE id = ? AND created_at >= ? RETURNING \
         client_redirect_uri, client_state, client_code_challenge, oidc_pkce_verifier, nonce, kind",
    )
    .bind(&oidc_state)
    .bind(now - PENDING_TTL_SECS)
    .fetch_optional(&state.db)
    .await?;
    let Some((client_redirect, client_state, client_challenge, pkce_verifier, nonce, kind)) =
        pending
    else {
        return Err(bad(
            "This sign-in link has expired or was already used. Start again from Typeset.",
        ));
    };

    let settings = state.settings();
    let client = oidc_client(&settings).await?;
    let result = exchange(&client, code, pkce_verifier, nonce).await;
    let claims = match result {
        Ok(c) => c,
        Err(e) => {
            forget_oidc_client().await;
            return Err(e.into());
        }
    };

    if !settings.allowed_emails.is_empty() {
        let allowed = claims.email_verified != Some(false)
            && claims
                .email
                .as_ref()
                .is_some_and(|e| settings.allowed_emails.contains(&e.to_lowercase()));
        if !allowed {
            return Err(LoginError(
                StatusCode::FORBIDDEN,
                "This account is not allowed to use this server.".into(),
            ));
        }
    }

    let user_id: String = sqlx::query_scalar(
        "INSERT INTO users (id, issuer, subject, email, name, created_at) VALUES (?, ?, ?, ?, ?, ?) \
         ON CONFLICT (issuer, subject) DO UPDATE SET email = excluded.email, name = excluded.name \
         RETURNING id",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(&claims.issuer)
    .bind(&claims.subject)
    .bind(&claims.email)
    .bind(&claims.name)
    .bind(now)
    .fetch_one(&state.db)
    .await?;

    if kind != KIND_DESKTOP {
        if kind == KIND_WEB_CLAIM {
            // Only takes effect while there is no admin at all.
            sqlx::query(
                "UPDATE users SET is_admin = 1 WHERE id = ? \
                 AND NOT EXISTS (SELECT 1 FROM users WHERE is_admin = 1)",
            )
            .bind(&user_id)
            .execute(&state.db)
            .await?;
        }
        let token = random_token();
        sqlx::query(
            "INSERT INTO sessions (token_hash, user_id, device_name, created_at, last_used_at, \
             expires_at) VALUES (?, ?, 'Admin page', ?, ?, ?)",
        )
        .bind(sha256_hex(token.as_bytes()))
        .bind(&user_id)
        .bind(now)
        .bind(now)
        .bind(now + WEB_SESSION_TTL_SECS)
        .execute(&state.db)
        .await?;
        let secure = settings.public_url.starts_with("https://");
        let cookie = set_cookie(SESSION_COOKIE, &token, WEB_SESSION_TTL_SECS, secure);
        return Ok(([(header::SET_COOKIE, cookie)], Redirect::to("/admin")).into_response());
    }

    let code = random_token();
    sqlx::query(
        "INSERT INTO auth_codes (code_hash, user_id, code_challenge, expires_at) VALUES (?, ?, ?, ?)",
    )
    .bind(sha256_hex(code.as_bytes()))
    .bind(&user_id)
    .bind(&client_challenge)
    .bind(now + AUTH_CODE_TTL_SECS)
    .execute(&state.db)
    .await?;

    let target = append_query(
        &client_redirect,
        &[("code", &code), ("state", &client_state)],
    )
    .ok_or_else(|| bad("Invalid client redirect."))?;
    Ok(Redirect::to(&target).into_response())
}

/// The ID token claims Typeset cares about.
struct Identity {
    issuer: String,
    subject: String,
    email: Option<String>,
    email_verified: Option<bool>,
    name: Option<String>,
}

/// Exchanges the authorization code and verifies the ID token.
async fn exchange(
    client: &OidcClient,
    code: String,
    pkce_verifier: String,
    nonce: String,
) -> Result<Identity, AppError> {
    let upstream = |what: &str, e: &dyn std::error::Error| {
        AppError::Upstream(format!("OIDC {what}: {}", error_chain(e)))
    };
    let response = client
        .exchange_code(AuthorizationCode::new(code))
        .map_err(|e| AppError::Internal(format!("OIDC provider has no token endpoint: {e}")))?
        .set_pkce_verifier(PkceCodeVerifier::new(pkce_verifier))
        .request_async(http_client())
        .await
        .map_err(|e| upstream("token exchange failed", &e))?;
    let id_token = response
        .id_token()
        .ok_or_else(|| AppError::Upstream("OIDC provider returned no ID token".into()))?;
    let nonce = Nonce::new(nonce);
    let claims = id_token
        .claims(&client.id_token_verifier(), &nonce)
        .map_err(|e| upstream("ID token verification failed", &e))?;

    // Prefer the display name; fall back to the username.
    let name = claims
        .name()
        .and_then(|n| n.get(None))
        .map(|n| n.as_str().to_owned())
        .or_else(|| claims.preferred_username().map(|u| u.as_str().to_owned()));
    Ok(Identity {
        issuer: claims.issuer().as_str().to_owned(),
        subject: claims.subject().as_str().to_owned(),
        email: claims.email().map(|e| e.as_str().to_owned()),
        email_verified: claims.email_verified(),
        name,
    })
}

async fn token(
    State(state): State<AppState>,
    Json(req): Json<TokenRequest>,
) -> AppResult<Json<TokenResponse>> {
    let now = db::now();
    // Single use, even if the checks below fail.
    let row: Option<(String, String, i64)> = sqlx::query_as(
        "DELETE FROM auth_codes WHERE code_hash = ? RETURNING user_id, code_challenge, expires_at",
    )
    .bind(sha256_hex(req.code.as_bytes()))
    .fetch_optional(&state.db)
    .await?;
    let invalid = || AppError::BadRequest("invalid or expired code".into());
    let (user_id, challenge, expires_at) = row.ok_or_else(invalid)?;
    if expires_at < now || !verify_pkce(&req.code_verifier, &challenge) {
        return Err(invalid());
    }

    let token = random_token();
    let device_name = req
        .device_name
        .map(|d| d.chars().take(100).collect::<String>())
        .filter(|d| !d.trim().is_empty());
    sqlx::query(
        "INSERT INTO sessions (token_hash, user_id, device_name, created_at, last_used_at, expires_at) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(sha256_hex(token.as_bytes()))
    .bind(&user_id)
    .bind(device_name)
    .bind(now)
    .bind(now)
    .bind(now + SESSION_TTL_SECS)
    .execute(&state.db)
    .await?;

    let user = load_user(&state.db, &user_id).await?;
    Ok(Json(TokenResponse { token, user }))
}

async fn logout(State(state): State<AppState>, user: AuthUser) -> AppResult<StatusCode> {
    sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
        .bind(&user.token_hash)
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn me(State(state): State<AppState>, user: AuthUser) -> AppResult<Json<User>> {
    Ok(Json(load_user(&state.db, &user.user_id).await?))
}

async fn load_user(db: &SqlitePool, user_id: &str) -> AppResult<User> {
    let (id, email, name): (String, Option<String>, Option<String>) =
        sqlx::query_as("SELECT id, email, name FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_optional(db)
            .await?
            .ok_or(AppError::Unauthorized)?;
    Ok(User { id, email, name })
}

// ---------------------------------------------------------------------------
// Helpers

fn error_page(status: StatusCode, message: &str) -> Response {
    let body = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Typeset sign-in</title>\
         <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"></head>\
         <body style=\"font-family:system-ui,sans-serif;max-width:32rem;margin:4rem auto;padding:0 1rem\">\
         <h1>Sign-in failed</h1><p>{}</p></body></html>",
        html_escape(message)
    );
    (status, Html(body)).into_response()
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Reads one cookie from the request.
pub fn cookie_value<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v)
}

/// Builds a `Set-Cookie` value scoped to the admin page. `Lax` lets the
/// cookie ride the redirect back from the identity provider but keeps it
/// off cross-site POSTs. A `max_age` of 0 deletes the cookie.
pub fn set_cookie(name: &str, value: &str, max_age: i64, secure: bool) -> HeaderValue {
    let secure = if secure { "; Secure" } else { "" };
    HeaderValue::from_str(&format!(
        "{name}={value}; Path=/admin; Max-Age={max_age}; HttpOnly; SameSite=Lax{secure}"
    ))
    .expect("cookie values are base64url")
}

/// Compares two strings in constant time.
pub fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// 32 random bytes, base64url without padding.
pub fn random_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    B64.encode(bytes)
}

pub fn sha256_hex(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .fold(String::with_capacity(64), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

/// A base64url-encoded SHA-256 digest is exactly 43 characters.
fn valid_code_challenge(c: &str) -> bool {
    c.len() == 43
        && c.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Only loopback `http` URLs may receive the one-time code, so it can't be
/// redirected to an attacker's site.
fn is_loopback_redirect(uri: &str) -> bool {
    let Ok(url) = Url::parse(uri) else {
        return false;
    };
    url.scheme() == "http"
        && url.username().is_empty()
        && url.password().is_none()
        && match url.host() {
            Some(Host::Domain(d)) => d == "localhost",
            Some(Host::Ipv4(ip)) => ip.octets() == [127, 0, 0, 1],
            Some(Host::Ipv6(ip)) => ip.is_loopback() && ip.octets()[15] == 1,
            None => false,
        }
}

/// Checks `BASE64URL(SHA256(verifier)) == challenge` in constant time.
fn verify_pkce(verifier: &str, challenge: &str) -> bool {
    constant_time_eq(&B64.encode(Sha256::digest(verifier.as_bytes())), challenge)
}

/// Appends URL-encoded query parameters, preserving any existing ones.
fn append_query(uri: &str, params: &[(&str, &str)]) -> Option<String> {
    let mut url = Url::parse(uri).ok()?;
    url.query_pairs_mut().extend_pairs(params);
    Some(url.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_redirects() {
        for ok in [
            "http://127.0.0.1:5000/callback",
            "http://localhost/cb?x=1",
            "http://localhost:8123",
            "http://[::1]:9/x",
            "http://LOCALHOST:1/x",
        ] {
            assert!(is_loopback_redirect(ok), "{ok}");
        }
        for bad in [
            "https://127.0.0.1/cb",
            "http://evil.com/cb",
            "http://127.0.0.1.evil.com/cb",
            "http://localhost@evil.com/",
            "http://user:pw@localhost/",
            "http://127.0.0.2/cb",
            "http://[::2]/cb",
            "http://0.0.0.0/cb",
            "javascript:alert(1)",
            "/relative",
            "",
        ] {
            assert!(!is_loopback_redirect(bad), "{bad}");
        }
    }

    #[test]
    fn pkce() {
        // RFC 7636 appendix B.
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
        assert!(verify_pkce(verifier, challenge));
        assert!(!verify_pkce("other", challenge));
        assert!(!verify_pkce(verifier, "short"));
        assert!(valid_code_challenge(challenge));
        assert!(!valid_code_challenge("short"));
    }

    #[test]
    fn query_appending() {
        assert_eq!(
            append_query(
                "http://127.0.0.1:1/cb",
                &[("code", "a b"), ("state", "x&y=z")]
            )
            .unwrap(),
            "http://127.0.0.1:1/cb?code=a+b&state=x%26y%3Dz"
        );
        assert_eq!(
            append_query("http://localhost/cb?keep=1#frag", &[("code", "c")]).unwrap(),
            "http://localhost/cb?keep=1&code=c#frag"
        );
    }

    #[test]
    fn hashing_and_tokens() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let t = random_token();
        assert_eq!(t.len(), 43);
        assert_ne!(t, random_token());
    }

    #[test]
    fn cookies() {
        let mut headers = HeaderMap::new();
        headers.append(header::COOKIE, "a=1; typeset_session=tok".parse().unwrap());
        headers.append(header::COOKIE, "b=2".parse().unwrap());
        assert_eq!(cookie_value(&headers, SESSION_COOKIE), Some("tok"));
        assert_eq!(cookie_value(&headers, "b"), Some("2"));
        assert_eq!(cookie_value(&headers, "session"), None);
        assert_eq!(
            set_cookie("n", "v", 60, true),
            "n=v; Path=/admin; Max-Age=60; HttpOnly; SameSite=Lax; Secure"
        );
    }

    #[test]
    fn escapes_html() {
        assert_eq!(
            html_escape("<a href=\"x\">&"),
            "&lt;a href=&quot;x&quot;&gt;&amp;"
        );
    }
}
