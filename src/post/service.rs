use crate::cache::redis::RedisCache;
use crate::post::model::{
    CreatePostRequest, Post, PostResponse, Tag, UpdatePostRequest, UserBrief,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use thiserror::Error;
use tracing::{error, info};
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum PostError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("Cache error: {0}")]
    CacheError(#[from] redis::RedisError),

    #[error("Post not found")]
    NotFound,

    #[error("Slug already exists")]
    SlugExists,

    #[error("Title already exists")]
    TitleExists,

    #[error("Unauthorized access")]
    Unauthorized,

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Internal server error: {0}")]
    InternalError(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DataGenerationRequest {
    pub refresh_existing: Option<bool>,
    pub batch_size: Option<i64>,
}

pub struct PostService {
    pool: PgPool,
    redis_cache: Option<RedisCache>,
}

impl PostService {
    pub fn new(pool: PgPool, redis_cache: Option<RedisCache>) -> Self {
        Self { pool, redis_cache }
    }

    // Helper function to sanitize and render markdown
    fn process_markdown(&self, content: &str) -> Result<String, PostError> {
        // In a real implementation, we would sanitize and convert markdown to HTML
        // For this example, we're just returning the content with a simple formatting
        Ok(format!("<div class=\"markdown\">{}</div>", content))
    }

    // Helper to check if slug exists
    async fn check_slug_exists(
        &self,
        slug: &str,
        exclude_id: Option<i64>,
    ) -> Result<bool, PostError> {
        let query = match exclude_id {
            Some(id) => {
                sqlx::query("SELECT EXISTS(SELECT 1 FROM global.posts WHERE slug = $1 AND id != $2 AND is_deleted = false)")
                    .bind(slug)
                    .bind(id)
            },
            None => {
                sqlx::query("SELECT EXISTS(SELECT 1 FROM global.posts WHERE slug = $1 AND is_deleted = false)")
                    .bind(slug)
            }
        };

        let exists: bool = query.fetch_one(&self.pool).await?.get(0);

        Ok(exists)
    }

    // Helper to check if title exists
    async fn check_title_exists(
        &self,
        title: &str,
        exclude_id: Option<i64>,
    ) -> Result<bool, PostError> {
        let query = match exclude_id {
            Some(id) => {
                sqlx::query("SELECT EXISTS(SELECT 1 FROM global.posts WHERE title = $1 AND id != $2 AND is_deleted = false)")
                    .bind(title)
                    .bind(id)
            },
            None => {
                sqlx::query("SELECT EXISTS(SELECT 1 FROM global.posts WHERE title = $1 AND is_deleted = false)")
                    .bind(title)
            }
        };

        let exists: bool = query.fetch_one(&self.pool).await?.get(0);

        Ok(exists)
    }

    // Create a new post
    pub async fn create_post(
        &self,
        user_id: Uuid,
        post: CreatePostRequest,
    ) -> Result<Post, PostError> {
        // Check if slug already exists
        if self.check_slug_exists(&post.slug, None).await? {
            return Err(PostError::SlugExists);
        }

        // Check if title already exists
        if self.check_title_exists(&post.title, None).await? {
            return Err(PostError::TitleExists);
        }

        // Process markdown content
        let content_html = self.process_markdown(&post.content)?;

        // Start transaction
        let mut tx = self.pool.begin().await?;

        // Insert post
        let post_result = sqlx::query_as::<_, Post>(
            r#"
            INSERT INTO global.posts (
                title, slug, content, content_html, user_id, views, likes, 
                is_draft, is_deleted, cover_image_url, created_at, updated_at
            ) 
            VALUES ($1, $2, $3, $4, $5, 0, 0, $6, false, $7, $8, $8)
            RETURNING *
            "#,
        )
        .bind(&post.title)
        .bind(&post.slug)
        .bind(&post.content)
        .bind(&content_html)
        .bind(user_id)
        .bind(post.is_draft)
        .bind(post.cover_image_url)
        .bind(Utc::now())
        .fetch_one(&mut *tx)
        .await?;

        // Insert tags
        for tag_name in &post.tags {
            // Upsert tag
            let tag_id: i64 = sqlx::query(
                r#"
                INSERT INTO global.tags (name) 
                VALUES ($1) 
                ON CONFLICT (name) DO UPDATE SET name = $1
                RETURNING id
                "#,
            )
            .bind(tag_name)
            .fetch_one(&mut *tx)
            .await?
            .get(0);

            // Associate tag with post
            sqlx::query(
                r#"
                INSERT INTO global.post_tags (post_id, tag_id)
                VALUES ($1, $2)
                "#,
            )
            .bind(post_result.id)
            .bind(tag_id)
            .execute(&mut *tx)
            .await?;
        }

        // Commit transaction
        tx.commit().await?;

        // Invalidate caches
        if let Some(cache) = &self.redis_cache {
            // This is a new post, so we only need to invalidate popular posts cache
            let _ = cache.invalidate_popular_posts().await;
        }

        info!("Created post with ID: {}", post_result.id);
        Ok(post_result)
    }

    // Get post by ID
    pub async fn get_post_by_id(&self, id: i64) -> Result<PostResponse, PostError> {
        // Try to get from cache first
        if let Some(cache) = &self.redis_cache {
            if let Ok(Some(cached_post)) = cache.get_post_by_id(id).await {
                info!("Retrieved post with ID: {} from cache", id);
                // Deserialize and return
                return match serde_json::from_str(&cached_post) {
                    Ok(post) => Ok(post),
                    Err(e) => {
                        error!("Error deserializing cached post: {}", e);
                        // Continue to DB retrieval if cache deserialization fails
                        self.get_post_from_db(id).await
                    }
                };
            }
        }

        // Not in cache or cache error, get from DB
        self.get_post_from_db(id).await
    }

    // Get post by slug
    pub async fn get_post_by_slug(&self, slug: &str) -> Result<PostResponse, PostError> {
        // Try to get from cache first
        if let Some(cache) = &self.redis_cache {
            if let Ok(Some(cached_post)) = cache.get_post_by_slug(slug).await {
                info!("Retrieved post with slug: {} from cache", slug);
                // Deserialize and return
                return match serde_json::from_str(&cached_post) {
                    Ok(post) => Ok(post),
                    Err(e) => {
                        error!("Error deserializing cached post: {}", e);
                        // Continue to DB retrieval if cache deserialization fails
                        self.get_post_from_db_by_slug(slug).await
                    }
                };
            }
        }

        // Not in cache or cache error, get from DB
        self.get_post_from_db_by_slug(slug).await
    }

    // Helper to get post from DB by ID
    async fn get_post_from_db(&self, id: i64) -> Result<PostResponse, PostError> {
        // Get post
        let post = sqlx::query_as::<_, Post>(
            r#"
            SELECT * FROM global.posts
            WHERE id = $1 AND is_deleted = false
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(PostError::NotFound)?;

        // Get author info
        let author = sqlx::query_as::<_, UserBrief>(
            r#"
            SELECT id, username as name FROM global.users
            WHERE id = $1
            "#,
        )
        .bind(post.user_id)
        .fetch_one(&self.pool)
        .await?;

        // Get tags
        let tags = sqlx::query_as::<_, Tag>(
            r#"
            SELECT t.id, t.name FROM global.tags t
            JOIN global.post_tags pt ON pt.tag_id = t.id
            WHERE pt.post_id = $1
            "#,
        )
        .bind(post.id)
        .fetch_all(&self.pool)
        .await?;

        // Construct response
        let post_response = PostResponse {
            id: post.id,
            title: post.title,
            slug: post.slug,
            content: post.content,
            content_html: post.content_html,
            author,
            tags: tags.into_iter().map(|t| t.name).collect(),
            views: post.views,
            likes: post.likes,
            cover_image_url: post.cover_image_url,
            is_draft: post.is_draft,
            created_at: post.created_at,
            updated_at: post.updated_at,
        };

        // Cache the result
        if let Some(cache) = &self.redis_cache {
            // Serialize and cache
            if let Ok(json_data) = serde_json::to_string(&post_response) {
                let _ = cache.cache_post_by_id(id, &json_data).await;
                let _ = cache
                    .cache_post_by_slug(&post_response.slug, &json_data)
                    .await;

                // Increment views asynchronously
                let _ = cache.increment_post_views(id).await;

                // Log the view in Redis
                if let Some(ref cache) = self.redis_cache {
                    // Log view asynchronously
                    let cache_clone = cache.clone();
                    let post_id = id.clone();

                    tokio::spawn(async move {
                        // Convert timestamp to a hash of the IP address
                        let ip_hash = Some(format!("timestamp-{}", chrono::Utc::now().timestamp()));

                        if let Err(e) = cache_clone.log_post_view(post_id, None, ip_hash).await {
                            error!("Failed to log post view: {}", e);
                        }
                    });
                }
            }
        }

        // Update view count in database asynchronously
        let pool = self.pool.clone();
        let post_id = post.id;
        tokio::spawn(async move {
            let _ = sqlx::query("UPDATE global.posts SET views = views + 1 WHERE id = $1")
                .bind(post_id)
                .execute(&pool)
                .await;
        });

        info!("Retrieved post with ID: {}", id);
        Ok(post_response)
    }

    // Helper to get post from DB by slug
    async fn get_post_from_db_by_slug(&self, slug: &str) -> Result<PostResponse, PostError> {
        // Get post
        let post = sqlx::query_as::<_, Post>(
            r#"
            SELECT * FROM global.posts
            WHERE slug = $1 AND is_deleted = false
            "#,
        )
        .bind(slug)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(PostError::NotFound)?;

        // Use the existing method to get the full post with author and tags
        self.get_post_from_db(post.id).await
    }

    // Update post
    pub async fn update_post(
        &self,
        post_id: i64,
        user_id: Uuid,
        update: UpdatePostRequest,
    ) -> Result<PostResponse, PostError> {
        // Check if post exists and user is authorized
        let post = self.get_post_from_db(post_id).await?;

        // Get the post's user_id from the database directly
        let post_user_id = sqlx::query("SELECT user_id FROM global.posts WHERE id = $1")
            .bind(post_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| {
                error!("Error fetching post owner: {:?}", e);
                PostError::DatabaseError(e)
            })?
            .get::<Uuid, _>("user_id");

        // Check if the user is the author
        if post_user_id != user_id {
            return Err(PostError::Unauthorized);
        }

        // Clone slug and title for existence checks if provided
        let slug_check = update
            .slug
            .as_ref()
            .map(|s| self.check_slug_exists(s, Some(post_id)));
        let title_check = update
            .title
            .as_ref()
            .map(|t| self.check_title_exists(t, Some(post_id)));

        // Run the checks concurrently if needed
        if let Some(check) = slug_check {
            if check.await? {
                return Err(PostError::SlugExists);
            }
        }

        if let Some(check) = title_check {
            if check.await? {
                return Err(PostError::TitleExists);
            }
        }

        // Prepare content_html if content is updated
        let content_html = if let Some(ref content) = update.content {
            Some(self.process_markdown(content)?)
        } else {
            None
        };

        // Create a transaction
        let mut tx = self.pool.begin().await.map_err(|e| {
            error!("Error starting transaction: {:?}", e);
            PostError::DatabaseError(e)
        })?;

        // Update post attributes
        if let Some(title) = &update.title {
            sqlx::query("UPDATE global.posts SET title = $1 WHERE id = $2")
                .bind(title)
                .bind(post_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    error!("Error updating post title: {:?}", e);
                    PostError::DatabaseError(e)
                })?;
        }

        if let Some(slug) = &update.slug {
            sqlx::query("UPDATE global.posts SET slug = $1 WHERE id = $2")
                .bind(slug)
                .bind(post_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    error!("Error updating post slug: {:?}", e);
                    PostError::DatabaseError(e)
                })?;
        }

        if let Some(content) = &update.content {
            sqlx::query("UPDATE global.posts SET content = $1, content_html = $2 WHERE id = $3")
                .bind(content)
                .bind(content_html.unwrap_or_default())
                .bind(post_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    error!("Error updating post content: {:?}", e);
                    PostError::DatabaseError(e)
                })?;
        }

        if let Some(cover_image_url) = &update.cover_image_url {
            sqlx::query("UPDATE global.posts SET cover_image_url = $1 WHERE id = $2")
                .bind(cover_image_url)
                .bind(post_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    error!("Error updating post cover image: {:?}", e);
                    PostError::DatabaseError(e)
                })?;
        }

        if let Some(is_draft) = update.is_draft {
            sqlx::query("UPDATE global.posts SET is_draft = $1 WHERE id = $2")
                .bind(is_draft) // Directly binding the boolean value
                .bind(post_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    error!("Error updating post draft status: {:?}", e);
                    PostError::DatabaseError(e)
                })?;
        }

        // Always update the updated_at timestamp
        sqlx::query("UPDATE global.posts SET updated_at = $1 WHERE id = $2")
            .bind(Utc::now())
            .bind(post_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                error!("Error updating post timestamp: {:?}", e);
                PostError::DatabaseError(e)
            })?;

        // Update tags if provided
        if let Some(tags) = &update.tags {
            // Remove existing tags
            sqlx::query("DELETE FROM global.post_tags WHERE post_id = $1")
                .bind(post_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    error!("Error removing existing tags: {:?}", e);
                    PostError::DatabaseError(e)
                })?;

            // Add new tags
            for tag_name in tags {
                // Upsert tag
                let tag_id: i64 = sqlx::query(
                    r#"
                    INSERT INTO global.tags (name) 
                    VALUES ($1) 
                    ON CONFLICT (name) DO UPDATE SET name = $1
                    RETURNING id
                    "#,
                )
                .bind(tag_name)
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| {
                    error!("Error upserting tag: {:?}", e);
                    PostError::DatabaseError(e)
                })?
                .get(0);

                // Associate tag with post
                sqlx::query(
                    r#"
                    INSERT INTO global.post_tags (post_id, tag_id)
                    VALUES ($1, $2)
                    "#,
                )
                .bind(post_id)
                .bind(tag_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    error!("Error associating tag with post: {:?}", e);
                    PostError::DatabaseError(e)
                })?;
            }
        }

        // Commit the transaction
        tx.commit().await.map_err(|e| {
            error!("Error committing transaction: {:?}", e);
            PostError::DatabaseError(e)
        })?;

        // Clear cache if using Redis
        if let Some(ref cache) = self.redis_cache {
            // Use methods from RedisCache instead of directly calling del
            if let Err(e) = cache.invalidate_post(post_id, &post.slug).await {
                error!("Failed to clear Redis cache for post: {:?}", e);
            }

            // Clear popular posts cache
            if let Err(e) = cache.invalidate_popular_posts().await {
                error!("Failed to clear Redis cache for popular posts: {:?}", e);
            }
        }

        // Return the updated post with author info
        self.get_post_by_id(post_id).await
    }

    // Delete post (soft delete)
    pub async fn delete_post(&self, id: i64, user_id: Uuid) -> Result<(), PostError> {
        // Check if post exists and belongs to user (or user is admin)
        let post = sqlx::query_as::<_, Post>(
            r#"
            SELECT * FROM global.posts
            WHERE id = $1 AND is_deleted = false
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(PostError::NotFound)?;

        // Get the post's user_id from the database directly
        let post_user_id = sqlx::query("SELECT user_id FROM global.posts WHERE id = $1")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| {
                error!("Error fetching post owner: {:?}", e);
                PostError::DatabaseError(e)
            })?
            .get::<Uuid, _>("user_id");

        // Check ownership
        if post_user_id != user_id {
            // Todo: check if user is admin
            return Err(PostError::Unauthorized);
        }

        // Soft delete the post
        sqlx::query(
            r#"
            UPDATE global.posts
            SET is_deleted = true, updated_at = $1
            WHERE id = $2
            "#,
        )
        .bind(Utc::now())
        .bind(id)
        .execute(&self.pool)
        .await?;

        // Invalidate caches
        if let Some(cache) = &self.redis_cache {
            let _ = cache.invalidate_post(id, &post.slug).await;
            let _ = cache.invalidate_popular_posts().await;
        }

        Ok(())
    }

    // Get popular posts
    pub async fn get_popular_posts(&self, limit: i64) -> Result<Vec<PostResponse>, PostError> {
        // Try to get from cache first
        if let Some(cache) = &self.redis_cache {
            if let Ok(Some(cached_posts)) = cache.get_popular_posts().await {
                info!("Retrieved popular posts from cache");
                // Deserialize and return
                match serde_json::from_str::<Vec<PostResponse>>(&cached_posts) {
                    Ok(posts) => return Ok(posts),
                    Err(e) => {
                        error!("Error deserializing cached popular posts: {}", e);
                        // Continue to DB retrieval if cache deserialization fails
                    }
                }
            }
        }

        // Calculate popular posts using weightings for various factors
        let posts = sqlx::query_as::<_, Post>(
            r#"
            SELECT * FROM global.posts
            WHERE is_draft = false AND is_deleted = false
            ORDER BY (views * 0.6 + likes * 0.3) DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        // Get additional data for each post
        let mut post_responses = Vec::new();
        for post in posts {
            // Get author info
            let author = sqlx::query_as::<_, UserBrief>(
                r#"
                SELECT id, username as name FROM global.users
                WHERE id = $1
                "#,
            )
            .bind(post.user_id)
            .fetch_one(&self.pool)
            .await?;

            // Get tags
            let tags = sqlx::query_as::<_, Tag>(
                r#"
                SELECT t.id, t.name FROM global.tags t
                JOIN global.post_tags pt ON pt.tag_id = t.id
                WHERE pt.post_id = $1
                "#,
            )
            .bind(post.id)
            .fetch_all(&self.pool)
            .await?;

            // Construct response
            let post_response = PostResponse {
                id: post.id,
                title: post.title,
                slug: post.slug,
                content: post.content,
                content_html: post.content_html,
                author,
                tags: tags.into_iter().map(|t| t.name).collect(),
                views: post.views,
                likes: post.likes,
                cover_image_url: post.cover_image_url,
                is_draft: post.is_draft,
                created_at: post.created_at,
                updated_at: post.updated_at,
            };

            post_responses.push(post_response);
        }

        // Cache the result
        if let Some(cache) = &self.redis_cache {
            if let Ok(json_data) = serde_json::to_string(&post_responses) {
                let _ = cache.cache_popular_posts(&json_data).await;
            }
        }

        info!("Retrieved {} popular posts", post_responses.len());
        Ok(post_responses)
    }

    /// Trigger an asynchronous data generation process
    pub async fn trigger_data_generation(
        &self,
        _request: &DataGenerationRequest,
    ) -> Result<String, PostError> {
        info!("Data generation skipped due to database schema issues");
        Ok("Data generation skipped due to database schema issues".to_string())
    }
}
