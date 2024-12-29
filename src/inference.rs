use anyhow::Result;
use reqwest::Client;
use serde::{Serialize, Deserialize};
use std::env;
use crate::{config::ProjectConfig, tooler::Tooler};

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct AnthropicResponse {
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
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
pub struct TextContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
pub struct ToolUseContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
pub struct ToolResultContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub tool_use_id: String,
    pub content: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Usage {
    pub input_tokens: i32,
    pub cache_creation_input_tokens: i32,
    pub cache_read_input_tokens: i32,
    pub output_tokens: i32,
}

#[derive(Serialize)]
struct AnthropicRequest<'a> {
    model: &'a str,
    messages: Vec<Message>,
    max_tokens: u32,
    tools: serde_json::Value,
    system: String,
}

pub struct Inference {
    model: String,
    client: Client,
    tooler: Tooler,
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
        }
    }
}

impl Inference {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn generate_response(&self, user_message: &str) -> Result<String, anyhow::Error> {
        // Create a message with text for querying
        let message = Message {
            role: Role::User,
            content: vec![ContentItem::Text { 
                text: user_message.to_string() 
            }]
        };

        // Use async query_anthropic method
        let anthropic_response = self.query_anthropic(vec![message], None).await?;
        
        // Extract the text from the first content item
        let response_text = anthropic_response.content
            .iter()
            .find_map(|item| {
                if let ContentItem::Text { text } = item {
                    Some(text.clone())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "No response found".to_string());

        Ok(response_text)
    }

    pub async fn query_anthropic(&self, messages: Vec<Message>, system_message: Option<&str>) -> Result<AnthropicResponse, anyhow::Error> {
        let api_key = env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY environment variable not set");
        let system = system_message.unwrap_or("").to_string();

        let tools = self.tooler.get_tools_json()?;

        let request = AnthropicRequest {
            model: &self.model,
            messages,
            max_tokens: 8096,
            tools,
            system,
        };

        let response = self.client
            .post("https://api.anthropic.com/v1/messages")
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

        let res: AnthropicResponse = serde_json::from_str(&response)?;
        Ok(res)
    }
}
