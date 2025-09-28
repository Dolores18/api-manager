use axum::{
    routing::{post, get, put},
    Router, http::HeaderValue,
};
use sqlx::SqlitePool;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use crate::handlers::api::{
    chat_completion::{handle_chat_completion, ChatCompletionRequest, ChatCompletionResponse, ErrorResponse, Message},
    provider::{add_provider, batch_add_providers, get_all_providers, AddProviderRequest, AddProviderResponse, BatchAddProviderRequest, ProviderInfoDTO, ProviderListResponse},
    pricing::{add_pricing, get_all_pricing, get_pricing, update_pricing, AddPricingRequest, UpdatePricingRequest, PricingResponse},
};
use crate::services::{ProviderPoolState, provider_pool::{initialize_provider_pool}};
use crate::models::model_pricing::{ModelPricing, ModelPricingSummary};
use utoipa::{OpenApi, IntoParams};
use utoipa_swagger_ui::SwaggerUi;
use tower_http::cors::{CorsLayer, Any};
use axum::http::{Method};

/// API文档
#[derive(OpenApi)]
#[openapi(
    paths(
        crate::handlers::api::chat_completion::handle_chat_completion,
        crate::handlers::api::provider::add_provider,
        crate::handlers::api::provider::batch_add_providers,
        crate::handlers::api::provider::get_all_providers,
        crate::handlers::api::pricing::add_pricing,
        crate::handlers::api::pricing::get_all_pricing,
        crate::handlers::api::pricing::get_pricing,
        crate::handlers::api::pricing::update_pricing
    ),
    components(
        schemas(
            ChatCompletionRequest,
            ChatCompletionResponse,
            ErrorResponse,
            Message,
            AddProviderRequest,
            AddProviderResponse,
            BatchAddProviderRequest,
            ProviderInfoDTO,
            ProviderListResponse,
            AddPricingRequest,
            UpdatePricingRequest,
            PricingResponse,
            ModelPricing,
            ModelPricingSummary
        )
    ),
    tags(
        (name = "chat", description = "聊天相关的API"),
        (name = "providers", description = "API提供商管理"),
        (name = "pricing", description = "模型定价管理")
    )
)]
struct ApiDoc;

// 应用程序状态
#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub provider_pool: Arc<Mutex<ProviderPoolState>>,
    pub config: crate::config::AppConfig,
}

// 配置API路由
pub async fn app_routes(pool: SqlitePool, config: crate::config::AppConfig) -> Router {
    // 初始化provider pool
    let provider_pool = Arc::new(Mutex::new(
        initialize_provider_pool(&pool)
            .await
            .expect("Failed to initialize provider pool")
    ));

    // 创建应用程序状态
    let state = AppState {
        db: pool,
        provider_pool,
        config,
    };

    // 配置CORS - 简单配置
    let cors = CorsLayer::new()
        // 允许所有来源
        .allow_origin(Any)
        // 允许任何方法(GET, POST等)，包括OPTIONS
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        // 明确列出允许的请求头
        .allow_headers([
            axum::http::header::AUTHORIZATION,
            axum::http::header::CONTENT_TYPE,
            axum::http::header::ACCEPT,
            axum::http::header::ORIGIN,
            axum::http::header::ACCEPT_ENCODING,
            axum::http::header::ACCESS_CONTROL_REQUEST_HEADERS,
            axum::http::header::ACCESS_CONTROL_REQUEST_METHOD,
        ])
        // 不允许带认证信息
        .allow_credentials(false)
        // 公开响应头
        .expose_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::CONTENT_LENGTH,
        ])
        // 缓存CORS预检请求结果1小时
        .max_age(Duration::from_secs(3600));

    Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .route("/v1/chat/completions", post(handle_chat_completion))
        .route("/v1/providers", post(add_provider))
        .route("/v1/providers", get(get_all_providers))
        .route("/v1/providers/batch", post(batch_add_providers))
        // 模型定价相关路由
        .route("/v1/pricing", post(add_pricing))
        .route("/v1/pricing", get(get_all_pricing))
        .route("/v1/pricing/:name/:model", get(get_pricing))
        .route("/v1/pricing/:name/:model", put(update_pricing))
        .layer(cors)
        .with_state(state)
}

// 简单的健康检查API
async fn health_check() -> &'static str {
    "OK"
}
