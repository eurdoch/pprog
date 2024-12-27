mod inference;
mod chat;
mod tree;
mod tooler;
mod config;

use std::fs::OpenOptions;

use chat::ChatUI;
use clap::{CommandFactory, Parser, Subcommand};
use config::ProjectConfig;
use crossterm::{event::{self, Event, KeyCode}, terminal};
use env_logger::{Builder, Target};
use inference::{ContentItem, Message, Role};
use tree::GitTree;
use std::io::Write;

struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
    }
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    //#[arg(short, long)]
    //force: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Initialize config for project")]
    Init,
    #[command(about = "Open chat window")]
    Chat,
}

fn setup_logger() -> Result<(), anyhow::Error> {
    let home_dir = dirs::home_dir().expect("Failed to get home directory");
    let cmon_dir = home_dir.join(".cmon");
    if !cmon_dir.exists() {
        std::fs::create_dir_all(&cmon_dir).expect("Failed to create .cmon directory");
    }
    let log_file_path = cmon_dir.join("log");

    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&log_file_path)?;

    let mut builder = Builder::from_default_env();
    builder
        .target(Target::Pipe(Box::new(file)))
        .format_timestamp_secs()
        // Add explicit level filter
        .filter_level(log::LevelFilter::Info)
        .init();

    Ok(())
}

async fn run_chat() -> Result<(), Box<dyn std::error::Error>> {
    terminal::enable_raw_mode()?;
    let _guard = TerminalGuard;  
    
    let mut chat = ChatUI::new();
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
                        if chat.input_buffer == "/exit" {
                            chat.cleanup()?;
                            break;
                        }
                        let user_input = std::mem::take(&mut chat.input_buffer);
                        let new_message = Message {
                            role: Role::User,
                            content: vec![
                                ContentItem::Text { text: user_input } 
                            ]
                        };
                        chat.add_message(new_message).await?;
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
    setup_logger()?;

    match &cli.command {
        Some(Commands::Init) => {
            if let Err(e) = ProjectConfig::init() {
                eprintln!("Failed to initialize project: {}", e);
            } else {
                    let git_root = match GitTree::get_git_root() {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("Git root not found: {}.  Please setup git before initializing.", e);
                        std::process::exit(1);
                    }
                };

                let gitignore_path = git_root.join(".gitignore");
                if gitignore_path.exists() {
                    let mut gitignore = std::fs::OpenOptions::new()
                        .write(true)
                        .append(true)
                        .open(gitignore_path)
                        .unwrap();
                    writeln!(gitignore, "cmon.toml").unwrap();
                }
                println!("Init successful.");
            }
        }
        Some(Commands::Chat) => {
            if let Err(e) = run_chat().await {
                // Ensure we clean up even on error
                terminal::disable_raw_mode()?;
                return Err(e);
            }
        }
        None => {
            let mut cmd = Cli::command();
            cmd.print_help()?;
        }
    }

    Ok(())
}

