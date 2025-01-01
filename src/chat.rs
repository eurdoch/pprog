use std::process::Command;

use tokenizers::Tokenizer;

use crate::{
    inference::{
        ContentItem,
        Inference,
        Message,
        Role,
        ModelResponse
    },
    tree::GitTree,
    config::ProjectConfig
};

static TOKENIZER_JSON: &[u8] = include_bytes!("../tokenizers/gpt2.json");

pub struct Chat {
    pub messages: Vec<Message>,
    inference: Inference,
    tokenizer: Tokenizer,
    max_tokens: usize,
}

impl Chat {
    pub fn new() -> Self {
        let tokenizer = Tokenizer::from_bytes(TOKENIZER_JSON).expect("Failed to load tokenizer.");
        let config = ProjectConfig::load().unwrap_or_default();
        Self {
            messages: Vec::new(),
            inference: Inference::new(),
            tokenizer,
            max_tokens: config.max_content,
        }
    }

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
                // Combine role and content for complete message token count
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

    fn extract_string_field<'a>(
        input: &'a serde_json::Value,
        field_name: &str
    ) -> Result<&'a str, anyhow::Error> {
        input.get(field_name)
            .ok_or_else(|| anyhow::anyhow!("Missing '{}' field in tool input: {:?}", field_name, input))?
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("'{}' field is not a string: {:?}", field_name, input.get(field_name)))
    }

    pub async fn send_message(&mut self, message: Message) -> Result<ModelResponse, anyhow::Error> {
        if message.role == Role::User {
            let tree_string = GitTree::get_tree()?;
            let system_message = format!(
                r#"
                You are a coding assistant working on a project.
                
                File tree structure:
                {}

                The user will give you instructions on how to change the project code.

                If you use tool 'write_file' successfully and tool 'compile_check' is available, call compile_check.  If compile_check shows any errors, make subsequent calls to correct the errors. Continue checking and rewriting until there are no more errors.  If there are warnings then do not try to fix them, just let the user know.  If any bash commands are needed like installing packages use tool 'execute'.

                Never make any changes outside of the project's root directory.

                The user may also general questions and in that case simply answer but do not execute any tools.
                "#,
                &tree_string,
            );
            self.trim_messages_to_token_limit();
            
            let response = self.inference.query_model(self.messages.clone(), Some(&system_message)).await?;
            Ok(response)
        } else {
            Err(anyhow::anyhow!("Can only send messages with user role when querying model."))
        }
    }

    pub async fn handle_tool_use(&mut self, content_item: &ContentItem) -> Result<String, anyhow::Error> {
        match content_item {
            ContentItem::ToolUse { name, input, .. } => {
                match GitTree::get_git_root() {
                    Ok(root_path) => {
                        let tool_result = match name.as_str() {
                            "write_file" => {
                                let content = Self::extract_string_field(input, "content")?;
                                let file_path = Self::extract_string_field(input, "path")?;
                                let full_path = root_path.join(file_path);
                                match std::fs::write(full_path.clone(), content) {
                                    Ok(_) => format!("Successfully wrote content to file {:?}.", full_path),
                                    Err(e) => format!("Error writing to file {:?}: {:?}.", full_path, e),
                                }
                            },
                            "read_file" => {
                                let file_path = Self::extract_string_field(input, "path")?;
                                let full_path = root_path.join(file_path);
                                match std::fs::read_to_string(full_path.clone()) {
                                    Ok(file_content) => file_content,
                                    Err(e) => format!("Error reading file {:?}: {:?}.", full_path, e),
                                }
                            },
                            "compile_check" => {
                                let check_cmd = Self::extract_string_field(input, "cmd")?;
                                let output = Command::new("bash")
                                    .arg("-c")
                                    .arg(format!("{} & sleep 1; kill $!", check_cmd))
                                    .current_dir(root_path)
                                    .output()
                                    .expect("Failed to execute command");

                                let stdout = String::from_utf8_lossy(&output.stdout);
                                let stderr = String::from_utf8_lossy(&output.stderr);
                                format!("Stdout:
{}
Stderr:
{}", stdout, stderr)
                            },
                            "execute" => {
                                let statement = Self::extract_string_field(input, "statement")?;
                                let output = Command::new("bash")
                                    .arg("-c")
                                    .arg(statement)
                                    .current_dir(root_path)
                                    .output()
                                    .expect("Failed to execute command");

                                let stdout = String::from_utf8_lossy(&output.stdout);
                                let stderr = String::from_utf8_lossy(&output.stderr);
                                format!("Stdout:
{}
Stderr:
{}", stdout, stderr)
                            },
                            _ => format!("Unknown tool: {}", name)
                        };

                        Ok(tool_result)
                    },
                    Err(e) => Err(anyhow::anyhow!("Error getting git root: {}", e))
                }
            },
            _ => Err(anyhow::anyhow!("Not a tool use content item"))
        }
    }
}