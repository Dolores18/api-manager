use sqlx::SqlitePool;
use crate::config::DatabaseConfig;

use anyhow::Result;


/// 创建SQLite数据库连接池
pub async fn create_sqlite_pool(config: &DatabaseConfig) -> Result<SqlitePool> {
    tracing::info!("创建SQLite连接池，数据库路径: {:?}", config.path);
    tracing::info!("数据库URL: {}", config.url);
    
    // 确保数据库目录存在
    if let Some(parent) = config.path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
            tracing::info!("创建数据库目录: {:?}", parent);
        }
    }

    // 构建连接选项
    let mut options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(&config.path)
        .create_if_missing(true);

    // 配置WAL模式
    if config.enable_wal {
        options = options.journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);
        tracing::info!("启用WAL模式");
    }

    // 配置外键约束
    if config.enable_foreign_keys {
        options = options.foreign_keys(true);
        tracing::info!("启用外键约束");
    }

    // 创建连接池
    let pool = SqlitePool::connect_with(options)
        .await?;

    tracing::info!("SQLite连接池创建成功，最大连接数: {}", config.max_connections);
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
