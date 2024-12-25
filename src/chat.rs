use std::io::{self, Write};
use crossterm::{
    cursor,
    execute,
    terminal::{Clear, ClearType},
    style::{Color, Print, ResetColor, SetForegroundColor},
};

pub struct ChatUI {
    pub messages: Vec<(String, bool)>,
    pub input_buffer: String,
}

impl ChatUI {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            input_buffer: String::new(),
        }
    }

    pub fn add_message(&mut self, message: String, is_user: bool) {
        if !message.trim().eq_ignore_ascii_case("/exit") {
            self.messages.push((message.trim().to_string(), is_user));
        } else {
            std::process::exit(0);
        }
    }

    pub fn render(&self) -> io::Result<()> {
        // Clear screen and reset cursor
        execute!(
            io::stdout(),
            Clear(ClearType::All),
            cursor::MoveTo(0, 0)
        )?;

        // Print messages with proper alignment
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

        // Print input prompt at the bottom
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
