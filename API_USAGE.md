# 如何调用 SiliconFlow API

本文档介绍了如何调用 SiliconFlow API 来查询账户余额和发起聊天对话。

## 1. 查询账户余额

查询余额是一个独立的API调用，使用专用的端点，并不会消耗对话token。

- **方法**: `GET`
- **URL**: `https://api.siliconflow.cn/v1/user/info`
- **请求头**:
    - `Authorization`: `Bearer <YOUR_API_KEY>`

### cURL 示例

请将 `<YOUR_API_KEY>` 替换为您的API密钥。

```bash
curl -X GET https://api.siliconflow.cn/v1/user/info \
-H "Authorization: Bearer <YOUR_API_KEY>"
```

## 2. 发起聊天对话

发起聊天对话会根据您的输入和模型的输出消耗token。

- **方法**: `POST`
- **URL**: `https://api.siliconflow.cn/v1/chat/completions`
- **请求头**:
    - `Authorization`: `Bearer <YOUR_API_KEY>`
    - `Content-Type`: `application/json`
- **请求体**:
    ```json
    {
        "model": "deepseek-ai/DeepSeek-V3",
        "messages": [
            {"role": "user", "content": "你好"}
        ]
    }
    ```

### cURL 示例

请将 `<YOUR_API_KEY>` 替换为您的API密钥。

```bash
curl -X POST https://api.siliconflow.cn/v1/chat/completions \
-H "Authorization: Bearer <YOUR_API_KEY>" \
-H "Content-Type: application/json" \
-d '{
    "model": "deepseek-ai/DeepSeek-V3",
    "messages": [
        {"role": "user", "content": "你好"}
    ]
}'
```
