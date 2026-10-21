use serde::{Serialize, Serializer};

pub type AppResult<T> = Result<T, AppError>;

/// Error returned from every Tauri command. Serializes to its message so the
/// frontend gets a plain string in the rejected promise.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("no server configured")]
    NoServer,
    #[error("not signed in")]
    NotSignedIn,
    #[error("server returned {status}: {message}")]
    Api { status: u16, message: String },
    #[error("network error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("keychain error: {0}")]
    Keyring(#[from] keyring::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Other(String),
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}
