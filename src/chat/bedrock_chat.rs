use crate::inference::AWSBedrockInference;
use crate::config::ProjectConfig;
use crate::tree::GitTree;

use super::chat::{Chat, CommonMessage, Role};
use async_trait::async_trait;

pub struct BedrockChat {
    pub messages: Vec<CommonMessage>,
    inference: AWSBedrockInference,
    max_tokens: usize,
}

#[async_trait]
impl Chat for BedrockChat {
    async fn new() -> Self where Self: Sized {
        let config = ProjectConfig::load().unwrap_or_default();

        let bedrock_inference = AWSBedrockInference::new(
            config.model.clone(),           // model_id
            0.2,                            // temperature 
            Some(config.max_output_tokens as i32), // max_tokens
        ).await.expect("Failed to initialize Bedrock inference");

        Self {
            messages: Vec::new(),
            inference: bedrock_inference,
            max_tokens: config.max_context,
        }
    }
    
    async fn handle_message(&mut self, message: &CommonMessage) -> Result<CommonMessage, anyhow::Error> {
        Ok(self.send_message(message.clone()).await?)
    }

    fn get_messages(&self) -> Vec<CommonMessage> {
        self.messages.clone()
    }

    fn clear(&mut self) {
        self.messages.clear();
    }
}

impl BedrockChat {
    async fn send_message(&mut self, message: CommonMessage) -> Result<CommonMessage, anyhow::Error> {
        if message.role == Role::User {
            let tree_string = GitTree::get_tree()?;
            let system_message = format!(
                r#"
                You are a coding assistant working on a project.
                
                File tree structure:
                {}

                The user will give you instructions on how to change the project code.

                Always call 'compile_check' tool after completing changes that the user requests.  If compile_check shows any errors, make subsequent calls to correct the errors. Continue checking and rewriting until there are no more errors.  If there are warnings then do not try to fix them, just let the user know.  If any bash commands are needed like installing packages use tool 'execute'.

                Never make any changes outside of the project's root directory.
                Always read and write entire file contents.  Never write partial contents of a file.

                The user may also general questions and in that case simply answer but do not execute any tools.
                "#,
                &tree_string,
            );
            //self.trim_messages_to_token_limit();
            self.messages.push(message);
            
            match self.inference.query_model(self.messages.clone(), Some(&system_message)).await {
                Ok(response) => {
                    let new_msg = CommonMessage {
                        role: Role::Assistant,
                        content: response.content.clone()
                    };
                    self.messages.push(new_msg.clone());
                    Ok(new_msg)
                },
                Err(e) => {
                    self.messages.pop();
                    Err(anyhow::anyhow!("Bedrock Inference Error: {}", e))
                }
            }
        } else {
            Err(anyhow::anyhow!("Can only send messages with user role when querying model."))
        }
    }

    }
