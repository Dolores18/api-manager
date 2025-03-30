use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// AI模型的类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ModelType {
    ChatCompletion,
    TextCompletion,
    Embedding,
    ImageGeneration,
    AudioTranscription,
    Other(String),
}

/// AI模型信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiModel {
    /// 唯一标识符
    pub id: String,
    /// 模型名称
    pub name: String,
    /// 提供商ID（关联到ApiProvider）
    pub provider_id: String,
    /// 模型类型
    pub model_type: ModelType,
    /// 模型的版本
    pub version: String,
    /// 模型的最大上下文窗口大小（token数）
    pub context_window: Option<u32>,
    /// 模型的输入费用 (按每百万tokens计算)
    pub input_price_per_million_tokens: Option<f64>,
    /// 模型的输出费用 (按每百万tokens计算)
    pub output_price_per_million_tokens: Option<f64>,
    /// 模型的其他属性和能力
    pub capabilities: HashMap<String, String>,
    /// 是否启用
    pub is_enabled: bool,
    /// 创建时间
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// 最后更新时间
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl AiModel {
    /// 创建新的AI模型
    pub fn new(
        id: String,
        name: String,
        provider_id: String,
        model_type: ModelType,
        version: String,
        context_window: Option<u32>,
        input_price_per_million_tokens: Option<f64>,
        output_price_per_million_tokens: Option<f64>,
        capabilities: HashMap<String, String>,
    ) -> Self {
        let now = chrono::Utc::now();
        Self {
            id,
            name,
            provider_id,
            model_type,
            version,
            context_window,
            input_price_per_million_tokens,
            output_price_per_million_tokens,
            capabilities,
            is_enabled: true,
            created_at: now,
            updated_at: now,
        }
    }

    /// 启用模型
    pub fn enable(&mut self) {
        self.is_enabled = true;
        self.updated_at = chrono::Utc::now();
    }

    /// 禁用模型
    pub fn disable(&mut self) {
        self.is_enabled = false;
        self.updated_at = chrono::Utc::now();
    }

    /// 更新模型价格
    pub fn update_pricing(
        &mut self,
        input_price_per_million_tokens: Option<f64>,
        output_price_per_million_tokens: Option<f64>,
    ) {
        self.input_price_per_million_tokens = input_price_per_million_tokens;
        self.output_price_per_million_tokens = output_price_per_million_tokens;
        self.updated_at = chrono::Utc::now();
    }
} 