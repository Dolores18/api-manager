use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{time::Duration, sync::Arc};
use tokio::sync::Mutex;
use tracing::{error, info};
use sqlx::SqlitePool;
use anyhow::Result;
use crate::routes::api::AppState;
use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use axum::body::Body;
use std::pin::Pin;
use crate::services::{ProviderInfo, TokenManager};
use crate::services::provider_pool::ProviderPoolState;
use utoipa::ToSchema;

// 配置常量
const RETRY_DELAY: Duration = Duration::from_secs(1);        // 重试延迟

// OpenAI格式的消息
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Message {
    /// 消息角色（system/user/assistant）
    pub role: String,
    /// 消息内容
    pub content: String,
}

// 请求格式
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChatCompletionRequest {
    /// 模型名称，可选，默认使用deepseek-ai/DeepSeek-V3
    pub model: Option<String>,
    /// 对话消息列表
    pub messages: Vec<Message>,
    /// 最大生成token数，可选，默认1024
    pub max_tokens: Option<u32>,
    /// 温度参数，可选，默认0.7
    pub temperature: Option<f32>,
    /// 是否使用流式响应，可选，默认false
    pub stream: Option<bool>,
}

// DeepSeek API请求格式
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
struct DeepSeekRequest {
    model: String,
    messages: Vec<Message>,
    max_tokens: u32,
    temperature: f32,
    stream: bool,
}

// DeepSeek API响应格式
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
struct DeepSeekResponse {
    id: String,
    object: String,
    created: u64,
    model: String,
    choices: Vec<Choice>,
    usage: Usage,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
struct Choice {
    index: u32,
    message: Message,
    finish_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

// 我们的API响应格式
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChatCompletionResponse {
    /// 使用的模型名称
    pub model: String,
    /// 生成的回复内容
    pub content: String,
    /// Token使用统计
    pub usage: Option<Usage>,
}

// API错误响应
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ErrorResponse {
    /// 错误信息
    pub error: String,
}

/// 处理聊天完成请求
#[utoipa::path(
    post,
    path = "/v1/chat/completions",
    request_body = ChatCompletionRequest,
    responses(
        (status = 200, description = "成功处理聊天请求", body = ChatCompletionResponse),
        (status = 503, description = "服务不可用", body = ErrorResponse),
    ),
    tag = "chat"
)]
pub async fn handle_chat_completion(
    State(state): State<AppState>,
    Json(request): Json<ChatCompletionRequest>,
) -> Response {
    let model_name = format!("deepseek-ai/{}", 
        request.model.as_deref().unwrap_or("DeepSeek-V3"));

    info!(
        "收到聊天完成请求, 模型: {}, 消息数: {}, 流式请求: {}", 
        model_name,
        request.messages.len(),
        request.stream.unwrap_or(false)
    );

    // 根据请求中的 stream 参数决定使用哪种响应模式
    if request.stream.unwrap_or(false) {
        handle_stream_response(state, request).await
    } else {
        handle_normal_response(state, request).await.into_response()
    }
}

// 处理流式响应
async fn handle_stream_response(state: AppState, request: ChatCompletionRequest) -> Response {
    use std::error::Error as StdError;
    
    let stream: Pin<Box<dyn Stream<Item = Result<Bytes, Box<dyn StdError + Send + Sync>>> + Send>> = Box::pin(async_stream::try_stream! {
        let model_name = format!("deepseek-ai/{}", 
            request.model.as_deref().unwrap_or("DeepSeek-V3"));
        let token_manager = match TokenManager::new(state.provider_pool.clone(), &model_name, "RoundRobin").await {
            Some(manager) => {
                info!("流式请求：选择提供商成功\nURL: {}\nAPI Key: {}", 
                    manager.provider.base_url,
                    manager.provider.api_key
                );
                manager
            },
            None => {
                error!("流式请求：无法获取可用的提供商");
                yield Bytes::from("data: {\"error\":\"无法获取可用的提供商\"}\n\n");
                return;
            }
        };

        // 构建 DeepSeek 请求
        let deepseek_request = DeepSeekRequest {
            model: model_name.clone(),
            messages: request.messages,
            max_tokens: request.max_tokens.unwrap_or(1024),
            temperature: request.temperature.unwrap_or(0.7),
            stream: true,
        };

        info!("流式请求：准备发送请求\nURL: {}\n请求体: {}", 
            token_manager.provider.base_url,
            serde_json::to_string_pretty(&deepseek_request).unwrap_or_default()
        );

        let client = Client::builder()
            .timeout(Duration::from_secs(300))  // 流式请求需要更长的超时时间
            .build()
            .map_err(|e| Box::new(e) as Box<dyn StdError + Send + Sync>)?;

        let response = match client
            .post(&token_manager.provider.base_url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", token_manager.provider.api_key))
            .json(&deepseek_request)
            .send()
            .await {
                Ok(res) => {
                    if !res.status().is_success() {
                        error!("流式请求：API调用失败\n状态码: {}\nURL: {}", 
                            res.status(), token_manager.provider.base_url
                        );
                        yield Bytes::from(format!("data: {{\"error\":\"API调用失败，状态码: {}\"}}\n\n", res.status()));
                        return;
                    }
                    info!("流式请求：连接建立成功\n状态码: {}", res.status());
                    res
                },
                Err(e) => {
                    error!("流式请求：发送请求失败\n错误: {}\nURL: {}", 
                        e, token_manager.provider.base_url
                    );
                    yield Bytes::from(format!("data: {{\"error\":\"请求失败: {}\"}}\n\n", e));
                    return;
                }
            };

        info!("流式请求：开始接收数据流");
        let mut stream = response.bytes_stream();
        let mut chunk_count = 0;
        
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(data) => {
                    chunk_count += 1;
                    info!("流式请求：接收到第 {} 个数据块\n内容: {}", 
                        chunk_count,
                        String::from_utf8_lossy(&data)
                    );
                    yield data;
                },
                Err(e) => {
                    let err: Box<dyn StdError + Send + Sync> = Box::new(e);
                    error!("流式请求：接收数据流错误\n错误: {}\n已接收块数: {}", err, chunk_count);
                    yield Bytes::from(format!("data: {{\"error\":\"接收数据流错误: {}\"}}\n\n", err));
                    return;
                }
            }
        }
        info!("流式请求：数据流接收完成，共接收 {} 个数据块", chunk_count);
    });

    Response::builder()
        .header("Content-Type", "text/event-stream")
        .header("Cache-Control", "no-cache")
        .header("Connection", "keep-alive")
        .body(Body::from_stream(stream))
        .unwrap()
}

// 处理普通响应
async fn handle_normal_response(
    state: AppState,
    request: ChatCompletionRequest,
) -> Response {
    // 获取模型名称并添加前缀
    let model_name = format!("deepseek-ai/{}", 
        request.model.as_deref().unwrap_or("DeepSeek-V3"));
    
    // 构建 deepseek 请求
    let deepseek_request = DeepSeekRequest {
        model: model_name.clone(),
        messages: request.messages,
        max_tokens: request.max_tokens.unwrap_or(1000),
        temperature: request.temperature.unwrap_or(0.7),
        stream: request.stream.unwrap_or(false),
    };

    // 尝试不同的token
    let mut last_error = None;
    let strategies = ["RoundRobin", "LeastConnections", "LeastTokens"];
    
    for strategy in strategies.iter() {
        info!("尝试使用 {} 策略选择提供商", strategy);
        
        // 获取token管理器
        let token_manager = match TokenManager::new(state.provider_pool.clone(), &model_name, strategy).await {
            Some(manager) => {
                info!(
                    "选择提供商成功, URL: {}, 策略: {}", 
                    manager.provider.base_url, strategy
                );
                manager
            },
            None => {
                info!("使用 {} 策略无法获取可用提供商，尝试下一个策略", strategy);
                continue
            },
        };

        // 调用deepseek API
        match call_deepseek_api(deepseek_request.clone(), &token_manager.provider).await {
            Ok(response) => {
                let total_tokens = response.usage.total_tokens;
                // 更新使用情况
                token_manager.update_usage(total_tokens).await;
                
                let chat_response = ChatCompletionResponse {
                    model: response.model,
                    content: response.choices.get(0)
                        .map(|choice| choice.message.content.clone())
                        .unwrap_or_default(),
                    usage: Some(response.usage),
                };

                info!(
                    "请求完成, 提供商: {}, 总tokens: {}", 
                    token_manager.provider.base_url,
                    total_tokens
                );

                return Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_string(&chat_response).unwrap()))
                    .unwrap();
            }
            Err(err) => {
                error!(
                    "使用token {} 调用API失败: {}, 策略: {}", 
                    token_manager.provider.api_key, err, strategy
                );
                last_error = Some(err);
                // 继续尝试下一个策略
            }
        }
    }

    // 所有token都尝试失败
    let error_message = format!("所有可用的API提供商都失败了。最后的错误: {}", 
        last_error.unwrap_or_else(|| "未知错误".to_string()));
    error!("{}", error_message);
    
    Response::builder()
        .status(StatusCode::SERVICE_UNAVAILABLE)
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&ErrorResponse { error: error_message }).unwrap()))
        .unwrap()
}

// 调用DeepSeek API
async fn call_deepseek_api(request: DeepSeekRequest, provider: &ProviderInfo) -> Result<DeepSeekResponse, String> {
    info!(
        "准备调用DeepSeek API\nURL: {}\nAPI Key: {}\n请求体: {}", 
        provider.base_url,
        provider.api_key,
        serde_json::to_string_pretty(&request).unwrap_or_default()
    );

    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .pool_max_idle_per_host(provider.max_connections as usize)
        .pool_idle_timeout(Duration::from_millis(provider.idle_timeout_ms as u64))
        .build()
        .map_err(|e| format!("创建HTTP客户端失败: {}", e))?;

    let headers = reqwest::header::HeaderMap::from_iter([
        (
            reqwest::header::CONTENT_TYPE,
            reqwest::header::HeaderValue::from_static("application/json"),
        ),
        (
            reqwest::header::AUTHORIZATION,
            reqwest::header::HeaderValue::from_str(&format!("Bearer {}", provider.api_key))
                .map_err(|e| format!("无效的API密钥: {}", e))?,
        ),
    ]);

    // 使用提供商的重试配置
    for attempt in 0..provider.retry_attempts {
        info!(
            "发送请求到 {}, 尝试次数: {}/{}", 
            provider.base_url, attempt + 1, provider.retry_attempts
        );

        match client
            .post(&provider.base_url)
            .headers(headers.clone())
            .json(&request)
            .send()
            .await
        {
            Ok(response) => {
                let status = response.status();
                if status.is_success() {
                    // 先获取原始响应文本
                    let response_text = response.text().await.map_err(|e| format!("读取响应失败: {}", e))?;
                    info!("收到原始响应: {}", response_text);
                    
                    // 解析响应
                    match serde_json::from_str::<DeepSeekResponse>(&response_text) {
                        Ok(deepseek_response) => {
                            info!(
                                "请求成功\n模型: {}\n总tokens: {}\nprompt_tokens: {}\ncompletion_tokens: {}\n响应内容: {}", 
                                deepseek_response.model,
                                deepseek_response.usage.total_tokens,
                                deepseek_response.usage.prompt_tokens,
                                deepseek_response.usage.completion_tokens,
                                serde_json::to_string_pretty(&deepseek_response.choices).unwrap_or_default()
                            );
                            return Ok(deepseek_response)
                        },
                        Err(e) => {
                            error!("解析响应失败: {}\n原始响应: {}", e, response_text);
                            return Err(format!("解析响应失败: {}", e))
                        },
                    }
                } else {
                    let error_text = response.text().await.unwrap_or_default();
                    error!(
                        "API调用失败\n状态码: {}\nURL: {}\n错误响应: {}", 
                        status, provider.base_url, error_text
                    );
                    if attempt < provider.retry_attempts - 1 {
                        info!("请求失败，正在重试({}/{})", attempt + 1, provider.retry_attempts);
                        tokio::time::sleep(RETRY_DELAY).await;
                        continue;
                    }
                    return Err(format!("API调用失败，状态码: {}，错误: {}", status, error_text));
                }
            }
            Err(e) => {
                if e.is_timeout() && attempt < provider.retry_attempts - 1 {
                    info!("请求超时，正在重试({}/{})", attempt + 1, provider.retry_attempts);
                    tokio::time::sleep(RETRY_DELAY).await;
                    continue;
                }
                error!("请求发送失败: {}", e);
                return Err(format!("请求失败: {}", e));
            }
        }
    }

    error!(
        "达到最大重试次数({}), URL: {}", 
        provider.retry_attempts, provider.base_url
    );
    Err(format!("达到最大重试次数({})，请求失败", provider.retry_attempts))
} 