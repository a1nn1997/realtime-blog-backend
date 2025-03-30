use axum::{routing::get, Router};
use std::sync::Arc;

use crate::websocket::notifications::{ws_handler, NotificationState};

/// Create a router for notifications
pub fn routes(notification_state: Arc<NotificationState>) -> Router {
    Router::new()
        .route("/api/notifications/ws", get(ws_handler))
        .with_state(notification_state)
}

/// Configure notification routes
pub fn notification_routes(notification_state: Arc<NotificationState>) -> Router {
    Router::new()
        .route("/ws", get(ws_handler))
        .with_state(notification_state)
}
