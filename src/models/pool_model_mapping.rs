use serde::{Deserialize, Serialize};

/// 连接池与模型的映射关系
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolModelMapping {
    /// 唯一标识符
    pub id: String,
    /// 连接池ID
    pub pool_id: String,
    /// 模型ID
    pub model_id: String,
    /// 优先级(数字越小优先级越高)
    pub priority: i32,
    /// 权重(用于加权负载均衡)
    pub weight: i32,
    /// 创建时间
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// 更新时间
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl PoolModelMapping {
    /// 创建新的池-模型映射
    pub fn new(
        id: String,
        pool_id: String,
        model_id: String,
        priority: i32,
        weight: i32,
    ) -> Self {
        let now = chrono::Utc::now();
        Self {
            id,
            pool_id,
            model_id,
            priority,
            weight,
            created_at: now,
            updated_at: now,
        }
    }

    /// 更新优先级和权重
    pub fn update_priority_and_weight(&mut self, priority: i32, weight: i32) {
        self.priority = priority;
        self.weight = weight;
        self.updated_at = chrono::Utc::now();
    }
} 