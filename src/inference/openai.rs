use reqwest::Client;
use serde::{Serialize, Deserialize};
use anyhow::Result;

use crate::{config::ProjectConfig, tooler::Tooler};
use super::types::{
    ContentItem, InferenceError, Message, ModelResponse, Role, Usage
};

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
    usage: OpenAIUsage,
}

#[derive(Debug, Deserialize)]
struct OpenAIChoice {
    message: OpenAIMessage,
    finish_reason: String,
}

#[derive(Debug, Deserialize)]
struct OpenAIMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenAIUsage {
    prompt_tokens: i32,
    completion_tokens: i32,
}

pub struct OpenAIInference {
    model: String,
    client: Client,
    tooler: Tooler,
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
            tooler: Tooler::new(),
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

    pub async fn query_model(&self, mut messages: Vec<Message>, system_message: Option<&str>) -> Result<ModelResponse, InferenceError> {
        if self.api_key.is_empty() {
            return Err(InferenceError::MissingApiKey("OpenAI API key not found".to_string()));
        }

        if let Some(sys_msg) = system_message {
            messages.insert(0, Message {
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
                },
                "content": content
            })
        }).collect();

        let tools = self.tooler.get_tools_json()
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

        Ok(ModelResponse {
            content: vec![ContentItem::Text {
                text: openai_response.choices[0].message.content.clone(),
            }],
            id: openai_response.id,
            model: openai_response.model,
            role: openai_response.choices[0].message.role.clone(),
            message_type: "text".to_string(),
            stop_reason: openai_response.choices[0].finish_reason.clone(),
            stop_sequence: None,
            usage: Usage {
                input_tokens: openai_response.usage.prompt_tokens,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                output_tokens: openai_response.usage.completion_tokens,
            },
        })
    }
}
