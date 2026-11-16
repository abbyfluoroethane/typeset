use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::SqliteConnection;

/// Current Unix time in seconds.
pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Bumps the server-wide revision counter and returns the new value. Call
/// inside the same transaction as the write it stamps.
pub async fn next_rev(conn: &mut SqliteConnection) -> sqlx::Result<i64> {
    sqlx::query_scalar("UPDATE rev_counter SET value = value + 1 WHERE id = 1 RETURNING value")
        .fetch_one(conn)
        .await
}
