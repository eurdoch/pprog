use crate::inference::types::{Message, InferenceError as Error};
use async_trait::async_trait;

#[async_trait]
pub trait Chat: Send + Sync {
    /// Initialize a new chat instance
    /// This is async because AWS Bedrock init requires async
    async fn new() -> Self where Self: Sized;
    
    /// Handle an incoming message and return a response
    async fn handle_message<'a>(&mut self, message: &'a Message) -> Result<Message, Error>;

    /// Send message using inference
    async fn send_message(&mut self, message: Message) -> Result<Message, anyhow::Error>; 
    
    /// Get all messages in the conversation
    fn get_messages(&self) -> &Vec<Message>;
    
    /// Clear the chat history
    fn clear(&mut self);
}
