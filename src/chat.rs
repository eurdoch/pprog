use std::{future::Future, io::{self, Write}, pin::Pin, process::Command};
use crossterm::{
    cursor,
    execute,
    terminal::{Clear, ClearType, disable_raw_mode, enable_raw_mode},
    style::{Color, Print, ResetColor, SetForegroundColor},
    QueueableCommand,
};
use unicode_segmentation::UnicodeSegmentation;
use textwrap::{wrap, Options};
use crate::{inference::{ContentItem, Inference, Message, Role}, tree::GitTree};

pub struct ChatUI {
    pub messages: Vec<Message>,
    pub input_buffer: String,
    inference: Inference,
}

impl ChatUI {
    pub fn new() -> Self {
        // Enable raw mode when creating the UI
        enable_raw_mode().unwrap();

        Self {
            messages: Vec::new(),
            input_buffer: String::new(),
            inference: Inference::new(),
        }
    }

    fn extract_string_field<'a>(
        &self,
        input: &'a serde_json::Value,
        field_name: &str
    ) -> Result<&'a str, String> {
        input.get(field_name)
            .ok_or_else(|| format!("Missing '{}' field in tool input: {:?}", field_name, input))?
            .as_str()
            .ok_or_else(|| format!("'{}' field is not a string: {:?}", field_name, input.get(field_name)))
    }

    pub fn add_message(&mut self, message: Message) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>> + '_>> {
        Box::pin(async move {
            self.messages.push(message.clone());
            self.render()?;

            match message.role {
                Role::User => {
                    let tree_string = GitTree::get_tree()?;
                    // TODO this should be moved to Inference structure
                    let system_message = format!(
                        r#"
                        You are a coding assistant working on a project.
                        
                        File tree structure:
                        {}

                        The user will give you instructions on how to change the project code.

                        If you use tool 'write_file' successfully and tool 'compile_check' is available, call compile_check.  If compile_check shows any errors, make subsequent calls to correct the errors. Continue checking and rewriting until there are no more errors.  If there are warnings then do not try to fix them, just let the user know.  If any bash commands are needed like installing packages use tool 'execute'.

                        Never make any changes outside of the project's root directory.
                        "#,
                        &tree_string,
                    );
                    let response = self.inference.query_anthropic(self.messages.clone(), Some(&system_message)).await?;

                    for content_item in &response.content {
                        match content_item {
                            ContentItem::Text { text, .. } => {
                                let new_message = Message {
                                    role: Role::Assistant,
                                    content: vec![
                                        ContentItem::Text { text: text.to_string() }
                                    ]
                                };
                                self.add_message(new_message).await?;
                            }
                            ContentItem::ToolUse { name, input, id, .. } => {
                                self.add_message(Message {
                                    role: Role::Assistant,
                                    content: vec![
                                        ContentItem::ToolUse { 
                                            id: id.to_string(), 
                                            name: name.to_string(), 
                                            input: input.clone(),
                                        }
                                    ]
                                }).await?;

                                match GitTree::get_git_root() {
                                    Ok(root_path) => {
                                        if name == "write_file" {
                                            let content = match self.extract_string_field(input, "content") {
                                                Ok(content) => content,
                                                Err(error_msg) => {
                                                    self.add_message(Message {
                                                        role: Role::Assistant,
                                                        content: vec![
                                                            ContentItem::Text { text: error_msg }
                                                        ]
                                                    }).await?;
                                                    return Ok(());
                                                }
                                            };
                                            let file_path = match self.extract_string_field(input, "path") {
                                                Ok(content) => content,
                                                Err(error_msg) => {
                                                    self.add_message(Message {
                                                        role: Role::Assistant,
                                                        content: vec![
                                                            ContentItem::Text { text: error_msg }
                                                        ]
                                                    }).await?;
                                                    return Ok(());
                                                }
                                            };

                                            let full_path = root_path.join(file_path);
                                            let tool_result_message = match std::fs::write(full_path.clone(), content) {
                                                Ok(_) => format!("Successfully wrote content to file {:?}.", full_path), 
                                                Err(e) => format!("Error writing to file {:?}: {:?}.", full_path, e), 
                                            };
                                            self.add_message(Message {
                                                role: Role::User,
                                                content: vec![
                                                    ContentItem::ToolResult { 
                                                        tool_use_id: id.to_string(), 
                                                        content: tool_result_message,
                                                    }
                                                ]
                                            }).await?;
                                            // TODO do compile check and if errors or warnings make
                                            // another call
                                        } else if name == "read_file" {
                                            let file_path = match self.extract_string_field(input, "path") {
                                                Ok(content) => content,
                                                Err(error_msg) => {
                                                    self.add_message(Message {
                                                        role: Role::Assistant,
                                                        content: vec![
                                                            ContentItem::Text { text: error_msg }
                                                        ]
                                                    }).await?;
                                                    return Ok(());
                                                }
                                            };
                                            let full_path = root_path.join(file_path);
                                            let tool_result_message = match std::fs::read_to_string(full_path.clone()) {
                                                Ok(file_content) => file_content,
                                                Err(e) => format!("Error reading file {:?}: {:?}.", full_path, e),
                                            };
                                            self.add_message(Message {
                                                role: Role::User,
                                                content: vec![
                                                    ContentItem::ToolResult { 
                                                        tool_use_id: id.to_string(), 
                                                        content: tool_result_message,
                                                    }
                                                ]
                                            }).await?;
                                        } else if name == "compile_check" {
                                            let check_cmd = match self.extract_string_field(input, "cmd") {
                                                Ok(cmd) => cmd,
                                                Err(e) => {
                                                    self.add_message(Message {
                                                        role: Role::Assistant,
                                                        content: vec![
                                                            ContentItem::Text { text: e }
                                                        ]
                                                    }).await?;
                                                    return Ok(());
                                                }
                                            };
                                            let output = Command::new("bash")
                                                .arg("-c")
                                                .arg(format!("{} & sleep 1; kill $!", check_cmd))  // Run command in background, wait 1 sec, then kill it
                                                .current_dir(root_path)
                                                .output()
                                                .expect("Failed to execute command");

                                            let stdout = String::from_utf8_lossy(&output.stdout);
                                            let stderr = String::from_utf8_lossy(&output.stderr);
                                            let tool_result_message = format!("Stdout:
{}
Stderr:
{}", stdout, stderr);
                                            self.add_message(Message {
                                                role: Role::User,
                                                content: vec![
                                                    ContentItem::ToolResult {
                                                        tool_use_id: id.to_string(),
                                                        content: tool_result_message,
                                                    }
                                                ]
                                            }).await?;
                                       } else if name == "execute" {
                                            let statement = match self.extract_string_field(input, "statement") {
                                                Ok(cmd) => cmd,
                                                Err(e) => {
                                                    self.add_message(Message {
                                                        role: Role::Assistant,
                                                        content: vec![
                                                            ContentItem::Text { text: e }
                                                        ]
                                                    }).await?;
                                                    return Ok(());
                                                }
                                            };
                                            let output = Command::new("bash")
                                                .arg("-c")
                                                .arg(statement)
                                                .current_dir(root_path)
                                                .output()
                                                .expect("Failed to execute command");

                                            let stdout = String::from_utf8_lossy(&output.stdout);
                                            let stderr = String::from_utf8_lossy(&output.stderr);
                                            let tool_result_message = format!("Stdout:
{}
Stderr:
{}", stdout, stderr);
                                            self.add_message(Message {
                                                role: Role::User,
                                                content: vec![
                                                    ContentItem::ToolResult {
                                                        tool_use_id: id.to_string(),
                                                        content: tool_result_message,
                                                    }
                                                ]
                                            }).await?;
                                       }
                                    },

                                    Err(e) => {
                                        self.add_message(Message {
                                            role: Role::Assistant,
                                            content: vec![
                                                ContentItem::Text { text: format!("Error getting git root: {}", e) }
                                            ]
                                        }).await?;
                                        return Ok(());
                                    }
                                };
                            }
                            _ => {}
                        }
                    }
                    Ok(())
                },
                Role::Assistant => Ok(()),
            }
        })
    }

    pub fn cleanup(&self) -> io::Result<()> {
        execute!(
            io::stdout(),
            Clear(ClearType::All),
            cursor::MoveTo(0, 0),
            cursor::Show,
            ResetColor
        )?;
        
        disable_raw_mode()?;
        Ok(())
    }

    fn write_wrapped_text<W: Write>(
        writer: &mut W,
        text: &str,
        prefix_width: usize,
        max_width: usize,
    ) -> io::Result<()> {
        let wrap_width = max_width.saturating_sub(prefix_width);
        let options = Options::new(wrap_width)
            .break_words(true)
            .word_splitter(textwrap::WordSplitter::NoHyphenation);
        
        let wrapped_lines = wrap(text, options);
        
        for (i, line) in wrapped_lines.iter().enumerate() {
            if i > 0 {
                writer.queue(cursor::MoveToNextLine(1))?;
                writer.queue(Print(" ".repeat(prefix_width)))?;
            }
            writer.queue(Print(line))?;
        }
        
        Ok(())
    }

    pub fn render(&self) -> io::Result<()> {
        let mut stdout = io::stdout();
        
        // Get current terminal dimensions
        let (width, height) = crossterm::terminal::size().unwrap_or((80, 24));
        let max_width = width as usize;
        let max_height = height as usize;
        
        // Clear screen and move to top
        stdout.queue(Clear(ClearType::All))?;
        stdout.queue(cursor::MoveTo(0, 0))?;

        // Calculate total lines needed for all messages
        let mut total_lines = 0;
        let messages_with_lines: Vec<(usize, &Message)> = self.messages.iter().map(|message| {
            let mut message_lines = 0;
            
            // Count lines for each content item
            for content_item in &message.content {
                let text = match content_item {
                    ContentItem::Text { text } => text,
                    ContentItem::ToolResult { content, .. } => {
                        message_lines += 1; // Account for the tool result header
                        content
                    },
                    ContentItem::ToolUse { .. } => {
                        message_lines += 1; // Account for the tool use header
                        continue;
                    }
                };
                
                // Calculate wrapped lines for the text
                let prefix_width = 5; // "You: " or "Bot: "
                let wrap_width = max_width.saturating_sub(prefix_width);
                let options = Options::new(wrap_width)
                    .break_words(true)
                    .word_splitter(textwrap::WordSplitter::NoHyphenation);
                
                message_lines += wrap(text, options).len();
            }
            
            message_lines += 1; // Account for the newline after each message
            total_lines += message_lines;
            (message_lines, message)
        }).collect();

        // Calculate how many messages we can show
        // Reserve 2 lines for input prompt
        let available_lines = max_height.saturating_sub(2);
        let mut lines_to_show = 0;
        let mut start_index = messages_with_lines.len();

        // Find the starting message that will fit in the available space
        for (msg_lines, _) in messages_with_lines.iter().rev() {
            if lines_to_show + msg_lines > available_lines {
                break;
            }
            lines_to_show += msg_lines;
            start_index -= 1;
        }

        // Render only the messages that fit
        let mut current_line = 0;
        for (msg_lines, message) in messages_with_lines.iter().skip(start_index) {
            let is_user = message.role == "user";
            let color = if is_user { Color::Green } else { Color::Blue };
            let prefix = if is_user { "You: " } else { "Bot: " };
            let prefix_width = UnicodeSegmentation::graphemes(prefix, true).count();
            
            stdout.queue(cursor::MoveTo(0, current_line as u16))?;
            stdout.queue(SetForegroundColor(color))?;
            stdout.queue(Print(prefix))?;
            stdout.queue(ResetColor)?;

            for content_item in &message.content {
                match content_item {
                    ContentItem::Text { text } => {
                        Self::write_wrapped_text(&mut stdout, text, prefix_width, max_width)?;
                    },
                    ContentItem::ToolResult { tool_use_id, .. } => {
                        let result_text = format!("Tool result - {}", tool_use_id);
                        Self::write_wrapped_text(&mut stdout, &result_text, prefix_width, max_width)?;
                    },
                    ContentItem::ToolUse { id, name, .. } => {
                        let result_text = format!("Tool use {} - {}", id, name);
                        Self::write_wrapped_text(&mut stdout, &result_text, prefix_width, max_width)?;
                    }
                }
            }
            
            current_line += *msg_lines as u16;
        }

        // Render input prompt at the bottom
        stdout
            .queue(cursor::MoveTo(0, (max_height - 1) as u16))?
            .queue(SetForegroundColor(Color::Yellow))?
            .queue(Print("> "))?
            .queue(ResetColor)?
            .queue(Print(&self.input_buffer))?;

        stdout.flush()?;
        Ok(())
    }
}

impl Drop for ChatUI {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}
