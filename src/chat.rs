use std::io::{self, Write};
use crossterm::{
    cursor,
    execute,
    terminal::{self, Clear, ClearType, disable_raw_mode, enable_raw_mode},
    style::{Color, Print, ResetColor, SetForegroundColor},
};

pub struct ChatUI {
    pub messages: Vec<(String, bool)>,
    pub input_buffer: String,
}

impl ChatUI {
    pub fn new() -> Self {
        // Enable raw mode when creating the UI
        enable_raw_mode().unwrap();
        Self {
            messages: Vec::new(),
            input_buffer: String::new(),
        }
    }

    pub fn add_message(&mut self, message: &str, is_user: bool) {
        if !message.trim().eq_ignore_ascii_case("/exit") {
            self.messages.push((message.trim().to_string(), is_user));
        } else {
            self.cleanup().unwrap();
            std::process::exit(0);
        }
    }

    // Add cleanup method
    pub fn cleanup(&self) -> io::Result<()> {
        // Clear the screen one last time
        execute!(
            io::stdout(),
            Clear(ClearType::All),
            cursor::MoveTo(0, 0)
        )?;
        
        // Disable raw mode
        disable_raw_mode()?;
        
        // Show the cursor and reset color
        execute!(
            io::stdout(),
            cursor::Show,
            ResetColor
        )?;
        
        Ok(())
    }

    pub fn render(&self) -> io::Result<()> {
        execute!(
            io::stdout(),
            Clear(ClearType::All),
            cursor::MoveTo(0, 0)
        )?;

        for (msg, is_user) in &self.messages {
            let color = if *is_user { Color::Green } else { Color::Blue };
            let prefix = if *is_user { "You: " } else { "Bot: " };
            
            execute!(
                io::stdout(),
                cursor::MoveToColumn(0),
                SetForegroundColor(color),
                Print(prefix),
                ResetColor,
                Print(msg),
                cursor::MoveToNextLine(1)
            )?;
        }

        execute!(
            io::stdout(),
            cursor::MoveToColumn(0),
            SetForegroundColor(Color::Yellow),
            Print("> "),
            ResetColor,
            Print(&self.input_buffer)
        )?;

        io::stdout().flush()?;
        Ok(())
    }
}

// Implement Drop to ensure cleanup happens even on panic
impl Drop for ChatUI {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}
