use anyhow::Result;
use reqwest::Client;
use serde::{Serialize, Deserialize};
use std::env;
use crate::tooler::Tooler;

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
#[serde(untagged)] // Important! This allows matching based on fields present
pub enum ContentItem {
    Text {
        #[serde(rename = "type")]
        content_type: String,
        text: String,
    },
    ToolUse {
        #[serde(rename = "type")]
        content_type: String,
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        #[serde(rename = "type")]
        content_type: String,
        tool_use_id: String,
        content: String,
    },
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Items(Vec<ContentItem>),
}

// Iterator struct to hold temporary content
pub struct MessageContentIter<'a> {
    // We need to hold both the temporary item and the original iterator
    temp_text_item: Option<ContentItem>,
    items_iter: Option<std::slice::Iter<'a, ContentItem>>,
}

impl MessageContent {
    pub fn iter(&mut self) -> impl Iterator<Item = &ContentItem> {
        match self {
            MessageContent::Text(text) => {
                // Convert to Items variant
                *self = MessageContent::Items(vec![ContentItem::Text {
                    content_type: "text".to_string(),
                    text: text.clone(),
                }]);
                
                match self {
                    MessageContent::Items(items) => items.iter(),
                    _ => unreachable!(),
                }
            }
            MessageContent::Items(items) => items.iter(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct Message {
    pub role: Role,
    pub content: MessageContent,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

impl PartialEq<&str> for Role {
    fn eq(&self, other: &&str) -> bool {
        match self {
            Role::User => other.eq_ignore_ascii_case(&"user"),
            Role::Assistant => other.eq_ignore_ascii_case(&"assistant"),
        }
    }
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
    // TODO this will change eventually to be String | Content
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

    pub async fn query_anthropic(&self, messages: Vec<Message>, system_message: Option<&str>) -> Result<AnthropicResponse, anyhow::Error> {
        let api_key = env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY environment variable not set");
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

