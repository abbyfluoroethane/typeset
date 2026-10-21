//! Authenticated HTTP client for the Typeset server, plus keychain storage
//! for the session token.

use reqwest::{Method, RequestBuilder, Response};
use serde::{Serialize, de::DeserializeOwned};
use typeset_core::ApiError;

use crate::{
    error::{AppError, AppResult},
    state::AppState,
};

const KEYRING_SERVICE: &str = "dev.typeset.desktop";

fn keyring_entry(server_url: &str) -> AppResult<keyring::Entry> {
    Ok(keyring::Entry::new(KEYRING_SERVICE, server_url)?)
}

/// Reads the session token for `server_url` from the OS keychain.
pub fn load_token(server_url: &str) -> AppResult<Option<String>> {
    match keyring_entry(server_url)?.get_password() {
        Ok(token) => Ok(Some(token)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn store_token(server_url: &str, token: &str) -> AppResult<()> {
    Ok(keyring_entry(server_url)?.set_password(token)?)
}

pub fn delete_token(server_url: &str) -> AppResult<()> {
    match keyring_entry(server_url)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// A client bound to the configured server and the stored token.
pub struct Api {
    http: reqwest::Client,
    pub base_url: String,
    token: Option<String>,
}

impl Api {
    /// Builds a client from settings. Fails with `NoServer` when no server
    /// URL is configured. The token may be absent; authenticated calls then
    /// fail with `NotSignedIn`.
    pub async fn new(state: &AppState) -> AppResult<Self> {
        let base_url = state
            .settings()
            .await?
            .server_url
            .ok_or(AppError::NoServer)?;
        let token = load_token(&base_url)?;
        Ok(Self {
            http: state.http.clone(),
            base_url,
            token,
        })
    }

    pub fn is_signed_in(&self) -> bool {
        self.token.is_some()
    }

    /// Starts a request to `path` (which begins with `/`) with the bearer
    /// token attached.
    pub fn request(&self, method: Method, path: &str) -> AppResult<RequestBuilder> {
        let token = self.token.as_deref().ok_or(AppError::NotSignedIn)?;
        Ok(self
            .http
            .request(method, format!("{}{}", self.base_url, path))
            .bearer_auth(token))
    }

    /// Like `request`, but without a token (for `/auth/token`).
    pub fn request_anonymous(&self, method: Method, path: &str) -> RequestBuilder {
        self.http
            .request(method, format!("{}{}", self.base_url, path))
    }

    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> AppResult<T> {
        send_json(self.request(Method::GET, path)?).await
    }

    pub async fn put<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> AppResult<T> {
        send_json(self.request(Method::PUT, path)?.json(body)).await
    }

    pub async fn delete<T: DeserializeOwned>(&self, path: &str) -> AppResult<T> {
        send_json(self.request(Method::DELETE, path)?).await
    }

    pub async fn get_bytes(&self, path: &str) -> AppResult<Vec<u8>> {
        let response = check(self.request(Method::GET, path)?.send().await?).await?;
        Ok(response.bytes().await?.to_vec())
    }
}

/// Sends a request and decodes a JSON body, mapping non-2xx responses to
/// `AppError::Api` (or `NotSignedIn` for 401).
pub async fn send_json<T: DeserializeOwned>(request: RequestBuilder) -> AppResult<T> {
    Ok(check(request.send().await?).await?.json().await?)
}

/// Maps non-2xx responses to errors, using the server's `ApiError` body.
pub async fn check(response: Response) -> AppResult<Response> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Err(AppError::NotSignedIn);
    }
    let text = response.text().await.unwrap_or_default();
    let message = serde_json::from_str::<ApiError>(&text)
        .map(|e| e.error)
        .unwrap_or(text);
    Err(AppError::Api {
        status: status.as_u16(),
        message,
    })
}

/// Percent-encodes one path segment (family keys contain spaces and `:`).
pub fn encode_segment(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}
