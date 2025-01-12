use serde::{Serialize, Deserialize};
use serde::de::Error as SerdeError;
use anyhow::Result;

use crate::chat::ContentItem;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ModelResponse {
    pub content: Vec<ContentItem>,
    pub model: String,
    pub role: String,
    #[serde(rename = "type")]
    pub message_type: String,
    pub stop_reason: String,
    pub stop_sequence: Option<String>,
    pub output_tokens: u64,
}

impl ModelResponse {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        let value: serde_json::Value = serde_json::from_slice(bytes)?;
        
        let content = value.get("content")
            .and_then(|v| v.as_array())
            .ok_or_else(|| serde_json::Error::missing_field("content"))?
            .iter()
            .map(ContentItem::deserialize)
            .collect::<Result<Vec<_>, _>>()?;

        let model = value.get("model")
            .and_then(|v| v.as_str())
            .ok_or_else(|| serde_json::Error::missing_field("model"))?
            .to_string();

        let role = value.get("role")
            .and_then(|v| v.as_str())
            .ok_or_else(|| serde_json::Error::missing_field("role"))?
            .to_string();

        let message_type = value.get("type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| serde_json::Error::missing_field("type"))?
            .to_string();

        let stop_reason = value.get("stop_reason")
            .and_then(|v| v.as_str())
            .ok_or_else(|| serde_json::Error::missing_field("stop_reason"))?
            .to_string();

        // Optional fields
        let stop_sequence = value.get("stop_sequence")
            .and_then(|v| v.as_str())
            .map(String::from);

        let output_tokens = value.get("output_tokens")
                .and_then(|v| v.as_str().map(|s| s.parse::<u64>().ok()).flatten())
                .unwrap_or(0);

        Ok(ModelResponse {
            content,
            model,
            role,
            message_type,
            stop_reason,
            stop_sequence,
            output_tokens,
        })
    }
}

#[derive(Debug)]
pub enum InferenceError {
    NetworkError(String),
    ApiError(reqwest::StatusCode, String),
    InvalidResponse(String),
    MissingApiKey(String),
    SerializationError(String),
}

impl From<serde_json::Error> for InferenceError {
    fn from(_error: serde_json::Error) -> Self {
        InferenceError::SerializationError("Failed to parse inputs for tool use.".to_string())
    }
}

impl std::fmt::Display for InferenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            InferenceError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            InferenceError::ApiError(status, msg) => write!(f, "API error ({}): {}", status, msg),
            InferenceError::InvalidResponse(msg) => write!(f, "Invalid response: {}", msg),
            InferenceError::MissingApiKey(msg) => write!(f, "Missing API key: {}", msg),
            InferenceError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
        }
    }
}

impl std::error::Error for InferenceError {}
