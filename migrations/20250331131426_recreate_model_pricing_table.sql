-- 首先删除旧的表和触发器（如果存在）
DROP TRIGGER IF EXISTS model_pricing_update_trigger;
DROP TABLE IF EXISTS model_pricing_history;
DROP TABLE IF EXISTS model_pricing;

-- 创建新的模型定价表
CREATE TABLE model_pricing (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,          -- 提供商名称，如 OpenAI、Anthropic
    model TEXT NOT NULL,          -- 模型名称
    prompt_token_price REAL NOT NULL,  -- 输入token单价（每千token）
    completion_token_price REAL NOT NULL, -- 输出token单价（每千token）
    currency TEXT NOT NULL DEFAULT 'USD', -- 货币单位
    effective_date TIMESTAMP NOT NULL,    -- 价格生效日期
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    -- 确保同一提供商和模型在同一生效日期只有一个有效价格
    UNIQUE(name, model, effective_date)
);

-- 为提供商名称和模型创建索引，用于快速查询
CREATE INDEX idx_model_pricing_provider_model ON model_pricing(name, model);
CREATE INDEX idx_model_pricing_effective_date ON model_pricing(effective_date);

-- 创建历史价格表，用于保存价格变更历史
CREATE TABLE model_pricing_history (
    id TEXT PRIMARY KEY,
    original_id TEXT NOT NULL,    -- 关联到原始价格记录ID
    name TEXT NOT NULL,           -- 提供商名称
    model TEXT NOT NULL,          -- 模型名称
    prompt_token_price REAL NOT NULL,  -- 输入token单价
    completion_token_price REAL NOT NULL, -- 输出token单价
    currency TEXT NOT NULL,       -- 货币单位
    effective_date TIMESTAMP NOT NULL, -- 价格生效日期
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    archived_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP -- 归档时间
);

-- 创建触发器，当价格记录更新时，自动保存历史版本
CREATE TRIGGER model_pricing_update_trigger
AFTER UPDATE ON model_pricing
FOR EACH ROW
BEGIN
    INSERT INTO model_pricing_history (
        id, original_id, name, model, 
        prompt_token_price, completion_token_price, 
        currency, effective_date, created_at, updated_at
    )
    VALUES (
        hex(randomblob(16)), -- 生成随机UUID作为历史记录ID
        OLD.id, OLD.name, OLD.model, 
        OLD.prompt_token_price, OLD.completion_token_price,
        OLD.currency, OLD.effective_date, OLD.created_at, OLD.updated_at
    );
END;

-- 插入一些默认价格数据
INSERT INTO model_pricing (
    id, name, model, prompt_token_price, 
    completion_token_price, currency, effective_date, 
    created_at, updated_at
) VALUES 
-- OpenAI模型价格
(hex(randomblob(16)), 'OpenAI', 'gpt-3.5-turbo', 0.0015, 0.002, 'USD', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
(hex(randomblob(16)), 'OpenAI', 'gpt-4', 0.03, 0.06, 'USD', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
(hex(randomblob(16)), 'OpenAI', 'gpt-4-turbo', 0.01, 0.03, 'USD', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
(hex(randomblob(16)), 'OpenAI', 'gpt-4o', 0.01, 0.03, 'USD', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),

-- Anthropic模型价格
(hex(randomblob(16)), 'Anthropic', 'claude-3-haiku', 0.00025, 0.00125, 'USD', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
(hex(randomblob(16)), 'Anthropic', 'claude-3-sonnet', 0.003, 0.015, 'USD', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
(hex(randomblob(16)), 'Anthropic', 'claude-3-opus', 0.015, 0.075, 'USD', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),

-- DeepSeek模型价格
(hex(randomblob(16)), 'DeepSeek', 'DeepSeek-V3', 0.0005, 0.001, 'USD', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP); 