use std::{env, net::SocketAddr, path::PathBuf};

/// Server configuration, read from `TYPESET_*` environment variables.
#[derive(Debug, Clone)]
pub struct Config {
    pub bind: SocketAddr,
    /// Externally reachable base URL, e.g. `https://fonts.example.com`.
    /// The OIDC redirect URI is `<public_url>/auth/callback`.
    pub public_url: String,
    pub data_dir: PathBuf,
    pub database_url: String,
    pub oidc_issuer: String,
    pub oidc_client_id: String,
    /// Empty for public clients.
    pub oidc_client_secret: Option<String>,
    /// Lower-cased emails allowed to sign in. Empty allows anyone the
    /// identity provider authenticates.
    pub allowed_emails: Vec<String>,
    pub google_fonts_api_key: String,
    /// How long a cached catalog stays fresh, in seconds.
    pub catalog_ttl_secs: i64,
    pub max_upload_bytes: usize,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let data_dir = PathBuf::from(var_or("TYPESET_DATA_DIR", "data"));
        let database_url = env::var("TYPESET_DATABASE_URL").unwrap_or_else(|_| {
            format!(
                "sqlite://{}?mode=rwc",
                data_dir.join("typeset.db").display()
            )
        });
        let public_url = var_or("TYPESET_PUBLIC_URL", "http://localhost:8080")
            .trim_end_matches('/')
            .to_owned();

        Ok(Self {
            bind: var_or("TYPESET_BIND", "0.0.0.0:8080")
                .parse()
                .map_err(|e| format!("TYPESET_BIND: {e}"))?,
            public_url,
            data_dir,
            database_url,
            oidc_issuer: required("TYPESET_OIDC_ISSUER")?,
            oidc_client_id: required("TYPESET_OIDC_CLIENT_ID")?,
            oidc_client_secret: env::var("TYPESET_OIDC_CLIENT_SECRET")
                .ok()
                .filter(|s| !s.is_empty()),
            allowed_emails: env::var("TYPESET_ALLOWED_EMAILS")
                .unwrap_or_default()
                .split(',')
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect(),
            google_fonts_api_key: required("TYPESET_GOOGLE_FONTS_API_KEY")?,
            catalog_ttl_secs: var_or("TYPESET_CATALOG_TTL_SECS", "21600")
                .parse()
                .map_err(|e| format!("TYPESET_CATALOG_TTL_SECS: {e}"))?,
            max_upload_bytes: var_or("TYPESET_MAX_UPLOAD_BYTES", "33554432")
                .parse()
                .map_err(|e| format!("TYPESET_MAX_UPLOAD_BYTES: {e}"))?,
        })
    }

    pub fn redirect_uri(&self) -> String {
        format!("{}/auth/callback", self.public_url)
    }

    pub fn uploads_dir(&self) -> PathBuf {
        self.data_dir.join("uploads")
    }
}

fn var_or(name: &str, default: &str) -> String {
    env::var(name).unwrap_or_else(|_| default.to_owned())
}

fn required(name: &str) -> Result<String, String> {
    env::var(name)
        .ok()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("{name} must be set"))
}
