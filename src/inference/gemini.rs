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

#[derive(Debug, Serialize)]
struct GeminiContent {
    role: String,
    parts: GeminiParts,
}

#[derive(Debug, Serialize)]
struct GeminiParts {
    text: String,
}

#[derive(Debug, Serialize)]
struct GeminiFunctionDeclaration {
    name: String,
    description: String,
    parameters: GeminiParameters,
}

#[derive(Debug, Serialize)]
struct GeminiParameters {
    #[serde(rename = "type")]
    parameter_type: String,
    properties: HashMap<String, GeminiProperty>,
    required: Vec<String>,
}

#[derive(Debug, Serialize)]
struct GeminiProperty {
    #[serde(rename = "type")]
    property_type: String,
    description: String,
}

#[derive(Debug, Serialize)]
struct GeminiTool {
    function_declarations: Vec<GeminiFunctionDeclaration>,
}

#[derive(Debug, Serialize)]
struct GeminiFunctionCallingConfig {
    mode: String,
}

#[derive(Debug, Serialize)]
struct GeminiToolConfig {
    function_calling_config: GeminiFunctionCallingConfig,
}

#[derive(Debug, Serialize)]
struct GeminiRequest {
    contents: GeminiContent,
    tools: Option<Vec<GeminiTool>>,
    tool_config: Option<GeminiToolConfig>,
}

#[derive(Debug, Deserialize)]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
}

#[derive(Debug, Deserialize)]
struct GeminiCandidate {
    content: GeminiResponseContent,
    finish_reason: String,
}

#[derive(Debug, Deserialize)]
struct GeminiResponseContent {
    role: String,
    parts: Vec<GeminiResponsePart>,
}

#[derive(Debug, Deserialize)]
struct GeminiResponsePart {
    text: Option<String>,
    function_call: Option<GeminiFunctionCall>,
}

#[derive(Debug, Deserialize)]
struct GeminiFunctionCall {
    name: String,
    args: serde_json::Value,
}

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

    pub async fn query_model(&self, messages: Vec<CommonMessage>, system_message: Option<&str>) -> Result<ModelResponse, InferenceError> {
        if self.api_key.is_empty() {
            return Err(InferenceError::MissingApiKey("Gemini API key not found".to_string()));
        }

        // Get the last user message as Gemini only supports single message input
        let last_message = messages.last()
            .ok_or_else(|| InferenceError::InvalidResponse("No messages provided".to_string()))?;

        let content = last_message.content.iter()
            .filter_map(|item| {
                match item {
                    ContentItem::Text { text } => Some(text.clone()),
                    _ => None
                }
            })
            .collect::<Vec<String>>()
            .join(" ");

        // Convert tools to Gemini format
        let tools = self.get_tools();
        let function_declarations: Vec<GeminiFunctionDeclaration> = tools.iter()
            .map(|tool| self.convert_tool_to_function_declaration(tool))
            .collect();

        let request = GeminiRequest {
            contents: GeminiContent {
                role: "user".to_string(),
                parts: GeminiParts {
                    text: content,
                },
            },
            tools: Some(vec![GeminiTool {
                function_declarations,
            }]),
            tool_config: Some(GeminiToolConfig {
                function_calling_config: GeminiFunctionCallingConfig {
                    mode: "ANY".to_string(),
                },
            }),
        };

        let url = format!("{}/models/gemini-pro:generateContent?key={}", self.base_url, self.api_key);
        
        let response = self.client
            .post(&url)
            .header("Content-Type", "application/json")
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

        let gemini_response: GeminiResponse = serde_json::from_str(&response_text)
            .map_err(|e| InferenceError::InvalidResponse(format!("Failed to parse Gemini response: {}", e)))?;

        if gemini_response.candidates.is_empty() {
            return Err(InferenceError::InvalidResponse("No candidates in Gemini response".to_string()));
        }

        let first_candidate = &gemini_response.candidates[0];
        let mut content_items = Vec::new();

        // Convert Gemini response parts to ContentItems
        for part in &first_candidate.content.parts {
            if let Some(text) = &part.text {
                content_items.push(ContentItem::Text { text: text.clone() });
            }
            if let Some(function_call) = &part.function_call {
                content_items.push(ContentItem::ToolUse {
                    id: "1".to_string(), // Gemini doesn't provide IDs, using default
                    name: function_call.name.clone(),
                    input: function_call.args.clone(),
                });
            }
        }

        Ok(ModelResponse {
            content: content_items,
            id: "gemini-response".to_string(), // Gemini doesn't provide response IDs
            model: "gemini-pro".to_string(),
            role: first_candidate.content.role.clone(),
            message_type: "text".to_string(),
            stop_reason: first_candidate.finish_reason.clone(),
            stop_sequence: None,
        })
    }
}