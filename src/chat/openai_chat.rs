use tokenizers::Tokenizer;
use async_trait::async_trait;

use crate::inference::{
    types::{ContentItem, Message, Role, InferenceError as Error},
    OpenAIInference,
};
use crate::config::ProjectConfig;
use crate::tree::GitTree;

use super::chat::Chat;

static TOKENIZER_JSON: &[u8] = include_bytes!("../../tokenizers/gpt2.json");

pub struct OpenAIChat {
    pub messages: Vec<Message>,
    inference: OpenAIInference,
    tokenizer: Tokenizer,
    max_tokens: usize,
}

#[async_trait]
impl Chat for OpenAIChat {
    async fn new() -> Self {
        let tokenizer = Tokenizer::from_bytes(TOKENIZER_JSON).expect("Failed to load tokenizer.");
        let config = ProjectConfig::load().unwrap_or_default();

        Self {
            messages: Vec::new(),
            inference: OpenAIInference::new(),
            tokenizer,
            max_tokens: config.max_context,
        }
    }

    async fn handle_message(&mut self, message: &Message) -> Result<Message, Error> {
        self.send_message(message.clone()).await.map_err(|e| Error::InvalidResponse(e.to_string()))
    }
    
    async fn send_message(&mut self, message: Message) -> Result<Message, anyhow::Error> {
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
            self.trim_messages_to_token_limit();
            self.messages.push(message);
            
            match self.inference.query_model(self.messages.clone(), Some(&system_message)).await {
                Ok(response) => {
                    let new_msg = Message {
                        role: Role::Assistant,
                        content: response.content.clone()
                    };
                    self.messages.push(new_msg.clone());
                    Ok(new_msg)
                },
                Err(e) => {
                    self.messages.pop();
                    Err(e.into())
                }
            }
        } else {
            Err(anyhow::anyhow!("Can only send messages with user role when querying model."))
        }
    }

    fn get_messages(&self) -> &Vec<Message> {
        &self.messages
    }

    fn clear(&mut self) {
        self.messages.clear();
    }
}

impl OpenAIChat {
    fn content_to_string(content: &[ContentItem]) -> String {
        content.iter()
            .map(|item| match item {
                ContentItem::Text { text } => text.clone(),
                ContentItem::ToolUse { name, input, .. } => format!("tool {} with input: {:?}", name, input),
                ContentItem::ToolResult { content, .. } => format!("tool result: {}", content),
            })
            .collect::<Vec<String>>()
            .join(" ")
    }

    fn calculate_total_tokens(&self) -> usize {
        self.messages.iter()
            .map(|msg| {
                let text = format!("{:?} {}", msg.role, Self::content_to_string(&msg.content));
                let encoding = self.tokenizer.encode(text, false).unwrap();
                encoding.len()
            })
            .sum()
    }

    fn trim_messages_to_token_limit(&mut self) {
        while self.calculate_total_tokens() > self.max_tokens && !self.messages.is_empty() {
            self.messages.remove(0);
        }
    }
}
