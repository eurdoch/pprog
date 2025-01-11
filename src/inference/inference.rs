use anyhow::Result;
use async_trait::async_trait;

use crate::chat::CommonMessage;
use super::types::{InferenceError, ModelResponse};

#[async_trait]
pub trait Inference: Send + Sync {
    fn new() -> Self where Self: Sized;
    
    async fn query_model(
        &self, 
        messages: Vec<CommonMessage>, 
        system_message: Option<&str>
    ) -> Result<ModelResponse, InferenceError>;
}
