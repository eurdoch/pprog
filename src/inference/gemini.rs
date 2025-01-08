use std::collections::HashMap;
use reqwest::Client;
use anyhow::Result;

use crate::chat::chat::{CommonMessage, ContentItem, Role};
use crate::chat::gemini_chat::{GeminiMessage, GeminiContent, GeminiFunctionDeclaration, GeminiParameters, 
    GeminiProperty, GeminiTool, GeminiToolConfig, GeminiFunctionCallingConfig, GeminiPart};
use crate::config::ProjectConfig;
use super::types::InferenceError;
use super::tools::{OpenAITool, OpenAIToolFunction, InputSchema, PropertySchema};

pub struct GeminiInference {
    model: String,
    client: Client,
    base_url: String,
    api_key: String,
    max_output_tokens: u32,
}

impl std::default::Default for GeminiInference {
    fn default() -> Self {
        let config = match ProjectConfig::load() {
            Ok(config) => config,
            Err(_) => ProjectConfig::default(),
        };
        
        GeminiInference {
            model: config.model,
            client: Client::new(),
            base_url: "https://generativelanguage.googleapis.com/v1beta".to_string(),
            api_key: config.api_key,
            max_output_tokens: config.max_output_tokens,
        }
    }
}

impl GeminiInference {
    pub fn new() -> Self {
        Self::default()
    }

    fn convert_tool_to_function_declaration(&self, tool: &OpenAITool) -> GeminiFunctionDeclaration {
        let mut properties = HashMap::new();
        
        for (name, prop) in &tool.function.parameters.properties {
            properties.insert(name.clone(), GeminiProperty {
                property_type: prop.property_type.clone(),
                description: prop.description.clone(),
            });
        }

        GeminiFunctionDeclaration {
            name: tool.name.clone(),
            description: tool.description.clone(),
            parameters: GeminiParameters {
                parameter_type: "object".to_string(),
                properties,
                required: tool.function.parameters.required.clone(),
            },
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

    pub async fn query_model(
        &self,
        mut messages: Vec<GeminiMessage>,
        system_message: Option<&str>
    ) -> Result<GeminiMessage, InferenceError> {
        if self.api_key.is_empty() {
            return Err(InferenceError::MissingApiKey("Gemini API key not found".to_string()));
        }

        let url = format!("{}/models/gemini-pro:generateContent?key={}", self.base_url, self.api_key);
        
        let response = self.client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&messages[messages.len() - 1])  // Send only the last message as request
            .send()
            .await
            .map_err(|e| InferenceError::NetworkError(e.to_string()))?;

        let status = response.status();
        let response_text = response.text().await
            .map_err(|e| InferenceError::NetworkError(e.to_string()))?;

        println!("Gemini response: {}", response_text);

        if !status.is_success() {
            return Err(InferenceError::ApiError(status, response_text));
        }

        serde_json::from_str(&response_text)
            .map_err(|e| InferenceError::InvalidResponse(format!("Failed to parse Gemini response: {}", e)))
    }
}
