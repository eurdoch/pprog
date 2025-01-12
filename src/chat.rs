use std::fmt;
use serde::{Deserialize, Serialize};

use crate::inference::{AnthropicInference, OpenAIInference};
use crate::{config::ProjectConfig, tree::GitTree};
use crate::inference::inference::Inference;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(tag = "type")]
pub enum ContentItem {
    #[serde(rename = "text")]
    Text {
        text: String,
    },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct CommonMessage {
    pub role: Role,
    pub content: Vec<ContentItem>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    System,
    Developer, // because OpenAI just had to change the system name
    Tool,      // Deepseek API uses tool role for tool results
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Role::User => write!(f, "user"),
            Role::Assistant => write!(f, "assistant"),
            Role::System => write!(f, "system"),
            Role::Developer => write!(f, "developer"),
            Role::Tool => write!(f, "tool"),
        }
    }
}

pub struct Chat {
    pub messages: Vec<CommonMessage>,
    inference: Box<dyn Inference>,
    max_tokens: usize,
    total_tokens: u64,
}

impl Chat {
    pub fn new() -> Self {
        let config = ProjectConfig::load().unwrap_or_default();
        let inference: Box<dyn Inference> = match config.provider.as_str() {
            "anthropic" => Box::new(AnthropicInference::new()),
            "openai" => Box::new(OpenAIInference::new()),
            _ => Box::new(AnthropicInference::new()),
        };

        Self {
            messages: Vec::new(),
            inference,
            max_tokens: config.max_context,
            total_tokens: 0,
        }
    }

    async fn prune_messages(&mut self) -> Result<(), anyhow::Error> {
        let system_message = self.get_system_message()?;
        
        while !self.messages.is_empty() {
            let token_count = self.inference.get_token_count(self.messages.clone(), Some(&system_message)).await?;
            
            if token_count <= self.max_tokens as u64 {
                break;
            }
            
            // Remove the oldest non-system message
            // Find the first non-system message
            if let Some(index) = self.messages.iter()
                .position(|msg| msg.role != Role::System) {
                self.messages.remove(index);
            } else {
                // If no non-system messages found, break to avoid infinite loop
                break;
            }
        }
        Ok(())
    }

    pub async fn handle_message(&mut self, message: &CommonMessage) -> Result<CommonMessage, anyhow::Error> {
        self.messages.push(message.clone());
        
        // After first message, check and prune if needed
        if self.messages.len() > 1 {
            self.prune_messages().await?;
        }
        
        let return_msg = self.send_messages().await?;
        self.messages.push(return_msg.clone());
        Ok(return_msg)
    }

    fn get_system_message(&self) -> Result<String, anyhow::Error> {
        let tree_string = GitTree::get_tree()?;
        Ok(format!(
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
        ))
    }

    pub async fn send_messages(&mut self) -> Result<CommonMessage, anyhow::Error> {
        let system_message = self.get_system_message()?;
        
        match self.inference.query_model(self.messages.clone(), Some(&system_message)).await {
            Ok(response) => {
                self.total_tokens = response.total_tokens;
                let new_msg = CommonMessage {
                    role: Role::Assistant,
                    content: response.content.clone()
                };
                Ok(new_msg)
            },
            Err(e) => {
                Err(anyhow::anyhow!("Inference Error: {}", e))
            }
        }
    }

    pub fn get_messages(&self) -> Vec<CommonMessage> {
        self.messages.clone()
    }

    pub fn clear(&mut self) {
        self.messages.clear();
    }
}
