use std::io::{self, Write};
use std::process::Command;
use crossterm::{
    cursor,
    execute,
    terminal::{Clear, ClearType, disable_raw_mode, enable_raw_mode},
    style::{Color, Print, ResetColor, SetForegroundColor},
    QueueableCommand,
};
use unicode_segmentation::UnicodeSegmentation;
use textwrap::{wrap, Options};
use crate::{inference::{ContentItem, Inference, Message, Role, ModelResponse}, tree::GitTree};

// Core chat functionality separated from UI
pub struct Chat {
    pub messages: Vec<Message>,
    inference: Inference,
}

impl Chat {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            inference: Inference::new(),
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

    // TODO get rid of message parameter and just use existingb messages
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
                "#,
                &tree_string,
            );
            let response = self.inference.query_anthropic(self.messages.clone(), Some(&system_message)).await?;
            Ok(response)
        } else {
            Err(anyhow::anyhow!("Can only send messages with user role when querying model."))
        }
    }


    // TODO should refactor to Tooler struct
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
                                format!("Stdout:\n{}\nStderr:\n{}", stdout, stderr)
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
                                format!("Stdout:\n{}\nStderr:\n{}", stdout, stderr)
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

// UI Component that uses the Chat functionality
pub struct ChatUI {
    pub chat: Chat,
    pub input_buffer: String,
    scroll_offset: usize,
}

impl ChatUI {
    pub fn new() -> Self {
        enable_raw_mode().unwrap();

        Self {
            chat: Chat::new(),
            input_buffer: String::new(),
            scroll_offset: 0,
        }
    }

    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    pub fn scroll_down(&mut self, max_scroll: usize) {
        if self.scroll_offset < max_scroll {
            self.scroll_offset += 1;
        }
    }

    fn calculate_message_height(&self, message: &Message, max_width: usize) -> usize {
        let mut total_lines = 0;
        let prefix = if message.role == Role::User { "You: " } else { "Bot: " };
        let prefix_width = UnicodeSegmentation::graphemes(prefix, true).count();
        
        for content_item in message.content.iter() {
            match content_item {
                ContentItem::Text { text } => {
                    let wrap_width = max_width.saturating_sub(prefix_width);
                    let options = Options::new(wrap_width)
                        .break_words(true)
                        .word_splitter(textwrap::WordSplitter::NoHyphenation);
                    
                    for paragraph in text.split('\n') {
                        if !paragraph.trim().is_empty() {
                            total_lines += wrap(paragraph.trim(), options.clone()).len();
                        }
                    }
                },
                ContentItem::ToolUse { id, name, .. } => {
                    let tool_text = format!("Tool use {} - {}", id, name);
                    let wrap_width = max_width.saturating_sub(prefix_width);
                    let options = Options::new(wrap_width)
                        .break_words(true)
                        .word_splitter(textwrap::WordSplitter::NoHyphenation);
                    
                    total_lines += wrap(&tool_text, options).len();
                },
                ContentItem::ToolResult { tool_use_id, .. } => {
                    let tool_text = format!("Tool result {}", tool_use_id);
                    let wrap_width = max_width.saturating_sub(prefix_width);
                    let options = Options::new(wrap_width)
                        .break_words(true)
                        .word_splitter(textwrap::WordSplitter::NoHyphenation);
                    
                    total_lines += wrap(&tool_text, options).len();
                }
            }
        }
        total_lines
    }

    fn auto_scroll(&mut self) -> io::Result<()> {
        let (width, height) = crossterm::terminal::size()?;
        let max_width = width as usize;
        let visible_height = height.saturating_sub(2) as usize;

        let mut total_lines = 0;
        for message in &self.chat.messages {
            total_lines += self.calculate_message_height(message, max_width);
        }

        if total_lines > visible_height {
            self.scroll_offset = total_lines.saturating_sub(visible_height);
        }
        
        Ok(())
    }

    // TODO should probablyu remove parameter and use state within Chat structure
    pub async fn process_message(&mut self, message: Message) -> Result<(), anyhow::Error> {
        self.chat.messages.push(message.clone());
        self.render()?;
        
        let response = self.chat.send_message(message).await?; 
        self.chat.messages.push(Message {
            role: Role::Assistant,
            content: response.content.clone(),
        });
        self.render()?;
        
        for content_item in response.content {
            if let ContentItem::ToolUse { ref id, .. } = content_item {
                let tool_result_content = self.chat.handle_tool_use(&content_item).await?;
                let future = Box::pin(self.process_message(Message {
                    role: Role::User,
                    content: vec![ContentItem::ToolResult { 
                        tool_use_id: id.clone(), 
                        content: tool_result_content 
                    }],
                }));
                future.await?;
            }
        }
        
        self.render()?;
        self.auto_scroll()?;
        Ok(())
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
        current_line: &mut u16,
    ) -> io::Result<()> {
        let wrap_width = max_width.saturating_sub(prefix_width);
        let options = Options::new(wrap_width)
            .break_words(true)
            .word_splitter(textwrap::WordSplitter::NoHyphenation);
        
        let paragraphs: Vec<&str> = text.split("\n").collect();
        let mut first_line = true;
        
        for paragraph in paragraphs {
            if !paragraph.trim().is_empty() {
                let wrapped_lines = wrap(paragraph.trim(), options.clone());
                
                for line in wrapped_lines {
                    if !first_line {
                        writer.queue(cursor::MoveTo(prefix_width as u16, *current_line))?;
                    }
                    writer.queue(Print(&line))?;
                    first_line = false;
                    *current_line += 1;
                }
            }
        }
        
        Ok(())
    }

    pub fn render(&mut self) -> io::Result<()> {
        let mut stdout = io::stdout();
        
        let (width, height) = crossterm::terminal::size().unwrap_or((80, 24));
        let max_width = width as usize;
        let max_height = height as usize;
        
        stdout.queue(Clear(ClearType::All))?;
        stdout.queue(cursor::MoveTo(0, 0))?;

        let mut current_line: u16 = 0;
        let visible_height = max_height.saturating_sub(2);

        let mut total_lines = 0;
        for message in self.chat.messages.iter() {
            total_lines += self.calculate_message_height(message, max_width);
        }

        let max_scroll = total_lines.saturating_sub(visible_height);
        let effective_scroll = std::cmp::min(self.scroll_offset, max_scroll);
        let mut lines_skipped = 0;
        let mut message_index = 0;

        while lines_skipped < effective_scroll && message_index < self.chat.messages.len() {
            let message_height = self.calculate_message_height(&self.chat.messages[message_index], max_width);
            if lines_skipped + message_height <= effective_scroll {
                lines_skipped += message_height;
                message_index += 1;
            } else {
                break;
            }
        }

        for message in self.chat.messages.iter().skip(message_index) {
            if current_line >= visible_height as u16 {
                break;
            }

            let is_user = message.role == Role::User;
            let color = if is_user { Color::Green } else { Color::Blue };
            let prefix = if is_user { "You: " } else { "Bot: " };
            let prefix_width = UnicodeSegmentation::graphemes(prefix, true).count();

            stdout.queue(cursor::MoveTo(0, current_line))?;
            stdout.queue(SetForegroundColor(color))?;
            stdout.queue(Print(prefix))?;
            stdout.queue(ResetColor)?;

            for content_item in &message.content {
                match content_item {
                    ContentItem::Text { text } => {
                        Self::write_wrapped_text(&mut stdout, text, prefix_width, max_width, &mut current_line)?;
                    },
                    ContentItem::ToolResult { tool_use_id, .. } => {
                        let text = format!("Tool Result - {}", tool_use_id);
                        Self::write_wrapped_text(&mut stdout, &text, prefix_width, max_width, &mut current_line)?;
                    },
                    ContentItem::ToolUse { id, name, .. } => {
                        let tool_text = format!("Tool use {} - {}", id, name);
                        Self::write_wrapped_text(&mut stdout, &tool_text, prefix_width, max_width, &mut current_line)?;
                    }
                }
            }
        }

        if effective_scroll > 0 {
            stdout.queue(cursor::MoveTo(width - 1, 0))?
                .queue(SetForegroundColor(Color::DarkGrey))?
                .queue(Print("↑"))?
                .queue(ResetColor)?;
        }
        if effective_scroll < max_scroll {
            stdout.queue(cursor::MoveTo(width - 1, (max_height - 3) as u16))?
                .queue(SetForegroundColor(Color::DarkGrey))?
                .queue(Print("↓"))?
                .queue(ResetColor)?;
        }

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
