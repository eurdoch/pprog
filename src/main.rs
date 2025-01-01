mod inference;
mod chat;
mod tree;
mod config;
mod server;

use std::fs::OpenOptions;
use std::io::Write;

use clap::{CommandFactory, Parser, Subcommand};
use config::ProjectConfig;
use env_logger::{Builder, Target};
use tree::GitTree;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Initialize config for project")]
    Init,
    #[command(about = "Start the API server")]
    Serve {
        #[arg(short, long, default_value = "127.0.0.1")]
        host: String,
        #[arg(short, long, default_value_t = 8080)]
        port: u16,
    },
}

fn setup_logger() -> Result<(), anyhow::Error> {
    let home_dir = dirs::home_dir().expect("Failed to get home directory");
    let pprog_dir = home_dir.join(".pprog");
    if !pprog_dir.exists() {
        std::fs::create_dir_all(&pprog_dir).expect("Failed to create .pprog directory");
    }
    let log_file_path = pprog_dir.join("log");

    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&log_file_path)?;

    let mut builder = Builder::from_default_env();
    builder
        .target(Target::Pipe(Box::new(file)))
        .format_timestamp_secs()
        .filter_level(log::LevelFilter::Info)
        .init();

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
                let mut gitignore_contents = String::new();

                if gitignore_path.exists() {
                    gitignore_contents = std::fs::read_to_string(&gitignore_path).unwrap();
                }

                if !gitignore_contents.contains("pprog.toml") {
                    println!("Adding config to .gitignore.");
                    let mut gitignore = std::fs::OpenOptions::new()
                        .write(true)
                        .append(true)
                        .create(true)
                        .open(gitignore_path)
                        .unwrap();

                    writeln!(gitignore, r#"
# pprog config
pprog.toml
"#)?;
                }

                println!("Init successful.");
            }
        }
        Some(Commands::Serve { host, port }) => {
            server::start_server(host.clone(), *port).await?;
        }
        None => {
            let mut cmd = Cli::command();
            cmd.print_help()?;
        }
    }

    Ok(())
}