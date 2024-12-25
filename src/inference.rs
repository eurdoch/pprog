use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;

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
pub struct Content {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

#[derive(Debug, Deserialize)]
pub struct Usage {
    pub input_tokens: i32,
    pub cache_creation_input_tokens: i32,
    pub cache_read_input_tokens: i32,
    pub output_tokens: i32,
}

pub async fn query_anthropic(prompt: &str, system_message: Option<&str>) -> Result<AnthropicResponse, reqwest::Error> {
    let client = Client::new();
    let api_key = env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY environment variable not set");
    let mut messages = vec![serde_json::json!({
        "role": "user",
        "content": prompt
    })];
    if let Some(system_message) = system_message {
        messages.insert(0, serde_json::json!({
            "role": "system",
            "content": system_message
        }));
    }
    let res = client
        .post("https://api.anthropic.com/v1/messages")
        .header("Content-Type", "application/json")
        .header("X-API-Key", api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&serde_json::json!({
            "model": "claude-3-5-sonnet-20241022",
            "messages": messages,
            "max_tokens": 1024
        }))
        .send()
        .await?
        .json()
        .await?;

    Ok(res)
}

