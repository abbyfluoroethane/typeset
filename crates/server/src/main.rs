mod admin;
mod auth;
mod catalog;
mod config;
mod db;
mod error;
mod settings;
mod state;
mod sync;
mod uploads;

use std::str::FromStr;

use axum::{Router, routing::get};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use crate::{config::Config, settings::Settings, state::AppState};

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
    let settings = Settings::load(&config, &db).await?;

    // Setup stays open until the server is configured and has an admin.
    // Until then, the web page asks for this token.
    let has_admin: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM users WHERE is_admin = 1)")
            .fetch_one(&db)
            .await?;
    let setup_token = (!settings.is_configured() || !has_admin).then(auth::random_token);
    if let Some(token) = &setup_token {
        tracing::warn!(
            "setup is not finished. Open {}/setup?token={token} in a browser, \
             or open the server's address and paste the token.",
            settings.public_url
        );
    }
    let setup_token_hash = setup_token.map(|t| auth::sha256_hex(t.as_bytes()));

    let max_upload_bytes = config.max_upload_bytes;
    let state = AppState::new(db, config, settings, setup_token_hash);

    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .merge(admin::router())
        .merge(auth::router())
        .merge(catalog::router())
        .merge(sync::router())
        .merge(uploads::router(max_upload_bytes))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(bind).await?;
    tracing::info!("listening on {bind}");
    axum::serve(listener, app).await?;
    Ok(())
}
