use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub mod types {
    use serde::{Deserialize, Serialize};

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
        Tool,      // Deepseek API uses tool role for tool results
    }
}

pub mod convert {
    use super::types::{CommonMessage, ContentItem, Role};
    use crate::chat::gemini_chat::{
        GeminiContent, GeminiFunctionCall, GeminiFunctionResponse, GeminiFunctionResponseData,
        GeminiMessage, GeminiPart,
    };
    use crate::inference::deepseek::{DeepSeekMessage, Function, ToolCall};

    pub fn convert_gemini_to_common(msg: &GeminiMessage) -> Result<CommonMessage, anyhow::Error> {
        match msg {
            GeminiMessage::Request { contents, .. } => {
                let mut content = Vec::new();

                // Process all content items
                for content_item in contents {
                    for part in &content_item.parts {
                        if let Some(text) = &part.text {
                            content.push(ContentItem::Text {
                                text: text.clone(),
                            });
                        }
                        if let Some(function_call) = &part.function_call {
                            content.push(ContentItem::ToolUse {
                                id: "1".to_string(), // Gemini doesn't provide IDs, using default
                                name: function_call.name.clone(),
                                input: function_call.args.clone(),
                            });
                        }
                        if let Some(function_response) = &part.function_response {
                            content.push(ContentItem::ToolResult {
                                tool_use_id: "1".to_string(), // Gemini doesn't provide IDs, using default
                                content: serde_json::to_string(&function_response.response.content)?,
                            });
                        }
                    }
                }

                Ok(CommonMessage {
                    role: match contents.last().map(|c| c.role.as_str()).unwrap_or("user") {
                        "user" => Role::User,
                        "assistant" => Role::Assistant,
                        "system" => Role::System,
                        "model" => Role::Assistant,
                        "tool" => Role::Tool,
                        r => return Err(anyhow::anyhow!("Unknown role: {}", r)),
                    },
                    content,
                })
            }
            GeminiMessage::Response { candidates, .. } => {
                if candidates.is_empty() {
                    return Err(anyhow::anyhow!("No candidates in response"));
                }

                let candidate = &candidates[0];
                let mut content = Vec::new();

                for part in &candidate.content.parts {
                    if let Some(text) = &part.text {
                        content.push(ContentItem::Text {
                            text: text.clone(),
                        });
                    }
                    if let Some(function_call) = &part.function_call {
                        content.push(ContentItem::ToolUse {
                            id: "1".to_string(), // Gemini doesn't provide IDs, using default
                            name: function_call.name.clone(),
                            input: function_call.args.clone(),
                        });
                    }
                    if let Some(function_response) = &part.function_response {
                        content.push(ContentItem::ToolResult {
                            tool_use_id: "1".to_string(), // Gemini doesn't provide IDs, using default
                            content: serde_json::to_string(&function_response.response.content)?,
                        });
                    }
                }

                Ok(CommonMessage {
                    role: Role::Assistant,
                    content,
                })
            }
        }
    }

    pub fn convert_common_to_gemini(msg: &CommonMessage) -> Result<GeminiMessage, anyhow::Error> {
        let mut parts = Vec::new();

        for item in &msg.content {
            match item {
                ContentItem::Text { text } => {
                    parts.push(GeminiPart {
                        text: Some(text.clone()),
                        function_call: None,
                        function_response: None,
                    });
                }
                ContentItem::ToolUse { name, input, .. } => {
                    parts.push(GeminiPart {
                        text: None,
                        function_call: Some(GeminiFunctionCall {
                            name: name.clone(),
                            args: input.clone(),
                        }),
                        function_response: None,
                    });
                }
                ContentItem::ToolResult {
                    content: result_content,
                    ..
                } => {
                    parts.push(GeminiPart {
                        text: None,
                        function_call: None,
                        function_response: Some(GeminiFunctionResponse {
                            name: "tool_result".to_string(), // Generic name since we don't track it
                            response: GeminiFunctionResponseData {
                                name: "tool_result".to_string(),
                                content: serde_json::from_str(result_content)?,
                            },
                        }),
                    });
                }
            }
        }

        let content = GeminiContent {
            role: match msg.role {
                Role::User => "user".to_string(),
                Role::Assistant => "assistant".to_string(),
                Role::System => "system".to_string(),
                Role::Developer => "system".to_string(),
                Role::Tool => "tool".to_string(),
            },
            parts,
        };

        Ok(GeminiMessage::Request {
            contents: vec![content],
            tools: None,     // Tool declarations should be handled at inference layer
            tool_config: None, // Tool config should be handled at inference layer
        })
    }

    pub fn convert_to_common_message(msg: &DeepSeekMessage) -> CommonMessage {
        let mut content = Vec::new();

        match msg {
            DeepSeekMessage::Regular {
                role,
                content: msg_content,
                tool_calls,
            } => {
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
            }
            DeepSeekMessage::Tool {
                role,
                content: msg_content,
                tool_call_id,
            } => {
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
        let (content, tool_call_id) = msg
            .content
            .iter()
            .find_map(|item| match item {
                ContentItem::Text { text } => Some((text.clone(), None)),
                ContentItem::ToolResult {
                    tool_use_id,
                    content,
                } => Some((content.clone(), Some(tool_use_id.clone()))),
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
            let calls: Result<Vec<_>, anyhow::Error> = msg
                .content
                .iter()
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
                                Err(e) => Some(Err(anyhow::anyhow!(
                                    "Failed to parse JSON: {}",
                                    e
                                ))),
                            },
                        }
                    } else {
                        None
                    }
                })
                .collect();

            match calls {
                Ok(vec) => {
                    if vec.is_empty() {
                        None
                    } else {
                        Some(vec)
                    }
                }
                Err(e) => return Err(e),
            }
        };

        Ok(DeepSeekMessage::Regular {
            role: msg.role.clone(),
            content,
            tool_calls,
        })
    }
}

#[async_trait]
pub trait Chat: Send + Sync {
    /// Initialize a new chat instance
    /// This is async because AWS Bedrock init requires async
    async fn new() -> Self
    where
        Self: Sized;

    /// Handle an incoming message and return a response
    async fn handle_message(
        &mut self,
        message: &types::CommonMessage,
    ) -> Result<types::CommonMessage, anyhow::Error>;

    /// Get all messages in the conversation
    fn get_messages(&self) -> Vec<types::CommonMessage>;

    /// Clear the chat history
    fn clear(&mut self);
}

pub use convert::*;
pub use types::*;