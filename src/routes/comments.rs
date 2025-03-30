use crate::auth::middleware::{auth_middleware, optional_auth_middleware};
use crate::comment::controller::{create_comment, delete_comment, get_post_comments};
use crate::comment::service::CommentService;
use axum::{
    middleware,
    routing::{delete, get, post},
    Router,
};
use std::sync::Arc;

/// Create a router for comment routes
pub fn routes(comment_service: Arc<CommentService>) -> Router {
    Router::new()
        // Route for getting post comments (public, but with optional auth)
        .route(
            "/api/posts/:id/comments",
            get(get_post_comments).route_layer(middleware::from_fn(optional_auth_middleware)),
        )
        // Route for creating comments (requires authentication)
        .route(
            "/api/posts/:id/comments",
            post(create_comment).route_layer(middleware::from_fn(auth_middleware)),
        )
        // Route for deleting comments (requires authentication)
        .route(
            "/api/comments/:id",
            delete(delete_comment).route_layer(middleware::from_fn(auth_middleware)),
        )
        .layer(axum::extract::Extension(comment_service))
}
