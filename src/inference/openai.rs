use std::collections::HashMap;
use reqwest::Client;
use serde::{Serialize, Deserialize};
use anyhow::Result;
use serde_json::Value;
use async_trait::async_trait;

use crate::chat::{CommonMessage, ContentItem, Role};
use crate::config::ProjectConfig;
use super::types::{InferenceError, ModelResponse};
use super::tools::{OpenAITool, OpenAIToolFunction, InputSchema, PropertySchema};
use super::inference::Inference;

#[derive(Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<serde_json::Value>,
    max_tokens: Option<u32>,
    tools: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    model: String,
    choices: Vec<OpenAIChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAIChoice {
    message: OpenAIMessage,
    finish_reason: String,
}

fn deserialize_content<'de, D>(deserializer: D) -> Result<Vec<ContentItem>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum ContentWrapper {
        String(String),
        Null,
        Array(Vec<ContentItem>),
    }

    let wrapper = ContentWrapper::deserialize(deserializer)?;
    match wrapper {
        ContentWrapper::String(s) => Ok(vec![ContentItem::Text { text: s }]),
        ContentWrapper::Null => Ok(vec![]),
        ContentWrapper::Array(v) => Ok(v),
    }
}

#[derive(Debug, Deserialize)]
struct OpenAIMessage {
    role: String,
    #[serde(deserialize_with = "deserialize_content")]
    content: Vec<ContentItem>,
    #[serde(default)]
    tool_calls: Option<Vec<OpenAIToolCall>>,
}

#[derive(Debug, Deserialize)]
struct OpenAIToolCall {
    id: String,
    #[serde(rename = "type")]
    call_type: String,
    function: OpenAIFunctionCall,
}

#[derive(Debug, Deserialize)]
struct OpenAIFunctionCall {
    name: String,
    arguments: String,
}

pub struct OpenAIToolProvider {
    tools: Vec<OpenAITool>,
}

impl OpenAIToolProvider {
    pub fn new() -> Self {
        Self {
            tools: vec![
                Self::read_file_tool(),
                Self::write_file_tool(),
                Self::execute_tool(),
                Self::compile_check_tool(),
            ],
        }
    }

    pub fn get_tools_json(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(&self.tools)
    }

    fn read_file_tool() -> OpenAITool {
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

    fn write_file_tool() -> OpenAITool {
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

    fn execute_tool() -> OpenAITool {
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

    fn compile_check_tool() -> OpenAITool {
        OpenAITool {
            tool_type: "function".to_string(),
            function: OpenAIToolFunction {
                name: "compile_check".to_string(),
                description: "Check if project compiles or runs without error.".to_string(),
                parameters: InputSchema {
                    schema_type: "object".to_string(),
                    properties: HashMap::new(),
                    required: vec![],
                },
            },
        }
    }
}

pub struct OpenAIInference {
    model: String,
    client: Client,
    api_url: String,
    api_key: String,
    max_output_tokens: u32,
    tool_provider: OpenAIToolProvider,
}

impl std::default::Default for OpenAIInference {
    fn default() -> Self {
        let config = ProjectConfig::load().unwrap_or_default();
        
        OpenAIInference {
            model: config.model,
            client: Client::new(),
            api_url: config.api_url,
            api_key: config.api_key,
            max_output_tokens: config.max_output_tokens,
            tool_provider: OpenAIToolProvider::new(),
        }
    }
}

#[async_trait]
impl Inference for OpenAIInference {
    fn new() -> Self {
        Self::default()
    }

    async fn query_model(&self, mut messages: Vec<CommonMessage>, system_message: Option<&str>) -> Result<ModelResponse, InferenceError> {
        if let Some(sys_msg) = system_message {
            messages.insert(0, CommonMessage {
                role: Role::System,
                content: vec![ContentItem::Text { text: sys_msg.to_string() }],
            });
        }

        let openai_messages: Vec<Value> = messages.into_iter().map(|msg| {
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
                    Role::Developer => "developer",
                    Role::Tool => "tool",
                },
                "content": content
            })
        }).collect();

        let tools = self.tool_provider.get_tools_json()
            .map_err(|e| InferenceError::SerializationError(e.to_string())).ok();
        println!("{:#?}", openai_messages);

        let request = OpenAIRequest {
            model: self.model.clone(),
            messages: openai_messages,
            max_tokens: Some(self.max_output_tokens),
            tools,
        };

        let response = self.client
            .post(&self.api_url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await
            .map_err(|e| InferenceError::NetworkError(e.to_string()))?;

        let status = response.status();
        let response_text = response.text().await
            .map_err(|e| InferenceError::NetworkError(e.to_string()))?;

        if !status.is_success() {
            return Err(InferenceError::ApiError(status, response_text));
        }

        let openai_response: OpenAIResponse = serde_json::from_str(&response_text)
            .map_err(|e| InferenceError::InvalidResponse(format!("Failed to parse OpenAI response: {}", e)))?;

        if openai_response.choices.is_empty() {
            return Err(InferenceError::InvalidResponse("No choices in OpenAI response".to_string()));
        }

        Ok(ModelResponse {
            content: openai_response.choices[0].message.content.clone(),
            model: openai_response.model,
            role: openai_response.choices[0].message.role.clone(),
            message_type: "text".to_string(),
            stop_reason: openai_response.choices[0].finish_reason.clone(),
            stop_sequence: None,
        })
    }
}
