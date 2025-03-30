mod analytics;
mod api_doc;
mod auth;
mod cache;
mod comment;
mod db;
mod notification;
mod post;
mod recommendations;
mod routes;
mod schema_ext;
mod websocket;

use axum::{routing::get, Router};
use dotenv::dotenv;
use redis::Client;
use sqlx::postgres::PgPoolOptions;
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tracing::{error, info};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

// Import modules directly instead of using the crate name
use crate::analytics::service::AnalyticsService;
use crate::api_doc::ApiDoc;
use crate::cache::redis::RedisCache;
use crate::notification::service::NotificationService;
use crate::post::service::PostService;
use crate::websocket::notifications::NotificationState;

// Simple app config struct
#[derive(Debug, Clone)]
struct AppConfig {
    redis_url: Option<String>,
    // Add other config options as needed
}

// This handler is no longer used since we use SwaggerUi::new instead
// async fn api_docs_handler() -> impl axum::response::IntoResponse {
//     axum::response::Json(ApiDoc::openapi())
// }

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logger
    tracing_subscriber::fmt::init();

    // Load .env file if it exists
    dotenv().ok();

    // Create connection pool
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&std::env::var("DATABASE_URL").unwrap())
        .await?;

    // Check if the database is initialized
    if !db::check_db_initialized(&pool).await {
        db::init_db(&pool).await?;
    }

    // Create a simple app config
    let app_config = AppConfig {
        redis_url: std::env::var("REDIS_URL").ok(),
    };

    // Initialize Redis cache if configured
    let redis_cache = if let Some(url) = &app_config.redis_url {
        info!("Initializing Redis cache with URL: {}", url);
        match Client::open(url.clone()) {
            Ok(client) => {
                let cache = RedisCache::new(client, None);
                Some(Arc::new(cache))
            }
            Err(e) => {
                error!("Failed to connect to Redis: {}", e);
                None
            }
        }
    } else {
        info!("No Redis URL configured, proceeding without cache");
        None
    };

    // Create service instances with unwrapped redis_cache
    let redis_cache_for_services = redis_cache.as_ref().map(|arc| (**arc).clone());

    let analytics_service = Arc::new(AnalyticsService::new(
        pool.clone(),
        redis_cache_for_services.clone(),
    ));
    let post_service = Arc::new(PostService::new(
        pool.clone(),
        redis_cache_for_services.clone(),
    ));
    let notification_service = Arc::new(NotificationService::new(
        pool.clone(),
        redis_cache_for_services.clone(),
    ));

    // Initialize comment service with required dependencies
    let comment_service = Arc::new(comment::service::CommentService::new(
        pool.clone(),
        redis_cache_for_services.clone(),
        analytics_service.clone(),
        notification_service.clone(),
    ));

    // Configure notification routes with NotificationState
    let notification_state = Arc::new(NotificationState {
        connections: Arc::new(Mutex::new(HashMap::new())),
        redis_cache: redis_cache.clone(),
    });

    // Build the router
    let app = Router::new()
        // API documentation
        .merge(SwaggerUi::new("/docs").url("/api-docs/openapi.json", ApiDoc::openapi()))
        // Health routes
        .merge(routes::health::routes(pool.clone()))
        // Auth routes
        .merge(routes::auth::routes(pool.clone()))
        // Add post routes
        .merge(routes::posts::routes(
            pool.clone(),
            redis_cache_for_services.clone(),
        ))
        // Analytics routes
        .merge(routes::analytics::routes(
            pool.clone(),
            redis_cache_for_services.clone(),
        ))
        // Add recommendations routes
        .merge(routes::recommendations::routes(
            pool.clone(),
            redis_cache_for_services.clone(),
        ))
        // Add comment routes
        .merge(routes::comments::routes(comment_service.clone()))
        // Add welcome route
        .route(
            "/",
            get(|| async { "Welcome to Realtime Blog Backend API" }),
        );

    // Try different ports
    let mut port = 9500;
    let max_tries = 5;
    for attempt in 1..=max_tries {
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        match axum::Server::try_bind(&addr) {
            Ok(server) => {
                println!(
                    "🚀 Server started successfully at http://localhost:{}",
                    port
                );
                println!("📄 API Documentation: http://localhost:{}/docs", port);
                println!("🔌 WebSocket Notifications API: ws://localhost:{}/api/notifications/ws?token=<JWT>", port);
                println!("📊 Analytics API: http://localhost:{}/api/analytics", port);
                println!(
                    "🧠 Recommendations API: http://localhost:{}/api/recommendations",
                    port
                );
                return server
                    .serve(app.into_make_service())
                    .await
                    .map_err(|e| e.into());
            }
            Err(_) => {
                if attempt == max_tries {
                    return Err("Failed to bind to any port".into());
                }
                port += 1;
            }
        }
    }

    Err("Failed to bind to any port".into())
}
