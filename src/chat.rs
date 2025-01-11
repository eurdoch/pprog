use serde::{Deserialize, Serialize};
use serde_json::Number;

use crate::{config::ProjectConfig, inference::AnthropicInference, tree::GitTree};

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
pub struct Usage {
    cache_creation_input_tokens: Number,
    cache_read_input_tokens: Number,
    input_tokens: Number,
    output_tokens: Number,
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

pub struct Chat {
    pub messages: Vec<CommonMessage>,
    inference: AnthropicInference,
    max_tokens: usize,
}

impl Chat {
    pub fn new() -> Self {
        let config = ProjectConfig::load().unwrap_or_default();

        Self {
            messages: Vec::new(),
            inference: AnthropicInference::new(),
            max_tokens: config.max_context,
        }
    }

    pub async fn handle_message(&mut self, message: &CommonMessage) -> Result<CommonMessage, anyhow::Error> {
        self.messages.push(message.clone());
        let return_msg = self.send_messages().await?;
        self.messages.push(return_msg.clone());
        Ok(return_msg)
    }

    pub async fn send_messages(&mut self) -> Result<CommonMessage, anyhow::Error> {
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
        
        match self.inference.query_model(self.messages.clone(), Some(&system_message)).await {
            Ok(response) => {
                let new_msg = CommonMessage {
                    role: Role::Assistant,
                    content: response.content.clone()
                };
                Ok(new_msg)
            },
            Err(e) => {
                Err(anyhow::anyhow!("Anthropic Inference Error: {}", e))
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
