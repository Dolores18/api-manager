use sqlx::SqlitePool;
use crate::config::DatabaseConfig;

use anyhow::Result;


/// 创建SQLite数据库连接池
pub async fn create_sqlite_pool(config: &DatabaseConfig) -> Result<SqlitePool> {
    // 确保数据库目录存在
    if let Some(parent) = config.path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }

    // 构建连接选项
    let mut options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(&config.path)
        .create_if_missing(true);

    // 配置WAL模式
    if config.enable_wal {
        options = options.journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);
    }

    // 配置外键约束
    if config.enable_foreign_keys {
        options = options.foreign_keys(true);
    }

    // 创建连接池
    let pool = SqlitePool::connect_with(options)
        .await?;

    Ok(pool)
}

/// 运行数据库迁移
pub async fn run_migrations(pool: &SqlitePool) -> Result<()> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await?;
    
    Ok(())
}

/// 初始化数据库
/// 包括创建连接池和运行迁移
pub async fn initialize_database(config: &DatabaseConfig) -> Result<SqlitePool> {
    let pool = create_sqlite_pool(config).await?;
    
    // 运行迁移
    run_migrations(&pool).await?;
    
    Ok(pool)
}
