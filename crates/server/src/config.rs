use std::{env, net::SocketAddr, path::PathBuf};

/// Process configuration, read once from `TYPESET_*` environment variables.
///
/// The values in [`EnvOverrides`] can also be saved from the web setup page;
/// see [`crate::settings::Settings`] for how the two are merged.
#[derive(Debug, Clone)]
pub struct Config {
    pub bind: SocketAddr,
    pub data_dir: PathBuf,
    pub database_url: String,
    /// How long a cached catalog stays fresh, in seconds.
    pub catalog_ttl_secs: i64,
    pub max_upload_bytes: usize,
    /// Google Fonts Developer API endpoint. Only tests change this.
    pub google_fonts_url: String,
    pub env: EnvOverrides,
}

/// Settings given in the environment. A value set here wins over the one
/// saved in the database and cannot be changed from the web page.
#[derive(Debug, Clone, Default)]
pub struct EnvOverrides {
    pub public_url: Option<String>,
    pub oidc_issuer: Option<String>,
    pub oidc_client_id: Option<String>,
    pub oidc_client_secret: Option<String>,
    pub allowed_emails: Option<String>,
    pub google_fonts_api_key: Option<String>,
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

        Ok(Self {
            bind: var_or("TYPESET_BIND", "0.0.0.0:8080")
                .parse()
                .map_err(|e| format!("TYPESET_BIND: {e}"))?,
            data_dir,
            database_url,
            catalog_ttl_secs: var_or("TYPESET_CATALOG_TTL_SECS", "21600")
                .parse()
                .map_err(|e| format!("TYPESET_CATALOG_TTL_SECS: {e}"))?,
            max_upload_bytes: var_or("TYPESET_MAX_UPLOAD_BYTES", "33554432")
                .parse()
                .map_err(|e| format!("TYPESET_MAX_UPLOAD_BYTES: {e}"))?,
            google_fonts_url: var_or(
                "TYPESET_GOOGLE_FONTS_URL",
                "https://www.googleapis.com/webfonts/v1/webfonts",
            ),
            env: EnvOverrides {
                public_url: optional("TYPESET_PUBLIC_URL"),
                oidc_issuer: optional("TYPESET_OIDC_ISSUER"),
                oidc_client_id: optional("TYPESET_OIDC_CLIENT_ID"),
                oidc_client_secret: optional("TYPESET_OIDC_CLIENT_SECRET"),
                allowed_emails: optional("TYPESET_ALLOWED_EMAILS"),
                google_fonts_api_key: optional("TYPESET_GOOGLE_FONTS_API_KEY"),
            },
        })
    }

    pub fn uploads_dir(&self) -> PathBuf {
        self.data_dir.join("uploads")
    }
}

fn var_or(name: &str, default: &str) -> String {
    env::var(name).unwrap_or_else(|_| default.to_owned())
}

/// An empty variable counts as unset.
fn optional(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}
