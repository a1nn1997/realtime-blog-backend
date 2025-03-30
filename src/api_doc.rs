use utoipa::openapi::security::{Http, HttpAuthScheme, SecurityScheme};
use utoipa::{Modify, OpenApi};

/// Security scheme configuration for OpenAPI
pub struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        // Get or create components section
        let components = openapi.components.get_or_insert_with(Default::default);

        // Add bearer token security scheme
        components.add_security_scheme(
            "bearer_auth",
            SecurityScheme::Http(Http::new(HttpAuthScheme::Bearer)),
        );
    }
}

/// API documentation
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Realtime Blog Backend API",
        version = "0.1.0",
        description = "REST API for the Realtime Blog Backend"
    ),
    paths(
        // Add health check endpoints
        crate::routes::health::health_check,
        crate::routes::health::protected_health_check,
        // Add authentication endpoints 
        crate::auth::controller::login,
        crate::auth::controller::register,
        // Add post endpoints
        crate::post::controller::create_post,
        crate::post::controller::get_post,
        crate::post::controller::update_post,
        crate::post::controller::delete_post,
        crate::post::controller::get_popular_posts,
        // Add comment endpoints
        crate::comment::controller::create_comment,
        crate::comment::controller::get_post_comments,
        crate::comment::controller::delete_comment,
        // Add analytics endpoints
        crate::analytics::controller::get_user_engagement,
        crate::analytics::controller::get_user_engagement_by_id,
        crate::analytics::controller::get_post_stats,
        crate::analytics::controller::get_post_stats_by_id,
        crate::analytics::controller::get_post_stats_by_time,
        crate::analytics::controller::refresh_analytics_views,
        // Add recommendation endpoints
        crate::recommendations::controller::get_recommended_posts,
        crate::recommendations::controller::get_similar_posts,
        crate::recommendations::controller::refresh_recommendation_model
    ),
    components(
        schemas(
            // Auth schemas
            crate::auth::controller::RegisterRequest,
            crate::auth::controller::LoginRequest,
            crate::auth::controller::AuthResponse,
            crate::auth::controller::ErrorResponse,
            // Health schemas
            crate::routes::health::HealthResponse,
            // Post schemas
            crate::post::model::CreatePostRequest,
            crate::post::model::UpdatePostRequest,
            crate::post::model::PostResponse,
            crate::post::model::PopularPostsResponse,
            crate::post::model::UserBrief,
            crate::post::model::Tag,
            crate::post::controller::ErrorResponse,
            // Comment schemas
            crate::comment::model::CreateCommentRequest,
            crate::comment::model::CommentResponse,
            crate::comment::model::CommentsListResponse,
            crate::comment::model::CommentAuthor,
            crate::comment::model::CommentErrorResponse,
            // Analytics schemas
            crate::analytics::model::UserEngagement,
            crate::analytics::model::PostStats,
            crate::analytics::model::EngagementParams,
            crate::analytics::model::PostStatsParams,
            crate::analytics::model::InteractionType,
            // Recommendation schemas
            crate::recommendations::model::PostRecommendation,
            crate::recommendations::model::RecommendationParams,
            crate::recommendations::model::RecommendationResponse,
            // External type schemas
            crate::schema_ext::DateTimeWrapper,
            crate::schema_ext::UuidWrapper
        )
    ),
    tags(
        (name = "authentication", description = "Authentication endpoints"),
        (name = "health", description = "Health check endpoints"),
        (name = "posts", description = "Blog post management endpoints"),
        (name = "comments", description = "Comment management endpoints"),
        (name = "analytics", description = "Analytics and statistics endpoints"),
        (name = "recommendations", description = "Content recommendation endpoints")
    ),
    security(
        ("bearer_auth" = [])
    ),
    modifiers(&SecurityAddon)
)]
pub struct ApiDoc;
