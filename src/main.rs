use std::sync::Arc;
use tokio::time::{interval, Duration};
use api_manager::{
    config::AppConfig,
    database::initialize_database,
    routes::api::app_routes,
    services::{balance_checker::BalanceChecker, provider_pool::initialize_provider_pool},
};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

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
    let balance_checker = Arc::new(BalanceChecker::new(db_pool.clone()));

    // 启动定期余额检查任务
    let pool_clone = provider_pool.clone();
    let checker_clone = balance_checker.clone();
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(300)); // 每5分钟检查一次
        loop {
            interval.tick().await;
            info!("开始定期余额检查...");
            let mut pool_state = pool_clone.lock().await;
            checker_clone.check_all_providers(pool_state.get_providers()).await;
        }
    });

    info!("API代理池初始化成功");

    // 创建路由
    let app = app_routes((*db_pool).clone()).await;

    // 启动服务器
    let addr = config.socket_addr();
    info!("Starting server on {}", addr);
    axum::serve(
        tokio::net::TcpListener::bind(&addr).await?,
        app.into_make_service(),
    )
    .await?;

    Ok(())
}
