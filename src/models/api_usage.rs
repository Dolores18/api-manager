use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// API调用状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ApiCallStatus {
    Success,          // 成功
    Error,            // 错误
    RateLimited,      // 速率限制
    Timeout,          // 超时
    InvalidRequest,   // 无效请求
}

impl Default for ApiCallStatus {
    fn default() -> Self {
        Self::Success
    }
}

/// API使用量记录
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ApiUsage {
    /// 唯一标识符
    pub id: String,
    
    /// 关联的API提供商密钥
    pub provider_api_key: String,
    
    /// 请求时间
    pub request_time: chrono::DateTime<chrono::Utc>,
    
    /// 模型名称
    pub model: String,
    
    /// 输入token数量
    pub prompt_tokens: i32,
    
    /// 输出token数量
    pub completion_tokens: i32,
    
    /// 总token数量
    pub total_tokens: i32,
    
    /// 调用状态
    pub status: String,
    
    /// 客户端IP
    pub client_ip: Option<String>,
    
    /// 请求ID
    pub request_id: Option<String>,
}

impl ApiUsage {
    /// 创建新的API使用量记录
    pub fn new(
        provider_api_key: String,
        model: String,
        prompt_tokens: i32,
        completion_tokens: i32,
        status: ApiCallStatus,
        client_ip: Option<String>,
        request_id: Option<String>,
    ) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            provider_api_key,
            request_time: now,
            model,
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
            status: format!("{:?}", status),
            client_ip,
            request_id,
        }
    }
    
    /// 计算估计成本（如果知道token价格）
    pub fn estimate_cost(&self, prompt_token_price: f64, completion_token_price: f64) -> f64 {
        (self.prompt_tokens as f64 * prompt_token_price) + 
        (self.completion_tokens as f64 * completion_token_price)
    }
}

/// API使用量统计摘要
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiUsageSummary {
    /// 总请求次数
    pub total_requests: i64,
    
    /// 总提示token
    pub total_prompt_tokens: i64,
    
    /// 总完成token
    pub total_completion_tokens: i64,
    
    /// 总token
    pub total_tokens: i64,
    
    /// 成功请求数
    pub successful_requests: i64,
    
    /// 错误请求数
    pub failed_requests: i64,
    
    /// 按提供商分组的统计
    pub provider_stats: Option<Vec<ProviderStats>>,
    
    /// 按模型分组的统计
    pub model_stats: Option<Vec<ModelStats>>,
}

/// 按提供商的使用统计
#[derive(Debug, Serialize, Deserialize)]
pub struct ProviderStats {
    /// 提供商API密钥
    pub provider_api_key: String,
    
    /// 总请求次数
    pub request_count: i64,
    
    /// 总token
    pub total_tokens: i64,
}

/// 按模型的使用统计
#[derive(Debug, Serialize, Deserialize)]
pub struct ModelStats {
    /// 模型名称
    pub model: String,
    
    /// 总请求次数
    pub request_count: i64,
    
    /// 总提示token
    pub total_prompt_tokens: i64,
    
    /// 总完成token
    pub total_completion_tokens: i64,
    
    /// 总token
    pub total_tokens: i64,
} 