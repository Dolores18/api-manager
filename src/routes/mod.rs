pub mod api;
pub mod web;

use axum::Router;
use sqlx::SqlitePool;

// 创建应用路由
pub async fn create_routes(pool: SqlitePool) -> Router {
    Router::new()
        .nest("/api", api::app_routes(pool).await)
}
