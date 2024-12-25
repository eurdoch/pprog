mod inference;
mod chat;
mod tree;
mod tooler;

use chat::ChatUI;
use clap::{Parser, Subcommand};
use crossterm::{event::{self, Event, KeyCode}, terminal};
use tree::GitTree;
use inference::{Content, Inference};

struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
    }
}

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

async fn run_chat() -> Result<(), Box<dyn std::error::Error>> {
    terminal::enable_raw_mode()?;
    let _guard = TerminalGuard;  
    
    let mut chat = ChatUI::new();
    let inference = Inference::new();
    chat.render()?;

    loop {
        if let Event::Key(key_event) = event::read()? {
            match key_event.code {
                KeyCode::Esc => {
                    chat.cleanup()?;
                    break;
                }
                KeyCode::Enter => {
                    if !chat.input_buffer.is_empty() {
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
                        let message = std::mem::take(&mut chat.input_buffer);
                        chat.add_message(&message, true);
                        let response = inference.query_anthropic(&message, Some(&system_message)).await?;
                        for content in &response.content {
                            match content {
                                Content::Text(text_content) => {
                                    chat.add_message(&text_content.text, false);
                                }
                                Content::ToolUse(tool_use_content) => {
                                    if tool_use_content.name == "write_file" {
                                        match GitTree::get_git_root() {
                                            Ok(git_root_path) => {
                                                let path = tool_use_content.input.get("path")
                                                    .and_then(|v| v.as_str())
                                                    .ok_or_else(|| anyhow::anyhow!("Missing or invalid path in tool input"))?;
                                                let full_path = git_root_path.join(path);
                                                let content = tool_use_content.input.get("content")
                                                    .and_then(|v| v.as_str())
                                                    .ok_or_else(|| anyhow::anyhow!("Missing or invalid path in tool input"))?;

                                                chat.add_message(
                                                    &format!("Writing to file {:?}...", full_path),
                                                    false
                                                );
                                            }
                                            Err(e) => {
                                                chat.add_message(&format!("Error: {}", e), false);
                                            }
                                        }
                                    }

                                    chat.add_message(&format!("{:#?}", tool_use_content), false);
                                }
                            }
                        }
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

    Ok(())
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
            if let Err(e) = run_chat().await {
                // Ensure we clean up even on error
                terminal::disable_raw_mode()?;
                return Err(e);
            }
        }
        None => {
            println!("No subcommand provided");
        }
    }

    Ok(())
}
