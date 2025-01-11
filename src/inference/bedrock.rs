use std::sync::Arc;
use std::collections::HashMap;

use anyhow::Result;
use aws_sdk_bedrockruntime::Client as BedrockClient;
use aws_sdk_bedrockruntime::primitives::Blob;
use serde_json::json;

use crate::chat::CommonMessage;

use super::types::{ModelResponse, InferenceError};
use super::tools::{AnthropicTool, InputSchema, PropertySchema};

pub struct AWSBedrockInference {
    client: Arc<BedrockClient>, 
    model_id: String,
    temperature: f32,
    max_tokens: Option<i32>,
}

impl AWSBedrockInference {
    pub async fn new(
        model_id: String,
        temperature: f32,
        max_tokens: Option<i32>,
    ) -> Result<Self> {
        let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        let client = Arc::new(BedrockClient::new(&config));

        Ok(Self {
            client,
            model_id,
            temperature,
            max_tokens,
        })
    }

    fn get_anthropic_tools(&self) -> Vec<AnthropicTool> {
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

    pub async fn query_model(&self, messages: Vec<CommonMessage>, system_message: Option<&str>) -> Result<ModelResponse, InferenceError> {
        let body = if self.model_id.contains("anthropic") {
            // Anthropic Claude models
            let tools_json = match serde_json::to_value(self.get_anthropic_tools()) {
                Ok(tools) => tools,
                Err(_) => json!(null),
            };
            let sys_msg = match system_message {
                Some(m) => m,
                None => "",
            };

            json!({
                "anthropic_version": "bedrock-2023-05-31",
                "system": sys_msg,
                "messages": messages,
                "max_tokens": self.max_tokens.unwrap_or(2000),
                "temperature": self.temperature,
                "tools": tools_json
            })
        } else {
            return Err(InferenceError::InvalidResponse(format!("Unsupported model: {}", self.model_id)));
        };

        let response = self.client
            .invoke_model()
            .model_id(&self.model_id)
            .accept("application/json")
            .content_type("application/json")
            .body(Blob::new(serde_json::to_string(&body).map_err(|e| 
                InferenceError::SerializationError(e.to_string())
            )?.into_bytes()))
            .send()
            .await
            .map_err(|e| InferenceError::NetworkError(e.to_string()))?;

        ModelResponse::from_bytes(&response.body.into_inner())
            .map_err(|e| InferenceError::InvalidResponse(e.to_string()))
    }
}
