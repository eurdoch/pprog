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
    scroll_offset: usize,
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
            scroll_offset: 0, 
        }
    }

    pub fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    pub fn scroll_down(&mut self, max_scroll: usize) {
        if self.scroll_offset < max_scroll {
            self.scroll_offset += 1;
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

    fn calculate_message_height(&self, message: &Message, max_width: usize) -> usize {
        let mut total_lines = 0;
        let prefix = if message.role == "user" { "You: " } else { "Bot: " };
        let prefix_width = UnicodeSegmentation::graphemes(prefix, true).count();
        
        for content_item in message.content.iter() {
            match content_item {
                ContentItem::Text { text } | ContentItem::ToolResult { content: text, .. } => {
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
                }
            }
        }
        total_lines
    }

    fn auto_scroll(&mut self) -> io::Result<()> {
        let (width, height) = crossterm::terminal::size()?;
        let max_width = width as usize;
        let visible_height = height.saturating_sub(2) as usize;

        // Calculate total lines needed for all messages
        let mut total_lines = 0;
        for message in &self.messages {
            total_lines += self.calculate_message_height(message, max_width);
        }

        // If total lines exceed visible height, auto-scroll to show latest messages
        if total_lines > visible_height {
            self.scroll_offset = total_lines.saturating_sub(visible_height);
        }
        
        Ok(())
    }

    pub fn add_message(&mut self, message: Message) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>> + '_>> {
        Box::pin(async move {
            self.messages.push(message.clone());
            self.auto_scroll()?;  // Auto-scroll after adding new message
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
        current_line: &mut u16,
    ) -> io::Result<()> {
        let wrap_width = max_width.saturating_sub(prefix_width);
        let options = Options::new(wrap_width)
            .break_words(true)
            .word_splitter(textwrap::WordSplitter::NoHyphenation);
        
        // Split text into paragraphs and handle each separately
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

    pub fn render(&self) -> io::Result<()> {
        let mut stdout = io::stdout();
        
        let (width, height) = crossterm::terminal::size().unwrap_or((80, 24));
        let max_width = width as usize;
        let max_height = height as usize;
        
        stdout.queue(Clear(ClearType::All))?;
        stdout.queue(cursor::MoveTo(0, 0))?;

        let mut current_line: u16 = 0;
        let visible_height = max_height.saturating_sub(2);

        // Calculate what messages to show based on scroll position
        let mut total_lines = 0;

        for message in self.messages.iter() {
            total_lines += self.calculate_message_height(message, max_width);
        }

        let max_scroll = total_lines.saturating_sub(visible_height);
        let effective_scroll = std::cmp::min(self.scroll_offset, max_scroll);
        let mut lines_skipped = 0;
        let mut message_index = 0;

        // Skip messages until we reach the correct scroll position
        while lines_skipped < effective_scroll && message_index < self.messages.len() {
            let message_height = self.calculate_message_height(&self.messages[message_index], max_width);
            if lines_skipped + message_height <= effective_scroll {
                lines_skipped += message_height;
                message_index += 1;
            } else {
                break;
            }
        }

        // Render messages starting from the correct position
        for message in self.messages.iter().skip(message_index) {
            if current_line >= visible_height as u16 {
                break;
            }

            let is_user = message.role == "user";
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

        // Render scroll indicators
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

        // Render input prompt
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
