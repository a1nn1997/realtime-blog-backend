use crate::analytics::model::InteractionType;
use crate::analytics::service::AnalyticsService;
use crate::cache::redis::RedisCache;
use crate::comment::model::{
    Comment, CommentAuthor, CommentError, CommentResponse, CreateCommentRequest,
};
use crate::notification::model::{NotificationPayload, NotificationType};
use crate::notification::service::NotificationService;
use crate::websocket::notifications::publish_notification;
use chrono::Utc;
use redis::AsyncCommands;
use sqlx::{PgPool, Row};
use std::sync::Arc;
use tracing::{error, info, warn};
use uuid::Uuid;

// Constants
const MAX_NESTING_DEPTH: i32 = 3;
const COMMENTS_PER_PAGE: i64 = 20;
const COMMENT_RATE_LIMIT_SECONDS: u64 = 100;

#[derive(Clone)]
pub struct CommentService {
    pool: PgPool,
    redis_cache: Option<RedisCache>,
    analytics_service: Arc<AnalyticsService>,
    notification_service: Arc<NotificationService>,
}

impl CommentService {
    pub fn new(
        pool: PgPool,
        redis_cache: Option<RedisCache>,
        analytics_service: Arc<AnalyticsService>,
        notification_service: Arc<NotificationService>,
    ) -> Self {
        Self {
            pool,
            redis_cache,
            analytics_service,
            notification_service,
        }
    }

    // Helper function to sanitize and render markdown
    fn process_markdown(
        &self,
        content: &str,
        markdown_enabled: bool,
    ) -> Result<String, CommentError> {
        if !markdown_enabled {
            // If markdown is disabled, just escape HTML characters
            return Ok(html_escape::encode_safe(content).to_string());
        }

        // In a real implementation, we would sanitize and convert markdown to HTML
        // For this example, we're just returning the content with a simple formatting
        Ok(format!("<div class=\"markdown\">{}</div>", content))
    }

    // Check if user can add a comment (rate limiting)
    async fn check_rate_limit(&self, user_id: &Uuid) -> Result<bool, CommentError> {
        if let Some(cache) = &self.redis_cache {
            let rate_limit_key = format!("rate_limit:comment:{}", user_id);

            // Check if rate limit key exists
            let exists: bool = cache
                .get_client()
                .get_multiplexed_async_connection()
                .await
                .map_err(CommentError::CacheError)?
                .exists(&rate_limit_key)
                .await
                .map_err(CommentError::CacheError)?;

            if exists {
                return Ok(true);
            }

            // Set rate limit key with expiration
            cache
                .get_client()
                .get_multiplexed_async_connection()
                .await
                .map_err(CommentError::CacheError)?
                .set_ex(&rate_limit_key, "1", COMMENT_RATE_LIMIT_SECONDS)
                .await
                .map_err(CommentError::CacheError)?;
        }

        Ok(false)
    }

    // Get the nesting level of a comment
    async fn get_parent_nesting_level(&self, parent_id: i64) -> Result<i32, CommentError> {
        let result = sqlx::query("SELECT nesting_level FROM global.comments WHERE id = $1")
            .bind(parent_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(CommentError::DatabaseError)?;

        match result {
            Some(row) => Ok(row.get::<i32, _>("nesting_level")),
            None => Err(CommentError::ParentCommentNotFound),
        }
    }

    // Create a new comment
    pub async fn create_comment(
        &self,
        post_id: i64,
        user_id: Uuid,
        comment_data: CreateCommentRequest,
    ) -> Result<CommentResponse, CommentError> {
        // Check rate limit
        if !self.check_rate_limit(&user_id).await? {
            return Err(CommentError::RateLimitExceeded);
        }

        // Check if post exists
        let post_exists = sqlx::query(
            "SELECT EXISTS(SELECT 1 FROM global.posts WHERE id = $1 AND is_deleted = false)",
        )
        .bind(post_id)
        .fetch_one(&self.pool)
        .await
        .map_err(CommentError::DatabaseError)?
        .get::<bool, _>(0);

        if !post_exists {
            return Err(CommentError::PostNotFound);
        }

        // Get parent comment author if this is a reply
        let parent_author_id = if let Some(parent_id) = comment_data.parent_comment_id {
            let result = sqlx::query("SELECT user_id FROM global.comments WHERE id = $1")
                .bind(parent_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(CommentError::DatabaseError)?;

            match result {
                Some(row) => Some(row.get::<Uuid, _>("user_id")),
                None => return Err(CommentError::ParentCommentNotFound),
            }
        } else {
            None
        };

        // Calculate nesting level and validate max depth
        let nesting_level = if let Some(parent_id) = comment_data.parent_comment_id {
            let parent_level = self.get_parent_nesting_level(parent_id).await?;
            let new_level = parent_level + 1;

            if new_level > MAX_NESTING_DEPTH {
                return Err(CommentError::MaxNestingDepthReached);
            }

            new_level
        } else {
            0 // Root level comment
        };

        // Process markdown content
        let content_html =
            self.process_markdown(&comment_data.content, comment_data.markdown_enabled)?;

        // Start transaction
        let mut tx = self.pool.begin().await.map_err(|e| {
            error!("Failed to begin transaction: {}", e);
            CommentError::DatabaseError(e)
        })?;

        // Insert comment
        let comment_result = sqlx::query_as::<_, Comment>(
            r#"
            INSERT INTO global.comments (
                post_id, user_id, parent_comment_id, content, content_html, 
                is_deleted, markdown_enabled, nesting_level, created_at, updated_at
            ) 
            VALUES ($1, $2, $3, $4, $5, false, $6, $7, $8, $8)
            RETURNING *
            "#,
        )
        .bind(post_id)
        .bind(user_id)
        .bind(comment_data.parent_comment_id)
        .bind(&comment_data.content)
        .bind(&content_html)
        .bind(comment_data.markdown_enabled)
        .bind(nesting_level)
        .bind(Utc::now())
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| {
            error!("Failed to insert comment: {}", e);
            CommentError::DatabaseError(e)
        })?;

        // Commit transaction
        tx.commit().await.map_err(|e| {
            error!("Failed to commit transaction: {}", e);
            CommentError::DatabaseError(e)
        })?;

        // Get author info for response
        let author = sqlx::query_as::<_, CommentAuthor>(
            r#"
            SELECT id, username as name FROM global.users
            WHERE id = $1
            "#,
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await
        .map_err(CommentError::DatabaseError)?;

        // Send notification if this is a reply and parent author is not the same as current user
        if let Some(parent_author) = parent_author_id {
            if parent_author != user_id {
                // Send notification asynchronously - don't block the response
                let comment_clone = comment_result.clone();
                let self_clone = self.clone();
                tokio::spawn(async move {
                    if let Err(e) = self_clone
                        .send_reply_notification(&comment_clone, &parent_author)
                        .await
                    {
                        error!("Failed to send notification: {:?}", e);
                    }
                });
            }
        }

        // If the comment was for a post, invalidate that post's comment cache
        if let Some(cache) = &self.redis_cache {
            let cache_key = format!("comments:post:{}", post_id);

            // Delete the comments cache
            let _ = cache
                .get_client()
                .get_multiplexed_async_connection()
                .await
                .map_err(CommentError::CacheError)?
                .del(&cache_key)
                .await
                .map_err(CommentError::CacheError)?;

            // Increment comment count in cache if exists
            let count_key = format!("post:comment_count:{}", post_id);
            let _ = cache
                .get_client()
                .get_multiplexed_async_connection()
                .await
                .map_err(CommentError::CacheError)?
                .incr(&count_key, 1)
                .await
                .map_err(CommentError::CacheError)?;

            // Publish realtime event via Redis
            if let Ok(mut conn) = cache.get_client().get_multiplexed_async_connection().await {
                let _: Result<String, redis::RedisError> = conn
                    .xadd(
                        "stream:comments",
                        "*",
                        &[
                            ("event", "comment_created"),
                            ("post_id", &post_id.to_string()),
                            ("comment_id", &comment_result.id.to_string()),
                            (
                                "parent_id",
                                &comment_data
                                    .parent_comment_id
                                    .map(|id| id.to_string())
                                    .unwrap_or_else(|| "null".to_string()),
                            ),
                        ],
                    )
                    .await;
            }
        }

        // Construct response
        let comment_response = CommentResponse {
            id: comment_result.id,
            content_html: content_html,
            author,
            created_at: comment_result.created_at,
            parent_comment_id: comment_result.parent_comment_id,
            replies: None, // New comment has no replies
        };

        info!(
            "Created comment with ID: {} for post: {}",
            comment_result.id, post_id
        );
        Ok(comment_response)
    }

    // Get comments for a post (with threading)
    pub async fn get_post_comments(
        &self,
        post_id: i64,
        page: Option<i64>,
        with_cache: bool,
    ) -> Result<Vec<CommentResponse>, CommentError> {
        let page = page.unwrap_or(1);
        let offset = (page - 1) * COMMENTS_PER_PAGE;

        if with_cache && self.redis_cache.is_some() {
            let cache_key = format!("comments:post:{}", post_id);

            // Try to get from cache first
            let cache_result = self
                .redis_cache
                .as_ref()
                .unwrap()
                .get_client()
                .get_multiplexed_async_connection()
                .await
                .map_err(|e| {
                    error!("Error accessing cache: {}", e);
                    CommentError::CacheError(e)
                })?
                .get::<_, Option<String>>(&cache_key)
                .await;

            // If we have a cached result, use it
            if let Ok(Some(cached_data)) = cache_result {
                return serde_json::from_str::<Vec<CommentResponse>>(&cached_data).map_err(|e| {
                    error!("Error deserializing cached data: {}", e);
                    CommentError::DeserializationError
                });
            }
        }

        // Get all comments for the post (limited to root comments + pagination)
        let root_comments = sqlx::query_as::<_, Comment>(
            r#"
            SELECT * FROM global.comments
            WHERE post_id = $1 AND parent_comment_id IS NULL AND is_deleted = false
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(post_id)
        .bind(COMMENTS_PER_PAGE)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(CommentError::DatabaseError)?;

        let mut comment_responses = Vec::new();

        // Process each root comment
        for comment in root_comments {
            let replies = self.get_comment_replies(comment.id).await?;

            // Get author info
            let author = sqlx::query_as::<_, CommentAuthor>(
                r#"
                SELECT id, username as name FROM global.users
                WHERE id = $1
                "#,
            )
            .bind(comment.user_id)
            .fetch_one(&self.pool)
            .await
            .map_err(CommentError::DatabaseError)?;

            // Build response
            let comment_response = CommentResponse {
                id: comment.id,
                content_html: comment.content_html,
                author,
                created_at: comment.created_at,
                parent_comment_id: None,
                replies: Some(replies),
            };

            comment_responses.push(comment_response);
        }

        // Cache the results if a cache client is available
        if let Some(cache) = &self.redis_cache {
            let cache_key = format!("comments:post:{}", post_id);
            let json_data = serde_json::to_string(&comment_responses).unwrap_or_default();
            let _ = cache
                .get_client()
                .get_multiplexed_async_connection()
                .await
                .map_err(CommentError::CacheError)?
                .set_ex(&cache_key, &json_data, 3600) // 1 hour cache
                .await
                .map_err(CommentError::CacheError)?;
        }

        info!(
            "Retrieved {} comments for post {}",
            comment_responses.len(),
            post_id
        );
        Ok(comment_responses)
    }

    // Get replies for a specific comment (non-recursive implementation to avoid infinite futures)
    async fn get_comment_replies(
        &self,
        comment_id: i64,
    ) -> Result<Vec<CommentResponse>, CommentError> {
        // Get all direct replies to this comment
        let comment_replies = sqlx::query(
            r#"
            SELECT c.*, u.username as author_name, u.id as author_id
            FROM global.comments c
            JOIN global.users u ON c.user_id = u.id
            WHERE c.parent_comment_id = $1 AND c.is_deleted = false
            ORDER BY c.created_at ASC
            "#,
        )
        .bind(comment_id)
        .fetch_all(&self.pool)
        .await
        .map_err(CommentError::DatabaseError)?;

        let mut replies = Vec::with_capacity(comment_replies.len());

        // Process each reply
        for row in comment_replies {
            let reply_id: i64 = row.get("id");
            let created_at: chrono::DateTime<chrono::Utc> = row.get("created_at");
            let parent_comment_id: Option<i64> = row.get("parent_comment_id");
            let content_html: String = row.get("content_html");
            let author_id: uuid::Uuid = row.get("author_id");
            let author_name: String = row.get("author_name");

            // We'll use a non-recursive approach for nested replies
            // by fetching them explicitly for each level
            let nested_replies = if row.get::<i32, _>("nesting_level") < MAX_NESTING_DEPTH {
                // Get 2nd level replies using a separate query
                let second_level_replies = sqlx::query(
                    r#"
                    SELECT c.*, u.username as author_name, u.id as author_id
                    FROM global.comments c
                    JOIN global.users u ON c.user_id = u.id
                    WHERE c.parent_comment_id = $1 AND c.is_deleted = false
                    ORDER BY c.created_at ASC
                    "#,
                )
                .bind(reply_id)
                .fetch_all(&self.pool)
                .await
                .map_err(CommentError::DatabaseError)?;

                // Only process if we have replies
                if !second_level_replies.is_empty() {
                    let mut level2_replies = Vec::with_capacity(second_level_replies.len());

                    for l2_row in second_level_replies {
                        let l2_reply_id: i64 = l2_row.get("id");
                        let l2_created_at: chrono::DateTime<chrono::Utc> = l2_row.get("created_at");
                        let l2_parent_comment_id: Option<i64> = l2_row.get("parent_comment_id");
                        let l2_content_html: String = l2_row.get("content_html");
                        let l2_author_id: uuid::Uuid = l2_row.get("author_id");
                        let l2_author_name: String = l2_row.get("author_name");

                        // Check for 3rd level of nesting (final level)
                        let l3_replies =
                            if l2_row.get::<i32, _>("nesting_level") < MAX_NESTING_DEPTH {
                                let third_level_replies = sqlx::query(
                                    r#"
                                SELECT c.*, u.username as author_name, u.id as author_id
                                FROM global.comments c
                                JOIN global.users u ON c.user_id = u.id
                                WHERE c.parent_comment_id = $1 AND c.is_deleted = false
                                ORDER BY c.created_at ASC
                                "#,
                                )
                                .bind(l2_reply_id)
                                .fetch_all(&self.pool)
                                .await
                                .map_err(CommentError::DatabaseError)?;

                                if !third_level_replies.is_empty() {
                                    let mut l3_replies_vec =
                                        Vec::with_capacity(third_level_replies.len());

                                    for l3_row in third_level_replies {
                                        let l3_reply = CommentResponse {
                                            id: l3_row.get("id"),
                                            content_html: l3_row.get("content_html"),
                                            author: CommentAuthor {
                                                id: l3_row.get("author_id"),
                                                name: l3_row.get("author_name"),
                                            },
                                            created_at: l3_row.get("created_at"),
                                            parent_comment_id: l3_row.get("parent_comment_id"),
                                            replies: None, // No more nesting
                                        };
                                        l3_replies_vec.push(l3_reply);
                                    }

                                    Some(l3_replies_vec)
                                } else {
                                    None
                                }
                            } else {
                                None
                            };

                        // Add level 2 reply
                        let l2_reply = CommentResponse {
                            id: l2_reply_id,
                            content_html: l2_content_html,
                            author: CommentAuthor {
                                id: l2_author_id,
                                name: l2_author_name,
                            },
                            created_at: l2_created_at,
                            parent_comment_id: l2_parent_comment_id,
                            replies: l3_replies,
                        };

                        level2_replies.push(l2_reply);
                    }

                    Some(level2_replies)
                } else {
                    None
                }
            } else {
                None
            };

            // Add main reply
            let reply = CommentResponse {
                id: reply_id,
                content_html,
                author: CommentAuthor {
                    id: author_id,
                    name: author_name,
                },
                created_at,
                parent_comment_id,
                replies: nested_replies,
            };

            replies.push(reply);
        }

        Ok(replies)
    }

    // Delete a comment (soft delete)
    pub async fn delete_comment(
        &self,
        comment_id: i64,
        user_id: Uuid,
        is_admin: bool,
    ) -> Result<i64, CommentError> {
        // Get the comment
        let comment = sqlx::query_as::<_, Comment>(
            r#"
            SELECT * FROM global.comments
            WHERE id = $1 AND is_deleted = false
            "#,
        )
        .bind(comment_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(CommentError::DatabaseError)?
        .ok_or(CommentError::NotFound)?;

        // Check ownership
        if comment.user_id != user_id && !is_admin {
            return Err(CommentError::Unauthorized);
        }

        // Soft delete the comment
        sqlx::query(
            r#"
            UPDATE global.comments
            SET 
                is_deleted = true, 
                content = '[deleted]',
                content_html = '<p>[deleted]</p>',
                deleted_by = $1,
                deleted_at = $2,
                updated_at = $2
            WHERE id = $3
            "#,
        )
        .bind(user_id)
        .bind(Utc::now())
        .bind(comment_id)
        .execute(&self.pool)
        .await
        .map_err(CommentError::DatabaseError)?;

        // Invalidate caches
        if let Some(cache) = &self.redis_cache {
            // Invalidate post comments cache
            let cache_key = format!("comments:post:{}", comment.post_id);
            let _ = cache
                .get_client()
                .get_multiplexed_async_connection()
                .await
                .map_err(CommentError::CacheError)?
                .del(&cache_key)
                .await
                .map_err(CommentError::CacheError)?;

            // Update comment count in cache
            let count_key = format!("post:comment_count:{}", comment.post_id);
            let _ = cache
                .get_client()
                .get_multiplexed_async_connection()
                .await
                .map_err(CommentError::CacheError)?
                .decr(&count_key, 1)
                .await
                .map_err(CommentError::CacheError)?;

            // Push to comment events stream
            if let Ok(mut conn) = cache.get_client().get_multiplexed_async_connection().await {
                let _: Result<String, redis::RedisError> = conn
                    .xadd(
                        "stream:comments",
                        "*",
                        &[
                            ("event", "comment_deleted"),
                            ("post_id", &comment.post_id.to_string()),
                            ("comment_id", &comment_id.to_string()),
                        ],
                    )
                    .await;
            }
        }

        info!("Comment {} deleted by user {}", comment_id, user_id);
        Ok(comment_id)
    }

    // Get comment count for a post (cached)
    pub async fn get_comment_count(&self, post_id: i64) -> Result<i64, CommentError> {
        // Try to get from cache first
        if let Some(cache) = &self.redis_cache {
            let count_key = format!("post:comment_count:{}", post_id);

            if let Ok(cached_count) = cache
                .get_client()
                .get_multiplexed_async_connection()
                .await
                .map_err(CommentError::CacheError)?
                .get::<_, Option<i64>>(&count_key)
                .await
            {
                if let Some(count) = cached_count {
                    return Ok(count);
                }
            }
        }

        // Cache miss, get from DB
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM global.comments WHERE post_id = $1 AND is_deleted = false",
        )
        .bind(post_id)
        .fetch_one(&self.pool)
        .await
        .map_err(CommentError::DatabaseError)?;

        // Update cache
        if let Some(cache) = &self.redis_cache {
            let count_key = format!("post:comment_count:{}", post_id);
            let _ = cache
                .get_client()
                .get_multiplexed_async_connection()
                .await
                .map_err(CommentError::CacheError)?
                .set_ex(&count_key, count.to_string(), 3600)
                .await
                .map_err(CommentError::CacheError)?;
        }

        Ok(count)
    }

    // Helper function to send a notification for a new comment reply
    async fn send_reply_notification(
        &self,
        comment: &Comment,
        reply_to_user_id: &Uuid,
    ) -> Result<(), CommentError> {
        if let Some(redis_cache) = &self.redis_cache {
            let notification = NotificationPayload {
                recipient_id: *reply_to_user_id,
                notification_type: NotificationType::CommentReply,
                object_id: comment.id,
                related_object_id: Some(comment.post_id),
                actor_id: comment.user_id,
                content: format!("You have a new reply to your comment."),
            };

            // Publish notification
            if let Err(e) = publish_notification(redis_cache, reply_to_user_id, notification).await
            {
                error!("Failed to publish notification: {}", e);
                // Don't fail the whole operation if notification fails
            }
        }

        Ok(())
    }

    async fn send_comment_notifications(
        &self,
        comment: &Comment,
        _tx: &sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> Result<(), CommentError> {
        // Get post author (simplified)
        let post_author = match sqlx::query_scalar!(
            "SELECT user_id FROM global.posts WHERE id = $1",
            comment.post_id
        )
        .fetch_optional(&self.pool)
        .await
        {
            Ok(Some(author_id)) => Some(author_id),
            Ok(None) => {
                warn!(
                    "Post not found when sending notifications: {}",
                    comment.post_id
                );
                None
            }
            Err(e) => {
                error!("Error fetching post author: {}", e);
                return Err(CommentError::DatabaseError(e));
            }
        };

        // Only send notification if post author exists and is not the commenter
        if let Some(author_id) = post_author {
            if author_id != comment.user_id {
                let notification = NotificationPayload {
                    recipient_id: author_id,
                    notification_type: NotificationType::NewComment,
                    object_id: comment.id,
                    related_object_id: Some(comment.post_id),
                    actor_id: comment.user_id,
                    content: format!("New comment on your post"),
                };

                match self
                    .notification_service
                    .create_notification(notification)
                    .await
                {
                    Ok(_) => info!("Notification sent successfully"),
                    Err(e) => warn!("Failed to send notification to post author: {}", e),
                }
            }
        }

        Ok(())
    }
}
