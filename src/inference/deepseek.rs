use std::collections::HashMap;
use reqwest::Client;
use serde::{Serialize, Deserialize};
use anyhow::Result;
use async_trait::async_trait;

use crate::chat::{Role, CommonMessage, ContentItem};
use crate::config::ProjectConfig;
use super::types::{InferenceError, ModelResponse};
use super::tools::{OpenAITool, OpenAIToolFunction, InputSchema, PropertySchema};
use super::inference::Inference;

#[derive(Debug, Serialize, Deserialize)]
pub struct DeepSeekModelResponse {
    pub choices: Vec<Choice>,
    pub created: i64,
    pub id: String,
    pub model: String,
    pub object: String,
    pub system_fingerprint: String,
    pub usage: Usage,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Choice {
    pub finish_reason: String,
    pub index: i32,
    pub logprobs: Option<serde_json::Value>,
    pub message: DeepSeekMessage,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeepSeekMessage {
    role: Role,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolCall {
    pub function: Function,
    pub id: String,
    pub index: i32,
    #[serde(rename = "type")]
    pub call_type: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Function {
    pub arguments: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Usage {
    pub completion_tokens: i32,
    pub prompt_cache_hit_tokens: i32,
    pub prompt_cache_miss_tokens: i32,
    pub prompt_tokens: i32,
    pub total_tokens: i32,
}

#[derive(Serialize)]
struct DeepSeekRequest {
    model: String,
    messages: Vec<DeepSeekMessage>,
    max_tokens: Option<u32>,
    tools: Option<serde_json::Value>,
}

pub struct DeepSeekInference {
    model: String,
    client: Client,
    api_url: String,
    api_key: String,
    max_output_tokens: u32,
}

#[async_trait]
impl Inference for DeepSeekInference {
    fn new() -> Self {
        let config = match ProjectConfig::load() {
            Ok(config) => config,
            Err(_) => ProjectConfig::default(),
        };
        
        DeepSeekInference {
            model: config.model,
            client: Client::new(),
            api_url: config.api_url,
            api_key: config.api_key,
            max_output_tokens: config.max_output_tokens,
        }
    }

    async fn query_model(
        &self,
        messages: Vec<CommonMessage>,
        system_message: Option<&str>
    ) -> Result<ModelResponse, InferenceError> {
        let deepseek_response = self.query_deepseek(messages, system_message).await?;
        let mut content: Vec<ContentItem> = Vec::new();
        if !deepseek_response.choices[0].message.content.is_empty() {
            content.push(ContentItem::Text { text: deepseek_response.choices[0].message.content.clone() });
        }
        if let Some(tool_calls) = deepseek_response.choices[0].message.tool_calls.clone() {
            for tool_call in tool_calls {
                let input = serde_json::from_str(&tool_call.function.arguments)?;
                content.push(
                    ContentItem::ToolUse {
                        id: tool_call.id,
                        name: tool_call.function.name,
                        input,
                    }
                )

            }
        }
        
        Ok(ModelResponse {
            model: deepseek_response.model,
            role: deepseek_response.choices[0].message.role.to_string(),
            message_type: "message".to_string(),
            stop_reason: deepseek_response.choices[0].finish_reason.clone(),
            stop_sequence: None,
            content,
        })
    }
}

impl DeepSeekInference {
    fn get_tools(&self) -> Vec<OpenAITool> {
        vec![
            self.read_file_tool(),
            self.write_file_tool(),
            self.execute_tool(),
            self.compile_check_tool(),
        ]
    }

    fn read_file_tool(&self) -> OpenAITool {
        OpenAITool {
            tool_type: "function".to_string(),
            function: OpenAIToolFunction {
                name: "read_file".to_string(),
                description: "Read file as string using path relative to root directory of project.".to_string(),
                parameters: InputSchema {
                    schema_type: "object".to_string(),
                    properties: {
                        let mut map = HashMap::new();
                        map.insert(
                            "path".to_string(),
                            PropertySchema {
                                property_type: "string".to_string(),
                                description: "The file path relative to the project root directory".to_string(),
                            },
                        );
                        map
                    },
                    required: vec!["path".to_string()],
                },
            },
        }
    }

    fn write_file_tool(&self) -> OpenAITool {
        OpenAITool {
            tool_type: "function".to_string(),
            function: OpenAIToolFunction {
                name: "write_file".to_string(),
                description: "Write string to file at path relative to root directory of project.".to_string(),
                parameters: InputSchema {
                    schema_type: "object".to_string(),
                    properties: {
                        let mut map = HashMap::new();
                        map.insert(
                            "path".to_string(),
                            PropertySchema {
                                property_type: "string".to_string(),
                                description: "The file path relative to the project root directory".to_string(),
                            },
                        );
                        map.insert(
                            "content".to_string(),
                            PropertySchema {
                                property_type: "string".to_string(),
                                description: "The content to write to the file".to_string(),
                            },
                        );
                        map
                    },
                    required: vec!["path".to_string(), "content".to_string()],
                },
            },
        }
    }

    fn execute_tool(&self) -> OpenAITool {
        OpenAITool {
            tool_type: "function".to_string(),
            function: OpenAIToolFunction {
                name: "execute".to_string(),
                description: "Execute bash statements as a single string..".to_string(),
                parameters: InputSchema {
                    schema_type: "object".to_string(),
                    properties: {
                        let mut map = HashMap::new();
                        map.insert(
                            "statement".to_string(),
                            PropertySchema {
                                property_type: "string".to_string(),
                                description: "The bash statement to be executed.".to_string(),
                            },
                        );
                        map
                    },
                    required: vec!["statement".to_string()],
                },
            },
        }
    }

    fn compile_check_tool(&self) -> OpenAITool {
        OpenAITool {
            tool_type: "function".to_string(),
            function: OpenAIToolFunction {
                name: "compile_check".to_string(),
                description: "Check if project compiles or runs without error.".to_string(),
                parameters: InputSchema {
                    schema_type: "object".to_string(),
                    properties: {
                        let mut map = HashMap::new();
                        map.insert(
                            "cmd".to_string(),
                            PropertySchema {
                                property_type: "string".to_string(),
                                description: "The command to check for compiler/interpreter errors.".to_string(),
                            },
                        );
                        map
                    },
                    required: vec!["cmd".to_string()],
                },
            },
        }
    }

    fn get_tools_json(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(self.get_tools())
    }

    async fn query_deepseek(
        &self,
        messages: Vec<CommonMessage>,
        system_message: Option<&str>
    ) -> Result<DeepSeekModelResponse, InferenceError> {
        if self.api_key.is_empty() {
            return Err(InferenceError::MissingApiKey("DeepSeek API key not found".to_string()));
        }

        let mut deepseek_messages: Vec<DeepSeekMessage> = messages.iter().map(|message| {
            let mut parsed_message: DeepSeekMessage = DeepSeekMessage { 
                role: Role::User,
                content: "".to_string(),
                tool_calls: None,
                tool_call_id: None,
            };

            for content_item in message.content.clone() {
                match content_item {
                    ContentItem::Text { text } => {
                        parsed_message.content = text;
                    },
                    ContentItem::ToolUse { id, name, input } => {
                        parsed_message.tool_calls = Some(vec![ToolCall {
                            function: Function {
                                arguments: input.to_string(),
                                name,
                            },
                            id,
                            index: 0,
                            call_type: "function".to_string(),
                        }]);
                    },
                    ContentItem::ToolResult { tool_use_id, content } => {
                        parsed_message.role = Role::Tool;
                        parsed_message.tool_call_id = Some(tool_use_id);
                        parsed_message.content = content;
                    }
                }
            }
            parsed_message
        }).collect();

        if let Some(sys_msg) = system_message {
            deepseek_messages.insert(0, DeepSeekMessage {
                role: Role::System,
                content: sys_msg.to_string(),
                tool_calls: None,
                tool_call_id: None,
            });
        }

        let tools = self.get_tools_json()
            .map_err(|e| InferenceError::SerializationError(e.to_string())).ok();

        let request = DeepSeekRequest {
            model: self.model.clone(),
            messages: deepseek_messages,
            max_tokens: Some(self.max_output_tokens),
            tools,
        };

        let response = self.client
            .post(format!("{}", self.api_url))
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await
            .map_err(|e| InferenceError::NetworkError(e.to_string()))?;
        let status = response.status();
        let response_text = response.text().await
            .map_err(|e| InferenceError::NetworkError(e.to_string()))?;

        //DEBUG
        //let response_json: Value = serde_json::from_str(&response_text).map_err(|e| println!("{}", e.to_string())).unwrap();
        //println!("{:#?}", response_json);

        if !status.is_success() {
            return Err(InferenceError::ApiError(status, response_text));
        }
        let deepseek_response: DeepSeekModelResponse = serde_json::from_str(&response_text)
            .map_err(|e| InferenceError::InvalidResponse(format!("Failed to parse DeepSeek response: {}", e)))?;
        if deepseek_response.choices.is_empty() {
            Err(InferenceError::InvalidResponse("No choices in DeepSeek response".to_string()))
        } else {
            Ok(deepseek_response)
        }
    }
}
