use axum::{
    extract::{Json, State, ConnectInfo},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{time::Duration, sync::Arc, net::SocketAddr};
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
use crate::models::api_usage::{ApiUsage, ApiCallStatus};
use uuid;
use chrono;

// 配置常量
const RETRY_DELAY: Duration = Duration::from_secs(1);        // 重试延迟

// OpenAI格式的消息
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Message {
    /// 消息角色（system/user/assistant）
    pub role: String,
    /// 消息内容
    pub content: String,
    /// 拒绝原因（Grok API 特有，可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
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

// 通用 API 请求格式（支持 DeepSeek、Grok 等）
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
struct ApiRequest {
    model: String,
    messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    temperature: f32,
    stream: bool,
}

// 通用 API 响应格式（支持 DeepSeek、Grok 等）
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
struct ApiResponse {
    id: String,
    object: String,
    created: u64,
    model: String,
    choices: Vec<Choice>,
    usage: Usage,
    // Grok API 特有字段（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    system_fingerprint: Option<String>,
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
    // Grok API 扩展字段（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt_tokens_details: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    completion_tokens_details: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_sources_used: Option<u32>,
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
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(request): Json<ChatCompletionRequest>,
) -> Response {
    let model_name = request.model.clone().unwrap_or_else(|| "DeepSeek-V3".to_string());
    let client_ip = addr.ip().to_string();

    info!(
        "收到聊天完成请求, 模型: {}, 消息数: {}, 流式请求: {}, 客户端IP: {}", 
        model_name,
        request.messages.len(),
        request.stream.unwrap_or(false),
        client_ip
    );

    // 根据请求中的 stream 参数决定使用哪种响应模式
    if request.stream.unwrap_or(false) {
        handle_stream_response(state, request, client_ip).await
    } else {
        handle_normal_response(state, request, client_ip).await.into_response()
    }
}

// 处理流式响应
async fn handle_stream_response(state: AppState, request: ChatCompletionRequest, client_ip: String) -> Response {
    use std::error::Error as StdError;
    
    let stream: Pin<Box<dyn Stream<Item = Result<Bytes, Box<dyn StdError + Send + Sync>>> + Send>> = Box::pin(async_stream::try_stream! {
        let model_name = request.model.clone().unwrap_or_else(|| "DeepSeek-V3".to_string());
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

        // 构建 API 请求
        let api_request = build_api_request(&request, &model_name, true);
        
        let messages_without_refusal: Vec<Message> = api_request.messages.iter().map(|m| Message {
            role: m.role.clone(),
            content: m.content.clone(),
            refusal: None, // 请求中不包含 refusal
        }).collect();

        info!("流式请求：准备发送请求\nURL: {}\n请求体: {}", 
            token_manager.provider.base_url,
            serde_json::to_string_pretty(&api_request).unwrap_or_default()
        );

        let client = Client::builder()
            .timeout(Duration::from_secs(300))  // 流式请求需要更长的超时时间
            .build()
            .map_err(|e| Box::new(e) as Box<dyn StdError + Send + Sync>)?;

        let response = match client
            .post(&token_manager.provider.base_url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", token_manager.provider.api_key))
            .json(&api_request)
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
        let mut latest_usage: Option<Usage> = None;  // 跟踪最新的usage信息
        
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(data) => {
                    chunk_count += 1;
                    let text = String::from_utf8_lossy(&data);
                    
                    // 检查是否包含usage信息
                    if text.contains("\"usage\"") {
                        // 处理带有data:前缀的流式响应格式
                        let json_text = if text.starts_with("data: ") {
                            text.trim_start_matches("data: ")
                                .trim_end_matches("\n\n")
                        } else {
                            &text
                        };
                        
                        // 尝试解析JSON获取usage信息
                        match serde_json::from_str::<serde_json::Value>(json_text) {
                            Ok(json) => {
                                if let Some(usage) = json.get("usage") {
                                    if let (Some(prompt), Some(completion), Some(total)) = (
                                        usage.get("prompt_tokens").and_then(|v| v.as_u64()),
                                        usage.get("completion_tokens").and_then(|v| v.as_u64()),
                                        usage.get("total_tokens").and_then(|v| v.as_u64())
                                    ) {
                                        latest_usage = Some(Usage {
                                            prompt_tokens: prompt as u32,
                                            completion_tokens: completion as u32,
                                            total_tokens: total as u32,
                                            prompt_tokens_details: None,
                                            completion_tokens_details: None,
                                            num_sources_used: None,
                                        });
                                        
                                        info!("流式请求：获取到usage信息：prompt={}, completion={}, total={}", 
                                            prompt, completion, total);
                                    }
                                }
                            },
                            Err(e) => {
                                info!("流式请求：解析JSON失败: {}, 原始文本: {}", e, json_text);
                            }
                        }
                    }
                    
                    info!("流式请求：接收到第 {} 个数据块\n内容: {}", 
                        chunk_count,
                        text
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
        
        // 请求结束后，记录usage信息
        if let Some(usage) = latest_usage {
            // 更新token使用情况
            token_manager.update_usage(usage.total_tokens).await;
            
            // 记录到数据库
            let _ = sqlx::query(
                r#"
                INSERT INTO api_usage (
                    id, provider_api_key, request_time, model, 
                    prompt_tokens, completion_tokens, total_tokens, 
                    status, client_ip, request_id
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#
            )
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(&token_manager.provider.api_key)
            .bind(chrono::Utc::now())
            .bind(&model_name)
            .bind(usage.prompt_tokens)
            .bind(usage.completion_tokens)
            .bind(usage.total_tokens)
            .bind("Success")
            .bind(&client_ip)
            .bind(None::<String>) // request_id
            .execute(&state.db)
            .await
            .map_err(|e| {
                error!("记录流式API使用情况失败: {}", e);
            });
            
            info!("流式请求：已记录usage信息：prompt={}, completion={}, total={}", 
                usage.prompt_tokens, usage.completion_tokens, usage.total_tokens);
        } else {
            // 没有usage信息，记录部分成功的请求
            let _ = sqlx::query(
                r#"
                INSERT INTO api_usage (
                    id, provider_api_key, request_time, model, 
                    prompt_tokens, completion_tokens, total_tokens, 
                    status, client_ip, request_id
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#
            )
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(&token_manager.provider.api_key)
            .bind(chrono::Utc::now())
            .bind(&model_name)
            .bind(0) // 没有usage信息时默认为0
            .bind(0)
            .bind(0)
            .bind(if chunk_count > 0 { "PartialSuccess" } else { "Error" })
            .bind(&client_ip)
            .bind(None::<String>)
            .execute(&state.db)
            .await
            .map_err(|e| {
                error!("记录流式API使用失败情况失败: {}", e);
            });
            
            info!("流式请求：未获取到usage信息，记录为{}状态", 
                if chunk_count > 0 { "PartialSuccess" } else { "Error" });
        }
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
    client_ip: String,
) -> Response {
    // 获取模型名称，直接使用前端传入的值
    let model_name = request.model.clone().unwrap_or_else(|| "DeepSeek-V3".to_string());
    
    // 构建 API 请求
    let api_request = build_api_request(&request, &model_name, request.stream.unwrap_or(false));

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

        // 调用 API
        match call_api(api_request.clone(), &token_manager.provider).await {
            Ok(response) => {
                let total_tokens = response.usage.total_tokens;
                // 更新使用情况
                token_manager.update_usage(total_tokens).await;
                
                // 记录API使用情况
                let _ = sqlx::query(
                    r#"
                    INSERT INTO api_usage (
                        id, provider_api_key, request_time, model, 
                        prompt_tokens, completion_tokens, total_tokens, 
                        status, client_ip, request_id
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    "#
                )
                .bind(uuid::Uuid::new_v4().to_string())
                .bind(&token_manager.provider.api_key)
                .bind(chrono::Utc::now())
                .bind(&response.model)
                .bind(response.usage.prompt_tokens)
                .bind(response.usage.completion_tokens)
                .bind(total_tokens)
                .bind("Success")
                .bind(&client_ip)
                .bind(None::<String>) // request_id
                .execute(&state.db)
                .await
                .map_err(|e| {
                    error!("记录API使用情况失败: {}", e);
                });
                
                info!(
                    "请求完成, 提供商: {}, 总tokens: {}", 
                    token_manager.provider.base_url,
                    total_tokens
                );

                // 直接转发原始响应，保持与 OpenAI 格式一致
                return Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_string(&response).unwrap()))
                    .unwrap();
            }
            Err(err) => {
                error!(
                    "使用token {} 调用API失败: {}, 策略: {}", 
                    token_manager.provider.api_key, err, strategy
                );
                
                // 记录失败的请求
                let _ = sqlx::query(
                    r#"
                    INSERT INTO api_usage (
                        id, provider_api_key, request_time, model, 
                        prompt_tokens, completion_tokens, total_tokens, 
                        status, client_ip, request_id
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    "#
                )
                .bind(uuid::Uuid::new_v4().to_string())
                .bind(&token_manager.provider.api_key)
                .bind(chrono::Utc::now())
                .bind(&model_name)
                .bind(0)
                .bind(0)
                .bind(0)
                .bind("Error")
                .bind(&client_ip)
                .bind(None::<String>) // request_id
                .execute(&state.db)
                .await
                .map_err(|e| {
                    error!("记录API失败使用情况失败: {}", e);
                });
                
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

// 构建 API 请求
fn build_api_request(request: &ChatCompletionRequest, model_name: &str, stream: bool) -> ApiRequest {
    ApiRequest {
        model: model_name.to_string(),
        messages: request.messages.iter().map(|m| Message {
            role: m.role.clone(),
            content: m.content.clone(),
            refusal: None, // 请求中不包含 refusal
        }).collect(),
        max_tokens: request.max_tokens.or(Some(1000)), // 总是包含 max_tokens，API 会忽略不需要的参数
        temperature: request.temperature.unwrap_or(0.7),
        stream,
    }
}

// 调用通用 API
async fn call_api(request: ApiRequest, provider: &ProviderInfo) -> Result<ApiResponse, String> {
    info!(
        "准备调用 API\nURL: {}\nAPI Key: {}\n请求体: {}", 
        provider.base_url,
        provider.api_key,
        serde_json::to_string_pretty(&request).unwrap_or_default()
    );

    let client = Client::builder()
        .timeout(Duration::from_secs(300))
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
                    match serde_json::from_str::<ApiResponse>(&response_text) {
                        Ok(api_response) => {
                            info!(
                                "请求成功\n模型: {}\n总tokens: {}\nprompt_tokens: {}\ncompletion_tokens: {}\n响应内容: {}", 
                                api_response.model,
                                api_response.usage.total_tokens,
                                api_response.usage.prompt_tokens,
                                api_response.usage.completion_tokens,
                                serde_json::to_string_pretty(&api_response.choices).unwrap_or_default()
                            );
                            return Ok(api_response)
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