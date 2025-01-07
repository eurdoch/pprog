use std::collections::HashMap;
use reqwest::Client;
use serde::{Serialize, Deserialize};
use anyhow::Result;

use crate::chat::chat::{CommonMessage, ContentItem, Role};
use crate::config::ProjectConfig;
use super::types::{
    InferenceError, ModelResponse
};
use super::tools::{OpenAITool, OpenAIToolFunction, InputSchema, PropertySchema};

#[derive(Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<serde_json::Value>,
    max_tokens: Option<u32>,
    tools: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    id: String,
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

pub struct OpenAIInference {
    model: String,
    client: Client,
    base_url: String,
    api_key: String,
    max_output_tokens: u32,
}

impl std::default::Default for OpenAIInference {
    fn default() -> Self {
        let config = match ProjectConfig::load() {
            Ok(config) => config,
            Err(_) => ProjectConfig::default(),
        };
        
        OpenAIInference {
            model: config.model,
            client: Client::new(),
            base_url: config.base_url,
            api_key: config.api_key,
            max_output_tokens: config.max_output_tokens,
        }
    }
}

impl OpenAIInference {
    pub fn new() -> Self {
        Self::default()
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

    pub async fn query_model(&self, mut messages: Vec<CommonMessage>, system_message: Option<&str>) -> Result<ModelResponse, InferenceError> {
        if self.api_key.is_empty() {
            return Err(InferenceError::MissingApiKey("OpenAI API key not found".to_string()));
        }

        if let Some(sys_msg) = system_message {
            messages.insert(0, CommonMessage {
                role: Role::System,
                content: vec![ContentItem::Text { text: sys_msg.to_string() }],
            });
        }

        let openai_messages = messages.into_iter().map(|msg| {
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

        let tools = self.get_tools_json()
            .map_err(|e| InferenceError::SerializationError(e.to_string())).ok();

        let request = OpenAIRequest {
            model: self.model.clone(),
            messages: openai_messages,
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
        log::info!("{:?}", response_text);

        if !status.is_success() {
            return Err(InferenceError::ApiError(status, response_text));
        }

        let openai_response: OpenAIResponse = serde_json::from_str(&response_text)
            .map_err(|e| InferenceError::InvalidResponse(format!("Failed to parse OpenAI response: {}", e)))?;

        if openai_response.choices.is_empty() {
            return Err(InferenceError::InvalidResponse("No choices in OpenAI response".to_string()));
        }

        let first_choice = &openai_response.choices[0].message;
        let mut content = first_choice.content.clone();

        // Handle tool calls if present
        if let Some(tool_calls) = &first_choice.tool_calls {
            for tool_call in tool_calls {
                if tool_call.call_type == "function" {
                    // Parse the arguments as JSON Value
                    let input: serde_json::Value = serde_json::from_str(&tool_call.function.arguments)
                        .map_err(|e| InferenceError::SerializationError(format!("Failed to parse tool arguments: {}", e)))?;

                    content.push(ContentItem::ToolUse {
                        id: tool_call.id.clone(),
                        name: tool_call.function.name.clone(),
                        input,
                    });
                }
            }
        }

        Ok(ModelResponse {
            content,
            id: openai_response.id,
            model: openai_response.model,
            role: first_choice.role.clone(),
            message_type: "text".to_string(),
            stop_reason: openai_response.choices[0].finish_reason.clone(),
            stop_sequence: None,
            //usage: Some(Usage {
            //    input_tokens: openai_response.usage.prompt_tokens,
            //    cache_creation_input_tokens: 0,
            //    cache_read_input_tokens: 0,
            //    output_tokens: openai_response.usage.completion_tokens,
            //}),
        })
    }
}
