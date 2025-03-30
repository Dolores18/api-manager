use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::net::SocketAddr;
use std::str::FromStr;
use std::path::PathBuf;

/// 应用程序配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// 环境模式
    pub environment: Environment,
    /// 服务器地址和端口
    pub server: ServerConfig,
    /// 数据库配置
    pub database: DatabaseConfig,
    /// 认证配置
    pub auth: AuthConfig,
    /// 连接池配置
    pub connection_pool: ConnectionPoolConfig,
    /// 健康检查配置
    pub health_check: HealthCheckConfig,
    /// API提供商配置
    pub api_providers: HashMap<String, ApiProviderConfig>,
}

/// 环境模式
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Environment {
    Development,
    Production,
    Testing,
}

impl FromStr for Environment {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "development" => Ok(Environment::Development),
            "production" => Ok(Environment::Production),
            "testing" => Ok(Environment::Testing),
            _ => Err(format!("Unknown environment: {}", s)),
        }
    }
}

/// 服务器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// 服务器主机地址
    pub host: String,
    /// 服务器端口
    pub port: u16,
    /// 日志级别
    pub log_level: String,
    /// CORS允许的域名
    pub cors_allowed_origins: Vec<String>,
}

/// 数据库配置 - SQLite版本
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// 数据库URL (sqlite:///path/to/db.sqlite3)
    pub url: String,
    /// 数据库文件路径
    pub path: PathBuf,
    /// 是否启用WAL模式
    pub enable_wal: bool,
    /// 是否启用外键约束
    pub enable_foreign_keys: bool,
    /// 最大连接数
    pub max_connections: u32,
}

/// 认证配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// JWT密钥
    pub jwt_secret: String,
    /// JWT过期时间(秒)
    pub jwt_expiration: u64,
    /// 默认管理员信息
    pub admin: AdminConfig,
}

/// 管理员配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminConfig {
    /// 管理员用户名
    pub username: String,
    /// 管理员邮箱
    pub email: String,
    /// 管理员初始密码
    pub password: String,
}

/// 连接池配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionPoolConfig {
    /// 最大连接数
    pub max_size: u32,
    /// 空闲超时(秒)
    pub idle_timeout: u64,
}

/// 健康检查配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    /// 检查间隔(秒)
    pub interval: u64,
    /// 超时时间(毫秒)
    pub timeout: u64,
}

/// API提供商配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiProviderConfig {
    /// API密钥
    pub api_key: String,
    /// 基础URL
    pub base_url: String,
}

impl AppConfig {
    /// 从环境变量加载配置
    pub fn from_env() -> Result<Self, config::ConfigError> {
        // 加载.env文件
        dotenv::dotenv().ok();

        // 解析环境
        let environment = env::var("APP_ENVIRONMENT")
            .unwrap_or_else(|_| "development".to_string())
            .parse::<Environment>()
            .unwrap_or(Environment::Development);

        // 服务器配置
        let host = env::var("APP_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let port = env::var("APP_PORT")
            .unwrap_or_else(|_| "3000".to_string())
            .parse::<u16>()
            .unwrap_or(3000);
        let log_level = env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
        let cors_allowed_origins = env::var("CORS_ALLOWED_ORIGINS")
            .unwrap_or_else(|_| "http://localhost:3000".to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();

        // SQLite数据库配置
        let db_path = env::var("SQLITE_PATH").unwrap_or_else(|_| "database.sqlite3".to_string());
        let db_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
            format!("sqlite://{}?mode=rwc", db_path)
        });
        let enable_wal = env::var("SQLITE_ENABLE_WAL")
            .unwrap_or_else(|_| "true".to_string())
            .parse::<bool>()
            .unwrap_or(true);
        let enable_foreign_keys = env::var("SQLITE_ENABLE_FOREIGN_KEYS")
            .unwrap_or_else(|_| "true".to_string())
            .parse::<bool>()
            .unwrap_or(true);
        let max_connections = env::var("SQLITE_MAX_CONNECTIONS")
            .unwrap_or_else(|_| "5".to_string())
            .parse::<u32>()
            .unwrap_or(5);

        // 认证配置
        let jwt_secret = env::var("JWT_SECRET").unwrap_or_else(|_| "default_secret_key".to_string());
        let jwt_expiration = env::var("JWT_EXPIRATION")
            .unwrap_or_else(|_| "86400".to_string())
            .parse::<u64>()
            .unwrap_or(86400);

        // 管理员配置
        let admin_username = env::var("ADMIN_USERNAME").unwrap_or_else(|_| "admin".to_string());
        let admin_email = env::var("ADMIN_EMAIL").unwrap_or_else(|_| "admin@example.com".to_string());
        let admin_password = env::var("ADMIN_PASSWORD").unwrap_or_else(|_| "changeme".to_string());

        // 连接池配置
        let pool_max_size = env::var("POOL_MAX_SIZE")
            .unwrap_or_else(|_| "10".to_string())
            .parse::<u32>()
            .unwrap_or(10);
        let pool_idle_timeout = env::var("POOL_IDLE_TIMEOUT")
            .unwrap_or_else(|_| "300".to_string())
            .parse::<u64>()
            .unwrap_or(300);

        // 健康检查配置
        let health_check_interval = env::var("HEALTH_CHECK_INTERVAL")
            .unwrap_or_else(|_| "60".to_string())
            .parse::<u64>()
            .unwrap_or(60);
        let health_check_timeout = env::var("HEALTH_CHECK_TIMEOUT")
            .unwrap_or_else(|_| "5000".to_string())
            .parse::<u64>()
            .unwrap_or(5000);

        // API提供商配置
        let mut api_providers = HashMap::new();
        
        // OpenAI
        if let (Ok(key), Ok(url)) = (env::var("OPENAI_API_KEY"), env::var("OPENAI_BASE_URL")) {
            api_providers.insert(
                "openai".to_string(),
                ApiProviderConfig {
                    api_key: key,
                    base_url: url,
                },
            );
        }
        
        // Anthropic
        if let (Ok(key), Ok(url)) = (env::var("ANTHROPIC_API_KEY"), env::var("ANTHROPIC_BASE_URL")) {
            api_providers.insert(
                "anthropic".to_string(),
                ApiProviderConfig {
                    api_key: key,
                    base_url: url,
                },
            );
        }
        
        // DeepSeek
        if let (Ok(key), Ok(url)) = (env::var("DEEPSEEK_API_KEY"), env::var("DEEPSEEK_BASE_URL")) {
            api_providers.insert(
                "deepseek".to_string(),
                ApiProviderConfig {
                    api_key: key,
                    base_url: url,
                },
            );
        }

        Ok(Self {
            environment,
            server: ServerConfig {
                host,
                port,
                log_level,
                cors_allowed_origins,
            },
            database: DatabaseConfig {
                url: db_url,
                path: PathBuf::from(db_path),
                enable_wal,
                enable_foreign_keys,
                max_connections,
            },
            auth: AuthConfig {
                jwt_secret,
                jwt_expiration,
                admin: AdminConfig {
                    username: admin_username,
                    email: admin_email,
                    password: admin_password,
                },
            },
            connection_pool: ConnectionPoolConfig {
                max_size: pool_max_size,
                idle_timeout: pool_idle_timeout,
            },
            health_check: HealthCheckConfig {
                interval: health_check_interval,
                timeout: health_check_timeout,
            },
            api_providers,
        })
    }

    /// 获取Socket地址
    pub fn socket_addr(&self) -> SocketAddr {
        format!("{}:{}", self.server.host, self.server.port)
            .parse()
            .expect("Failed to parse socket address")
    }

    /// 是否为开发环境
    pub fn is_development(&self) -> bool {
        self.environment == Environment::Development
    }

    /// 是否为生产环境
    pub fn is_production(&self) -> bool {
        self.environment == Environment::Production
    }

    /// 是否为测试环境
    pub fn is_testing(&self) -> bool {
        self.environment == Environment::Testing
    }
}
