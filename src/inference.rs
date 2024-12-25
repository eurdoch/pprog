use anyhow::Result;
use reqwest::Client;
use serde::{Serialize, Deserialize};
use std::env;
use crate::tooler::Tooler;

#[derive(Debug, Deserialize)]
pub struct AnthropicResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub message_type: String,
    pub role: String,
    pub model: String,
    pub content: Vec<Content>,
    pub stop_reason: String,
    pub stop_sequence: Option<String>,
    pub usage: Usage,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum Content {
    #[serde(rename = "text")]
    Text(TextContent),
    #[serde(rename = "tool_use")]
    ToolUse(ToolUseContent),
}

#[derive(Debug, Deserialize)]
pub struct TextContent {
    pub text: String,
}

#[derive(Debug, Deserialize)]
pub struct ToolUseContent {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct Usage {
    pub input_tokens: i32,
    pub cache_creation_input_tokens: i32,
    pub cache_read_input_tokens: i32,
    pub output_tokens: i32,
}

#[derive(Serialize)]
struct AnthropicRequest<'a> {
    model: &'a str,
    messages: Vec<serde_json::Value>,
    max_tokens: u32,
    tools: serde_json::Value,
    system: String,
}

pub struct Inference {
    client: Client,
    tooler: Tooler,
}

impl Inference {
    pub fn new() -> Self {
        Inference {
            client: Client::new(),
            tooler: Tooler::new(),
        }
    }

    pub async fn query_anthropic(&self, prompt: &str, system_message: Option<&str>) -> Result<AnthropicResponse, anyhow::Error> {
        let api_key = env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY environment variable not set");
        let messages = vec![serde_json::json!({
            "role": "user",
            "content": prompt
        })];
        let system = system_message.unwrap_or("").to_string();

        let tools = self.tooler.get_tools_json()?;

        let request = AnthropicRequest {
            model: "claude-3-5-sonnet-20241022",
            messages,
            max_tokens: 8096,
            tools,
            system,
        };

        let res = self.client
            .post("https://api.anthropic.com/v1/messages")
            .header("Content-Type", "application/json")
            .header("X-API-Key", api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&request)
            .send()
            .await?
            .json()
            .await?;

        Ok(res)
    }

}

