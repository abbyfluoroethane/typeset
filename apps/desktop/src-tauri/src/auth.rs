//! Desktop side of sign-in. See `crates/server/src/auth.rs` for the flow:
//! listen on a random 127.0.0.1 port, open `<server>/auth/login` in the
//! system browser with a PKCE challenge, receive `?code&state` on the
//! loopback callback, exchange it at `POST /auth/token`, and store the token
//! in the keychain via `api::store_token`.

use std::time::Duration;

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::RngCore;
use reqwest::Method;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
};
use typeset_core::{TokenRequest, TokenResponse, User};

use crate::{
    api::{self, Api},
    error::{AppError, AppResult},
    state::AppState,
};

/// How long the user has to finish signing in in the browser.
const LOGIN_TIMEOUT: Duration = Duration::from_secs(5 * 60);
/// How long one loopback connection may take to send its request line.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

#[tauri::command]
pub async fn login(app: AppHandle, state: State<'_, AppState>) -> AppResult<User> {
    let api = Api::new(&state).await?;

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}/callback");

    let verifier = random_token(32);
    let challenge = pkce_challenge(&verifier);
    let oauth_state = random_token(16);

    let url = build_login_url(&api.base_url, &redirect_uri, &oauth_state, &challenge);
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| AppError::Other(format!("could not open the browser: {e}")))?;

    let code = tokio::time::timeout(LOGIN_TIMEOUT, wait_for_callback(&listener, &oauth_state))
        .await
        .map_err(|_| AppError::Other("sign-in timed out".into()))??;
    // The listener is only needed until the callback arrives.
    drop(listener);

    let body = TokenRequest {
        code,
        code_verifier: verifier,
        device_name: Some(device_name()),
    };
    let response: TokenResponse = api::send_json(
        api.request_anonymous(Method::POST, "/auth/token")
            .json(&body),
    )
    .await?;

    api::store_token(&api.base_url, &response.token)?;
    state.set_setting("user", &response.user).await?;
    Ok(response.user)
}

/// Signs out. The local library mirror is cleared too: it belongs to the
/// account that was signed in, and keeping it would let a different account
/// (or server) inherit stale rows and a stale `last_rev`. Installed fonts are
/// left alone; they are tracked separately in `installed_files`.
#[tauri::command]
pub async fn logout(state: State<'_, AppState>) -> AppResult<()> {
    if let Ok(api) = Api::new(&state).await {
        if api.is_signed_in() {
            // Best effort: the token is dropped locally regardless.
            if let Ok(request) = api.request(Method::POST, "/auth/logout") {
                let _ = request.send().await;
            }
        }
        api::delete_token(&api.base_url)?;
    }
    sqlx::query("DELETE FROM settings WHERE key = 'user'")
        .execute(&state.db)
        .await?;
    crate::sync::reset_mirror(&state).await?;
    Ok(())
}

/// The signed-in user, or `None` when signed out or no server is set.
#[tauri::command]
pub async fn current_user(state: State<'_, AppState>) -> AppResult<Option<User>> {
    let api = match Api::new(&state).await {
        Ok(api) => api,
        Err(AppError::NoServer) => return Ok(None),
        Err(e) => return Err(e),
    };
    if !api.is_signed_in() {
        return Ok(None);
    }
    match api.get::<User>("/api/me").await {
        Ok(user) => {
            state.set_setting("user", &user).await?;
            Ok(Some(user))
        }
        Err(AppError::NotSignedIn) => {
            // The session expired or was revoked.
            api::delete_token(&api.base_url)?;
            sqlx::query("DELETE FROM settings WHERE key = 'user'")
                .execute(&state.db)
                .await?;
            Ok(None)
        }
        // Offline (or the server is down): trust the cached user.
        Err(AppError::Http(_)) | Err(AppError::Api { status: 500.., .. }) => {
            state.get_setting("user").await
        }
        Err(e) => Err(e),
    }
}

/// `n` random bytes, base64url without padding.
fn random_token(n: usize) -> String {
    let mut bytes = vec![0u8; n];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn device_name() -> String {
    gethostname::gethostname().to_string_lossy().into_owned()
}

fn build_login_url(server: &str, redirect_uri: &str, state: &str, challenge: &str) -> String {
    format!(
        "{server}/auth/login?redirect_uri={}&state={}&code_challenge={}&code_challenge_method=S256",
        api::encode_segment(redirect_uri),
        api::encode_segment(state),
        api::encode_segment(challenge),
    )
}

/// What the browser delivered to `/callback`.
#[derive(Debug, PartialEq)]
enum Callback {
    Success { code: String, state: String },
    Failure(String),
}

/// Accepts connections until a valid `/callback` arrives. Other paths get a
/// 404 and are ignored; a callback with the wrong state or an error aborts.
async fn wait_for_callback(listener: &TcpListener, expected_state: &str) -> AppResult<String> {
    loop {
        let (mut stream, _) = listener.accept().await?;
        let target =
            match tokio::time::timeout(REQUEST_TIMEOUT, read_request_target(&mut stream)).await {
                Ok(Some(target)) => target,
                _ => continue,
            };
        let Some(callback) = parse_callback(&target) else {
            respond(&mut stream, 404, "Not found").await;
            continue;
        };
        let outcome = match callback {
            Callback::Success { code, state } if state == expected_state => Ok(code),
            Callback::Success { .. } => Err("state mismatch".to_string()),
            Callback::Failure(message) => Err(message),
        };
        return match outcome {
            Ok(code) => {
                let page = result_page("Signed in to Typeset", "You can close this tab.");
                respond(&mut stream, 200, &page).await;
                Ok(code)
            }
            Err(message) => {
                let page = result_page("Sign-in failed", &message);
                respond(&mut stream, 400, &page).await;
                Err(AppError::Other(format!("sign-in failed: {message}")))
            }
        };
    }
}

/// Reads the HTTP request line and returns the request target for a GET.
async fn read_request_target(stream: &mut TcpStream) -> Option<String> {
    let mut line = String::new();
    // Cap the line so a misbehaving client cannot make us buffer forever.
    BufReader::new(stream.take(8192))
        .read_line(&mut line)
        .await
        .ok()?;
    parse_request_line(&line).map(str::to_owned)
}

/// Parses `GET /path?query HTTP/1.1` into the target, requiring GET.
fn parse_request_line(line: &str) -> Option<&str> {
    let mut parts = line.trim_end().split(' ');
    let (method, target, version) = (parts.next()?, parts.next()?, parts.next()?);
    (method == "GET" && version.starts_with("HTTP/") && parts.next().is_none()).then_some(target)
}

/// Interprets a request target as the OAuth callback, if it is one.
fn parse_callback(target: &str) -> Option<Callback> {
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if path != "/callback" {
        return None;
    }
    let (mut code, mut state, mut error, mut description) = (None, None, None, None);
    for pair in query.split('&') {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        let v = percent_decode(v);
        match percent_decode(k).as_str() {
            "code" => code = Some(v),
            "state" => state = Some(v),
            "error" => error = Some(v),
            "error_description" => description = Some(v),
            _ => {}
        }
    }
    if let Some(error) = error {
        return Some(Callback::Failure(match description {
            Some(d) if !d.is_empty() => format!("{error}: {d}"),
            _ => error,
        }));
    }
    match (code, state) {
        (Some(code), Some(state)) if !code.is_empty() => Some(Callback::Success { code, state }),
        _ => Some(Callback::Failure(
            "the callback was missing code or state".into(),
        )),
    }
}

/// Decodes `%XX` escapes and `+` in a query component.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if bytes.get(i + 1..i + 3).is_some() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
                match hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                    Some(b) => {
                        out.push(b);
                        i += 2;
                    }
                    None => out.push(b'%'),
                }
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// A small self-contained page shown in the browser after the redirect.
fn result_page(title: &str, detail: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{title}</title>\
         <style>body{{font-family:system-ui,sans-serif;display:grid;place-items:center;\
         height:100vh;margin:0;color:#222}}main{{text-align:center}}</style></head>\
         <body><main><h1>{title}</h1><p>{detail}</p></main></body></html>",
        title = html_escape(title),
        detail = html_escape(detail),
    )
}

async fn respond(stream: &mut TcpStream, status: u16, body: &str) {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        _ => "Not Found",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_line_parsing() {
        assert_eq!(
            parse_request_line("GET /callback?code=a HTTP/1.1\r\n"),
            Some("/callback?code=a")
        );
        assert_eq!(parse_request_line("POST /callback HTTP/1.1\r\n"), None);
        assert_eq!(parse_request_line("garbage"), None);
    }

    #[test]
    fn callback_parsing() {
        assert_eq!(
            parse_callback("/callback?code=a%20b&state=xyz"),
            Some(Callback::Success {
                code: "a b".into(),
                state: "xyz".into()
            })
        );
        assert_eq!(parse_callback("/favicon.ico"), None);
        assert_eq!(
            parse_callback("/callback?error=access_denied&error_description=not+allowed"),
            Some(Callback::Failure("access_denied: not allowed".into()))
        );
        assert!(matches!(
            parse_callback("/callback"),
            Some(Callback::Failure(_))
        ));
    }

    #[test]
    fn login_url_is_encoded() {
        let url = build_login_url("https://f.example", "http://127.0.0.1:1/callback", "s", "c");
        assert_eq!(
            url,
            "https://f.example/auth/login?redirect_uri=http%3A%2F%2F127.0.0.1%3A1%2Fcallback\
             &state=s&code_challenge=c&code_challenge_method=S256"
        );
    }

    #[test]
    fn pkce_matches_rfc7636_example() {
        assert_eq!(
            pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn escapes_html() {
        assert_eq!(
            html_escape("<a href=\"x\">&"),
            "&lt;a href=&quot;x&quot;&gt;&amp;"
        );
        assert_eq!(percent_decode("100%"), "100%");
    }
}
