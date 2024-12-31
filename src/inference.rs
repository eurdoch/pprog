use anyhow::Result;
use reqwest::{Client, StatusCode};
use serde::{Serialize, Deserialize};
use crate::{config::ProjectConfig, tooler::Tooler};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(tag = "type")]
pub enum ContentItem {
    #[serde(rename = "text")]
    Text {
        text: String,
    },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct Message {
    pub role: Role,
    pub content: Vec<ContentItem>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    System,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
pub struct Usage {
    pub input_tokens: i32,
    pub cache_creation_input_tokens: i32,
    pub cache_read_input_tokens: i32,
    pub output_tokens: i32,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ModelResponse {
    pub content: Vec<ContentItem>,
    pub id: String,
    pub model: String,
    pub role: String,
    #[serde(rename = "type")]
    pub message_type: String,
    pub stop_reason: String,
    pub stop_sequence: Option<String>,
    pub usage: Usage,
}

#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    id: String,
    model: String,
    choices: Vec<OpenAIChoice>,
    usage: OpenAIUsage,
}

#[derive(Debug, Deserialize)]
struct OpenAIChoice {
    message: OpenAIMessage,
    finish_reason: String,
}

#[derive(Debug, Deserialize)]
struct OpenAIMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenAIUsage {
    prompt_tokens: i32,
    completion_tokens: i32,
}

#[derive(Serialize)]
struct AnthropicRequest<'a> {
    model: &'a str,
    messages: Vec<Message>,
    max_tokens: u32,
    tools: serde_json::Value,
    system: String,
}

#[derive(Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<serde_json::Value>,
    max_tokens: Option<u32>,
    tools: Option<serde_json::Value>,
}

#[derive(Debug)]
pub enum InferenceError {
    NetworkError(String),
    ApiError(StatusCode, String),
    InvalidResponse(String),
    MissingApiKey(String),
    SerializationError(String),
}

impl std::fmt::Display for InferenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            InferenceError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            InferenceError::ApiError(status, msg) => write!(f, "API error ({}): {}", status, msg),
            InferenceError::InvalidResponse(msg) => write!(f, "Invalid response: {}", msg),
            InferenceError::MissingApiKey(msg) => write!(f, "Missing API key: {}", msg),
            InferenceError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
        }
    }
}

impl std::error::Error for InferenceError {}

pub struct Inference {
    model: String,
    client: Client,
    tooler: Tooler,
    base_url: String,
    api_key: String,
}

impl std::default::Default for Inference {
    fn default() -> Self {
        let config = match ProjectConfig::load() {
            Ok(config) => config,
            Err(_) => {
                ProjectConfig::default()
            }
        };
        Inference {
            model: config.model,
            client: Client::new(),
            tooler: Tooler::new(),
            base_url: if config.base_url.is_empty() { "https://api.anthropic.com/v1".to_string() } else { config.base_url.clone() },
            api_key: config.api_key,
        }
    }
}

impl Inference {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn query_model(&self, messages: Vec<Message>, system_message: Option<&str>) -> Result<ModelResponse, InferenceError> {
        if self.base_url.contains("anthropic.com") {
            self.query_anthropic(messages, system_message).await
        } else {
            self.query_openai(messages, system_message).await
        }
    }

    async fn query_anthropic(&self, messages: Vec<Message>, system_message: Option<&str>) -> Result<ModelResponse, InferenceError> {
        if self.api_key.is_empty() {
            return Err(InferenceError::MissingApiKey("API key not found in config".to_string()));
        }

        let system = system_message.unwrap_or("").to_string();

        let tools = self.tooler.get_tools_json()
            .map_err(|e| InferenceError::SerializationError(e.to_string()))?;

        let request = AnthropicRequest {
            model: &self.model,
            messages,
            max_tokens: 8096,
            tools,
            system,
        };

        let response = self.client
            .post(format!("{}/messages", self.base_url))
            .header("Content-Type", "application/json")
            .header("X-API-Key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&request)
            .send()
            .await
            .map_err(|e| InferenceError::NetworkError(e.to_string()))?;

        let status = response.status();
        let response_text = response.text().await
            .map_err(|e| InferenceError::NetworkError(e.to_string()))?;

        log::info!("Network response text: {}", response_text);

        if !status.is_success() {
            return Err(InferenceError::ApiError(status, response_text));
        }

        serde_json::from_str(&response_text)
            .map_err(|e| InferenceError::InvalidResponse(e.to_string()))
    }

    async fn query_openai(&self, mut messages: Vec<Message>, system_message: Option<&str>) -> Result<ModelResponse, InferenceError> {
        if self.api_key.is_empty() {
            return Err(InferenceError::MissingApiKey("API key not found in config".to_string()));
        }

        if let Some(sys_msg) = system_message {
            messages.insert(0, Message {
                role: Role::System,
                content: vec![ContentItem::Text { text: sys_msg.to_string() }],
            });
        }

        let openai_messages = messages.into_iter().map(|msg| {
            let content = msg.content.iter()
                .filter_map(|item| {
                    match item {
                        ContentItem::Text { text } => Some(text.clone()),
                        _ => None
                    }
                })
                .collect::<Vec<String>>()
                .join(" ");

            serde_json::json!({
                "role": match msg.role {
                    Role::User => "user",
                    Role::Assistant => "assistant",
                    Role::System => "system",
                },
                "content": content
            })
        }).collect();

        let tools = self.tooler.get_tools_json()
            .map_err(|e| InferenceError::SerializationError(e.to_string())).ok();

        let request = OpenAIRequest {
            model: self.model.clone(),
            messages: openai_messages,
            max_tokens: Some(8096),
            tools,
        };

        let response = self.client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await
            .map_err(|e| InferenceError::NetworkError(e.to_string()))?;

        let status = response.status();
        let response_text = response.text().await
            .map_err(|e| InferenceError::NetworkError(e.to_string()))?;

        log::info!("OpenAI API response text: {}", response_text);

        if !status.is_success() {
            return Err(InferenceError::ApiError(status, response_text));
        }

        // Parse the OpenAI response
        let openai_response: OpenAIResponse = serde_json::from_str(&response_text)
            .map_err(|e| InferenceError::InvalidResponse(format!("Failed to parse OpenAI response: {}", e)))?;

        if openai_response.choices.is_empty() {
            return Err(InferenceError::InvalidResponse("No choices in OpenAI response".to_string()));
        }

        // Convert OpenAI response to ModelResponse format
        Ok(ModelResponse {
            content: vec![ContentItem::Text {
                text: openai_response.choices[0].message.content.clone(),
            }],
            id: openai_response.id,
            model: openai_response.model,
            role: openai_response.choices[0].message.role.clone(),
            message_type: "text".to_string(),
            stop_reason: openai_response.choices[0].finish_reason.clone(),
            stop_sequence: None,
            usage: Usage {
                input_tokens: openai_response.usage.prompt_tokens,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                output_tokens: openai_response.usage.completion_tokens,
            },
        })
    }
}
