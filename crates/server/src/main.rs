mod auth;
mod catalog;
mod config;
mod db;
mod error;
mod state;
mod sync;
mod uploads;

use std::{str::FromStr, sync::Arc};

use axum::{Router, routing::get};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use crate::{config::Config, state::AppState};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,tower_http=info".into()),
        )
        .init();

    if let Err(e) = run().await {
        tracing::error!("{e}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    std::fs::create_dir_all(&config.data_dir)?;
    std::fs::create_dir_all(config.uploads_dir())?;

    let options = SqliteConnectOptions::from_str(&config.database_url)?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true);
    let db = SqlitePoolOptions::new().connect_with(options).await?;
    sqlx::migrate!().run(&db).await?;

    let bind = config.bind;
    let state = AppState {
        db,
        config: Arc::new(config),
    };

    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .merge(auth::router())
        .merge(catalog::router())
        .merge(sync::router())
        .merge(uploads::router(&state.config))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(bind).await?;
    tracing::info!("listening on {bind}");
    axum::serve(listener, app).await?;
    Ok(())
}
