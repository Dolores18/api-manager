use std::sync::Arc;
use reqwest::Client;
use serde::Deserialize;
use tracing::{error, info};
use chrono::Utc;
use sqlx::{SqlitePool, Row};
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

    async fn remove_invalid_provider(&self, api_key: &str) -> anyhow::Result<()> {
        let rows_affected = sqlx::query("DELETE FROM api_providers WHERE api_key = ?")
            .bind(api_key)
            .execute(&*self.db_pool)
            .await?
            .rows_affected();

        if rows_affected > 0 {
            info!(
                "已从数据库删除无效的提供商: api_key={}",
                api_key
            );
            self.provider_pool.lock().await.remove_provider(api_key);
        }
        Ok(())
    }

    // 检查单个提供商的余额并更新数据库
    async fn check_balance_and_update_db(&self, provider: &ProviderInfo) -> anyhow::Result<f64> {
        if !provider.support_balance_check {
            info!("提供商 {} 不支持余额检查", provider.api_key);
            return Ok(provider.balance);
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

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            error!("获取余额失败: HTTP 401 Unauthorized. 密钥 {} 无效或已过期。", provider.api_key);
            // 将余额设置为NULL表示无效
            self.update_provider_balance_to_null(&provider.api_key).await?;
            return Err(anyhow::anyhow!("获取余额失败: HTTP 401 Unauthorized"));
        }

        if !response.status().is_success() {
            error!("获取余额失败: HTTP {}", response.status());
            return Err(anyhow::anyhow!("获取余额失败: HTTP {}", response.status()));
        }

        let user_info: UserInfoResponse = response.json().await?;
        let balance = user_info.data.balance.parse::<f64>()?;
        
        // 更新数据库中的余额
        if let Err(e) = self.update_provider_balance_in_db(&provider.api_key, balance).await {
            error!("更新提供商 {} 数据库余额失败: {}", provider.api_key, e);
        }

        info!(
            "提供商 {} 余额获取成功: {}, 最后检查时间: {}",
            provider.api_key,
            balance,
            Utc::now()
        );

        Ok(balance)
    }

    // 验证API密钥有效性（用于新添加的提供商，不更新数据库）
    pub async fn verify_api_key(&self, provider: &ProviderInfo) -> anyhow::Result<f64> {
        if !provider.support_balance_check {
            info!("提供商 {} 不支持余额检查", provider.api_key);
            return Ok(provider.balance);
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
        
        info!("验证API密钥有效性, URL: {}", url);

        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", provider.api_key))
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            error!("API密钥无效: HTTP 401 Unauthorized. 密钥 {} 无效或已过期。", provider.api_key);
            return Err(anyhow::anyhow!("API密钥无效: HTTP 401 Unauthorized"));
        }

        if !response.status().is_success() {
            error!("验证API密钥失败: HTTP {}", response.status());
            return Err(anyhow::anyhow!("验证API密钥失败: HTTP {}", response.status()));
        }

        let user_info: UserInfoResponse = response.json().await?;
        let balance = user_info.data.balance.parse::<f64>()?;
        
        info!(
            "API密钥验证成功: api_key={}, balance={}",
            provider.api_key,
            balance
        );

        Ok(balance)
    }

    // 检查单个提供商的余额
    pub async fn check_balance(&self, provider: &mut ProviderInfo) -> anyhow::Result<()> {
        match self.check_balance_and_update_db(provider).await {
            Ok(balance) => {
                // 如果余额为0，尝试删除（包括数据库和内存）
                if balance <= 0.0 {
                    if let Err(e) = self.remove_zero_balance_provider(&provider.api_key).await {
                        error!("处理余额为0的提供商 {} 时出错: {}", provider.api_key, e);
                    }
                }
                Ok(())
            }
            Err(e) => {
                // 如果是401错误，删除无效的提供商
                if e.to_string().contains("HTTP 401 Unauthorized") {
                    if let Err(delete_err) = self.remove_invalid_provider(&provider.api_key).await {
                        error!("处理无效的提供商 {} 时出错: {}", provider.api_key, delete_err);
                    }
                }
                Err(e)
            }
        }
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

    // 更新数据库中的提供商余额（新方法）
    async fn update_provider_balance_in_db(&self, api_key: &str, balance: f64) -> anyhow::Result<()> {
        info!("开始更新数据库余额: api_key={}, balance={}", api_key, balance);
        
        let result = sqlx::query(
            r#"
            UPDATE api_providers 
            SET balance = ?, 
                last_balance_check = ?
            WHERE api_key = ?
            "#
        )
        .bind(balance)
        .bind(Utc::now())
        .bind(api_key)
        .execute(&*self.db_pool)
        .await?;

        info!(
            "数据库中的提供商余额已更新: api_key={}, balance={}, 影响行数={}", 
            api_key, 
            balance,
            result.rows_affected()
        );

        // 验证更新是否成功
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM api_providers WHERE api_key = ? AND balance = ?"
        )
        .bind(api_key)
        .bind(balance)
        .fetch_one(&*self.db_pool)
        .await?;
        
        info!("验证更新结果: api_key={}, 匹配记录数={}", api_key, count);

        Ok(())
    }

    // 将提供商余额设置为NULL（表示无效）
    async fn update_provider_balance_to_null(&self, api_key: &str) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE api_providers 
            SET balance = NULL, 
                last_balance_check = ?
            WHERE api_key = ?
            "#
        )
        .bind(Utc::now())
        .bind(api_key)
        .execute(&*self.db_pool)
        .await?;

        info!(
            "数据库中的提供商余额已设置为NULL（无效）: api_key={}", 
            api_key
        );

        Ok(())
    }

    // 批量删除余额为0或无效的提供商
    async fn batch_delete_providers(&self) -> anyhow::Result<(usize, usize)> {
        info!("开始批量删除提供商...");
        
        // 先查询要删除的记录数量
        let zero_balance_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM api_providers WHERE balance = 0.0 AND support_balance_check = 1"
        )
        .fetch_one(&*self.db_pool)
        .await?;
        
        let null_balance_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM api_providers WHERE balance IS NULL AND support_balance_check = 1"
        )
        .fetch_one(&*self.db_pool)
        .await?;
        
        info!("准备删除: 余额为0的提供商 {} 个, 余额为NULL的提供商 {} 个", zero_balance_count, null_balance_count);
        
        // 删除余额为0的提供商
        let zero_balance_result = sqlx::query(
            "DELETE FROM api_providers WHERE balance = 0.0 AND support_balance_check = 1"
        )
        .execute(&*self.db_pool)
        .await?;
        
        let zero_balance_deleted = zero_balance_result.rows_affected() as usize;
        
        // 删除余额为NULL的提供商（无效密钥）
        let invalid_result = sqlx::query(
            "DELETE FROM api_providers WHERE balance IS NULL AND support_balance_check = 1"
        )
        .execute(&*self.db_pool)
        .await?;
        
        let invalid_deleted = invalid_result.rows_affected() as usize;
        
        info!(
            "批量删除完成: 删除余额为0的提供商 {} 个, 删除无效的提供商 {} 个", 
            zero_balance_deleted, invalid_deleted
        );
        
        Ok((zero_balance_deleted, invalid_deleted))
    }

    // 检查所有提供商的余额
    // 从数据库加载所有提供商并检查余额
    pub async fn check_all_providers_from_db(&self) -> anyhow::Result<()> {
        info!("开始从数据库加载提供商进行余额检查...");
        
        // 从数据库加载所有活跃的提供商
        let rows = sqlx::query(
            r#"
            SELECT 
                id, name, provider_type, is_official, base_url, api_key,
                status, rate_limit, balance, last_balance_check, min_balance_threshold,
                support_balance_check, model_name, model_type, model_version
            FROM api_providers 
            WHERE status = 'Active'
            ORDER BY created_at DESC
            "#
        )
        .fetch_all(&*self.db_pool)
        .await?;
        
        let total_count = rows.len();
        info!("从数据库加载了 {} 个活跃提供商", total_count);
        
        if total_count == 0 {
            info!("没有活跃的提供商需要检查");
            return Ok(());
        }
        
        let mut success_count = 0;
        let mut failure_count = 0;
        let mut skipped_count = 0;
        
        // 第一阶段：检查所有提供商并更新数据库
        for (index, row) in rows.iter().enumerate() {
            let api_key: String = row.get("api_key");
            let support_balance_check: i64 = row.get("support_balance_check");
            let base_url: String = row.get("base_url");
            let balance: f64 = row.get("balance");
            let min_balance_threshold: f64 = row.get("min_balance_threshold");
            let model_name: String = row.get("model_name");
            let model_type: String = row.get("model_type");
            let model_version: String = row.get("model_version");
            
            info!("检查提供商 {}/{}: {}", index + 1, total_count, api_key);
            
            if support_balance_check == 0 {
                info!("提供商 {} 不支持余额检查，跳过", api_key);
                skipped_count += 1;
                continue;
            }
            
            // 创建临时的ProviderInfo用于余额检查
            let provider = ProviderInfo {
                base_url: base_url.clone(),
                api_key: api_key.clone(),
                max_connections: 10,
                min_connections: 1,
                acquire_timeout_ms: 3000,
                idle_timeout_ms: 600000,
                load_balance_strategy: "RoundRobin".to_string(),
                retry_attempts: 3,
                balance,
                last_balance_check: None,
                min_balance_threshold,
                support_balance_check: support_balance_check == 1,
                model_name: model_name.clone(),
                model_type: model_type.clone(),
                model_version: model_version.clone(),
            };
            
            match self.check_balance_and_update_db(&provider).await {
                Ok(_balance) => {
                    success_count += 1;
                }
                Err(e) => {
                    failure_count += 1;
                    error!(
                        "提供商 {} 余额检查失败: {}", 
                        api_key, 
                        e
                    );
                }
            }
        }
        
        info!(
            "余额检查阶段完成: 总计={}, 成功={}, 失败={}, 跳过={}", 
            total_count, success_count, failure_count, skipped_count
        );
        
        // 第二阶段：批量删除余额为0和无效的提供商
        match self.batch_delete_providers().await {
            Ok((zero_balance_deleted, invalid_deleted)) => {
                info!(
                    "完成一轮所有提供商余额检查: 总计={}, 成功={}, 失败={}, 跳过={}, 删除余额为0={}, 删除无效={}", 
                    total_count, success_count, failure_count, skipped_count, 
                    zero_balance_deleted, invalid_deleted
                );
            }
            Err(e) => {
                error!("批量删除提供商时出错: {}", e);
            }
        }
        
        Ok(())
    }

    pub async fn check_all_providers(&self, providers: &mut Vec<ProviderInfo>) {
        let total_count = providers.len();
        let mut success_count = 0;
        let mut failure_count = 0;
        let mut skipped_count = 0;
        
        info!("开始检查 {} 个提供商的余额", total_count);
        
        // 第一阶段：检查所有提供商并更新数据库
        for (index, provider) in providers.iter().enumerate() {
            info!("检查提供商 {}/{}: {}", index + 1, total_count, provider.api_key);
            
            if !provider.support_balance_check {
                info!("提供商 {} 不支持余额检查，跳过", provider.api_key);
                skipped_count += 1;
                continue;
            }
            
            match self.check_balance_and_update_db(provider).await {
                Ok(_balance) => {
                    success_count += 1;
                }
                Err(e) => {
                    failure_count += 1;
                    error!(
                        "提供商 {} 余额检查失败: {}", 
                        provider.api_key, 
                        e
                    );
                }
            }
        }
        
        info!(
            "余额检查阶段完成: 总计={}, 成功={}, 失败={}, 跳过={}", 
            total_count, success_count, failure_count, skipped_count
        );
        
        // 第二阶段：批量删除余额为0和无效的提供商
        match self.batch_delete_providers().await {
            Ok((zero_balance_deleted, invalid_deleted)) => {
                info!(
                    "完成一轮所有提供商余额检查: 总计={}, 成功={}, 失败={}, 跳过={}, 删除余额为0={}, 删除无效={}", 
                    total_count, success_count, failure_count, skipped_count, 
                    zero_balance_deleted, invalid_deleted
                );
            }
            Err(e) => {
                error!("批量删除提供商时出错: {}", e);
            }
        }
    }
} 