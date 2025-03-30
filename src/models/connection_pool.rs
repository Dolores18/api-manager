use serde::{Deserialize, Serialize};
use crate::models::ai_model::ModelType;

/// 连接池负载均衡策略
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LoadBalanceStrategy {
    /// 轮询策略
    RoundRobin,
    /// 随机策略
    Random,
    /// 加权轮询
    WeightedRoundRobin,
    /// 最少连接数
    LeastConnections,
}

/// 连接池配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionPoolConfig {
    /// 连接池ID
    pub id: String,
    /// 连接池名称
    pub name: String,
    /// 连接池对应的模型类型
    pub model_type: ModelType,
    /// 最大连接数
    pub max_connections: u32,
    /// 最小连接数
    pub min_connections: u32,
    /// 连接获取超时(毫秒)
    pub acquire_timeout_ms: u64,
    /// 空闲连接超时(毫秒)
    pub idle_timeout_ms: u64,
    /// 负载均衡策略
    pub load_balance_strategy: LoadBalanceStrategy,
    /// 是否启用
    pub is_enabled: bool,
    /// 重试次数
    pub retry_attempts: u32,
    /// 创建时间
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// 最后更新时间
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl ConnectionPoolConfig {
    /// 创建新的连接池配置
    pub fn new(
        id: String,
        name: String,
        model_type: ModelType,
        max_connections: u32,
        min_connections: u32,
        acquire_timeout_ms: u64,
        idle_timeout_ms: u64,
        load_balance_strategy: LoadBalanceStrategy,
        retry_attempts: u32,
    ) -> Self {
        let now = chrono::Utc::now();
        Self {
            id,
            name,
            model_type,
            max_connections,
            min_connections,
            acquire_timeout_ms,
            idle_timeout_ms,
            load_balance_strategy,
            is_enabled: true,
            retry_attempts,
            created_at: now,
            updated_at: now,
        }
    }

    /// 启用连接池
    pub fn enable(&mut self) {
        self.is_enabled = true;
        self.updated_at = chrono::Utc::now();
    }

    /// 禁用连接池
    pub fn disable(&mut self) {
        self.is_enabled = false;
        self.updated_at = chrono::Utc::now();
    }

    /// 更新连接池参数
    pub fn update_parameters(
        &mut self,
        max_connections: u32,
        min_connections: u32,
        acquire_timeout_ms: u64,
        idle_timeout_ms: u64,
    ) {
        self.max_connections = max_connections;
        self.min_connections = min_connections;
        self.acquire_timeout_ms = acquire_timeout_ms;
        self.idle_timeout_ms = idle_timeout_ms;
        self.updated_at = chrono::Utc::now();
    }

    /// 更新负载均衡策略
    pub fn update_load_balance_strategy(&mut self, strategy: LoadBalanceStrategy) {
        self.load_balance_strategy = strategy;
        self.updated_at = chrono::Utc::now();
    }
} 