use serde::{Deserialize, Serialize};

/// 健康检查状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum HealthStatus {
    /// 健康
    Healthy,
    /// 警告
    Warning,
    /// 不健康
    Unhealthy,
    /// 未知
    Unknown,
}

/// 健康检查记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckRecord {
    /// 健康检查ID
    pub id: String,
    /// 关联的API提供商ID
    pub provider_id: String,
    /// 检查时间
    pub check_time: chrono::DateTime<chrono::Utc>,
    /// 健康状态
    pub status: HealthStatus,
    /// 响应时间(毫秒)
    pub response_time_ms: u64,
    /// HTTP状态码
    pub http_status: Option<u16>,
    /// 错误消息
    pub error_message: Option<String>,
    /// 其他详细数据
    pub details: Option<serde_json::Value>,
}

impl HealthCheckRecord {
    /// 创建新的健康检查记录
    pub fn new(
        id: String,
        provider_id: String,
        status: HealthStatus,
        response_time_ms: u64,
        http_status: Option<u16>,
        error_message: Option<String>,
        details: Option<serde_json::Value>,
    ) -> Self {
        Self {
            id,
            provider_id,
            check_time: chrono::Utc::now(),
            status,
            response_time_ms,
            http_status,
            error_message,
            details,
        }
    }

    /// 检查是否健康
    pub fn is_healthy(&self) -> bool {
        self.status == HealthStatus::Healthy
    }

    /// 检查是否有警告
    pub fn has_warning(&self) -> bool {
        self.status == HealthStatus::Warning
    }

    /// 检查是否不健康
    pub fn is_unhealthy(&self) -> bool {
        self.status == HealthStatus::Unhealthy
    }
}

/// 健康检查配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    /// 配置ID
    pub id: String,
    /// 关联的API提供商ID
    pub provider_id: String,
    /// 检查间隔(秒)
    pub interval_seconds: u64,
    /// 超时时间(毫秒)
    pub timeout_ms: u64,
    /// 重试次数
    pub retry_count: u32,
    /// 警告阈值(毫秒) - 响应时间超过此值时进入警告状态
    pub warning_threshold_ms: u64,
    /// 是否启用
    pub is_enabled: bool,
    /// 创建时间
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// 最后更新时间
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl HealthCheckConfig {
    /// 创建新的健康检查配置
    pub fn new(
        id: String,
        provider_id: String,
        interval_seconds: u64,
        timeout_ms: u64,
        retry_count: u32,
        warning_threshold_ms: u64,
    ) -> Self {
        let now = chrono::Utc::now();
        Self {
            id,
            provider_id,
            interval_seconds,
            timeout_ms,
            retry_count,
            warning_threshold_ms,
            is_enabled: true,
            created_at: now,
            updated_at: now,
        }
    }

    /// 启用健康检查
    pub fn enable(&mut self) {
        self.is_enabled = true;
        self.updated_at = chrono::Utc::now();
    }

    /// 禁用健康检查
    pub fn disable(&mut self) {
        self.is_enabled = false;
        self.updated_at = chrono::Utc::now();
    }

    /// 更新健康检查参数
    pub fn update_parameters(
        &mut self,
        interval_seconds: u64,
        timeout_ms: u64,
        retry_count: u32,
        warning_threshold_ms: u64,
    ) {
        self.interval_seconds = interval_seconds;
        self.timeout_ms = timeout_ms;
        self.retry_count = retry_count;
        self.warning_threshold_ms = warning_threshold_ms;
        self.updated_at = chrono::Utc::now();
    }
} 