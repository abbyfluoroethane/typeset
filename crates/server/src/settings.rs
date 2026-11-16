//! Settings that can be changed while the server runs: the public URL, the
//! OIDC client and the Google Fonts key. Each one comes from its environment
//! variable if that is set, otherwise from the `settings` table, which the
//! web setup page writes.

use std::collections::HashMap;

use sqlx::SqlitePool;

use crate::config::{Config, EnvOverrides};

pub const PUBLIC_URL: &str = "public_url";
pub const OIDC_ISSUER: &str = "oidc_issuer";
pub const OIDC_CLIENT_ID: &str = "oidc_client_id";
pub const OIDC_CLIENT_SECRET: &str = "oidc_client_secret";
pub const ALLOWED_EMAILS: &str = "allowed_emails";
pub const GOOGLE_FONTS_API_KEY: &str = "google_fonts_api_key";

/// Every key, with the environment variable that overrides it.
pub const KEYS: [(&str, &str); 6] = [
    (PUBLIC_URL, "TYPESET_PUBLIC_URL"),
    (OIDC_ISSUER, "TYPESET_OIDC_ISSUER"),
    (OIDC_CLIENT_ID, "TYPESET_OIDC_CLIENT_ID"),
    (OIDC_CLIENT_SECRET, "TYPESET_OIDC_CLIENT_SECRET"),
    (ALLOWED_EMAILS, "TYPESET_ALLOWED_EMAILS"),
    (GOOGLE_FONTS_API_KEY, "TYPESET_GOOGLE_FONTS_API_KEY"),
];

#[derive(Debug, Clone, Default)]
pub struct Settings {
    /// Externally reachable base URL without a trailing slash, e.g.
    /// `https://fonts.example.com`.
    pub public_url: String,
    pub oidc_issuer: String,
    pub oidc_client_id: String,
    /// `None` for public clients.
    pub oidc_client_secret: Option<String>,
    /// Lower-cased emails allowed to sign in. Empty allows anyone the
    /// identity provider authenticates.
    pub allowed_emails: Vec<String>,
    pub google_fonts_api_key: String,
    /// Keys whose value comes from the environment.
    pub locked: Vec<&'static str>,
}

impl Settings {
    /// Merges the environment over the saved rows.
    pub fn merge(config: &Config, saved: &HashMap<String, String>) -> Self {
        let mut locked = Vec::new();
        let mut pick = |key: &'static str, env: &Option<String>| -> String {
            match env {
                Some(v) => {
                    locked.push(key);
                    v.clone()
                }
                None => saved.get(key).cloned().unwrap_or_default(),
            }
        };
        let EnvOverrides {
            public_url,
            oidc_issuer,
            oidc_client_id,
            oidc_client_secret,
            allowed_emails,
            google_fonts_api_key,
        } = &config.env;

        let public_url = pick(PUBLIC_URL, public_url);
        let public_url = if public_url.is_empty() {
            format!("http://localhost:{}", config.bind.port())
        } else {
            public_url.trim_end_matches('/').to_owned()
        };
        Self {
            public_url,
            oidc_issuer: pick(OIDC_ISSUER, oidc_issuer),
            oidc_client_id: pick(OIDC_CLIENT_ID, oidc_client_id),
            oidc_client_secret: Some(pick(OIDC_CLIENT_SECRET, oidc_client_secret))
                .filter(|s| !s.is_empty()),
            allowed_emails: parse_emails(&pick(ALLOWED_EMAILS, allowed_emails)),
            google_fonts_api_key: pick(GOOGLE_FONTS_API_KEY, google_fonts_api_key),
            locked,
        }
    }

    pub async fn load(config: &Config, db: &SqlitePool) -> sqlx::Result<Self> {
        let rows: Vec<(String, String)> = sqlx::query_as("SELECT key, value FROM settings")
            .fetch_all(db)
            .await?;
        Ok(Self::merge(config, &rows.into_iter().collect()))
    }

    /// True once sign-in and the catalog can work.
    pub fn is_configured(&self) -> bool {
        !self.oidc_issuer.is_empty()
            && !self.oidc_client_id.is_empty()
            && !self.google_fonts_api_key.is_empty()
    }

    pub fn is_locked(&self, key: &str) -> bool {
        self.locked.contains(&key)
    }

    pub fn redirect_uri(&self) -> String {
        format!("{}/auth/callback", self.public_url)
    }
}

/// Splits a comma- or whitespace-separated list of emails.
pub fn parse_emails(list: &str) -> Vec<String> {
    list.split(|c: char| c == ',' || c.is_whitespace())
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(env: EnvOverrides) -> Config {
        Config {
            bind: "127.0.0.1:8080".parse().unwrap(),
            data_dir: "data".into(),
            database_url: String::new(),
            catalog_ttl_secs: 1,
            max_upload_bytes: 1,
            google_fonts_url: String::new(),
            env,
        }
    }

    #[test]
    fn environment_wins_and_locks() {
        let saved = HashMap::from([
            (OIDC_ISSUER.to_owned(), "https://saved.example".to_owned()),
            (OIDC_CLIENT_ID.to_owned(), "saved-id".to_owned()),
            (PUBLIC_URL.to_owned(), "https://fonts.example/".to_owned()),
        ]);
        let s = Settings::merge(
            &config(EnvOverrides {
                oidc_issuer: Some("https://env.example".into()),
                ..Default::default()
            }),
            &saved,
        );
        assert_eq!(s.oidc_issuer, "https://env.example");
        assert_eq!(s.oidc_client_id, "saved-id");
        assert_eq!(s.public_url, "https://fonts.example");
        assert_eq!(s.redirect_uri(), "https://fonts.example/auth/callback");
        assert!(s.is_locked(OIDC_ISSUER));
        assert!(!s.is_locked(OIDC_CLIENT_ID));
        assert!(!s.is_configured(), "no Google Fonts key yet");
    }

    #[test]
    fn defaults_when_nothing_is_set() {
        let s = Settings::merge(&config(EnvOverrides::default()), &HashMap::new());
        assert_eq!(s.public_url, "http://localhost:8080");
        assert_eq!(s.oidc_client_secret, None);
        assert!(s.locked.is_empty());
        assert!(!s.is_configured());
    }

    #[test]
    fn parses_email_lists() {
        assert_eq!(
            parse_emails(" A@x.com, b@y.com\nc@z.com ,,"),
            ["a@x.com", "b@y.com", "c@z.com"]
        );
        assert!(parse_emails("").is_empty());
    }
}
