use std::{future::Future, io::{self, Write}, pin::Pin};
use crossterm::{
    cursor,
    execute,
    terminal::{Clear, ClearType, disable_raw_mode, enable_raw_mode},
    style::{Color, Print, ResetColor, SetForegroundColor},
    QueueableCommand,
};
use unicode_segmentation::UnicodeSegmentation;
use textwrap::{wrap, Options};

use crate::{inference::{ContentItem, Inference, Message, MessageContent, Role}, tree::GitTree};

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

    pub fn add_message(&mut self, message: Message) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>> + '_>> {
        Box::pin(async move {
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
                    for content_item in &response.content {
                        match content_item {
                            ContentItem::Text { text, .. } => {
                                let new_message = Message {
                                    role: Role::Assistant,
                                    content: MessageContent::Text(text.to_string())
                                };
                                self.add_message(new_message).await?;
                            }
                            ContentItem::ToolUse { name, input, .. } => {
                                // Tool use handling code here
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

    fn write_content<W: Write>(
        writer: &mut W,
        content_item: &ContentItem,
        prefix_width: usize,
        max_width: usize,
    ) -> io::Result<()> {
        match content_item {
            ContentItem::Text { text, .. } => {
                Self::write_wrapped_text(writer, text, prefix_width, max_width)?;
            }
            ContentItem::ToolUse { name, input, .. } => {
                let tool_text = format!(
                    "[Tool Use - {}: {}]",
                    name,
                    serde_json::to_string_pretty(&input).unwrap_or_default()
                );
                Self::write_wrapped_text(writer, &tool_text, prefix_width, max_width)?;
            }
            ContentItem::ToolResult { content, .. } => {
                let result_text = format!(
                    "[Tool Result: {}]",
                    content
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

            // Handle the content based on its type
            match &message.content {
                MessageContent::Text(text) => {
                    Self::write_wrapped_text(&mut stdout, text, prefix_width, max_width)?;
                }
                MessageContent::Items(items) => {
                    for (i, content) in items.iter().enumerate() {
                        if i > 0 {
                            stdout.queue(cursor::MoveToNextLine(1))?;
                            stdout.queue(Print(" ".repeat(prefix_width)))?;
                        }
                        Self::write_content(&mut stdout, content, prefix_width, max_width)?;
                    }
                }
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
