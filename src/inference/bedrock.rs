use std::sync::Arc;

use anyhow::Result;
use aws_sdk_bedrockruntime::types::ResponseStream;
use aws_sdk_bedrockruntime::Client as BedrockClient;
use aws_sdk_bedrockruntime::primitives::Blob;
use serde_json::{json, Value};

use super::types::{Message, Role, ContentItem, ModelResponse, Usage, Inference, InferenceError};

pub struct AWSBedrockInference {
    client: Arc<BedrockClient>, // TODO Arc is probably not necessary
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

    fn prepare_anthropic_prompt(&self, messages: &[Message]) -> String {
        let mut prompt = String::new();
        for msg in messages {
            match msg.role {
                Role::System => {
                    let content = msg.content.iter()
                        .filter_map(|item| match item {
                            ContentItem::Text { text } => Some(text.as_str()),
                            _ => None
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    prompt.push_str(&format!("\n\nSystem: {}", content));
                },
                Role::User => {
                    let content = msg.content.iter()
                        .filter_map(|item| match item {
                            ContentItem::Text { text } => Some(text.as_str()),
                            _ => None
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    prompt.push_str(&format!("\n\nHuman: {}", content));
                },
                Role::Assistant => {
                    let content = msg.content.iter()
                        .filter_map(|item| match item {
                            ContentItem::Text { text } => Some(text.as_str()),
                            _ => None
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    prompt.push_str(&format!("\n\nAssistant: {}", content));
                },
            }
        }
        prompt.push_str("\n\nAssistant: ");
        prompt
    }

    fn prepare_llama_prompt(&self, messages: &[Message]) -> String {
        let mut prompt = String::new();
        let mut system_content = String::new();

        // Collect and prepare system message first
        for msg in messages {
            if msg.role == Role::System {
                let content = msg.content.iter()
                    .filter_map(|item| match item {
                        ContentItem::Text { text } => Some(text.as_str()),
                        _ => None
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                system_content.push_str(&content);
            }
        }

        // Add system message block if not empty
        if !system_content.is_empty() {
            prompt.push_str(&format!("[INST] <<SYS>>\n{}\n<</SYS>>\n\n", system_content));
        }

        // Process other messages
        for msg in messages {
            let content = msg.content.iter()
                .filter_map(|item| match item {
                    ContentItem::Text { text } => Some(text.as_str()),
                    _ => None
                })
                .collect::<Vec<_>>()
                .join(" ");
            
            match msg.role {
                Role::System => continue, // Already handled
                Role::User => prompt.push_str(&format!("{} [/INST]", content)),
                Role::Assistant => prompt.push_str(&format!("{}\n\n[INST]", content)),
            }
        }
        
        prompt
    }
}

impl Inference for AWSBedrockInference {
    async fn query_model(&self, mut messages: Vec<Message>, system_message: Option<&str>) -> Result<ModelResponse, InferenceError> {
        if let Some(sys_msg) = system_message {
            messages.insert(0, Message {
                role: Role::System,
                content: vec![ContentItem::Text { text: sys_msg.to_string() }],
            });
        }

        let body = if self.model_id.contains("anthropic") {
            // Anthropic Claude models
            json!({
                "prompt": self.prepare_anthropic_prompt(&messages),
                "max_tokens_to_sample": self.max_tokens.unwrap_or(2000),
                "temperature": self.temperature,
                "top_p": 1,
                "top_k": 250,
            })
        } else if self.model_id.contains("meta") {
            // Meta Llama models
            json!({
                "prompt": self.prepare_llama_prompt(&messages),
                "max_gen_len": self.max_tokens.unwrap_or(2000),
                "temperature": self.temperature,
                "top_p": 0.9,
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

        let response_body: Value = serde_json::from_slice(&response.body.into_inner())
            .map_err(|e| InferenceError::InvalidResponse(e.to_string()))?;
        
        let content = if self.model_id.contains("anthropic") {
            response_body["completion"].as_str()
                .ok_or_else(|| InferenceError::InvalidResponse("Missing completion in response".to_string()))?
        } else if self.model_id.contains("meta") {
            response_body["generation"].as_str()
                .ok_or_else(|| InferenceError::InvalidResponse("Missing generation in response".to_string()))?
        } else {
            return Err(InferenceError::InvalidResponse(format!("Unsupported model: {}", self.model_id)));
        };

        Ok(ModelResponse {
            content: vec![ContentItem::Text { text: content.to_string() }],
            id: "bedrock".to_string(),
            model: self.model_id.clone(),
            role: "assistant".to_string(),
            message_type: "text".to_string(),
            stop_reason: "stop".to_string(),
            stop_sequence: None,
            usage: Usage {
                input_tokens: 0,  // Bedrock doesn't provide token counts
                output_tokens: 0,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            },
        })
    }

    async fn generate_stream(&self, _messages: &[Message]) -> Result<ResponseStream> {
        Err(anyhow::anyhow!("Streaming not yet implemented for AWS Bedrock"))
    }
}