use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::error::AppResult;

pub struct AppState {
    pub db: SqlitePool,
    pub http: reqwest::Client,
}

/// User-editable settings, stored under the `settings` key `app`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// Base URL of the Typeset server, e.g. `https://fonts.example.com`.
    pub server_url: Option<String>,
    /// Install and remove fonts automatically to match the synced library.
    pub auto_install: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            server_url: None,
            auto_install: true,
        }
    }
}

impl AppState {
    pub async fn settings(&self) -> AppResult<Settings> {
        Ok(self.get_setting("app").await?.unwrap_or_default())
    }

    pub async fn save_settings(&self, settings: &Settings) -> AppResult<()> {
        self.set_setting("app", settings).await
    }

    pub async fn get_setting<T: serde::de::DeserializeOwned>(
        &self,
        key: &str,
    ) -> AppResult<Option<T>> {
        let row: Option<String> = sqlx::query_scalar("SELECT value FROM settings WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.db)
            .await?;
        Ok(row.map(|v| serde_json::from_str(&v)).transpose()?)
    }

    pub async fn set_setting<T: Serialize>(&self, key: &str, value: &T) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO settings (key, value) VALUES (?, ?)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        )
        .bind(key)
        .bind(serde_json::to_string(value)?)
        .execute(&self.db)
        .await?;
        Ok(())
    }
}

/// Current Unix time in seconds.
pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
