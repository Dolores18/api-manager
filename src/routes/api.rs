use axum::{
    routing::{post, get, put},
    Router,
};
use sqlx::SqlitePool;
use std::sync::Arc;
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
}

// 配置API路由
pub async fn app_routes(pool: SqlitePool) -> Router {
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
    };

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
        .with_state(state)
}

// 简单的健康检查API
async fn health_check() -> &'static str {
    "OK"
}
