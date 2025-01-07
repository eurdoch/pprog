use crate::config::ProjectConfig;
use crate::inference::deepseek::DeepSeekMessage;
use crate::inference::DeepSeekInference;
use crate::tree::GitTree;

use super::chat::{convert_to_common_message, convert_to_deepseek_message, Chat, CommonMessage, Role};
use super::tools::Tools;
use async_trait::async_trait;

pub struct DeepSeekChat {
    pub messages: Vec<DeepSeekMessage>,
    inference: DeepSeekInference,
    max_tokens: usize,
}

#[async_trait]
impl Chat for DeepSeekChat {
    async fn new() -> Self where Self: Sized {
        let config = ProjectConfig::load().unwrap_or_default();

        Self {
            messages: Vec::new(),
            inference: DeepSeekInference::new(),
            max_tokens: config.max_context,
        }
    }

    // TODO add total token count when handling repsonse and check against hat value
    //self.trim_messages_to_token_limit();
    async fn handle_message(&mut self, common_message: &CommonMessage) -> Result<CommonMessage, anyhow::Error> {
        let deepseek_message = convert_to_deepseek_message(common_message)?;

        self.messages.push(deepseek_message.clone());
        if let Some(tool_calls_vec) = deepseek_message.tool_calls {
            // Only supports single tool use for now
            let tool = tool_calls_vec[0].clone();
            let tool_use_result = match Tools::handle_tool_use(tool.function.name, tool.function.arguments) {
                Ok(c) => Ok(c),
                Err(e) => {
                    self.messages.pop();
                    Err(e)
                }
            }?;

            let tool_result_msg = DeepSeekMessage {
                role: Role::Tool,
                tool_call_id: Some(tool.id),
                content: tool_use_result,
                tool_calls: None,
            };
            //self.messages.push(tool_result_msg.clone());
            let tool_result_common_msg = convert_to_common_message(&tool_result_msg);
            Ok(tool_result_common_msg)
        } else {
            println!("{:#?}", self.messages.clone());
            match self.send_messages().await {
                Ok(return_msg) => {
                    Ok(convert_to_common_message(&return_msg))  
                },
                Err(e) => Err(e),
            }
        }
    }

    fn get_messages(&self) -> Vec<CommonMessage> {
        let messages: Vec<CommonMessage> = self.messages.iter()
            .map(|msg| convert_to_common_message(msg))
            .collect();
        messages
    }


    fn clear(&mut self) {
        self.messages.clear();
    }
}

impl DeepSeekChat {    
    async fn send_messages(&mut self) -> Result<DeepSeekMessage, anyhow::Error> {
        if let Some(Role::User) = self.messages.last().map(|m| m.role.clone()) {
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
                // TODO add token counts from response to running total
                Ok(response) => Ok(response.choices[0].message.clone()),
                Err(e) => {
                    self.messages.pop();
                    Err(e.into())
                }
            }
        } else {
            Err(anyhow::anyhow!("Can only send messages with user role when querying model."))
        }
    }
}
