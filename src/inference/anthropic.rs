use std::collections::HashMap;
use reqwest::Client;
use serde::{Serialize, Deserialize};
use anyhow::Result;

use crate::config::ProjectConfig;
use super::types::{
    ContentItem, InferenceError, Message, ModelResponse, Usage
};
use super::tools::{AnthropicTool, InputSchema, PropertySchema};

#[derive(Serialize)]
struct AnthropicRequest<'a> {
    model: &'a str,
    messages: Vec<Message>,
    max_tokens: u32,
    tools: serde_json::Value,
    system: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    id: String,
    model: String,
    role: String,
    content: Vec<ContentItem>,
    stop_reason: String,
    stop_sequence: Option<String>,
    usage: AnthropicUsage,
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    input_tokens: i32,
    output_tokens: i32,
}

pub struct AnthropicInference {
    model: String,
    client: Client,
    base_url: String,
    api_key: String,
    max_output_tokens: u32,
}

impl std::default::Default for AnthropicInference {
    fn default() -> Self {
        let config = match ProjectConfig::load() {
            Ok(config) => config,
            Err(_) => ProjectConfig::default(),
        };
        
        AnthropicInference {
            model: config.model,
            client: Client::new(),
            base_url: config.base_url,
            api_key: config.api_key,
            max_output_tokens: config.max_output_tokens,
        }
    }
}

impl AnthropicInference {
    pub fn new() -> Self {
        Self::default()
    }

    fn get_tools(&self) -> Vec<AnthropicTool> {
        vec![
            self.read_file_tool(),
            self.write_file_tool(),
            self.execute_tool(),
            self.compile_check_tool(),
        ]
    }

    fn read_file_tool(&self) -> AnthropicTool {
        AnthropicTool {
            name: "read_file".to_string(),
            description: "Read file as string using path relative to root directory of project.".to_string(),
            input_schema: InputSchema {
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
        }
    }

    fn write_file_tool(&self) -> AnthropicTool {
        AnthropicTool {
            name: "write_file".to_string(),
            description: "Write string to file at path relative to root directory of project.".to_string(),
            input_schema: InputSchema {
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
        }
    }

    fn execute_tool(&self) -> AnthropicTool {
        AnthropicTool {
            name: "execute".to_string(),
            description: "Execute bash statements as a single string..".to_string(),
            input_schema: InputSchema {
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
        }
    }

    fn compile_check_tool(&self) -> AnthropicTool {
        AnthropicTool {
            name: "compile_check".to_string(),
            description: "Check if project compiles or runs without error.".to_string(),
            input_schema: InputSchema {
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
        }
    }

    fn get_tools_json(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(self.get_tools())
    }

    pub async fn query_model(&self, messages: Vec<Message>, system_message: Option<&str>) -> Result<ModelResponse, InferenceError> {
        if self.api_key.is_empty() {
            return Err(InferenceError::MissingApiKey("Anthropic API key not found".to_string()));
        }

        let system = system_message.unwrap_or("").to_string();

        let tools = self.get_tools_json()
            .map_err(|e| InferenceError::SerializationError(e.to_string()))?;

        let request = AnthropicRequest {
            model: &self.model,
            messages,
            max_tokens: self.max_output_tokens,
            tools,
            system,
        };

        let response = self.client
            .post(format!("{}/messages", self.base_url))
            .header("Content-Type", "application/json")
            .header("X-API-Key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&request)
            .send()
            .await
            .map_err(|e| InferenceError::NetworkError(e.to_string()))?;

        let status = response.status();
        let response_text = response.text().await
            .map_err(|e| InferenceError::NetworkError(e.to_string()))?;
        let response_json: serde_json::Value = serde_json::from_str(&response_text).unwrap();
        log::info!("{:#?}", response_json);

        if !status.is_success() {
            return Err(InferenceError::ApiError(status, response_text));
        }

        let anthropic_response: AnthropicResponse = serde_json::from_str(&response_text)
            .map_err(|e| InferenceError::InvalidResponse(e.to_string()))?;

        Ok(ModelResponse {
            content: anthropic_response.content,
            id: anthropic_response.id,
            model: anthropic_response.model,
            role: anthropic_response.role,
            message_type: "text".to_string(),
            stop_reason: anthropic_response.stop_reason,
            stop_sequence: anthropic_response.stop_sequence,
            usage: Usage {
                input_tokens: anthropic_response.usage.input_tokens,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                output_tokens: anthropic_response.usage.output_tokens,
            },
        })
    }
}
