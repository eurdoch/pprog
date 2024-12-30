use anyhow::Result;
use reqwest::Client;
use serde::{Serialize, Deserialize};
use std::env;
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

#[derive(Serialize)]
struct ModelRequest<'a> {
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

pub struct Inference {
    model: String,
    client: Client,
    tooler: Tooler,
    base_url: String,
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
        }
    }
}

impl Inference {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn query_anthropic(&self, mut messages: Vec<Message>, system_message: Option<&str>) -> Result<ModelResponse, anyhow::Error> {
        let api_key = env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY environment variable not set");
        let system = system_message.unwrap_or("").to_string();

        let tools = self.tooler.get_tools_json()?;

        // If a system message is provided, insert it at the beginning of the messages
        if let Some(sys_msg) = system_message {
            messages.insert(0, Message {
                role: Role::System,
                content: vec![ContentItem::Text { text: sys_msg.to_string() }],
            });
        }

        let request = ModelRequest {
            model: &self.model,
            messages,
            max_tokens: 8096,
            tools,
            system,
        };

        let response = self.client
            .post(format!("{}/messages", self.base_url))
            .header("Content-Type", "application/json")
            .header("X-API-Key", api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&request)
            .send()
            .await?
            .text()
            .await?;

        // TODO for errors add different type that can be returned to chat display error
        log::info!("Network response text: {}", response);

        let res: ModelResponse = serde_json::from_str(&response)?;
        Ok(res)
    }

    pub async fn query_openai(&self, mut messages: Vec<Message>, system_message: Option<&str>) -> Result<ModelResponse, anyhow::Error> {
        let api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY environment variable not set");

        // If a system message is provided, insert it at the beginning of the messages
        if let Some(sys_msg) = system_message {
            messages.insert(0, Message {
                role: Role::System,
                content: vec![ContentItem::Text { text: sys_msg.to_string() }],
            });
        }

        let openai_messages = messages.into_iter().map(|msg| {
            // Convert content to a single text string for OpenAI
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

        let tools = self.tooler.get_tools_json().ok();

        let request = OpenAIRequest {
            model: self.model.clone(),
            messages: openai_messages,
            max_tokens: Some(8096),
            tools,
        };

        let response = self.client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&request)
            .send()
            .await?
            .text()
            .await?;

        log::info!("OpenAI API response text: {}", response);

        // NOTE: You might need to adjust this to match OpenAI's response structure
        let res: ModelResponse = serde_json::from_str(&response)?;
        Ok(res)
    }
}
