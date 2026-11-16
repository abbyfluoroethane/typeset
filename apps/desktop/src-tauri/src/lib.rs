mod api;
mod auth;
mod error;
mod fonts;
mod state;
mod sync;

use std::str::FromStr;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use tauri::Manager;

use crate::state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let db_url = format!("sqlite://{}", data_dir.join("typeset.db").display());

            let db = tauri::async_runtime::block_on(async {
                let options = SqliteConnectOptions::from_str(&db_url)?
                    .create_if_missing(true)
                    .journal_mode(SqliteJournalMode::Wal);
                let db = SqlitePoolOptions::new().connect_with(options).await?;
                sqlx::migrate!().run(&db).await?;
                Ok::<_, Box<dyn std::error::Error>>(db)
            })?;

            let http = reqwest::Client::builder()
                .user_agent(concat!("Typeset/", env!("CARGO_PKG_VERSION")))
                .build()?;

            app.manage(AppState { db, http });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            auth::login,
            auth::logout,
            auth::current_user,
            sync::get_settings,
            sync::update_settings,
            sync::get_catalog,
            sync::get_local_state,
            sync::sync_now,
            sync::library_put,
            sync::library_delete,
            sync::collection_put,
            sync::collection_delete,
            sync::upload_fonts,
            sync::delete_upload,
            fonts::list_installed,
            fonts::install_family,
            fonts::uninstall_family,
            fonts::reconcile_fonts,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
