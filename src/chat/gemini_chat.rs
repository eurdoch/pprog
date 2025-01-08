use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

use crate::inference::GeminiInference;
use crate::config::ProjectConfig;
use crate::tree::GitTree;

use super::chat::{convert_gemini_to_common, convert_common_to_gemini, Chat, CommonMessage, ContentItem, Role};
use super::tools::Tools;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum GeminiMessage {
    #[serde(rename = "request")]
    Request {
        contents: Vec<GeminiContent>,
        tools: Option<Vec<GeminiTool>>,
        tool_config: Option<GeminiToolConfig>,
    },
    #[serde(rename = "response")]
    Response {
        candidates: Vec<GeminiCandidate>,
        prompt_feedback: Option<GeminiPromptFeedback>,
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeminiContent {
    pub role: String,
    pub parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeminiPart {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(rename = "functionCall", skip_serializing_if = "Option::is_none")]
    pub function_call: Option<GeminiFunctionCall>,
    #[serde(rename = "functionResponse", skip_serializing_if = "Option::is_none")]
    pub function_response: Option<GeminiFunctionResponse>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeminiFunctionCall {
    pub name: String,
    pub args: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeminiFunctionResponse {
    pub name: String,
    pub response: GeminiFunctionResponseData,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeminiFunctionResponseData {
    pub name: String,
    pub content: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeminiTool {
    pub function_declarations: Vec<GeminiFunctionDeclaration>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeminiFunctionDeclaration {
    pub name: String,
    pub description: String,
    pub parameters: GeminiParameters,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeminiParameters {
    #[serde(rename = "type")]
    pub parameter_type: String,
    pub properties: HashMap<String, GeminiProperty>,
    pub required: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeminiProperty {
    #[serde(rename = "type")]
    pub property_type: String,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeminiToolConfig {
    pub function_calling_config: GeminiFunctionCallingConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeminiFunctionCallingConfig {
    pub mode: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeminiCandidate {
    pub content: GeminiContent,
    #[serde(rename = "finishReason")]
    pub finish_reason: String,
    pub index: i32,
    #[serde(rename = "safetyRatings")]
    pub safety_ratings: Vec<GeminiSafetyRating>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeminiSafetyRating {
    pub category: String,
    pub probability: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeminiPromptFeedback {
    #[serde(rename = "safetyRatings")]
    pub safety_ratings: Vec<GeminiSafetyRating>,
}

pub struct GeminiChat {
    pub messages: Vec<GeminiMessage>,
    inference: GeminiInference,
    max_tokens: usize,
}

#[async_trait]
impl Chat for GeminiChat {
    async fn new() -> Self {
        let config = ProjectConfig::load().unwrap_or_default();

        Self {
            messages: Vec::new(),
            inference: GeminiInference::new(),
            max_tokens: config.max_context,
        }
    }

    async fn handle_message(&mut self, message: &CommonMessage) -> Result<CommonMessage, anyhow::Error> {
        let gemini_message = convert_common_to_gemini(message)?;
        self.messages.push(gemini_message.clone());
        
        // Check if message contains tool use
        let has_tool_use = message.content.iter().any(|item| matches!(item, ContentItem::ToolUse { .. }));
        
        if has_tool_use {
            if let Some(ContentItem::ToolUse { id, name, input }) = message.content.iter().find(|item| matches!(item, ContentItem::ToolUse { .. })) {
                let tool_use_result = match Tools::handle_tool_use(name.clone(), input.clone()) {
                    Ok(c) => Ok(c),
                    Err(e) => {
                        self.messages.pop();
                        Err(e)
                    }
                }?;

                let tool_result_msg = CommonMessage {
                    role: Role::Tool,
                    content: vec![ContentItem::ToolResult {
                        tool_use_id: id.clone(),
                        content: tool_use_result,
                    }],
                };
                Ok(tool_result_msg)
            } else {
                Err(anyhow::anyhow!("Tool use message malformed"))
            }
        } else {
            match self.send_messages().await {
                Ok(return_msg) => Ok(return_msg),
                Err(e) => Err(e),
            }
        }
    }

    fn get_messages(&self) -> Vec<CommonMessage> {
        self.messages.iter()
            .filter_map(|msg| convert_gemini_to_common(msg).ok())
            .collect()
    }

    fn clear(&mut self) {
        self.messages.clear();
    }
}
    
impl GeminiChat {
    async fn send_messages(&mut self) -> Result<CommonMessage, anyhow::Error> {
        // Convert last message role to CommonMessage to check role
        let last_msg = self.messages.last()
            .and_then(|msg| convert_gemini_to_common(msg).ok());

        match last_msg.map(|msg| msg.role) {
            Some(Role::User) | Some(Role::Tool) => {
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
                        let content = if let GeminiMessage::Response { candidates, .. } = response {
                            if let Some(first_candidate) = candidates.first() {
                                first_candidate.content.parts.iter()
                                    .filter_map(|part| part.text.clone())
                                    .map(|text| ContentItem::Text { text })
                                    .collect()
                            } else {
                                return Err(anyhow::anyhow!("No candidates in Gemini response"));
                            }
                        } else {
                            return Err(anyhow::anyhow!("Unexpected response type from Gemini"));
                        };

                        let new_msg = CommonMessage {
                            role: Role::Assistant,
                            content,
                        };
                        
                        // Convert CommonMessage back to GeminiMessage for storage
                        if let Ok(gemini_msg) = convert_common_to_gemini(&new_msg) {
                            self.messages.push(gemini_msg);
                        }
                        Ok(new_msg)
                    },
                    Err(e) => {
                        self.messages.pop();
                        Err(e.into())
                    }
                }
            }
            _ => Err(anyhow::anyhow!("Can only send messages with user or tool role when querying model."))
        }
    }
}
