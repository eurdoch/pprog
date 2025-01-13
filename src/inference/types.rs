use serde::{Serialize, Deserialize};

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
    pub total_tokens: u64,
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
