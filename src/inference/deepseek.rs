use std::collections::HashMap;
use reqwest::Client;
use serde::{Serialize, Deserialize};
use anyhow::Result;
use serde_json::Value;

use crate::chat::chat::Role;
use crate::config::ProjectConfig;
use super::types::{
    InferenceError
};
use super::tools::{OpenAITool, OpenAIToolFunction, InputSchema, PropertySchema};

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
    pub content: String,
    pub role: Role,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
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
    pub arguments: Value,
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
    base_url: String,
    api_key: String,
    max_output_tokens: u32,
}

impl DeepSeekInference {
    pub fn new() -> Self {
        let config = match ProjectConfig::load() {
            Ok(config) => config,
            Err(_) => ProjectConfig::default(),
        };
        
        DeepSeekInference {
            model: config.model,
            client: Client::new(),
            base_url: config.base_url,
            api_key: config.api_key,
            max_output_tokens: config.max_output_tokens,
        }
    }

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
            name: "read_file".to_string(),
            description: "Read file as string using path relative to root directory of project.".to_string(),
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
            name: "write_file".to_string(),
            description: "Write string to file at path relative to root directory of project.".to_string(),
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
            name: "execute".to_string(),
            description: "Execute bash statements as a single string..".to_string(),
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
            name: "compile_check".to_string(),
            description: "Check if project compiles or runs without error.".to_string(),
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

    pub async fn query_model(
        &self,
        mut messages: Vec<DeepSeekMessage>,
        system_message: Option<&str>
    ) -> Result<DeepSeekModelResponse, InferenceError> {
        if self.api_key.is_empty() {
            return Err(InferenceError::MissingApiKey("DeepSeek API key not found".to_string()));
        }

        if let Some(sys_msg) = system_message {
            messages.insert(0, DeepSeekMessage {
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
            messages,
            max_tokens: Some(self.max_output_tokens),
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
        //let response_json: Value = serde_json::from_str(&response_text).map_err(|e| println!("{:?}", e)).unwrap();

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
