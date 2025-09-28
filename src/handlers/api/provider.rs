use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
// use std::collections::HashMap; // 未使用，已注释
use tracing::{error, info};
use crate::routes::api::AppState;
use crate::models::api_provider::ProviderType;
use crate::services::balance_checker::BalanceChecker;
use crate::services::{ProviderInfo, provider_pool::initialize_provider_pool};
// use std::sync::Arc; // 未使用，已注释
use chrono::Utc;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AddProviderRequest {
    /// API密钥
    pub api_key: String,
    /// 提供商类型（OpenAI/Anthropic/DeepSeek/MistralAI/Custom）
    pub provider_type: String,
    /// 模型名称
    pub model_name: String,
    /// 提供商名称（可选，默认使用provider_type-uuid后8位）
    #[serde(default)]
    pub name: Option<String>,
    /// 基础URL（可选，根据provider_type自动设置）
    #[serde(default)]
    pub base_url: Option<String>,
    /// 是否为官方API（可选，默认false）
    #[serde(default)]
    pub is_official: bool,
    /// 费率限制（可选，默认10）
    #[serde(default = "default_rate_limit")]
    pub rate_limit: u32,
    /// 最小余额阈值（可选，默认0.0）
    #[serde(default = "default_min_balance_threshold")]
    pub min_balance_threshold: f64,
    /// 是否支持余额检查（可选，默认true）
    #[serde(default = "default_support_balance_check")]
    pub support_balance_check: bool,
    /// 模型类型（可选，默认ChatCompletion）
    #[serde(default = "default_model_type")]
    pub model_type: String,
    /// 模型版本（可选，默认v3）
    #[serde(default = "default_model_version")]
    pub model_version: String,
}

// 默认值函数
fn default_rate_limit() -> u32 { 10 }
fn default_min_balance_threshold() -> f64 { 1.0 }
fn default_support_balance_check() -> bool { true }
fn default_model_type() -> String { "ChatCompletion".to_string() }
fn default_model_version() -> String { "v3".to_string() }

impl AddProviderRequest {
    fn get_default_base_url(&self) -> String {
        match self.provider_type.as_str() {
            "DeepSeek" => "https://api.siliconflow.cn/v1/chat/completions".to_string(),
            "OpenAI" => "https://api.openai.com/v1/chat/completions".to_string(),
            "Anthropic" => "https://api.anthropic.com/v1/messages".to_string(),
            "MistralAI" => "https://api.mistral.ai/v1/chat/completions".to_string(),
            _ => "".to_string(),
        }
    }

    fn get_name(&self) -> String {
        if let Some(name) = &self.name {
            name.clone()
        } else {
            // 使用provider_type和uuid后8位作为默认名称
            let uuid = generate_uuid();
            let short_id = &uuid[uuid.len()-8..];
            format!("{}-{}", self.provider_type, short_id)
        }
    }

    fn get_base_url(&self) -> String {
        self.base_url.clone().unwrap_or_else(|| self.get_default_base_url())
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AddProviderResponse {
    /// 成功添加的提供商信息
    pub success: Vec<ProviderAddResult>,
    /// 添加失败的提供商信息
    pub failed: Vec<ProviderAddResult>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ProviderAddResult {
    /// 提供商ID
    pub id: Option<String>,
    /// 提供商名称
    pub name: String,
    /// API密钥
    pub api_key: String,
    /// 当前余额
    pub balance: Option<f64>,
    /// 失败原因（如果有）
    pub error: Option<String>,
    /// 创建时间
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BatchAddProviderRequest {
    /// API提供商列表
    pub providers: Vec<AddProviderRequest>,
}

/// 生成UUID作为提供商ID
fn generate_uuid() -> String {
    Uuid::new_v4().to_string()
}

/// 添加新的API提供商
#[utoipa::path(
    post,
    path = "/v1/providers",
    request_body = AddProviderRequest,
    responses(
        (status = 201, description = "成功添加API提供商", body = AddProviderResponse),
        (status = 400, description = "请求参数错误", body = ErrorResponse),
        (status = 500, description = "服务器内部错误", body = ErrorResponse),
    ),
    tag = "providers"
)]
pub async fn add_provider(
    State(state): State<AppState>,
    Json(request): Json<AddProviderRequest>,
) -> Response {
    info!("收到添加API提供商请求: {:?}", request);

    let mut success = Vec::new();
    let mut failed = Vec::new();

    // 生成UUID
    let id = generate_uuid();

    // 解析提供商类型
    let _provider_type = match request.provider_type.as_str() {
        "OpenAI" => ProviderType::OpenAI,
        "Anthropic" => ProviderType::Anthropic,
        "DeepSeek" => ProviderType::DeepSeek,
        "MistralAI" => ProviderType::MistralAI,
        custom => ProviderType::Custom(custom.to_string()),
    };

    // 创建临时的 ProviderInfo 用于检查余额
    let mut provider_info = ProviderInfo {
        base_url: request.get_base_url(),
        api_key: request.api_key.clone(),
        max_connections: 10,
        min_connections: 1,
        acquire_timeout_ms: 3000,
        idle_timeout_ms: 600000,
        load_balance_strategy: "RoundRobin".to_string(),
        retry_attempts: 3,
        balance: 0.0,
        last_balance_check: None,
        min_balance_threshold: request.min_balance_threshold,
        support_balance_check: request.support_balance_check,
        model_name: request.model_name.clone(),
        model_type: request.model_type.clone(),
        model_version: request.model_version.clone(),
    };

    // 初始化 BalanceChecker，传入 db 和 provider_pool
    let balance_checker = BalanceChecker::new(state.db.clone().into(), state.provider_pool.clone());

    // 检查余额
    if provider_info.support_balance_check {
        match balance_checker.check_balance(&mut provider_info).await {
            Ok(_) => {
                if provider_info.balance <= 0.0 {
                    failed.push(ProviderAddResult {
                        id: None,
                        name: request.get_name(),
                        api_key: request.api_key.clone(),
                        balance: Some(provider_info.balance),
                        error: Some("API key 余额为0，无法使用，请先充值后再添加".to_string()),
                        created_at: None,
                    });
                    return (StatusCode::OK, Json(AddProviderResponse { success, failed })).into_response();
                }
            }
            Err(e) => {
                failed.push(ProviderAddResult {
                    id: None,
                    name: request.get_name(),
                    api_key: request.api_key.clone(),
                    balance: None,
                    error: Some(format!("检查余额失败: {}", e)),
                    created_at: None,
                });
                return (StatusCode::OK, Json(AddProviderResponse { success, failed })).into_response();
            }
        }
    }

    // 保存到数据库 - 使用 INSERT OR REPLACE 来处理重复的 API key
    let now = Utc::now();
    match sqlx::query(
        r#"
        INSERT OR REPLACE INTO api_providers (
            id, name, provider_type, is_official, base_url, api_key,
            status, rate_limit, balance, last_balance_check, min_balance_threshold,
            support_balance_check, model_name, model_type, model_version,
            created_at, updated_at
        ) VALUES (
            COALESCE((SELECT id FROM api_providers WHERE api_key = ?), ?),
            ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
            COALESCE((SELECT created_at FROM api_providers WHERE api_key = ?), ?),
            ?
        )
        "#,
    )
    .bind(&request.api_key)  // 用于查找现有记录的 api_key
    .bind(&id)               // 新的 id（如果是新记录）
    .bind(&request.get_name())
    .bind(&request.provider_type)
    .bind(request.is_official)
    .bind(&request.get_base_url())
    .bind(&request.api_key)
    .bind("Active")
    .bind(request.rate_limit)  // 使用请求中的 rate_limit（已有默认值10）
    .bind(provider_info.balance)
    .bind(now)
    .bind(request.min_balance_threshold)
    .bind(request.support_balance_check)
    .bind(&request.model_name)
    .bind(&request.model_type)
    .bind(&request.model_version)
    .bind(&request.api_key)  // 用于查找现有记录的 created_at
    .bind(now)               // 新的 created_at（如果是新记录）
    .bind(now)               // updated_at 总是更新为当前时间
    .execute(&state.db)
    .await
    {
        Ok(_) => {
            success.push(ProviderAddResult {
                id: Some(id),
                name: request.get_name(),
                api_key: request.api_key,
                balance: Some(provider_info.balance),
                error: None,
                created_at: Some(now),
            });

            // 更新provider pool
            if let Ok(new_pool) = initialize_provider_pool(&state.db).await {
                let mut pool = state.provider_pool.lock().await;
                *pool = new_pool;
            }

            (StatusCode::CREATED, Json(AddProviderResponse { success, failed })).into_response()
        }
        Err(e) => {
            error!("保存提供商失败: {}", e);
            failed.push(ProviderAddResult {
                id: None,
                name: request.get_name(),
                api_key: request.api_key,
                balance: Some(provider_info.balance),
                error: Some(format!("保存提供商失败: {}", e)),
                created_at: None,
            });
            (StatusCode::OK, Json(AddProviderResponse { success, failed })).into_response()
        }
    }
}

/// 批量添加API提供商
#[utoipa::path(
    post,
    path = "/v1/providers/batch",
    request_body = BatchAddProviderRequest,
    responses(
        (status = 201, description = "成功添加API提供商", body = AddProviderResponse),
        (status = 400, description = "请求参数错误", body = ErrorResponse),
        (status = 500, description = "服务器内部错误", body = ErrorResponse),
    ),
    tag = "providers"
)]
pub async fn batch_add_providers(
    State(state): State<AppState>,
    Json(request): Json<BatchAddProviderRequest>,
) -> Response {
    info!("收到批量添加API提供商请求: {:?}", request);

    let mut success = Vec::new();
    let mut failed = Vec::new();

    for provider_request in request.providers {
        // 生成UUID
        let id = generate_uuid();

        // 解析提供商类型
        let _provider_type = match provider_request.provider_type.as_str() {
            "OpenAI" => ProviderType::OpenAI,
            "Anthropic" => ProviderType::Anthropic,
            "DeepSeek" => ProviderType::DeepSeek,
            "MistralAI" => ProviderType::MistralAI,
            custom => ProviderType::Custom(custom.to_string()),
        };

        // 创建临时的 ProviderInfo 用于检查余额
        let provider_info = ProviderInfo {
            base_url: provider_request.get_base_url(),
            api_key: provider_request.api_key.clone(),
            max_connections: 10,
            min_connections: 1,
            acquire_timeout_ms: 3000,
            idle_timeout_ms: 600000,
            load_balance_strategy: "RoundRobin".to_string(),
            retry_attempts: 3,
            balance: 0.0,
            last_balance_check: None,
            min_balance_threshold: provider_request.min_balance_threshold,
            support_balance_check: provider_request.support_balance_check,
            model_name: provider_request.model_name.clone(),
            model_type: provider_request.model_type.clone(),
            model_version: provider_request.model_version.clone(),
        };

        // 先验证API密钥有效性
        let balance_checker = BalanceChecker::new(state.db.clone().into(), state.provider_pool.clone());
        let verified_balance = if provider_info.support_balance_check {
            match balance_checker.verify_api_key(&provider_info).await {
                Ok(balance) => {
                    info!("API密钥验证成功: api_key={}, balance={}", 
                          provider_request.api_key, balance);
                    
                    // 检查余额是否满足最小阈值
                    if balance < provider_request.min_balance_threshold {
                        error!("API密钥余额不足: api_key={}, balance={}, 最小阈值={}", 
                               provider_request.api_key, balance, provider_request.min_balance_threshold);
                        failed.push(ProviderAddResult {
                            id: None,
                            name: provider_request.get_name(),
                            api_key: provider_request.api_key.clone(),
                            balance: Some(balance),
                            error: Some(format!("余额不足: {:.4} < {:.4}", balance, provider_request.min_balance_threshold)),
                            created_at: None,
                        });
                        continue;
                    }
                    
                    balance
                }
                Err(e) => {
                    error!("API密钥验证失败: api_key={}, 错误={}", 
                           provider_request.api_key, e);
                    failed.push(ProviderAddResult {
                        id: None,
                        name: provider_request.get_name(),
                        api_key: provider_request.api_key.clone(),
                        balance: None,
                        error: Some(format!("API密钥验证失败: {}", e)),
                        created_at: None,
                    });
                    continue;
                }
            }
        } else {
            provider_info.balance
        };

        // 验证通过后，保存到数据库
        let now = Utc::now();
        info!("开始保存已验证的提供商到数据库: api_key={}, name={}, balance={}", 
              provider_request.api_key, provider_request.get_name(), verified_balance);
        
        let result = sqlx::query(
            r#"
            INSERT OR REPLACE INTO api_providers (
                id, name, provider_type, is_official, base_url, api_key,
                status, rate_limit, balance, last_balance_check, min_balance_threshold,
                support_balance_check, model_name, model_type, model_version,
                created_at, updated_at
            ) VALUES (
                COALESCE((SELECT id FROM api_providers WHERE api_key = ?), ?),
                ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
                COALESCE((SELECT created_at FROM api_providers WHERE api_key = ?), ?),
                ?
            )
            "#,
        )
        .bind(&provider_request.api_key)  // 用于查找现有记录的 api_key
        .bind(&id)                        // 新的 id（如果是新记录）
        .bind(&provider_request.get_name())
        .bind(&provider_request.provider_type)
        .bind(provider_request.is_official)
        .bind(&provider_request.get_base_url())
        .bind(&provider_request.api_key)
        .bind("Active")
        .bind(provider_request.rate_limit)  // 使用请求中的 rate_limit（已有默认值10）
        .bind(verified_balance)
        .bind(now)
        .bind(provider_request.min_balance_threshold)
        .bind(provider_request.support_balance_check)
        .bind(&provider_request.model_name)
        .bind(&provider_request.model_type)
        .bind(&provider_request.model_version)
        .bind(&provider_request.api_key)  // 用于查找现有记录的 created_at
        .bind(now)                        // 新的 created_at（如果是新记录）
        .bind(now)                        // updated_at 总是更新为当前时间
        .execute(&state.db)
        .await;

        match result {
            Ok(exec_result) => {
                info!("提供商保存成功: api_key={}, 影响行数={}", 
                      provider_request.api_key, exec_result.rows_affected());
                
                // 验证数据是否真的保存到数据库
                let verify_count = sqlx::query_scalar::<_, i64>(
                    "SELECT COUNT(*) FROM api_providers WHERE api_key = ?"
                )
                .bind(&provider_request.api_key)
                .fetch_one(&state.db)
                .await;
                
                match verify_count {
                    Ok(count) => {
                        info!("验证保存结果: api_key={}, 数据库中的记录数={}", 
                              provider_request.api_key, count);
                    }
                    Err(e) => {
                        error!("验证保存结果失败: api_key={}, 错误={}", 
                               provider_request.api_key, e);
                    }
                }
                
                // 数据库保存成功，余额已在保存前验证过
                
                success.push(ProviderAddResult {
                    id: Some(id),
                    name: provider_request.get_name(),
                    api_key: provider_request.api_key,
                    balance: Some(verified_balance),
                    error: None,
                    created_at: Some(now),
                });
            }
            Err(e) => {
                error!("保存提供商失败: api_key={}, 错误={}", provider_request.api_key, e);
                failed.push(ProviderAddResult {
                    id: None,
                    name: provider_request.get_name(),
                    api_key: provider_request.api_key,
                    balance: Some(provider_info.balance),
                    error: Some(format!("保存提供商失败: {}", e)),
                    created_at: None,
                });
            }
        }
    }

    // 更新provider pool
    if !success.is_empty() {
        info!("开始重新加载提供商池，成功添加了 {} 个提供商", success.len());
        if let Ok(new_pool) = initialize_provider_pool(&state.db).await {
            let mut pool = state.provider_pool.lock().await;
            *pool = new_pool;
            info!("提供商池重新加载完成，当前有 {} 个提供商", pool.get_providers().len());
        }
    }

    info!("批量添加提供商完成: 成功={}, 失败={}", success.len(), failed.len());
    let response = AddProviderResponse { success, failed };
    (StatusCode::CREATED, Json(response)).into_response()
}

// 定义数据库查询结果DTO
#[derive(Debug, Serialize, sqlx::FromRow, ToSchema)]
pub struct ProviderInfoDTO {
    pub base_url: String,
    pub api_key: String,
    pub max_connections: i32,
    pub min_connections: i32,
    pub acquire_timeout_ms: i32,
    pub idle_timeout_ms: i32,
    pub load_balance_strategy: String,
    pub retry_attempts: i32,
    pub balance: f64,
    pub last_balance_check: Option<chrono::DateTime<chrono::Utc>>,
    pub min_balance_threshold: f64,
    pub support_balance_check: bool,
    pub model_name: String,
    pub model_type: String,
    pub model_version: String,
}

// 从DTO到ProviderInfo的转换
impl From<ProviderInfoDTO> for ProviderInfo {
    fn from(dto: ProviderInfoDTO) -> Self {
        Self {
            base_url: dto.base_url,
            api_key: dto.api_key,
            max_connections: dto.max_connections,
            min_connections: dto.min_connections,
            acquire_timeout_ms: dto.acquire_timeout_ms,
            idle_timeout_ms: dto.idle_timeout_ms,
            load_balance_strategy: dto.load_balance_strategy,
            retry_attempts: dto.retry_attempts,
            balance: dto.balance,
            last_balance_check: dto.last_balance_check,
            min_balance_threshold: dto.min_balance_threshold,
            support_balance_check: dto.support_balance_check,
            model_name: dto.model_name,
            model_type: dto.model_type,
            model_version: dto.model_version,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ProviderListResponse {
    pub providers: Vec<ProviderInfoDTO>,
    pub count: usize,
}

/// 获取所有API提供商
#[utoipa::path(
    get,
    path = "/v1/providers",
    responses(
        (status = 200, description = "成功获取所有API提供商", body = ProviderListResponse),
        (status = 500, description = "服务器内部错误", body = ErrorResponse),
    ),
    tag = "providers"
)]
pub async fn get_all_providers(
    State(state): State<AppState>,
) -> Response {
    info!("收到获取所有API提供商请求");

    match sqlx::query_as::<_, ProviderInfoDTO>(
        r#"
        SELECT 
            base_url,
            api_key,
            rate_limit as max_connections,
            1 as min_connections,
            3000 as acquire_timeout_ms,
            60000 as idle_timeout_ms,
            'RoundRobin' as load_balance_strategy,
            3 as retry_attempts,
            balance,
            last_balance_check,
            min_balance_threshold,
            support_balance_check,
            model_name,
            model_type,
            model_version
        FROM api_providers
        WHERE status = 'Active'
        "#
    )
    .fetch_all(&state.db)
    .await {
        Ok(providers) => {
            let count = providers.len();
            info!("成功获取API提供商列表，共 {} 条记录", count);
            
            let response = ProviderListResponse {
                providers,
                count,
            };
            
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            error!("获取API提供商列表失败: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("获取API提供商列表失败: {}", e),
                }),
            )
                .into_response()
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    /// 错误信息
    pub error: String,
} 



