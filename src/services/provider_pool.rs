use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, Semaphore};
use chrono::{DateTime, Utc};
use sqlx::{SqlitePool, Row};

use anyhow::Result;

                                // 最大重试次数

// 令牌使用记录
#[derive(Debug, Clone)]
pub struct TokenUsage {
    pub last_used: DateTime<Utc>,
    pub total_tokens: u32,
    pub request_count: u32,
}

// 代理池状态
#[derive(Debug)]
pub struct ProviderPoolState {
    providers: Vec<ProviderInfo>,
    current_index: usize,
    token_usage: HashMap<String, TokenUsage>,
    connection_semaphores: HashMap<String, Arc<Semaphore>>, // 每个提供商的并发控制
}

#[derive(Debug, Clone)]
pub struct ProviderInfo {
    pub base_url: String,
    pub api_key: String,
    pub max_connections: i32,
    pub min_connections: i32,
    pub acquire_timeout_ms: i32,
    pub idle_timeout_ms: i32,
    pub load_balance_strategy: String,
    pub retry_attempts: i32,
    pub balance: f64,
    pub last_balance_check: Option<DateTime<Utc>>,
    pub min_balance_threshold: f64,
    pub support_balance_check: bool,
    pub model_name: String,
    pub model_type: String,
    pub model_version: String,
}

impl ProviderPoolState {
    pub fn new(providers: Vec<ProviderInfo>) -> Self {
        let mut connection_semaphores = HashMap::new();
        
        // 为每个提供商创建信号量
        for provider in &providers {
            connection_semaphores.insert(
                provider.api_key.clone(),
                Arc::new(Semaphore::new(provider.max_connections as usize))
            );
        }
        
        Self {
            providers,
            current_index: 0,
            token_usage: HashMap::new(),
            connection_semaphores,
        }
    }

    // 获取提供商的并发控制信号量
    pub fn get_semaphore(&self, api_key: &str) -> Option<Arc<Semaphore>> {
        self.connection_semaphores.get(api_key).cloned()
    }

    // 根据负载均衡策略选择下一个可用的提供商
    pub fn select_provider(&self, model_name: &str, strategy: &str) -> Option<&ProviderInfo> {
        if self.providers.is_empty() {
            tracing::info!("没有可用的提供商");
            return None;
        }

        tracing::info!("正在查找模型: {}", model_name);
        for provider in &self.providers {
            tracing::info!(
                "检查提供商: base_url={}, model_name={}, balance={}, available={}", 
                provider.base_url,
                provider.model_name,
                provider.balance,
                self.is_provider_available(provider)
            );
        }

        // 先过滤出余额充足且支持指定模型的提供商
        let available_providers: Vec<&ProviderInfo> = self.providers.iter()
            .filter(|p| self.is_provider_available(p) && p.model_name == model_name)
            .collect();

        if available_providers.is_empty() {
            tracing::info!("没有找到支持模型 {} 的可用提供商", model_name);
            return None;
        }

        // 从可用的提供商中选择一个
        match strategy {
            "RoundRobin" => {
                let provider_index = self.current_index % available_providers.len();
                available_providers.get(provider_index).copied()
            }
            "LeastConnections" => {
                available_providers.iter()
                    .min_by_key(|p| {
                        self.token_usage
                            .get(&p.api_key)
                            .map(|u| u.request_count)
                            .unwrap_or(0)
                    })
                    .copied()
            }
            "LeastTokens" => {
                available_providers.iter()
                    .min_by_key(|p| {
                        self.token_usage
                            .get(&p.api_key)
                            .map(|u| u.total_tokens)
                            .unwrap_or(0)
                    })
                    .copied()
            }
            _ => {
                available_providers.first().copied()
            }
        }
    }

    // 更新轮询索引
    pub fn update_index(&mut self) {
        self.current_index = (self.current_index + 1) % self.providers.len();
    }

    // 更新令牌使用情况
    pub fn update_usage(&mut self, api_key: &str, tokens: u32) {
        let usage = self.token_usage.entry(api_key.to_string()).or_insert(TokenUsage {
            last_used: Utc::now(),
            total_tokens: 0,
            request_count: 0,
        });
        
        usage.last_used = Utc::now();
        usage.total_tokens += tokens;
        usage.request_count += 1;
    }

    // 检查提供商是否可用
    pub fn is_provider_available(&self, provider: &ProviderInfo) -> bool {
        // 检查token余额是否充足
        if provider.support_balance_check {
            // 如果支持余额检查，需要检查余额是否充足
            provider.balance >= provider.min_balance_threshold
        } else {
            true
        }
    }

    // 获取所有提供商
    pub fn get_providers(&mut self) -> &mut Vec<ProviderInfo> {
        &mut self.providers
    }
}

// 从数据库初始化代理池
pub async fn initialize_provider_pool(pool: &SqlitePool) -> Result<ProviderPoolState> {
    let providers = sqlx::query(
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
        "#,
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|row| ProviderInfo {
        base_url: row.get("base_url"),
        api_key: row.get("api_key"),
        max_connections: row.get("max_connections"),
        min_connections: row.get("min_connections"),
        acquire_timeout_ms: row.get("acquire_timeout_ms"),
        idle_timeout_ms: row.get("idle_timeout_ms"),
        load_balance_strategy: row.get("load_balance_strategy"),
        retry_attempts: row.get("retry_attempts"),
        balance: row.get("balance"),
        last_balance_check: row.get("last_balance_check"),
        min_balance_threshold: row.get("min_balance_threshold"),
        support_balance_check: row.get("support_balance_check"),
        model_name: row.get("model_name"),
        model_type: row.get("model_type"),
        model_version: row.get("model_version"),
    })
    .collect();

    Ok(ProviderPoolState::new(providers))
}

// Token管理器
pub struct TokenManager {
    pool: Arc<Mutex<ProviderPoolState>>,
    pub provider: ProviderInfo,
    _connection_permit: Option<tokio::sync::OwnedSemaphorePermit>,
}

impl TokenManager {
    pub async fn new(pool: Arc<Mutex<ProviderPoolState>>, model_name: &str, strategy: &str) -> Option<Self> {
        let (provider, semaphore) = {
            let mut state = pool.lock().await;
            
            // 选择提供商
            let selected = match state.select_provider(model_name, strategy) {
                Some(p) => {
                    tracing::info!("找到可用提供商: base_url={}, api_key={}", p.base_url, p.api_key);
                    let provider = p.clone();
                    // 更新索引（仅用于RoundRobin策略）
                    if strategy == "RoundRobin" {
                        state.update_index();
                    }
                    provider
                }
                None => {
                    tracing::info!("没有找到可用提供商");
                    return None;
                }
            };
            
            // 获取信号量
            let semaphore = match state.get_semaphore(&selected.api_key) {
                Some(s) => {
                    tracing::info!("获取到提供商的信号量");
                    s
                },
                None => {
                    tracing::error!("无法获取提供商的信号量: api_key={}", selected.api_key);
                    return None;
                }
            };
            
            (selected, semaphore)
        };

        // 尝试获取连接许可
        let permit = match semaphore.try_acquire_owned() {
            Ok(permit) => {
                tracing::info!("成功获取连接许可");
                Some(permit)
            },
            Err(e) => {
                tracing::error!("无法获取连接许可: {}", e);
                return None;
            }
        };
        
        Some(Self {
            pool: pool.clone(),
            provider,
            _connection_permit: permit,
        })
    }

    pub async fn update_usage(&self, tokens: u32) {
        let mut state = self.pool.lock().await;
        state.update_usage(&self.provider.api_key, tokens);
    }
} 