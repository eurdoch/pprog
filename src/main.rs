mod inference;
mod chat;

use chat::ChatUI;
use clap::{Parser, Subcommand};
use crossterm::{event::{self, Event, KeyCode}, terminal};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(short, long)]
    force: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Init,
    Chat,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Init) => {
            println!("Initializing new project");
            // Perform initialization logic here
        }
        Some(Commands::Chat) => {
            terminal::enable_raw_mode()?;
            let mut chat = ChatUI::new();
            
            chat.add_message("Welcome to Chat! Press Esc to exit.".to_string(), false);
            chat.render()?;

            loop {
                if let Event::Key(key_event) = event::read()? {
                    match key_event.code {
                        KeyCode::Esc => break,
                        KeyCode::Enter => {
                            if !chat.input_buffer.is_empty() {
                                let message = std::mem::take(&mut chat.input_buffer);
                                chat.add_message(message, true);
                                // Here you would typically send the message to your chat backend
                                chat.add_message("I received your message".to_string(), false);
                            }
                        }
                        KeyCode::Backspace => {
                            chat.input_buffer.pop();
                        }
                        KeyCode::Char(c) => {
                            chat.input_buffer.push(c);
                        }
                        _ => {}
                    }
                    chat.render()?;
                }
            }

            terminal::disable_raw_mode()?;
        }
        None => {
            println!("No subcommand provided");
        }
    }

    Ok(())
}

