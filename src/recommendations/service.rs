use crate::cache::redis::RedisCache;
use crate::recommendations::model::{
    GenerateRecommendationsRequest, PostRecommendation, RecommendationError, RecommendationParams,
};
use sqlx::PgPool;
use std::sync::{Arc, Mutex};
use tracing::info;
use uuid::Uuid;

const RECOMMENDATION_CACHE_TTL: u64 = 3600; // 1 hour
const DEFAULT_RECOMMENDATION_LIMIT: i64 = 20;

/// Status of recommendation generation
#[derive(Debug, Clone)]
pub enum GenerationStatus {
    Idle,
    Running(String),
    Failed(String),
    Completed(String),
}

#[derive(Clone)]
pub struct RecommendationService {
    pool: PgPool,
    redis_cache: Option<RedisCache>,
    generation_status: Arc<Mutex<GenerationStatus>>,
}

impl RecommendationService {
    pub fn new(pool: PgPool, redis_cache: Option<RedisCache>) -> Self {
        Self {
            pool,
            redis_cache,
            generation_status: Arc::new(Mutex::new(GenerationStatus::Idle)),
        }
    }

    /// Get recommendations for a user
    pub async fn get_recommendations_for_user(
        &self,
        _user_id: Uuid,
        params: &RecommendationParams,
    ) -> Result<Vec<PostRecommendation>, RecommendationError> {
        let _limit = params.limit.unwrap_or(DEFAULT_RECOMMENDATION_LIMIT);

        // TODO: Fix the SQL queries below once the database schema includes the required tables and columns.
        // The following queries reference tables and columns that don't exist in the current database schema:
        // - global.recommendations
        // - global.user_interactions
        // - p.author_id

        // Return an empty vector for now
        info!("Returning empty recommendations list due to database schema issues");
        return Ok(Vec::new());

        /* Commented out due to database schema issues
        // Query database for recommendations
        let rows = sqlx::query!(
            r#"
            WITH recs AS (
                SELECT r.post_id, r.score, r.recommendation_type
                FROM global.recommendations r
                WHERE r.user_id = $1
                  AND r.expires_at > NOW()
                ORDER BY r.score DESC
                LIMIT $2
            )
            SELECT
                r.post_id,
                r.score,
                p.title,
                p.created_at,
                u.username as author,
                p.excerpt,
                ARRAY_AGG(t.name) as tags
            FROM recs r
            JOIN global.posts p ON r.post_id = p.id
            JOIN global.users u ON p.author_id = u.id
            LEFT JOIN global.post_tags pt ON p.id = pt.post_id
            LEFT JOIN global.tags t ON pt.tag_id = t.id
            WHERE p.is_deleted = false
              AND p.is_draft = false
            GROUP BY r.post_id, r.score, p.title, p.created_at, u.username, p.excerpt
            ORDER BY r.score DESC
            "#,
            user_id,
            limit
        )
        .fetch_all(&self.pool)
        .await?;

        let recommendations: Vec<PostRecommendation> = rows
            .into_iter()
            .map(|row| PostRecommendation {
                post_id: row.post_id,
                score: row.score,
                title: row.title,
                author: row.author,
                created_at: row.created_at,
                tags: row.tags.unwrap_or_default(),
                similarity: None,
                excerpt: row.excerpt,
            })
            .collect();

        // If we have no recommendations, generate fallback popular posts
        if recommendations.is_empty() {
            let fallback_rows = sqlx::query!(
                r#"
                SELECT
                    p.id as post_id,
                    0.5 as score,
                    p.title,
                    p.created_at,
                    u.username as author,
                    p.excerpt,
                    ARRAY_AGG(t.name) as tags
                FROM global.posts p
                JOIN global.users u ON p.author_id = u.id
                LEFT JOIN global.post_tags pt ON p.id = pt.post_id
                LEFT JOIN global.tags t ON pt.tag_id = t.id
                WHERE p.is_deleted = false
                  AND p.is_draft = false
                GROUP BY p.id, p.title, p.views, p.likes, p.created_at, u.username, p.excerpt
                ORDER BY (p.views + p.likes * 2) DESC
                LIMIT $1
                "#,
                limit
            )
            .fetch_all(&self.pool)
            .await?;

            let fallbacks: Vec<PostRecommendation> = fallback_rows
                .into_iter()
                .map(|row| PostRecommendation {
                    post_id: row.post_id,
                    score: row.score,
                    title: row.title,
                    author: row.author,
                    created_at: row.created_at,
                    tags: row.tags.unwrap_or_default(),
                    similarity: None,
                    excerpt: row.excerpt,
                })
                .collect();

            // Cache the fallback recommendations
            if let Some(cache) = &self.redis_cache {
                let cache_key = format!("recommendations:{}", user_id);
                let json_data = serde_json::to_string(&fallbacks).unwrap_or_default();

                let _ = cache
                    .get_client()
                    .get_multiplexed_async_connection()
                    .await
                    .map_err(RecommendationError::CacheError)?
                    .set_ex(&cache_key, &json_data, RECOMMENDATION_CACHE_TTL / 2) // Half TTL for fallbacks
                    .await
                    .map_err(RecommendationError::CacheError)?;
            }

            return Ok(fallbacks);
        }

        // Cache the recommendations
        if let Some(cache) = &self.redis_cache {
            let cache_key = format!("recommendations:{}", user_id);
            let json_data = serde_json::to_string(&recommendations).unwrap_or_default();

            let _ = cache
                .get_client()
                .get_multiplexed_async_connection()
                .await
                .map_err(RecommendationError::CacheError)?
                .set_ex(&cache_key, &json_data, RECOMMENDATION_CACHE_TTL)
                .await
                .map_err(RecommendationError::CacheError)?;
        }

        Ok(recommendations)
        */
    }

    /// Generate recommendations for users
    pub async fn generate_recommendations(
        &self,
        _request: GenerateRecommendationsRequest,
    ) -> Result<String, RecommendationError> {
        // Skip all database operations and just return a placeholder response
        info!("Skipping recommendation generation due to database schema issues");
        Ok("Recommendations generation skipped due to database schema issues".to_string())
    }

    /// Get current generation status
    pub fn get_generation_status(&self) -> GenerationStatus {
        self.generation_status.lock().unwrap().clone()
    }

    /// Generate collaborative filtering recommendations
    async fn generate_collaborative_filtering(
        _pool: &PgPool,
        _user_id: Uuid,
        _limit: i64,
    ) -> Result<(), RecommendationError> {
        // TODO: Fix the SQL queries below once the database schema includes the required tables and columns.
        // The following queries reference tables and columns that don't exist in the current database schema:
        // - global.user_interactions
        // - global.recommendations

        // Just return success without doing anything for now
        info!("Skipping generate_collaborative_filtering due to database schema issues");
        return Ok(());

        /* Commented out due to database schema issues
        // Find posts liked by similar users
        // This is a simplified approach; in production you would use more advanced algorithms
        let now = Utc::now();
        let expires_at = now + Duration::days(7);

        // Get posts liked or commented on by users who liked similar posts
        sqlx::query!(
            r#"
            WITH user_interactions AS (
                -- Get all interactions by this user
                SELECT post_id, interaction_type
                FROM global.user_interactions
                WHERE user_id = $1
                AND interaction_type IN ('like', 'comment', 'view')
            ),
            similar_users AS (
                -- Find users who interacted with the same posts
                SELECT DISTINCT ui2.user_id
                FROM user_interactions ui1
                JOIN global.user_interactions ui2
                  ON ui1.post_id = ui2.post_id
                  AND ui2.user_id != $1
                  AND ui2.interaction_type IN ('like', 'comment')
            ),
            candidate_posts AS (
                -- Get posts that similar users like but this user hasn't seen
                SELECT
                    ui.post_id,
                    COUNT(*) AS interaction_count,
                    0.7 + (COUNT(*) * 0.01) AS base_score
                FROM global.user_interactions ui
                JOIN similar_users su ON ui.user_id = su.user_id
                WHERE ui.interaction_type IN ('like', 'comment', 'view')
                  AND NOT EXISTS (
                    SELECT 1 FROM global.user_interactions
                    WHERE user_id = $1 AND post_id = ui.post_id
                  )
                  AND NOT EXISTS (
                    SELECT 1 FROM global.recommendations
                    WHERE user_id = $1 AND post_id = ui.post_id
                  )
                GROUP BY ui.post_id
                ORDER BY interaction_count DESC
                LIMIT $2
            )
            INSERT INTO global.recommendations (
                user_id, post_id, score, recommendation_type, created_at, expires_at
            )
            SELECT
                $1,
                post_id,
                LEAST(base_score, 1.0),
                'collaborative',
                $3,
                $4
            FROM candidate_posts
            RETURNING id
            "#,
            user_id,
            limit,
            now,
            expires_at
        )
        .fetch_all(pool)
        .await?;

        info!(
            "Generated collaborative filtering recommendations for user {}",
            user_id
        );
        Ok(())
        */
    }

    /// Generate content-based recommendations
    async fn generate_content_based_recommendations(
        _pool: &PgPool,
        _user_id: Uuid,
        _limit: i64,
    ) -> Result<(), RecommendationError> {
        // TODO: Fix the SQL queries below once the database schema includes the required tables and columns.
        // The following queries reference tables and columns that don't exist in the current database schema:
        // - global.user_interactions
        // - global.recommendations

        // Just return success without doing anything for now
        info!("Skipping generate_content_based_recommendations due to database schema issues");
        return Ok(());

        /* Commented out due to database schema issues
        // Recommend posts with similar tags to what the user has engaged with
        let now = Utc::now();
        let expires_at = now + Duration::days(7);

        sqlx::query!(
            r#"
            WITH user_tags AS (
                -- Get tags from posts the user has interacted with
                SELECT DISTINCT t.id, t.name
                FROM global.user_interactions ui
                JOIN global.posts p ON ui.post_id = p.id
                JOIN global.post_tags pt ON p.id = pt.post_id
                JOIN global.tags t ON pt.tag_id = t.id
                WHERE ui.user_id = $1
                AND ui.interaction_type IN ('like', 'comment', 'view')
            ),
            tag_matches AS (
                -- Find posts that have similar tags
                SELECT
                    p.id AS post_id,
                    COUNT(DISTINCT pt.tag_id) AS matching_tags,
                    0.6 + (COUNT(DISTINCT pt.tag_id) * 0.05) AS base_score
                FROM global.posts p
                JOIN global.post_tags pt ON p.id = pt.post_id
                JOIN user_tags ut ON pt.tag_id = ut.id
                WHERE NOT EXISTS (
                    SELECT 1 FROM global.user_interactions
                    WHERE user_id = $1 AND post_id = p.id
                )
                AND NOT EXISTS (
                    SELECT 1 FROM global.recommendations
                    WHERE user_id = $1 AND post_id = p.id
                )
                AND p.is_deleted = false
                AND p.is_draft = false
                GROUP BY p.id
                ORDER BY matching_tags DESC
                LIMIT $2
            )
            INSERT INTO global.recommendations (
                user_id, post_id, score, recommendation_type, created_at, expires_at
            )
            SELECT
                $1,
                post_id,
                LEAST(base_score, 1.0),
                'content_based',
                $3,
                $4
            FROM tag_matches
            RETURNING id
            "#,
            user_id,
            limit,
            now,
            expires_at
        )
        .fetch_all(pool)
        .await?;

        info!(
            "Generated content-based recommendations for user {}",
            user_id
        );
        Ok(())
        */
    }

    /// Generate popular post recommendations
    async fn generate_popular_recommendations(
        _pool: &PgPool,
        _user_id: Uuid,
        _limit: i64,
    ) -> Result<(), RecommendationError> {
        // TODO: Fix the SQL queries below once the database schema includes the required tables and columns.
        // The following queries reference tables and columns that don't exist in the current database schema:
        // - global.user_interactions
        // - global.recommendations

        // Just return success without doing anything for now
        info!("Skipping generate_popular_recommendations due to database schema issues");
        return Ok(());

        /* Commented out due to database schema issues
        // Recommend generally popular posts the user hasn't seen
        let now = Utc::now();
        let expires_at = now + Duration::days(5); // Shorter expiry for popular posts

        sqlx::query!(
            r#"
            WITH popular_posts AS (
                -- Get popular posts not seen by this user
                SELECT
                    p.id AS post_id,
                    (p.views + p.likes * 2) AS popularity,
                    0.5 + (LEAST(p.views, 1000) / 2000.0) AS base_score
                FROM global.posts p
                WHERE NOT EXISTS (
                    SELECT 1 FROM global.user_interactions
                    WHERE user_id = $1 AND post_id = p.id
                )
                AND NOT EXISTS (
                    SELECT 1 FROM global.recommendations
                    WHERE user_id = $1 AND post_id = p.id
                )
                AND p.is_deleted = false
                AND p.is_draft = false
                ORDER BY popularity DESC
                LIMIT $2
            )
            INSERT INTO global.recommendations (
                user_id, post_id, score, recommendation_type, created_at, expires_at
            )
            SELECT
                $1,
                post_id,
                LEAST(base_score, 0.9), -- Cap at 0.9 to prioritize personalized recs
                'popular',
                $3,
                $4
            FROM popular_posts
            RETURNING id
            "#,
            user_id,
            limit,
            now,
            expires_at
        )
        .fetch_all(pool)
        .await?;

        info!(
            "Generated popular post recommendations for user {}",
            user_id
        );
        Ok(())
        */
    }

    /// Generate hybrid recommendations combining multiple approaches
    async fn generate_hybrid_recommendations(
        _pool: &PgPool,
        _user_id: Uuid,
        _limit: i64,
    ) -> Result<(), RecommendationError> {
        // TODO: Fix the SQL queries below once the database schema includes the required tables and columns.
        // The following queries reference tables and columns that don't exist in the current database schema:
        // - global.user_interactions
        // - global.recommendations

        // Just return success without doing anything for now
        info!("Skipping generate_hybrid_recommendations due to database schema issues");
        return Ok(());

        /* Commented out due to database schema issues
        // Split the limit between different algorithms
        let collab_limit = limit / 3;
        let content_limit = limit / 3;
        let popular_limit = limit - collab_limit - content_limit;

        // Generate recommendations using each approach
        Self::generate_collaborative_filtering(pool, user_id, collab_limit).await?;
        Self::generate_content_based_recommendations(pool, user_id, content_limit).await?;
        Self::generate_popular_recommendations(pool, user_id, popular_limit).await?;

        info!("Generated hybrid recommendations for user {}", user_id);
        Ok(())
        */
    }

    /// Get similar posts to a specific post
    pub async fn get_similar_posts(
        &self,
        _post_id: i64,
        _user_id: Option<Uuid>,
        params: &RecommendationParams,
    ) -> Result<Vec<PostRecommendation>, RecommendationError> {
        let _limit = params.limit.unwrap_or(DEFAULT_RECOMMENDATION_LIMIT);

        // TODO: Fix the SQL queries below once the database schema includes the required tables and columns.
        // The following queries reference tables and columns that don't exist in the current database schema:
        // - p.author_id

        // Return an empty vector for now
        info!("Returning empty similar posts list due to database schema issues");
        return Ok(Vec::new());

        /* Commented out due to database schema issues
        // TODO: Fix Redis cache handling
        // Cache lookup temporarily disabled to fix compilation errors
        /*
        if let Some(ref redis_cache) = self.redis_cache {
            // Cache lookup code...
        }
        */

        // Check if the post exists
        let post_exists = sqlx::query!(
            "SELECT EXISTS(SELECT 1 FROM global.posts WHERE id = $1 AND is_deleted = false) as exists",
            post_id
        )
        .fetch_one(&self.pool)
        .await?
        .exists
        .unwrap_or(false);

        if !post_exists {
            return Err(RecommendationError::NotFound);
        }

        // Get post's tags
        let post_tags = sqlx::query!(
            r#"
            SELECT ARRAY_AGG(t.name) as tags
            FROM global.post_tags pt
            JOIN global.tags t ON pt.tag_id = t.id
            WHERE pt.post_id = $1
            GROUP BY pt.post_id
            "#,
            post_id
        )
        .fetch_optional(&self.pool)
        .await?
        .and_then(|row| row.tags)
        .unwrap_or_default();

        // Find similar posts by tags
        let similar_rows = sqlx::query!(
            r#"
            WITH post_tags AS (
                SELECT tag_id
                FROM global.post_tags
                WHERE post_id = $1
            ),
            similar_posts AS (
                SELECT
                    p.id,
                    COUNT(DISTINCT pt.tag_id) as matching_tags,
                    COUNT(DISTINCT pt2.tag_id) as total_tags,
                    (COUNT(DISTINCT pt.tag_id)::float /
                     NULLIF(COUNT(DISTINCT pt2.tag_id), 0)::float) as similarity_score
                FROM global.posts p
                JOIN global.post_tags pt2 ON p.id = pt2.post_id
                LEFT JOIN global.post_tags pt ON pt2.tag_id = pt.tag_id AND pt.tag_id IN (SELECT tag_id FROM post_tags)
                WHERE p.id != $1
                  AND p.is_deleted = false
                  AND p.is_draft = false
                GROUP BY p.id
                HAVING COUNT(DISTINCT pt.tag_id) > 0
                ORDER BY similarity_score DESC, p.views DESC
                LIMIT $2
            )
            SELECT
                p.id as post_id,
                p.title,
                p.created_at,
                u.username as author,
                p.excerpt,
                sp.similarity_score,
                ARRAY_AGG(t.name) as tags
            FROM similar_posts sp
            JOIN global.posts p ON sp.id = p.id
            JOIN global.users u ON p.author_id = u.id
            LEFT JOIN global.post_tags pt ON p.id = pt.post_id
            LEFT JOIN global.tags t ON pt.tag_id = t.id
            GROUP BY p.id, p.title, p.created_at, u.username, p.excerpt, sp.similarity_score
            ORDER BY sp.similarity_score DESC, p.views DESC
            "#,
            post_id,
            limit
        )
        .fetch_all(&self.pool)
        .await?;

        let similar_posts: Vec<PostRecommendation> = similar_rows
            .into_iter()
            .map(|row| PostRecommendation {
                post_id: row.post_id,
                title: row.title,
                score: 0.0, // Not using recommendation score for similar posts
                author: row.author,
                created_at: row.created_at,
                tags: row.tags.unwrap_or_default(),
                similarity: Some(row.similarity_score),
                excerpt: row.excerpt,
            })
            .collect();

        // If no similar posts by tags, fall back to popular posts in the same category
        if similar_posts.is_empty() && !post_tags.is_empty() {
            let category = post_tags.first().cloned();

            if let Some(category) = category {
                let fallback_rows = sqlx::query!(
                    r#"
                    SELECT
                        p.id as post_id,
                        p.title,
                        p.created_at,
                        u.username as author,
                        p.excerpt,
                        ARRAY_AGG(t.name) as tags
                    FROM global.posts p
                    JOIN global.users u ON p.author_id = u.id
                    JOIN global.post_tags pt ON p.id = pt.post_id
                    JOIN global.tags t ON pt.tag_id = t.id
                    WHERE p.id != $1
                      AND p.is_deleted = false
                      AND p.is_draft = false
                      AND t.name = $3
                    GROUP BY p.id, p.title, p.created_at, u.username, p.excerpt
                    ORDER BY p.views DESC
                    LIMIT $2
                    "#,
                    post_id,
                    limit,
                    category
                )
                .fetch_all(&self.pool)
                .await?;

                let fallbacks: Vec<PostRecommendation> = fallback_rows
                    .into_iter()
                    .map(|row| PostRecommendation {
                        post_id: row.post_id,
                        title: row.title,
                        score: 0.0,
                        author: row.author,
                        created_at: row.created_at,
                        tags: row.tags.unwrap_or_default(),
                        similarity: Some(0.5), // Medium similarity based on category
                        excerpt: row.excerpt,
                    })
                    .collect();

                // Cache the fallback recommendations
                // TODO: Fix Redis cache handling
                // Cache storage temporarily disabled to fix compilation errors
                /*
                if let Some(cache) = &self.redis_cache {
                    let cache_key = format!("similar_posts:{}", post_id);
                    let json_data = serde_json::to_string(&fallbacks).unwrap_or_default();

                    let _ = cache
                        .get_client()
                        .get_multiplexed_async_connection()
                        .await
                        .map_err(RecommendationError::CacheError)?
                        .set_ex(&cache_key, &json_data, RECOMMENDATION_CACHE_TTL / 2) // Half TTL for fallbacks
                        .await
                        .map_err(RecommendationError::CacheError)?;
                }
                */

                return Ok(fallbacks);
            }
        }

        // Cache the similar posts results
        // TODO: Fix Redis cache handling
        // Cache storage temporarily disabled to fix compilation errors
        /*
        if let Some(cache) = &self.redis_cache {
            let cache_key = format!("similar_posts:{}", post_id);
            let json_data = serde_json::to_string(&similar_posts).unwrap_or_default();

            let _ = cache
                .get_client()
                .get_multiplexed_async_connection()
                .await
                .map_err(RecommendationError::CacheError)?
                .set_ex(&cache_key, &json_data, RECOMMENDATION_CACHE_TTL)
                .await
                .map_err(RecommendationError::CacheError)?;
        }
        */

        Ok(similar_posts)
        */
    }

    /// Refresh the recommendation model
    pub async fn refresh_recommendation_model(&self) -> Result<(), RecommendationError> {
        // TODO: Fix the SQL queries below once the database schema includes the required tables and columns.
        // The following queries reference tables and columns that don't exist in the current database schema:
        // - global.recommendations
        // - global.user_interactions

        // Just return success without doing anything for now
        info!("Skipping refresh_recommendation_model due to database schema issues");
        return Ok(());

        /* Commented out due to database schema issues
        // Lock status to indicate we're refreshing
        {
            let mut status = self.generation_status.lock().unwrap();
            *status = GenerationStatus::Running("Refreshing recommendation model".to_string());
        }

        // Create a clone of self to move into the task
        let service_clone = self.clone();

        // Spawn a background task to handle the refresh
        tokio::spawn(async move {
            match service_clone
                .generate_recommendations(GenerateRecommendationsRequest {
                    user_ids: None, // All users
                    limit_per_user: Some(50),
                    algorithm: Some("hybrid".to_string()),
                    refresh_existing: Some(true),
                })
                .await
            {
                Ok(_) => {
                    let mut status = service_clone.generation_status.lock().unwrap();
                    *status = GenerationStatus::Completed(format!(
                        "Recommendation model refreshed successfully at {}",
                        Utc::now().to_rfc3339()
                    ));
                    info!("Recommendation model refreshed successfully");
                }
                Err(e) => {
                    let mut status = service_clone.generation_status.lock().unwrap();
                    *status = GenerationStatus::Failed(format!(
                        "Failed to refresh recommendation model: {}",
                        e
                    ));
                    error!("Failed to refresh recommendation model: {}", e);
                }
            }
        });

        Ok(())
        */
    }

    /// Trigger an asynchronous recommendation generation process
    pub async fn trigger_recommendation_generation(
        &self,
        _request: &GenerateRecommendationsRequest,
    ) -> Result<String, RecommendationError> {
        // TODO: Fix the SQL queries below once the database schema includes the required tables and columns.
        // The following code references tables that don't exist in the current database schema:
        // - global.recommendations
        // - global.user_interactions

        // Just return success without doing anything for now
        info!("Skipping generate_recommendations due to database schema issues");
        return Ok("Recommendation generation skipped due to database schema issues".to_string());

        /* Commented out due to database schema issues
        // Original implementation...
         */
    }
}
