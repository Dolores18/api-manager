use std::sync::Arc;
use tokio::time::{interval, Duration};
use api_manager::{
    config::AppConfig,
    database::initialize_database,
    routes::api::app_routes,
    services::{balance_checker::BalanceChecker, provider_pool::initialize_provider_pool},
};
use tracing::{info, error};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("应用启动中...");

    // 加载配置
    let config = AppConfig::from_env()?;
    info!("环境: {:?}", config.environment);
    info!("监听地址: {}", config.socket_addr());

    // 初始化数据库
    let db_pool = initialize_database(&config.database).await?;
    let db_pool = Arc::new(db_pool);

    info!("初始化API代理池...");
    let provider_pool = Arc::new(tokio::sync::Mutex::new(
        initialize_provider_pool(&db_pool)
            .await
            .expect("Failed to initialize provider pool")
    ));

    // 创建余额检查器
    let balance_checker = Arc::new(BalanceChecker::new(db_pool.clone(), provider_pool.clone()));

    // 启动时立即执行一次余额检查（从数据库加载）
    info!("开始启动时余额检查...");
    if let Err(e) = balance_checker.check_all_providers_from_db().await {
        error!("启动时余额检查失败: {}", e);
    }

    // 启动定期余额检查任务（从数据库加载）
    let checker_clone = balance_checker.clone();
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(300)); // 每5分钟检查一次
        loop {
            interval.tick().await;
            info!("开始定期余额检查...");
            if let Err(e) = checker_clone.check_all_providers_from_db().await {
                error!("定期余额检查失败: {}", e);
            }
        }
    });

    info!("API代理池初始化成功");

    // 创建路由
    let app = app_routes((*db_pool).clone(), config.clone()).await;

    // 启动服务器
    let addr = config.socket_addr();
    info!("Starting server on {}", addr);
    axum::serve(
        tokio::net::TcpListener::bind(&addr).await?,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}
