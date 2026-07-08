use crate::config::DatabaseConfig;
use crate::error::AppResult;
use sqlx::{
    Pool, Sqlite,
    migrate::MigrateDatabase,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use std::str::FromStr;

pub async fn init_pool(config: &DatabaseConfig) -> AppResult<Pool<Sqlite>> {
    if !Sqlite::database_exists(&config.url).await.unwrap_or(false) {
        Sqlite::create_database(&config.url).await?;
    }

    let options = SqliteConnectOptions::from_str(&config.url)?
        .create_if_missing(true)
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(config.max_connections)
        .connect_with(options)
        .await?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .map_err(|e| crate::error::AppError::Internal(anyhow::anyhow!(e)))?;

    Ok(pool)
}
