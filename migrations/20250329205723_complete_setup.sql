-- Add migration script here

-- ========== 1. 创建API提供商表 ==========

-- 创建API提供商表(如果不存在)
CREATE TABLE IF NOT EXISTS api_providers (
    id TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' || substr(lower(hex(randomblob(2))),2) || '-' || substr('89ab',abs(random() % 4) + 1, 1) || substr(lower(hex(randomblob(2))),2) || '-' || lower(hex(randomblob(6)))),
    name TEXT NOT NULL,
    provider_type TEXT NOT NULL,
    is_official INTEGER DEFAULT 0,
    base_url TEXT NOT NULL,
    api_key TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL DEFAULT 'Active',
    rate_limit INTEGER,
    balance REAL DEFAULT 0.0,
    last_balance_check TEXT,
    min_balance_threshold REAL DEFAULT 3.0,
    support_balance_check INTEGER DEFAULT 0,
    model_name TEXT NOT NULL,
    model_type TEXT NOT NULL DEFAULT 'ChatCompletion',
    model_version TEXT NOT NULL DEFAULT 'v3',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_api_providers_name ON api_providers (name);
CREATE UNIQUE INDEX IF NOT EXISTS idx_api_providers_api_key ON api_providers (api_key);

-- ========== 2. 插入API提供商数据 ==========

-- 添加SiliconFlow提供商
INSERT INTO api_providers (
    name,
    provider_type,
    is_official,
    base_url,
    api_key,
    balance,
    support_balance_check,
    model_name,
    model_type,
    model_version
) VALUES (
    'SiliconFlow',
    'DeepSeek',
    0,
    'https://api.siliconflow.cn/v1/chat/completions',
    'sk-pfkwspsjowmfxjgncpdtmwzujoiuisscssgjsncqmpdhmwfn',
    100.0,
    1,
    'deepseek-ai/DeepSeek-V3',
    'ChatCompletion',
    'v3'
);

-- 添加第二个SiliconFlow提供商
INSERT INTO api_providers (
    name,
    provider_type,
    is_official,
    base_url,
    api_key,
    balance,
    support_balance_check,
    model_name,
    model_type,
    model_version
) VALUES (
    'SiliconFlow-2',
    'DeepSeek',
    0,
    'https://api.siliconflow.cn/v1/chat/completions',
    'sk-ssltvmmhplawptonclytjbbhkcvgqhzjksocyjprbeqlcemz',
    100.0,
    1,
    'deepseek-ai/DeepSeek-V3',
    'ChatCompletion',
    'v3'
);
