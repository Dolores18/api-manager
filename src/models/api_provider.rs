use serde::{Deserialize, Serialize};
// use std::collections::HashMap; // 未使用，已注释

/// API提供商枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ProviderType {
    OpenAI,
    Anthropic,
    DeepSeek,
    MistralAI,
    Custom(String),
}

/// API提供商状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProviderStatus {
    Active,
    Inactive,
    Limited,
    Maintenance,
}

/// API提供商模型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiProvider {
    /// 唯一标识符
    pub id: String,
    /// 提供商名称（显示用）
    pub name: String,
  
    /// 提供商类型
    pub provider_type: ProviderType,
    /// 是否为官方API
    pub is_official: bool,
    /// 基础URL
    pub base_url: String,
    /// API密钥
    pub api_key: String,
    /// 当前状态
    pub status: ProviderStatus,
    /// 创建时间
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// 最后更新时间
    pub updated_at: chrono::DateTime<chrono::Utc>,
    /// 最后一次健康检查时间
    pub last_health_check: Option<chrono::DateTime<chrono::Utc>>,
    /// 费率限制（请求/分钟）
    pub rate_limit: Option<u32>,
    /// 当前余额
    pub balance: f64,
    /// 最后一次余额检查时间
    pub last_balance_check: Option<chrono::DateTime<chrono::Utc>>,
    /// 最小余额阈值
    pub min_balance_threshold: f64,
    /// 是否支持余额检查
    pub support_balance_check: bool,
}

impl ApiProvider {
    /// 创建新的API提供商
    pub fn new(
        id: String,
        name: String,
        provider_name: Option<String>,
        provider_type: ProviderType,
        is_official: bool,
        base_url: String,
        api_key: String,
        rate_limit: Option<u32>,
    ) -> Self {
        let now = chrono::Utc::now();
        
        // 如果没有提供provider_name，则根据provider_type生成
        let _provider_name = provider_name.unwrap_or_else(|| {
            match provider_type {
                ProviderType::OpenAI => "OpenAI".to_string(),
                ProviderType::Anthropic => "Anthropic".to_string(),
                ProviderType::DeepSeek => "DeepSeek".to_string(),
                ProviderType::MistralAI => "MistralAI".to_string(),
                ProviderType::Custom(ref s) => s.clone(),
            }
        });
        
        Self {
            id,
            name,
         
            provider_type,
            is_official,
            base_url,
            api_key,
            status: ProviderStatus::Active,
            created_at: now,
            updated_at: now,
            last_health_check: None,
            rate_limit,
            balance: 0.0,
            last_balance_check: None,
            min_balance_threshold: 3.0,
            support_balance_check: false,
        }
    }

    /// 根据提供商类型获取标准化的提供商名称
    pub fn get_standard_provider_name(provider_type: &ProviderType) -> String {
        match provider_type {
            ProviderType::OpenAI => "OpenAI".to_string(),
            ProviderType::Anthropic => "Anthropic".to_string(),
            ProviderType::DeepSeek => "DeepSeek".to_string(),
            ProviderType::MistralAI => "MistralAI".to_string(),
            ProviderType::Custom(ref s) => s.clone(),
        }
    }

    /// 更新状态
    pub fn update_status(&mut self, status: ProviderStatus) {
        self.status = status;
        self.updated_at = chrono::Utc::now();
    }

    /// 更新健康检查时间
    pub fn update_health_check(&mut self) {
        self.last_health_check = Some(chrono::Utc::now());
    }

    /// 检查是否为活跃状态
    pub fn is_active(&self) -> bool {
        self.status == ProviderStatus::Active
    }

    /// 更新余额
    pub fn update_balance(&mut self, balance: f64) {
        self.balance = balance;
        self.last_balance_check = Some(chrono::Utc::now());
        self.updated_at = chrono::Utc::now();
    }

    /// 检查余额是否充足
    pub fn has_sufficient_balance(&self) -> bool {
        if !self.support_balance_check {
            return true;
        }
        self.balance >= self.min_balance_threshold
    }
} 