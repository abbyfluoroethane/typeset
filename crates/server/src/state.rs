use std::sync::{Arc, RwLock};

use sqlx::SqlitePool;

use crate::{config::Config, settings::Settings};

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub config: Arc<Config>,
    settings: Arc<RwLock<Arc<Settings>>>,
    /// SHA-256 of the setup token printed at startup, if one was made.
    pub setup_token_hash: Option<Arc<str>>,
}

impl AppState {
    pub fn new(
        db: SqlitePool,
        config: Config,
        settings: Settings,
        setup_token_hash: Option<String>,
    ) -> Self {
        Self {
            db,
            config: Arc::new(config),
            settings: Arc::new(RwLock::new(Arc::new(settings))),
            setup_token_hash: setup_token_hash.map(Into::into),
        }
    }

    /// The settings in effect now.
    pub fn settings(&self) -> Arc<Settings> {
        self.settings
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn set_settings(&self, settings: Settings) {
        *self.settings.write().unwrap_or_else(|e| e.into_inner()) = Arc::new(settings);
    }
}
