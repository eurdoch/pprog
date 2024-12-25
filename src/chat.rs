use std::io::{self, Write};
use crossterm::{
    cursor,
    execute,
    terminal::{Clear, ClearType, disable_raw_mode, enable_raw_mode},
    style::{Color, Print, ResetColor, SetForegroundColor},
    QueueableCommand,
};
use unicode_segmentation::UnicodeSegmentation;
use textwrap::{wrap, Options};

use crate::{inference::{ContentItem, Inference, Message, Role, TextContent}, tree::GitTree};

pub struct ChatUI {
    pub messages: Vec<Message>,
    pub input_buffer: String,
    terminal_width: u16,
    inference: Inference,
}

impl ChatUI {
    pub fn new() -> Self {
        // Enable raw mode when creating the UI
        enable_raw_mode().unwrap();
        
        // Get terminal size
        let (width, _) = crossterm::terminal::size().unwrap_or((80, 24));
        
        Self {
            messages: Vec::new(),
            input_buffer: String::new(),
            terminal_width: width,
            inference: Inference::new(),
        }
    }

    pub async fn add_message(&mut self, message: Message) -> Result<(), anyhow::Error> {
        self.messages.push(message.clone());
        match message.role {
            Role::User => {
                let tree_string = GitTree::get_tree()?;
                let system_message = format!(
                    r#"
                    You are a coding assistant working on a project.
                    
                    File tree structure:
                    {}

                    The user will give you instructions on how to change the project code.
                    "#,
                    &tree_string,
                );
                let response = self.inference.query_anthropic(self.messages.clone(), Some(&system_message)).await?;
                self.messages.push(Message {
                    role: Role::Assistant,
                    content: vec![ContentItem::Text(TextContent {
                        // TODO this should be changed to &str or enum
                        content_type: "text".to_string(),
                        text: format!("{:#?}", response)
                    })],
                });
                //for content_item in &response.content {
                //    match content_item {
                //        ContentItem::Text(_) => {
                //            let new_message = Message {
                //                role: Role::Assistant,
                //                content: vec![content_item.clone()],
                //            };
                //            self.add_message(new_message);
                //        }
                //        ContentItem::ToolUse(_tool_use_content) => {
                //            //if tool_use_content.name == "write_file" {
                //            //    match GitTree::get_git_root() {
                //            //        Ok(git_root_path) => {
                //            //            let path = tool_use_content.input.get("path")
                //            //                .and_then(|v| v.as_str())
                //            //                .ok_or_else(|| anyhow::anyhow!("Missing or invalid path in tool input"))?;
                //            //            let full_path = git_root_path.join(path);
                //            //            let content = tool_use_content.input.get("content")
                //            //                .and_then(|v| v.as_str())
                //            //                .ok_or_else(|| anyhow::anyhow!("Missing or invalid path in tool input"))?;

                //            //            self.add_message();
                //            //        }
                //            //        Err(e) => {
                //            //            self.add_message(Message {
                //            //                role: Role::Assistant,
                //            //                content: 
                //            //            });
                //            //        }
                //            //    }
                //            //}

                //            //chat.add_message(&format!("{:#?}", tool_use_content), false);
                //        }
                //        _ => {}
                //    }
                //}
                Ok(())
            },
            Role::Assistant => Ok(()),
        }

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

    fn write_content<W: Write>(
        writer: &mut W,
        content: &ContentItem,
        prefix_width: usize,
        max_width: usize,
    ) -> io::Result<()> {
        match content {
            ContentItem::Text(text_content) => {
                Self::write_wrapped_text(writer, &text_content.text, prefix_width, max_width)?;
            }
            ContentItem::ToolUse(tool_use) => {
                let tool_text = format!(
                    "[Tool Use - {}: {}]",
                    tool_use.name,
                    serde_json::to_string_pretty(&tool_use.input).unwrap_or_default()
                );
                Self::write_wrapped_text(writer, &tool_text, prefix_width, max_width)?;
            }
            ContentItem::ToolResult(tool_result) => {
                let result_text = format!(
                    "[Tool Result: {}]",
                    tool_result.content
                );
                Self::write_wrapped_text(writer, &result_text, prefix_width, max_width)?;
            }
        }
        Ok(())
    }

    pub fn render(&self) -> io::Result<()> {
        let mut stdout = io::stdout();
        
        stdout.queue(Clear(ClearType::All))?;
        stdout.queue(cursor::MoveTo(0, 0))?;

        let max_width = self.terminal_width as usize;
        
        for message in &self.messages {
            let is_user = message.role == "user";
            let color = if is_user { Color::Green } else { Color::Blue };
            let prefix = if is_user { "You: " } else { "Bot: " };
            let prefix_width = UnicodeSegmentation::graphemes(prefix, true).count();
            
            stdout.queue(cursor::MoveToColumn(0))?;
            stdout.queue(SetForegroundColor(color))?;
            stdout.queue(Print(prefix))?;
            stdout.queue(ResetColor)?;

            // Handle each content item in the message
            for (i, content) in message.content.iter().enumerate() {
                if i > 0 {
                    stdout.queue(cursor::MoveToNextLine(1))?;
                    stdout.queue(Print(" ".repeat(prefix_width)))?;
                }
                Self::write_content(&mut stdout, content, prefix_width, max_width)?;
            }
            
            stdout.queue(cursor::MoveToNextLine(1))?;
        }

        // Render input prompt
        stdout
            .queue(cursor::MoveToColumn(0))?
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
