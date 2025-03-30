use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// 用户角色
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum UserRole {
    /// 管理员
    Admin,
    /// 普通用户
    User,
    /// 只读用户
    ReadOnly,
    /// API消费者
    ApiConsumer,
}

/// 用户模型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    /// 用户ID
    pub id: String,
    /// 用户名
    pub username: String,
    /// 电子邮件
    pub email: String,
    /// 密码哈希(存储时应加密)
    #[serde(skip_serializing)]
    pub password_hash: String,
    /// 用户角色
    pub roles: HashSet<UserRole>,
    /// API密钥
    pub api_keys: Vec<String>,
    /// 是否启用
    pub is_active: bool,
    /// 最后登录时间
    pub last_login: Option<chrono::DateTime<chrono::Utc>>,
    /// 创建时间
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// 更新时间
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl User {
    /// 创建新用户
    pub fn new(
        id: String,
        username: String,
        email: String,
        password_hash: String,
        roles: HashSet<UserRole>,
    ) -> Self {
        let now = chrono::Utc::now();
        Self {
            id,
            username,
            email,
            password_hash,
            roles,
            api_keys: Vec::new(),
            is_active: true,
            last_login: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// 添加角色
    pub fn add_role(&mut self, role: UserRole) {
        self.roles.insert(role);
        self.updated_at = chrono::Utc::now();
    }

    /// 删除角色
    pub fn remove_role(&mut self, role: &UserRole) {
        self.roles.remove(role);
        self.updated_at = chrono::Utc::now();
    }

    /// 添加API密钥
    pub fn add_api_key(&mut self, key: String) {
        self.api_keys.push(key);
        self.updated_at = chrono::Utc::now();
    }

    /// 删除API密钥
    pub fn remove_api_key(&mut self, key: &str) {
        self.api_keys.retain(|k| k != key);
        self.updated_at = chrono::Utc::now();
    }

    /// 更新登录时间
    pub fn update_login(&mut self) {
        self.last_login = Some(chrono::Utc::now());
        self.updated_at = chrono::Utc::now();
    }

    /// 禁用用户
    pub fn deactivate(&mut self) {
        self.is_active = false;
        self.updated_at = chrono::Utc::now();
    }

    /// 启用用户
    pub fn activate(&mut self) {
        self.is_active = true;
        self.updated_at = chrono::Utc::now();
    }

    /// 检查是否是管理员
    pub fn is_admin(&self) -> bool {
        self.roles.contains(&UserRole::Admin)
    }

    /// 检查是否有某角色
    pub fn has_role(&self, role: &UserRole) -> bool {
        self.roles.contains(role)
    }
}
