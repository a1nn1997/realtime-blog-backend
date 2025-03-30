use crate::analytics::model::{
    AnalyticsError, EngagementParams, PostStats, PostStatsParams, UserEngagement,
};
use crate::cache::redis::RedisCache;
use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveDateTime, TimeZone, Timelike, Utc};
use redis::AsyncCommands;
use sqlx::PgPool;
use tracing::{error, info};
use uuid::Uuid;

const ENGAGEMENT_CACHE_TTL: u64 = 600; // 10 minutes
const POST_STATS_CACHE_TTL: u64 = 300; // 5 minutes

#[derive(Clone)]
pub struct AnalyticsService {
    pool: PgPool,
    redis_cache: Option<RedisCache>,
}

impl AnalyticsService {
    pub fn new(pool: PgPool, redis_cache: Option<RedisCache>) -> Self {
        Self { pool, redis_cache }
    }

    /// Record a user interaction
    pub async fn record_interaction(
        &self,
        user_id: Option<Uuid>,
        interaction_type: &str,
        post_id: Option<i64>,
        comment_id: Option<i64>,
        metadata: Option<serde_json::Value>,
    ) -> Result<i64, AnalyticsError> {
        // Insert interaction record
        let interaction_id = sqlx::query_scalar!(
            r#"
            INSERT INTO global.user_interactions (
                user_id, interaction_type, post_id, comment_id, metadata, created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id
            "#,
            user_id,
            interaction_type,
            post_id,
            comment_id,
            metadata,
            Utc::now()
        )
        .fetch_one(&self.pool)
        .await?;

        info!(
            "Recorded {} interaction for user {:?} on post {:?}, comment {:?}",
            interaction_type, user_id, post_id, comment_id
        );

        Ok(interaction_id)
    }

    /// Get user engagement metrics
    pub async fn get_user_engagement(
        &self,
        params: &EngagementParams,
    ) -> Result<Vec<UserEngagement>, AnalyticsError> {
        let limit = params.limit.unwrap_or(100);
        let offset = params.offset.unwrap_or(0);

        // Determine time range based on params
        let (start_date, end_date) = self.get_time_range(params)?;

        // Try to get from cache if available
        if let Some(cache) = &self.redis_cache {
            let cache_key = format!(
                "analytics:user_engagement:range:{}:{}:{}:{}",
                start_date.to_rfc3339(),
                end_date.to_rfc3339(),
                limit,
                offset
            );

            let cache_result = cache
                .get_client()
                .get_multiplexed_async_connection()
                .await
                .map_err(AnalyticsError::CacheError)?
                .get::<_, Option<String>>(&cache_key)
                .await
                .map_err(AnalyticsError::CacheError)?;

            if let Some(cached_data) = cache_result {
                return serde_json::from_str::<Vec<UserEngagement>>(&cached_data).map_err(|e| {
                    error!("Failed to deserialize cached engagement data: {}", e);
                    AnalyticsError::InvalidParameter(format!(
                        "Failed to deserialize cached data: {}",
                        e
                    ))
                });
            }
        }

        // Query database if not in cache
        let rows = sqlx::query!(
            r#"
            SELECT
                user_id,
                COUNT(*) FILTER (WHERE interaction_type = 'view') AS "views!",
                COUNT(*) FILTER (WHERE interaction_type = 'like') AS "likes!",
                COUNT(*) FILTER (WHERE interaction_type = 'comment') AS "comments!",
                COUNT(*) AS "total_interactions!"
            FROM global.user_interactions
            WHERE
                user_id IS NOT NULL AND
                created_at >= $1 AND
                created_at <= $2
            GROUP BY user_id
            ORDER BY "total_interactions!" DESC
            LIMIT $3
            OFFSET $4
            "#,
            start_date,
            end_date,
            limit,
            offset
        )
        .fetch_all(&self.pool)
        .await?;

        let engagement_data: Vec<UserEngagement> = rows
            .into_iter()
            .map(|row| UserEngagement {
                user_id: row.user_id.unwrap(),
                views: row.views,
                likes: row.likes,
                comments: row.comments,
                total_interactions: row.total_interactions,
                day: None,
            })
            .collect();

        // Cache the result
        if let Some(cache) = &self.redis_cache {
            let cache_key = format!(
                "analytics:user_engagement:range:{}:{}:{}:{}",
                start_date.to_rfc3339(),
                end_date.to_rfc3339(),
                limit,
                offset
            );

            let json_data = serde_json::to_string(&engagement_data).unwrap_or_default();
            let _ = cache
                .get_client()
                .get_multiplexed_async_connection()
                .await
                .map_err(AnalyticsError::CacheError)?
                .set_ex(&cache_key, &json_data, ENGAGEMENT_CACHE_TTL)
                .await
                .map_err(AnalyticsError::CacheError)?;
        }

        Ok(engagement_data)
    }

    /// Get engagement metrics for a specific user
    pub async fn get_user_engagement_by_id(
        &self,
        user_id: Uuid,
        params: &EngagementParams,
    ) -> Result<UserEngagement, AnalyticsError> {
        // Determine time range based on params
        let (start_date, end_date) = self.get_time_range(params)?;

        // Try to get from cache if available
        if let Some(cache) = &self.redis_cache {
            let cache_key = format!(
                "analytics:user_engagement:{}:{}:{}",
                user_id,
                start_date.to_rfc3339(),
                end_date.to_rfc3339()
            );

            let cache_result = cache
                .get_client()
                .get_multiplexed_async_connection()
                .await
                .map_err(AnalyticsError::CacheError)?
                .get::<_, Option<String>>(&cache_key)
                .await
                .map_err(AnalyticsError::CacheError)?;

            if let Some(cached_data) = cache_result {
                return serde_json::from_str::<UserEngagement>(&cached_data).map_err(|e| {
                    error!("Failed to deserialize cached user engagement data: {}", e);
                    AnalyticsError::InvalidParameter(format!(
                        "Failed to deserialize cached data: {}",
                        e
                    ))
                });
            }
        }

        // Query database if not in cache
        let row = sqlx::query!(
            r#"
            SELECT
                COUNT(*) FILTER (WHERE interaction_type = 'view') AS "views!",
                COUNT(*) FILTER (WHERE interaction_type = 'like') AS "likes!",
                COUNT(*) FILTER (WHERE interaction_type = 'comment') AS "comments!",
                COUNT(*) AS "total_interactions!"
            FROM global.user_interactions
            WHERE
                user_id = $1 AND
                created_at >= $2 AND
                created_at <= $3
            "#,
            user_id,
            start_date,
            end_date
        )
        .fetch_one(&self.pool)
        .await?;

        let engagement = UserEngagement {
            user_id,
            views: row.views,
            likes: row.likes,
            comments: row.comments,
            total_interactions: row.total_interactions,
            day: None,
        };

        // Cache the result
        if let Some(cache) = &self.redis_cache {
            let cache_key = format!(
                "analytics:user_engagement:{}:{}:{}",
                user_id,
                start_date.to_rfc3339(),
                end_date.to_rfc3339()
            );

            let json_data = serde_json::to_string(&engagement).unwrap_or_default();
            let _ = cache
                .get_client()
                .get_multiplexed_async_connection()
                .await
                .map_err(AnalyticsError::CacheError)?
                .set_ex(&cache_key, &json_data, ENGAGEMENT_CACHE_TTL)
                .await
                .map_err(AnalyticsError::CacheError)?;
        }

        Ok(engagement)
    }

    /// Get post statistics
    pub async fn get_post_stats(
        &self,
        params: &PostStatsParams,
    ) -> Result<Vec<PostStats>, AnalyticsError> {
        let limit = params.limit.unwrap_or(100);
        let offset = params.offset.unwrap_or(0);

        // Determine time range based on params
        let (start_date, end_date) = self.get_time_range(params)?;

        // Try to get from cache if available
        if let Some(cache) = &self.redis_cache {
            let cache_key = if let Some(post_id) = params.post_id {
                format!("analytics:post_stats:{}", post_id)
            } else {
                format!(
                    "analytics:post_stats:range:{}:{}:{}:{}",
                    start_date.to_rfc3339(),
                    end_date.to_rfc3339(),
                    limit,
                    offset
                )
            };

            let cache_result = cache
                .get_client()
                .get_multiplexed_async_connection()
                .await
                .map_err(AnalyticsError::CacheError)?
                .get::<_, Option<String>>(&cache_key)
                .await
                .map_err(AnalyticsError::CacheError)?;

            if let Some(cached_data) = cache_result {
                return serde_json::from_str::<Vec<PostStats>>(&cached_data).map_err(|e| {
                    error!("Failed to deserialize cached post stats data: {}", e);
                    AnalyticsError::InvalidParameter(format!(
                        "Failed to deserialize cached data: {}",
                        e
                    ))
                });
            }
        }

        // Build the query based on params
        let rows = sqlx::query!(
            r#"
            WITH post_data AS (
                SELECT
                    post_id,
                    COUNT(*) FILTER (WHERE interaction_type = 'view') AS views,
                    COUNT(*) FILTER (WHERE interaction_type = 'like') AS likes,
                    COUNT(*) FILTER (WHERE interaction_type = 'comment') AS comments,
                    COUNT(*) AS total_interactions
                FROM global.user_interactions
                WHERE
                    (CASE WHEN $1::BIGINT IS NOT NULL THEN post_id = $1 ELSE TRUE END) AND
                    created_at >= $2 AND
                    created_at <= $3
                GROUP BY post_id
            ),
            post_views AS (
                SELECT
                    post_id,
                    COUNT(*) AS view_count
                FROM global.user_interactions
                WHERE
                    (CASE WHEN $1::BIGINT IS NOT NULL THEN post_id = $1 ELSE TRUE END) AND
                    interaction_type = 'view'
                GROUP BY post_id
            )
            SELECT
                pd.post_id,
                pd.views,
                pd.likes,
                pd.comments,
                pd.total_interactions,
                CASE
                    WHEN pv.view_count > 0 THEN
                        ROUND((pd.likes + pd.comments)::numeric / pv.view_count, 2)
                    ELSE 0
                END AS engagement_rate
            FROM post_data pd
            LEFT JOIN post_views pv ON pd.post_id = pv.post_id
            ORDER BY pd.total_interactions DESC
            LIMIT (CASE WHEN $1::BIGINT IS NULL THEN $4::BIGINT ELSE NULL::BIGINT END)
            OFFSET (CASE WHEN $1::BIGINT IS NULL THEN $5::BIGINT ELSE 0 END)
            "#,
            params.post_id,
            start_date,
            end_date,
            limit as i64,
            offset as i64
        )
        .fetch_all(&self.pool)
        .await?;

        let post_stats: Vec<PostStats> = rows
            .into_iter()
            .map(|row| PostStats {
                post_id: row.post_id.unwrap(),
                views: row.views.unwrap_or(0),
                likes: row.likes.unwrap_or(0),
                comments: row.comments.unwrap_or(0),
                total_interactions: row.total_interactions.unwrap_or(0),
                engagement_rate: row
                    .engagement_rate
                    .unwrap_or_default()
                    .to_string()
                    .parse::<f64>()
                    .unwrap_or(0.0),
                day: None,
            })
            .collect();

        // Cache the result
        if let Some(cache) = &self.redis_cache {
            let cache_key = if let Some(post_id) = params.post_id {
                format!("analytics:post_stats:{}", post_id)
            } else {
                format!(
                    "analytics:post_stats:range:{}:{}:{}:{}",
                    start_date.to_rfc3339(),
                    end_date.to_rfc3339(),
                    limit,
                    offset
                )
            };

            let json_data = serde_json::to_string(&post_stats).unwrap_or_default();
            let _ = cache
                .get_client()
                .get_multiplexed_async_connection()
                .await
                .map_err(AnalyticsError::CacheError)?
                .set_ex(&cache_key, &json_data, POST_STATS_CACHE_TTL)
                .await
                .map_err(AnalyticsError::CacheError)?;
        }

        Ok(post_stats)
    }

    /// Get statistics for a specific post
    pub async fn get_post_stats_by_id(
        &self,
        post_id: i64,
        params: &PostStatsParams,
    ) -> Result<PostStats, AnalyticsError> {
        let mut params = params.clone();
        params.post_id = Some(post_id);

        let stats = self.get_post_stats(&params).await?;

        if stats.is_empty() {
            return Err(AnalyticsError::NotFound);
        }

        Ok(stats[0].clone())
    }

    /// Get time-based statistics for a post
    pub async fn get_post_stats_by_time(
        &self,
        post_id: i64,
        time_range: &str,
    ) -> Result<Vec<PostStats>, AnalyticsError> {
        // Determine time range based on params
        let (start_date, end_date) = match time_range {
            "day" => (Utc::now() - Duration::days(1), Utc::now()),
            "week" => (Utc::now() - Duration::days(7), Utc::now()),
            "month" => (Utc::now() - Duration::days(30), Utc::now()),
            "year" => (Utc::now() - Duration::days(365), Utc::now()),
            _ => {
                return Err(AnalyticsError::InvalidParameter(
                    "Invalid time range".to_string(),
                ))
            }
        };

        // Try to get from cache if available
        if let Some(cache) = &self.redis_cache {
            let cache_key = format!("analytics:post_stats:{}:time:{}", post_id, time_range);

            let cache_result = cache
                .get_client()
                .get_multiplexed_async_connection()
                .await
                .map_err(AnalyticsError::CacheError)?
                .get::<_, Option<String>>(&cache_key)
                .await
                .map_err(AnalyticsError::CacheError)?;

            if let Some(cached_data) = cache_result {
                return serde_json::from_str::<Vec<PostStats>>(&cached_data).map_err(|e| {
                    error!("Failed to deserialize cached time-based post stats: {}", e);
                    AnalyticsError::InvalidParameter(format!(
                        "Failed to deserialize cached data: {}",
                        e
                    ))
                });
            }
        }

        // Get time interval for grouping
        let interval = match time_range {
            "day" => "hour",
            "week" => "day",
            "month" => "day",
            "year" => "month",
            _ => "day",
        };

        // Query database
        let rows = sqlx::query!(
            r#"
            WITH time_data AS (
                SELECT
                    post_id,
                    DATE_TRUNC($1, created_at) AS time_bucket,
                    COUNT(*) FILTER (WHERE interaction_type = 'view') AS views,
                    COUNT(*) FILTER (WHERE interaction_type = 'like') AS likes,
                    COUNT(*) FILTER (WHERE interaction_type = 'comment') AS comments,
                    COUNT(*) AS total_interactions
                FROM global.user_interactions
                WHERE
                    post_id = $2 AND
                    created_at >= $3 AND
                    created_at <= $4
                GROUP BY post_id, DATE_TRUNC($1, created_at)
                ORDER BY time_bucket ASC
            ),
            bucket_views AS (
                SELECT
                    post_id,
                    DATE_TRUNC($1, created_at) AS time_bucket,
                    COUNT(*) AS view_count
                FROM global.user_interactions
                WHERE
                    post_id = $2 AND
                    interaction_type = 'view' AND
                    created_at >= $3 AND
                    created_at <= $4
                GROUP BY post_id, DATE_TRUNC($1, created_at)
            )
            SELECT
                td.post_id,
                td.time_bucket AS day,
                td.views,
                td.likes,
                td.comments,
                td.total_interactions,
                CASE
                    WHEN bv.view_count > 0 THEN
                        ROUND((td.likes + td.comments)::numeric / bv.view_count, 2)
                    ELSE 0
                END AS engagement_rate
            FROM time_data td
            LEFT JOIN bucket_views bv ON td.post_id = bv.post_id AND td.time_bucket = bv.time_bucket
            "#,
            interval,
            post_id,
            start_date,
            end_date
        )
        .fetch_all(&self.pool)
        .await?;

        let stats: Vec<PostStats> = rows
            .into_iter()
            .map(|row| PostStats {
                post_id: row.post_id.unwrap(),
                views: row.views.unwrap_or(0),
                likes: row.likes.unwrap_or(0),
                comments: row.comments.unwrap_or(0),
                total_interactions: row.total_interactions.unwrap_or(0),
                engagement_rate: row
                    .engagement_rate
                    .unwrap_or_default()
                    .to_string()
                    .parse::<f64>()
                    .unwrap_or(0.0),
                day: row.day,
            })
            .collect();

        // Cache the result
        if let Some(cache) = &self.redis_cache {
            let cache_key = format!("analytics:post_stats:{}:time:{}", post_id, time_range);

            let json_data = serde_json::to_string(&stats).unwrap_or_default();
            let _ = cache
                .get_client()
                .get_multiplexed_async_connection()
                .await
                .map_err(AnalyticsError::CacheError)?
                .set_ex(&cache_key, &json_data, POST_STATS_CACHE_TTL)
                .await
                .map_err(AnalyticsError::CacheError)?;
        }

        Ok(stats)
    }

    /// Helper to get the time range based on parameters
    fn get_time_range<T>(
        &self,
        params: &T,
    ) -> Result<(DateTime<Utc>, DateTime<Utc>), AnalyticsError>
    where
        T: HasTimeRange,
    {
        let now = Utc::now();

        if let (Some(start_str), Some(end_str)) = (params.start_date(), params.end_date()) {
            // Parse the date strings into DateTime objects
            let start = match chrono::NaiveDate::parse_from_str(&start_str, "%Y-%m-%d") {
                Ok(date) => {
                    // Convert to DateTime<Utc> with time set to beginning of day (00:00:00)
                    let datetime = date.and_hms_opt(0, 0, 0).unwrap();
                    DateTime::<Utc>::from_naive_utc_and_offset(datetime, Utc)
                }
                Err(e) => {
                    return Err(AnalyticsError::InvalidParameter(format!(
                        "Invalid start date format: {}",
                        e
                    )));
                }
            };

            let end = match chrono::NaiveDate::parse_from_str(&end_str, "%Y-%m-%d") {
                Ok(date) => {
                    // Convert to DateTime<Utc> with time set to end of day (23:59:59)
                    let datetime = date.and_hms_opt(23, 59, 59).unwrap();
                    DateTime::<Utc>::from_naive_utc_and_offset(datetime, Utc)
                }
                Err(e) => {
                    return Err(AnalyticsError::InvalidParameter(format!(
                        "Invalid end date format: {}",
                        e
                    )));
                }
            };

            if start > end {
                return Err(AnalyticsError::InvalidParameter(
                    "Start date must be before end date".to_string(),
                ));
            }

            return Ok((start, end));
        }

        // Default time ranges if start/end not provided
        match params.time_range().as_deref() {
            Some("day") => {
                let start = now - Duration::days(1);
                Ok((start, now))
            }
            Some("week") => {
                let start = now - Duration::weeks(1);
                Ok((start, now))
            }
            Some("month") => {
                let start = now - Duration::days(30);
                Ok((start, now))
            }
            Some("year") => {
                let start = now - Duration::days(365);
                Ok((start, now))
            }
            _ => {
                // Default to last 7 days if no valid time range specified
                let start = now - Duration::weeks(1);
                Ok((start, now))
            }
        }
    }

    /// Refresh materialized views for analytics
    pub async fn refresh_materialized_views(&self) -> Result<(), AnalyticsError> {
        info!("Refreshing analytics materialized views");

        sqlx::query("SELECT global.refresh_analytics_views()")
            .execute(&self.pool)
            .await?;

        info!("Analytics materialized views refreshed successfully");
        Ok(())
    }

    /// Clear analytics cache by prefix
    pub async fn clear_cache_by_prefix(&self, prefix: &str) -> Result<(), AnalyticsError> {
        if let Some(cache) = &self.redis_cache {
            // Get all keys with the prefix
            let keys = cache
                .get_client()
                .get_multiplexed_async_connection()
                .await
                .map_err(AnalyticsError::CacheError)?
                .keys::<_, Vec<String>>(format!("{}*", prefix))
                .await
                .map_err(AnalyticsError::CacheError)?;

            // Delete all keys
            for key in keys {
                cache
                    .get_client()
                    .get_multiplexed_async_connection()
                    .await
                    .map_err(AnalyticsError::CacheError)?
                    .del::<_, ()>(&key)
                    .await
                    .map_err(AnalyticsError::CacheError)?;
            }
        }
        Ok(())
    }

    /// Log a user interaction (view, like, comment, etc)
    pub async fn log_interaction(
        &self,
        user_id: Option<Uuid>,
        interaction_type: &str,
        post_id: Option<i64>,
        comment_id: Option<i64>,
        duration_ms: Option<i32>,
    ) -> Result<i64, AnalyticsError> {
        // Create metadata if we have duration
        let metadata = if let Some(duration) = duration_ms {
            Some(serde_json::json!({ "duration_ms": duration }))
        } else {
            None
        };

        // Record the interaction
        self.record_interaction(user_id, interaction_type, post_id, comment_id, metadata)
            .await
    }
}

// Add a trait to abstract time range parameters
trait HasTimeRange {
    fn start_date(&self) -> Option<String>;
    fn end_date(&self) -> Option<String>;
    fn time_range(&self) -> Option<String>;
}

// Implement for EngagementParams
impl HasTimeRange for EngagementParams {
    fn start_date(&self) -> Option<String> {
        self.start_date.clone()
    }

    fn end_date(&self) -> Option<String> {
        self.end_date.clone()
    }

    fn time_range(&self) -> Option<String> {
        self.time_range.clone()
    }
}

// Implement for PostStatsParams
impl HasTimeRange for PostStatsParams {
    fn start_date(&self) -> Option<String> {
        self.start_date.clone()
    }

    fn end_date(&self) -> Option<String> {
        self.end_date.clone()
    }

    fn time_range(&self) -> Option<String> {
        self.time_range.clone()
    }
}
