use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
// use tracing::{error, info}; // 未使用，已注释
use utoipa::ToSchema;
// use uuid::Uuid; // 未使用，已注释

use crate::models::model_pricing::{ModelPricing, ModelPricingSummary};
use crate::routes::api::AppState;

/// 添加模型定价请求
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AddPricingRequest {
    /// 提供商名称（如OpenAI、Anthropic等）
    pub name: String,
    /// 模型名称
    pub model: String,
    /// 输入token单价（每千token）
    pub prompt_token_price: f64,
    /// 输出token单价（每千token）
    pub completion_token_price: f64,
    /// 货币单位
    pub currency: Option<String>,
    /// 价格生效日期
    pub effective_date: Option<DateTime<Utc>>,
}

/// 更新模型定价请求
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdatePricingRequest {
    /// 输入token单价（每千token）
    pub prompt_token_price: f64,
    /// 输出token单价（每千token）
    pub completion_token_price: f64,
    /// 货币单位
    pub currency: Option<String>,
    /// 价格生效日期
    pub effective_date: Option<DateTime<Utc>>,
}

/// 模型定价响应
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PricingResponse {
    /// 操作状态
    pub success: bool,
    /// 消息
    pub message: String,
    /// 定价数据
    pub data: Option<ModelPricing>,
}

/// 添加新的模型定价
#[utoipa::path(
    post,
    path = "/v1/pricing",
    request_body = AddPricingRequest,
    responses(
        (status = 201, description = "成功添加模型定价", body = PricingResponse),
        (status = 400, description = "无效的请求", body = PricingResponse),
        (status = 500, description = "服务器错误", body = PricingResponse),
    ),
    tag = "pricing"
)]
pub async fn add_pricing(
    State(state): State<AppState>,
    Json(request): Json<AddPricingRequest>,
) -> Response {
    let currency = request.currency.unwrap_or_else(|| "USD".to_string());
    let effective_date = request.effective_date.unwrap_or_else(Utc::now);
    
    // 检查提供商是否存在
    let provider_exists = sqlx::query!(
        "SELECT COUNT(*) as count FROM api_providers WHERE name = ?",
        request.name
    )
    .fetch_one(&state.db)
    .await
    .map(|row| row.count > 0)
    .unwrap_or(false);
    
    if !provider_exists {
        return (
            StatusCode::BAD_REQUEST,
            Json(PricingResponse {
                success: false,
                message: format!("提供商 '{}' 不存在", request.name),
                data: None,
            }),
        )
        .into_response();
    }
    
    match ModelPricing::update_price(
        &state.db,
        &request.name,
        &request.model,
        request.prompt_token_price,
        request.completion_token_price,
        &currency,
        Some(effective_date),
    )
    .await {
        Ok(pricing) => (
            StatusCode::CREATED,
            Json(PricingResponse {
                success: true,
                message: "成功添加模型定价".to_string(),
                data: Some(pricing),
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(PricingResponse {
                success: false,
                message: format!("添加模型定价失败: {}", e),
                data: None,
            }),
        )
            .into_response(),
    }
}

/// 获取所有模型定价
#[utoipa::path(
    get,
    path = "/v1/pricing",
    responses(
        (status = 200, description = "成功获取所有模型定价", body = ModelPricingSummary),
        (status = 500, description = "服务器错误", body = PricingResponse),
    ),
    tag = "pricing"
)]
pub async fn get_all_pricing(
    State(state): State<AppState>,
) -> Response {
    match sqlx::query_as::<_, ModelPricing>(
        r#"
        SELECT * FROM model_pricing
        ORDER BY name, model, effective_date DESC
        "#
    )
    .fetch_all(&state.db)
    .await {
        Ok(pricing_list) => {
            let count = pricing_list.len();
            let currencies: Vec<String> = pricing_list
                .iter()
                .map(|p| p.currency.clone())
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();
                
            (
                StatusCode::OK,
                Json(ModelPricingSummary {
                    pricing_list,
                    count,
                    currencies,
                }),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(PricingResponse {
                success: false,
                message: format!("获取模型定价失败: {}", e),
                data: None,
            }),
        )
            .into_response(),
    }
}

/// 获取特定提供商和模型的定价
#[utoipa::path(
    get,
    path = "/v1/pricing/{name}/{model}",
    params(
        ("name" = String, Path, description = "提供商名称"),
        ("model" = String, Path, description = "模型名称"),
    ),
    responses(
        (status = 200, description = "成功获取模型定价", body = ModelPricing),
        (status = 404, description = "模型定价不存在", body = PricingResponse),
        (status = 500, description = "服务器错误", body = PricingResponse),
    ),
    tag = "pricing"
)]
pub async fn get_pricing(
    State(state): State<AppState>,
    Path((name, model)): Path<(String, String)>,
) -> Response {
    match ModelPricing::get_current_price(&state.db, &name, &model).await {
        Ok(Some(pricing)) => (StatusCode::OK, Json(pricing)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(PricingResponse {
                success: false,
                message: format!("未找到提供商 '{}' 和模型 '{}' 的定价", name, model),
                data: None,
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(PricingResponse {
                success: false,
                message: format!("获取模型定价失败: {}", e),
                data: None,
            }),
        )
            .into_response(),
    }
}

/// 更新模型定价
#[utoipa::path(
    put,
    path = "/v1/pricing/{name}/{model}",
    params(
        ("name" = String, Path, description = "提供商名称"),
        ("model" = String, Path, description = "模型名称"),
    ),
    request_body = UpdatePricingRequest,
    responses(
        (status = 200, description = "成功更新模型定价", body = PricingResponse),
        (status = 400, description = "无效的请求", body = PricingResponse),
        (status = 404, description = "模型定价不存在", body = PricingResponse),
        (status = 500, description = "服务器错误", body = PricingResponse),
    ),
    tag = "pricing"
)]
pub async fn update_pricing(
    State(state): State<AppState>,
    Path((name, model)): Path<(String, String)>,
    Json(request): Json<UpdatePricingRequest>,
) -> Response {
    // 检查提供商是否存在
    let provider_exists = sqlx::query!(
        "SELECT COUNT(*) as count FROM api_providers WHERE name = ?",
        name
    )
    .fetch_one(&state.db)
    .await
    .map(|row| row.count > 0)
    .unwrap_or(false);
    
    if !provider_exists {
        return (
            StatusCode::BAD_REQUEST,
            Json(PricingResponse {
                success: false,
                message: format!("提供商 '{}' 不存在", name),
                data: None,
            }),
        )
        .into_response();
    }
    
    // 获取当前价格记录
    match ModelPricing::get_current_price(&state.db, &name, &model).await {
        Ok(current) => {
            if current.is_none() {
                return (
                    StatusCode::NOT_FOUND,
                    Json(PricingResponse {
                        success: false,
                        message: format!("未找到提供商 '{}' 和模型 '{}' 的定价", name, model),
                        data: None,
                    }),
                )
                .into_response();
            }
            
            let currency = request.currency.unwrap_or_else(|| "USD".to_string());
            let effective_date = request.effective_date.unwrap_or_else(Utc::now);
            
            // 创建新的价格记录
            match ModelPricing::update_price(
                &state.db,
                &name,
                &model,
                request.prompt_token_price,
                request.completion_token_price,
                &currency,
                Some(effective_date),
            )
            .await {
                Ok(pricing) => (
                    StatusCode::OK,
                    Json(PricingResponse {
                        success: true,
                        message: "成功更新模型定价".to_string(),
                        data: Some(pricing),
                    }),
                )
                    .into_response(),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(PricingResponse {
                        success: false,
                        message: format!("更新模型定价失败: {}", e),
                        data: None,
                    }),
                )
                    .into_response(),
            }
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(PricingResponse {
                success: false,
                message: format!("获取当前模型定价失败: {}", e),
                data: None,
            }),
        )
            .into_response(),
    }
} 