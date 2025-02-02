use anyhow::Result;
use async_trait::async_trait;

use crate::chat::CommonMessage;
use super::types::{InferenceError, ModelResponse};

#[async_trait]
pub trait Inference: Send + Sync {
    fn new(
        model: String,
        api_url: String,
        api_key: String,
        max_output_tokens: u32
    ) -> Self where Self: Sized;
    
    async fn query_model(
        &self, 
        messages: Vec<CommonMessage>, 
        system_message: Option<&str>
    ) -> Result<ModelResponse, InferenceError>;

    async fn get_token_count(
        &self, 
        messages: Vec<CommonMessage>, 
        system_message: Option<&str>
    ) -> Result<u64, InferenceError>;
}
