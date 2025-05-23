use std::sync::Arc;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{error, info};
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use tokio::sync::Mutex;
use crate::services::provider_pool::{ProviderInfo, ProviderPoolState};

#[derive(Debug, Deserialize)]
struct UserInfoResponse {
    code: i32,
    message: String,
    status: bool,
    data: UserData,
}

#[derive(Debug, Deserialize)]
struct UserData {
    id: String,
    name: String,
    balance: String,
    status: String,
    #[serde(rename = "totalBalance")]
    total_balance: String,
}

pub struct BalanceChecker {
    client: Client,
    db_pool: Arc<SqlitePool>,
    provider_pool: Arc<Mutex<ProviderPoolState>>,
}

impl BalanceChecker {
    pub fn new(db_pool: Arc<SqlitePool>, provider_pool: Arc<Mutex<ProviderPoolState>>) -> Self {
        Self {
            client: Client::new(),
            db_pool,
            provider_pool,
        }
    }

    // 删除余额为0的提供商
    async fn remove_zero_balance_provider(&self, api_key: &str) -> anyhow::Result<()> {
        let rows_affected = sqlx::query(
            "DELETE FROM api_providers WHERE api_key = ? AND balance <= 0"
        )
        .bind(api_key)
        .execute(&*self.db_pool)
        .await?
        .rows_affected();

        if rows_affected > 0 {
            info!(
                "已从数据库删除余额为0的提供商: api_key={}",
                api_key
            );
            self.provider_pool.lock().await.remove_provider(api_key);
        } else {
             info!("尝试从数据库删除 {} 失败或记录不存在/余额不为0", api_key);
        }

        Ok(())
    }

    // 检查单个提供商的余额
    pub async fn check_balance(&self, provider: &mut ProviderInfo) -> anyhow::Result<()> {
        if !provider.support_balance_check {
            info!("提供商 {} 不支持余额检查", provider.api_key);
            return Ok(());
        }

        // 修改 URL 构建逻辑
        let base_url = if provider.base_url.contains("siliconflow") {
            "https://api.siliconflow.cn".to_string()
        } else {
            provider.base_url.split("/v1/").next()
                .ok_or_else(|| anyhow::anyhow!("无效的 base_url 格式"))?
                .to_string()
        };
        
        let url = format!("{}/v1/user/info", base_url);
        
        info!("检查提供商余额, URL: {}", url);

        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", provider.api_key))
            .send()
            .await?;

        if !response.status().is_success() {
            error!("获取余额失败: HTTP {}", response.status());
            return Err(anyhow::anyhow!("获取余额失败: HTTP {}", response.status()));
        }

        let user_info: UserInfoResponse = response.json().await?;
        let balance = user_info.data.balance.parse::<f64>()?;
        
        // 更新提供商的余额和最后检查时间
        provider.balance = balance;
        provider.last_balance_check = Some(Utc::now());

        // 更新数据库中的余额
        if let Err(e) = self.update_provider_balance(provider).await {
            error!("更新提供商 {} 数据库余额失败: {}", provider.api_key, e);
        }

        info!(
            "提供商 {} 余额获取成功: {}, 最后检查时间: {}",
            provider.api_key,
            balance,
            provider.last_balance_check.unwrap()
        );

        // 如果余额为0，尝试删除（包括数据库和内存）
        if balance <= 0.0 {
            if let Err(e) = self.remove_zero_balance_provider(&provider.api_key).await {
                error!("处理余额为0的提供商 {} 时出错: {}", provider.api_key, e);
            }
        }

        Ok(())
    }

    // 更新数据库中的提供商余额
    async fn update_provider_balance(&self, provider: &ProviderInfo) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE api_providers 
            SET balance = ?, 
                last_balance_check = ?
            WHERE api_key = ?
            "#
        )
        .bind(provider.balance)
        .bind(provider.last_balance_check)
        .bind(&provider.api_key)
        .execute(&*self.db_pool)
        .await?;

        info!(
            "数据库中的提供商余额已更新: api_key={}, balance={}", 
            provider.api_key, 
            provider.balance
        );

        Ok(())
    }

    // 检查所有提供商的余额
    pub async fn check_all_providers(&self, providers: &mut Vec<ProviderInfo>) {
        for provider in providers.iter_mut() {
            match self.check_balance(provider).await {
                Ok(_) => {
                    info!(
                        "提供商 {} 余额检查成功: balance={}, last_check={:?}", 
                        provider.api_key, 
                        provider.balance,
                        provider.last_balance_check
                    );
                }
                Err(e) => {
                    error!(
                        "提供商 {} 余额检查失败: {}", 
                        provider.api_key, 
                        e
                    );
                }
            }
        }
        info!("完成一轮所有提供商余额检查");
    }
} 