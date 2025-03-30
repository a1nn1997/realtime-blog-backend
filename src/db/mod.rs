pub mod queries;

use sqlx::{PgPool, Row};
use std::fs;
use std::path::Path;
use tracing::{error, info};

/// Initialize the database schema
pub async fn init_db(pool: &PgPool) -> Result<(), sqlx::Error> {
    info!("Initializing database schema...");

    // Read the schema SQL file
    let schema_path = Path::new("src/db/schema.sql");
    let schema_sql = match fs::read_to_string(schema_path) {
        Ok(content) => content,
        Err(e) => {
            error!("Failed to read schema.sql: {}", e);
            return Err(sqlx::Error::Io(e.into()));
        }
    };

    // Execute the SQL script
    match sqlx::query(&schema_sql).execute(pool).await {
        Ok(_) => {
            info!("Database schema initialized successfully");
        }
        Err(e) => {
            error!("Failed to initialize database schema: {}", e);
            return Err(e);
        }
    }

    // Read and execute the analytics schema SQL file
    let analytics_schema_path = Path::new("src/db/analytics_schema.sql");
    if analytics_schema_path.exists() {
        info!("Initializing analytics schema...");
        let analytics_schema_sql = match fs::read_to_string(analytics_schema_path) {
            Ok(content) => content,
            Err(e) => {
                error!("Failed to read analytics_schema.sql: {}", e);
                return Err(sqlx::Error::Io(e.into()));
            }
        };

        // Execute the analytics SQL script
        match sqlx::query(&analytics_schema_sql).execute(pool).await {
            Ok(_) => {
                info!("Analytics schema initialized successfully");
            }
            Err(e) => {
                error!("Failed to initialize analytics schema: {}", e);
                return Err(e);
            }
        }
    } else {
        info!("Analytics schema file not found, skipping");
    }

    Ok(())
}

/// Check if the user table exists
pub async fn check_db_initialized(pool: &PgPool) -> bool {
    let result = sqlx::query(
        "SELECT EXISTS (SELECT FROM information_schema.tables WHERE table_schema = 'global' AND table_name = 'users')",
    )
    .fetch_one(pool)
    .await;

    match result {
        Ok(row) => row.try_get::<bool, _>(0).unwrap_or(false),
        Err(_) => false,
    }
}
