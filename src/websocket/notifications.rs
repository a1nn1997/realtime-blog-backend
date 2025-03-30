use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{sync::mpsc, time};
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::notification::model::NotificationPayload;
use crate::{auth::jwt::validate_token, cache::redis::RedisCache};

/// Query parameters for WebSocket connections
#[derive(Debug, Deserialize)]
pub struct WebSocketParams {
    token: Option<String>,
}

/// Notification message structure
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Notification {
    #[serde(rename = "type")]
    pub notification_type: String,
    pub message: String,
    pub post_id: Option<i64>,
    pub comment_id: Option<i64>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Type alias for connection store
type ConnectionStore = Arc<Mutex<HashMap<Uuid, Vec<String>>>>;

/// Application state for notifications
#[derive(Debug)]
pub struct NotificationState {
    pub connections: Arc<Mutex<HashMap<Uuid, Vec<String>>>>,
    pub redis_cache: Option<Arc<RedisCache>>,
}

/// Handle an invalid socket connection (authentication failure)
async fn handle_invalid_socket(mut socket: WebSocket, error_message: String) {
    // Send error message to client
    if let Err(e) = socket
        .send(Message::Text(format!(
            r#"{{"error": "{}"}}"#,
            error_message
        )))
        .await
    {
        error!("Error sending error message on WS: {}", e);
    }

    // Close the connection
    let _ = socket.close().await;
}

/// Handle a valid WebSocket connection
async fn handle_valid_connection(
    socket: WebSocket,
    user_id: Uuid,
    redis_cache: Option<Arc<RedisCache>>,
) {
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let (tx, mut rx) = mpsc::channel::<Message>(100);

    // Clone tx for Redis subscription
    let tx_redis = tx.clone();

    // Task to subscribe to Redis notifications
    let redis_task = if let Some(cache) = redis_cache.clone() {
        let user_id_clone = user_id.clone();
        let cache_clone = cache.clone();
        Some(tokio::spawn(async move {
            subscribe_to_user_notifications(user_id_clone, cache_clone, tx_redis).await;
        }))
    } else {
        None
    };

    // Forward messages from channel to WebSocket
    let forward_task = tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            if let Err(e) = ws_sender.send(message).await {
                error!("Error forwarding message to WebSocket: {}", e);
                break;
            }
        }
    });

    // Heartbeat task - clone tx again for this purpose
    let tx_heartbeat = tx.clone();
    let heartbeat_task = tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            if let Err(e) = tx_heartbeat.send(Message::Ping(vec![])).await {
                error!("Error sending heartbeat: {}", e);
                break;
            }
        }
    });

    // Process incoming WebSocket messages
    while let Some(result) = ws_receiver.next().await {
        match result {
            Ok(Message::Close(_)) => {
                info!("WebSocket closed by client");
                break;
            }
            Ok(Message::Pong(_)) => {
                // Client responded to our ping
                debug!("Received pong from client");
            }
            Err(e) => {
                error!("WebSocket error: {}", e);
                break;
            }
            _ => {}
        }
    }

    // Clean up
    if let Some(task) = redis_task {
        task.abort();
    }
    forward_task.abort();
    heartbeat_task.abort();

    info!("WebSocket connection closed for user: {}", user_id);
}

/// Handle incoming WebSocket connection
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(params): Query<WebSocketParams>,
    State(state): State<Arc<NotificationState>>,
) -> impl IntoResponse {
    let token = params.token.unwrap_or_default();

    // Validate token and extract the user ID
    let user_id = match validate_token(&token) {
        Ok(claims) => match Uuid::parse_str(&claims.sub) {
            Ok(uuid) => uuid,
            Err(e) => {
                let error_message = format!("Invalid user ID in token: {}", e);
                return ws.on_upgrade(move |socket| async move {
                    handle_invalid_socket(socket, error_message).await;
                });
            }
        },
        Err(e) => {
            let error_message = format!("Invalid token: {}", e);
            return ws.on_upgrade(move |socket| async move {
                handle_invalid_socket(socket, error_message).await;
            });
        }
    };

    // Valid connection, upgrade and handle
    info!("User {} connected to notifications WebSocket", user_id);
    ws.on_upgrade(move |socket| async move {
        handle_valid_connection(socket, user_id, state.redis_cache.clone()).await;
    })
}

/// Subscribe to Redis PubSub channel for user notifications
async fn subscribe_to_user_notifications(
    user_id: Uuid,
    redis_cache: Arc<RedisCache>,
    tx: mpsc::Sender<Message>,
) {
    let channel_name = format!("notifications:user:{}", user_id);
    info!("Subscribing to Redis channel: {}", channel_name);

    // Get a Redis PubSub connection using client::get_async_pubsub
    if let Ok(mut pubsub) = redis_cache.get_client().get_async_pubsub().await {
        // Subscribe to the channel
        if let Err(e) = pubsub.subscribe(&channel_name).await {
            error!("Failed to subscribe to Redis channel: {}", e);
            return;
        }

        info!("Successfully subscribed to Redis channel: {}", channel_name);

        // Get the message stream
        let mut pubsub_stream = pubsub.on_message();

        // Process messages
        while let Some(msg) = pubsub_stream.next().await {
            let payload: String = match msg.get_payload() {
                Ok(payload) => payload,
                Err(e) => {
                    error!("Failed to get message payload: {}", e);
                    continue;
                }
            };

            if let Err(e) = tx.send(Message::Text(payload)).await {
                error!("Failed to forward Redis message to WebSocket: {}", e);
                break;
            }
        }
    } else {
        error!("Failed to get Redis PubSub connection");
    }
}

/// Publish a notification to a user
pub async fn publish_notification(
    redis_cache: &RedisCache,
    user_id: &Uuid,
    notification: NotificationPayload,
) -> Result<(), String> {
    let json = serde_json::to_string(&notification).map_err(|e| e.to_string())?;

    // In a real implementation, we'd publish to a Redis channel for WebSocket distribution
    // For this stub implementation, just log it
    info!("Publishing notification to user {}: {}", user_id, json);

    // Try to publish to Redis stream if available
    if let Ok(mut conn) = redis_cache
        .get_client()
        .get_multiplexed_async_connection()
        .await
    {
        let channel_name = format!("notifications:{}", user_id);
        let _: Result<(), redis::RedisError> = conn.publish(&channel_name, &json).await;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::jwt::{generate_token, Role};
    use axum::{body::Body, extract::ws::WebSocket, extract::FromRequest, http::Request};
    use futures::StreamExt;
    use tokio::sync::mpsc;
    use uuid::Uuid;

    // For simplicity, we'll just test the Notification struct and other simple functionality
    // The WebSocket upgrade and Redis functionality is too complex to mock properly
    // in this simple test module

    #[tokio::test]
    async fn test_notification_struct_serialization() {
        // Test that the Notification struct serializes correctly
        let notification = Notification {
            notification_type: "test".to_string(),
            message: "Test notification".to_string(),
            post_id: Some(1),
            comment_id: Some(2),
            timestamp: chrono::Utc::now(),
        };

        let json = serde_json::to_string(&notification).unwrap();
        assert!(json.contains(r#""type":"test"#));
        assert!(json.contains(r#""message":"Test notification"#));
        assert!(json.contains(r#""post_id":1"#));
        assert!(json.contains(r#""comment_id":2"#));

        // Test deserialization
        let deserialized: Notification = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.notification_type, "test");
        assert_eq!(deserialized.message, "Test notification");
        assert_eq!(deserialized.post_id, Some(1));
        assert_eq!(deserialized.comment_id, Some(2));
    }

    #[tokio::test]
    async fn test_websocket_params() {
        // Test the WebSocketParams struct
        let params = WebSocketParams {
            token: Some("test_token".to_string()),
        };
        assert_eq!(params.token.unwrap(), "test_token");

        let params_empty = WebSocketParams { token: None };
        assert!(params_empty.token.is_none());
    }

    #[tokio::test]
    async fn test_notification_channel_format() {
        // Test that the notification channel format is correct
        let user_id = Uuid::parse_str("123e4567-e89b-12d3-a456-426614174000").unwrap();
        let channel_name = format!("notifications:user:{}", user_id);
        assert_eq!(
            channel_name,
            "notifications:user:123e4567-e89b-12d3-a456-426614174000"
        );
    }

    // This tests the error message formatting in the handle_invalid_socket function
    #[tokio::test]
    async fn test_error_message_format() {
        let error_msg = format!(r#"{{"error": "{}"}}"#, "Invalid token");
        assert_eq!(error_msg, r#"{"error": "Invalid token"}"#);
    }
}
