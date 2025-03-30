use chrono;
use redis::{AsyncCommands, Client, RedisError};
use serde_json;
use std::collections::HashMap;
use std::time::Duration;
use tracing::{error, info};
use uuid::Uuid;

// Redis cache key prefixes
pub const POST_KEY_PREFIX: &str = "post";
pub const POPULAR_POSTS_KEY: &str = "popular_posts";
pub const POST_VIEWS_STREAM: &str = "post_views";
const POST_CACHE_TTL_SECONDS: u64 = 3600; // 1 hour
const POPULAR_POSTS_TTL_SECONDS: u64 = 3600; // 1 hour
const POST_STATS_TTL_SECONDS: u64 = 86400; // 24 hours
const USER_ENGAGEMENT_TTL_SECONDS: u64 = 86400; // 24 hours

// Error type for cache operations
#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("Redis error: {0}")]
    RedisError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Deserialization error: {0}")]
    DeserializationError(String),

    #[error("Cache operation failed: {0}")]
    OperationFailed(String),
}

// Post Stats model for cache
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PostStats {
    pub post_id: i64,
    pub views: i64,
    pub likes: i64,
    pub comments: i64,
    pub shares: Option<i64>,
}

// User engagement model for cache
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UserEngagement {
    pub user_id: Uuid,
    pub post_views: i64,
    pub post_likes: i64,
    pub comments: i64,
    pub shares: Option<i64>,
}

// Redis cache configuration
#[derive(Debug, Clone)]
pub struct RedisConfig {
    pub post_stats_ttl: Option<Duration>,
    pub user_engagement_ttl: Option<Duration>,
}

#[derive(Debug, Clone)]
pub struct RedisCache {
    client: Client,
    config: Option<RedisConfig>,
    prefix: Option<String>,
}

impl RedisCache {
    pub fn new(client: Client, config: Option<RedisConfig>) -> Self {
        // Just create the instance without validation
        // Connection validation will happen on first use
        Self {
            client,
            config,
            prefix: None,
        }
    }

    // Get the client
    pub fn get_client(&self) -> &Client {
        &self.client
    }

    // Cache a post by ID
    pub async fn cache_post_by_id(&self, id: i64, json_data: &str) -> Result<(), RedisError> {
        let key = format!("post:id:{}", id);
        self.get_client()
            .get_multiplexed_async_connection()
            .await?
            .set_ex(key, json_data, POST_CACHE_TTL_SECONDS)
            .await
            .map(|_: ()| ())
    }

    // Cache a post by slug
    pub async fn cache_post_by_slug(&self, slug: &str, json_data: &str) -> Result<(), RedisError> {
        let key = format!("post:slug:{}", slug);
        self.get_client()
            .get_multiplexed_async_connection()
            .await?
            .set_ex(key, json_data, POST_CACHE_TTL_SECONDS)
            .await
            .map(|_: ()| ())
    }

    // Get post by ID from cache
    pub async fn get_post_by_id(&self, id: i64) -> Result<Option<String>, RedisError> {
        let mut connection = self.client.get_multiplexed_async_connection().await?;
        let key = format!("{}{}", POST_KEY_PREFIX, id);

        let result: Option<String> = connection.get(key).await?;

        if result.is_some() {
            info!("Cache hit for post ID: {}", id);
        } else {
            info!("Cache miss for post ID: {}", id);
        }

        Ok(result)
    }

    // Get post by slug from cache
    pub async fn get_post_by_slug(&self, slug: &str) -> Result<Option<String>, RedisError> {
        let mut connection = self.client.get_multiplexed_async_connection().await?;
        let key = format!("{}{}", POST_KEY_PREFIX, slug);

        let result: Option<String> = connection.get(key).await?;

        if result.is_some() {
            info!("Cache hit for post slug: {}", slug);
        } else {
            info!("Cache miss for post slug: {}", slug);
        }

        Ok(result)
    }

    // Cache popular posts
    pub async fn cache_popular_posts(&self, json_data: &str) -> Result<(), RedisError> {
        self.get_client()
            .get_multiplexed_async_connection()
            .await?
            .set_ex(POPULAR_POSTS_KEY, json_data, POPULAR_POSTS_TTL_SECONDS)
            .await
            .map(|_: ()| ())
    }

    // Get popular posts from cache
    pub async fn get_popular_posts(&self) -> Result<Option<String>, RedisError> {
        let mut connection = self.client.get_multiplexed_async_connection().await?;

        let result: Option<String> = connection.get(POPULAR_POSTS_KEY).await?;

        if result.is_some() {
            info!("Cache hit for popular posts");
        } else {
            info!("Cache miss for popular posts");
        }

        Ok(result)
    }

    // Invalidate post cache
    pub async fn invalidate_post(&self, id: i64, slug: &str) -> Result<(), RedisError> {
        let mut connection = self.get_client().get_multiplexed_async_connection().await?;

        let id_key = format!("post:id:{}", id);
        let slug_key = format!("post:slug:{}", slug);

        connection.del(&[id_key, slug_key]).await?;
        info!(
            "Invalidated cache for post with ID: {} and slug: {}",
            id, slug
        );
        Ok(())
    }

    // Invalidate popular posts cache
    pub async fn invalidate_popular_posts(&self) -> Result<(), RedisError> {
        self.get_client()
            .get_multiplexed_async_connection()
            .await?
            .del(POPULAR_POSTS_KEY)
            .await
            .map(|_: ()| ())
    }

    // Log a post view
    pub async fn log_post_view(
        &self,
        post_id: i64,
        user_id: Option<Uuid>,
        ip_hash: Option<String>,
    ) -> Result<(), RedisError> {
        let mut connection = self.client.get_multiplexed_async_connection().await?;
        let stream_key = "stream:post_views";

        // Create timestamp
        let timestamp = chrono::Utc::now().timestamp();

        // Prepare fields for the stream entry
        let mut fields = Vec::new();
        fields.push(("post_id", post_id.to_string()));
        fields.push(("timestamp", timestamp.to_string()));

        // Add user ID if available
        let user_value = user_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "anonymous".to_string());
        fields.push(("user", user_value));

        // Add IP hash if available
        if let Some(ip) = ip_hash {
            fields.push(("ip_hash", ip));
        }

        connection.xadd(stream_key, "*", &fields).await?;

        info!("Logged view for post {}", post_id);
        Ok(())
    }

    // Increment post view count
    pub async fn increment_post_views(&self, post_id: i64) -> Result<(), RedisError> {
        let stats_key = format!("stats:post:{}", post_id);
        let mut connection = self.get_client().get_multiplexed_async_connection().await?;

        // Increment the view count in the hash
        connection.hincr(&stats_key, "views", 1).await?;

        // Refresh the TTL
        connection
            .expire(&stats_key, POST_CACHE_TTL_SECONDS as i64)
            .await?;

        info!("Incremented view count for post ID: {}", post_id);
        Ok(())
    }

    // Get post statistics
    pub async fn get_post_stats(&self, post_id: i64) -> Result<Option<PostStats>, CacheError> {
        let mut connection = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| {
                error!("Redis connection error while getting post stats: {}", e);
                CacheError::RedisError(e.to_string())
            })?;

        let cache_key = format!("post_stats:{}", post_id);
        let result: Option<String> = connection.get(&cache_key).await.map_err(|e| {
            error!("Redis error while getting post stats: {}", e);
            CacheError::RedisError(e.to_string())
        })?;

        match result {
            Some(data) => {
                let post_stats: PostStats = serde_json::from_str(&data).map_err(|e| {
                    error!("Failed to deserialize post stats: {}", e);
                    CacheError::DeserializationError(e.to_string())
                })?;
                Ok(Some(post_stats))
            }
            None => Ok(None),
        }
    }

    // Set post stats
    pub async fn set_post_stats(&self, post_id: i64, stats: &PostStats) -> Result<(), RedisError> {
        let stats_key = format!("stats:post:{}", post_id);
        let mut connection = self.get_client().get_multiplexed_async_connection().await?;

        // Convert PostStats to HashMap with safe conversions for Option types
        let mut fields = HashMap::new();
        fields.insert("views".to_string(), stats.views.to_string());
        fields.insert("likes".to_string(), stats.likes.to_string());
        fields.insert("comments".to_string(), stats.comments.to_string());

        // Handle Option<i64> safely by providing a default of 0
        let shares_str = stats.shares.unwrap_or(0).to_string();
        fields.insert("shares".to_string(), shares_str);

        // Set all fields in the hash as individual commands
        for (field, value) in &fields {
            connection.hset(&stats_key, field, value).await?;
        }

        // Set expiry
        connection
            .expire(&stats_key, POST_STATS_TTL_SECONDS as i64)
            .await?;

        info!("Cached stats for post ID: {}", post_id);
        Ok(())
    }

    // Invalidate post stats
    pub async fn invalidate_post_stats(&self, post_id: i64) -> Result<(), RedisError> {
        let stats_key = format!("stats:post:{}", post_id);
        self.get_client()
            .get_multiplexed_async_connection()
            .await?
            .del(stats_key)
            .await
            .map(|_: ()| ())
    }

    // Get user engagement
    pub async fn get_user_engagement(
        &self,
        user_id: Uuid,
    ) -> Result<Option<UserEngagement>, CacheError> {
        let mut connection = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| {
                error!(
                    "Redis connection error while getting user engagement: {}",
                    e
                );
                CacheError::RedisError(e.to_string())
            })?;

        let cache_key = format!("user_engagement:{}", user_id);
        let result: Option<String> = connection.get(&cache_key).await.map_err(|e| {
            error!("Redis error while getting user engagement: {}", e);
            CacheError::RedisError(e.to_string())
        })?;

        match result {
            Some(data) => {
                let user_engagement: UserEngagement = serde_json::from_str(&data).map_err(|e| {
                    error!("Failed to deserialize user engagement: {}", e);
                    CacheError::DeserializationError(e.to_string())
                })?;
                Ok(Some(user_engagement))
            }
            None => Ok(None),
        }
    }

    // Set user engagement
    pub async fn set_user_engagement(
        &self,
        user_id: Uuid,
        post_id: i64,
        engagement: &UserEngagement,
    ) -> Result<(), RedisError> {
        let engagement_key = format!("engagement:user:{}:post:{}", user_id, post_id);
        let mut connection = self.get_client().get_multiplexed_async_connection().await?;

        // Convert UserEngagement to HashMap with safe conversions for Option types
        let mut fields = HashMap::new();
        fields.insert("post_views".to_string(), engagement.post_views.to_string());
        fields.insert("post_likes".to_string(), engagement.post_likes.to_string());
        fields.insert("comments".to_string(), engagement.comments.to_string());

        // Handle Option<i64> safely by providing a default of 0
        let shares_str = engagement.shares.unwrap_or(0).to_string();
        fields.insert("shares".to_string(), shares_str);

        // Set all fields in the hash as individual commands
        for (field, value) in &fields {
            connection.hset(&engagement_key, field, value).await?;
        }

        // Set expiry
        connection
            .expire(&engagement_key, USER_ENGAGEMENT_TTL_SECONDS as i64)
            .await?;

        info!(
            "Cached engagement for user ID: {} and post ID: {}",
            user_id, post_id
        );
        Ok(())
    }

    // Invalidate user engagement
    pub async fn invalidate_user_engagement(
        &self,
        user_id: Uuid,
        post_id: i64,
    ) -> Result<(), RedisError> {
        let engagement_key = format!("engagement:user:{}:post:{}", user_id, post_id);
        self.get_client()
            .get_multiplexed_async_connection()
            .await?
            .del(engagement_key)
            .await
            .map(|_: ()| ())
    }
}
