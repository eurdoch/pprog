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

pub struct ChatUI {
    pub messages: Vec<(String, bool)>,
    pub input_buffer: String,
    terminal_width: u16,
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
        }
    }

    pub fn add_message(&mut self, message: &str, is_user: bool) {
        if !message.trim().eq_ignore_ascii_case("/exit") {
            // Sanitize and normalize the message
            let sanitized = message
                .replace('\t', "    ")  // Replace tabs with spaces
                .replace('\r', "")      // Remove carriage returns
                .trim()
                .to_string();
            
            self.messages.push((sanitized, is_user));
        } else {
            self.cleanup().unwrap();
            std::process::exit(0);
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
                // For continuation lines, add proper indentation
                writer.queue(cursor::MoveToNextLine(1))?;
                writer.queue(Print(" ".repeat(prefix_width)))?;
            }
            writer.queue(Print(line))?;
        }
        
        Ok(())
    }

    pub fn render(&self) -> io::Result<()> {
        let mut stdout = io::stdout();
        
        stdout.queue(Clear(ClearType::All))?;
        stdout.queue(cursor::MoveTo(0, 0))?;

        let max_width = self.terminal_width as usize;
        
        for (msg, is_user) in &self.messages {
            let color = if *is_user { Color::Green } else { Color::Blue };
            let prefix = if *is_user { "You: " } else { "Bot: " };
            let prefix_width = UnicodeSegmentation::graphemes(prefix, true).count();
            
            stdout.queue(cursor::MoveToColumn(0))?;
            stdout.queue(SetForegroundColor(color))?;
            stdout.queue(Print(prefix))?;
            stdout.queue(ResetColor)?;
            
            Self::write_wrapped_text(&mut stdout, msg, prefix_width, max_width)?;
            stdout.queue(cursor::MoveToNextLine(1))?;
        }

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
