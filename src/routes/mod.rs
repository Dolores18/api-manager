pub mod api;
pub mod web;

use axum::Router;
use sqlx::SqlitePool;
use crate::config::AppConfig;

// 创建应用路由
pub async fn create_routes(pool: SqlitePool, config: AppConfig) -> Router {
    Router::new()
        .nest("/api", api::app_routes(pool, config).await)
}
