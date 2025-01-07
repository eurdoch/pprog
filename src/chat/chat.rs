use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::inference::deepseek::{DeepSeekMessage, Function, ToolCall};

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
pub struct CommonMessage {
    pub role: Role,
    pub content: Vec<ContentItem>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    System,
    Developer, // because OpenAI just had to change the system name
    Tool, // Deepseek API uses tool role for tool results
}

pub fn convert_to_common_message(msg: &DeepSeekMessage) -> CommonMessage {
    let mut content = Vec::new();
    
    match msg {
        DeepSeekMessage::Regular { role, content: msg_content, tool_calls } => {
            // Handle tool calls if present
            if let Some(tool_calls) = tool_calls {
                for tool_call in tool_calls {
                    content.push(ContentItem::ToolUse {
                        id: tool_call.id.clone(),
                        name: tool_call.function.name.clone(),
                        input: serde_json::from_value(tool_call.function.arguments.clone())
                            .unwrap_or(serde_json::Value::Null),
                    });
                }
            }
            // Add text content if not empty
            if !msg_content.is_empty() {
                content.push(ContentItem::Text {
                    text: msg_content.clone(),
                });
            }
            CommonMessage {
                role: role.clone(),
                content,
            }
        },
        DeepSeekMessage::Tool { role, content: msg_content, tool_call_id } => {
            content.push(ContentItem::ToolResult {
                tool_use_id: tool_call_id.clone(),
                content: msg_content.clone(),
            });
            CommonMessage {
                role: role.clone(),
                content,
            }
        }
    }
}

pub fn convert_to_deepseek_message(msg: &CommonMessage) -> Result<DeepSeekMessage, anyhow::Error> {
    // Get the text content or tool result content and determine role
    let (content, tool_call_id) = msg.content.iter()
        .find_map(|item| match item {
            ContentItem::Text { text } => Some((text.clone(), None)),
            ContentItem::ToolResult { tool_use_id, content } => Some((content.clone(), Some(tool_use_id.clone()))),
            _ => None,
        })
        .unwrap_or_else(|| (String::new(), None));

    // If we have a tool_call_id, return a Tool message
    if let Some(id) = tool_call_id {
        return Ok(DeepSeekMessage::Tool {
            role: Role::Tool,
            content,
            tool_call_id: id,
        });
    }

    // Otherwise collect tool calls if they exist
    let tool_calls = {
        let calls: Result<Vec<_>, anyhow::Error> = msg.content.iter()
            .filter_map(|item| {
                if let ContentItem::ToolUse { id, name, input } = item {
                    match input.as_str() {
                        None => Some(Err(anyhow::anyhow!("Input could not be converted to string"))),
                        Some(input_string) => match serde_json::from_str(input_string) {
                            Ok(parsed_arguments) => Some(Ok(ToolCall {
                                id: id.clone(),
                                function: Function {
                                    name: name.clone(),
                                    arguments: parsed_arguments,
                                },
                                index: 0,
                                call_type: "function".to_string(),
                            })),
                            Err(e) => Some(Err(anyhow::anyhow!("Failed to parse JSON: {}", e)))
                        }
                    }
                } else {
                    None
                }
            })
            .collect();
        
        match calls {
            Ok(vec) => if vec.is_empty() { None } else { Some(vec) },
            Err(e) => return Err(e),
        }
    };
    
    Ok(DeepSeekMessage::Regular {
        role: msg.role.clone(),
        content,
        tool_calls,
    })
}

#[async_trait]
pub trait Chat: Send + Sync {
    /// Initialize a new chat instance
    /// This is async because AWS Bedrock init requires async
    async fn new() -> Self where Self: Sized;
    
    /// Handle an incoming message and return a response
    async fn handle_message(&mut self, message: &CommonMessage) -> Result<CommonMessage, anyhow::Error>;

    /// Get all messages in the conversation
    fn get_messages(&self) -> Vec<CommonMessage>;
    
    /// Clear the chat history
    fn clear(&mut self);
}
