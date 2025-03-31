use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use utoipa::ToSchema;

/// 模型定价记录
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct ModelPricing {
    /// 唯一标识符
    pub id: String,
    
    /// 关联的API提供商名称（如OpenAI、Anthropic等）
    pub name: String,
    
    /// 模型名称
    pub model: String,
    
    /// 输入token单价
    pub prompt_token_price: f64,
    
    /// 输出token单价
    pub completion_token_price: f64,
    
    /// 货币单位
    pub currency: String,
    
    /// 价格生效日期
    pub effective_date: DateTime<Utc>,
    
    /// 创建时间
    pub created_at: DateTime<Utc>,
    
    /// 更新时间
    pub updated_at: DateTime<Utc>,
}

impl ModelPricing {
    /// 创建新的价格记录
    pub fn new(
        name: &str,
        model: &str,
        prompt_token_price: f64,
        completion_token_price: f64,
        currency: &str,
        effective_date: Option<DateTime<Utc>>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            model: model.to_string(),
            prompt_token_price,
            completion_token_price,
            currency: currency.to_string(),
            effective_date: effective_date.unwrap_or(now),
            created_at: now,
            updated_at: now,
        }
    }
    
    /// 计算指定token数量的成本
    pub fn calculate_cost(&self, prompt_tokens: u32, completion_tokens: u32) -> f64 {
        (prompt_tokens as f64 * self.prompt_token_price / 1000.0) + 
        (completion_tokens as f64 * self.completion_token_price / 1000.0)
    }
    
    /// 从数据库获取某个提供商某个模型的当前价格
    pub async fn get_current_price(
        db: &sqlx::SqlitePool,
        name: &str,
        model: &str,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(
            r#"
            SELECT * FROM model_pricing
            WHERE name = ? AND model = ?
            ORDER BY effective_date DESC
            LIMIT 1
            "#
        )
        .bind(name)
        .bind(model)
        .fetch_optional(db)
        .await
    }
    
    /// 更新价格（创建新记录，保持价格历史）
    pub async fn update_price(
        db: &sqlx::SqlitePool,
        name: &str,
        model: &str,
        prompt_token_price: f64,
        completion_token_price: f64,
        currency: &str,
        effective_date: Option<DateTime<Utc>>,
    ) -> Result<Self, sqlx::Error> {
        let new_pricing = Self::new(
            name,
            model,
            prompt_token_price,
            completion_token_price,
            currency,
            effective_date,
        );
        
        sqlx::query(
            r#"
            INSERT INTO model_pricing (
                id, name, model, prompt_token_price,
                completion_token_price, currency, effective_date,
                created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(&new_pricing.id)
        .bind(&new_pricing.name)
        .bind(&new_pricing.model)
        .bind(new_pricing.prompt_token_price)
        .bind(new_pricing.completion_token_price)
        .bind(&new_pricing.currency)
        .bind(new_pricing.effective_date)
        .bind(new_pricing.created_at)
        .bind(new_pricing.updated_at)
        .execute(db)
        .await?;
        
        Ok(new_pricing)
    }
}

/// 多个服务商/模型的价格汇总
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ModelPricingSummary {
    /// 价格记录列表
    pub pricing_list: Vec<ModelPricing>,
    
    /// 记录总数
    pub count: usize,
    
    /// 支持的货币列表
    pub currencies: Vec<String>,
} 