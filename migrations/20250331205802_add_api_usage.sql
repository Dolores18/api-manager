-- Add migration script here

-- 创建API使用量记录表
CREATE TABLE IF NOT EXISTS api_usage (
    id TEXT PRIMARY KEY,
    provider_api_key TEXT NOT NULL,
    request_time TEXT NOT NULL,
    model TEXT NOT NULL,
    prompt_tokens INTEGER NOT NULL DEFAULT 0,
    completion_tokens INTEGER NOT NULL DEFAULT 0,
    total_tokens INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'Success',
    client_ip TEXT,
    request_id TEXT,
    FOREIGN KEY (provider_api_key) REFERENCES api_providers(api_key)
);

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_api_usage_provider ON api_usage (provider_api_key);
CREATE INDEX IF NOT EXISTS idx_api_usage_model ON api_usage (model);
CREATE INDEX IF NOT EXISTS idx_api_usage_request_time ON api_usage (request_time);
CREATE INDEX IF NOT EXISTS idx_api_usage_status ON api_usage (status); 