use log::info;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::tree::GitTree;

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub model: String,
    pub check_cmd: String,
    #[serde(default)]
    pub api_url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub max_context: usize,
    #[serde(default)]
    pub max_output_tokens: u32,
    #[serde(default)]
    pub provider: String,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        ProjectConfig {
            model: String::from("claude-3-5-haiku-latest"),
            check_cmd: String::new(),
            api_url: String::from("https://api.anthropic.com/v1/messages"),
            api_key: String::new(),
            max_context: 100000,
            max_output_tokens: 8096,
            provider: String::from("anthropic"),
        }
    }
}

impl ProjectConfig {
    const CONFIG_FILE: &'static str = "pprog.toml";

    fn detect_check_cmd() -> String {
        let root_path = match GitTree::get_git_root() {
            Ok(root) => root,
            Err(e) => {
                println!("Unable to determine Git root directory: {}", e);
                std::process::exit(1);
            }
        };

        // Check for Rust project (Cargo.toml)
        if root_path.join("Cargo.toml").exists() {
            println!("Detected Rust project");
            return String::from("cargo check");
        }
        
        // TODO if Typescript project should look through package.json first for any kind of build
        // command and then default to running typescript compiler -> node {result of tsc}
        if root_path.join("tsconfig.json").exists() {
            println!("Detected TypeScript project");
            return String::from("tsc --noEmit");
        }
        
        // Check for Java project (gradlew)
        if root_path.join("gradlew").exists() {
            println!("Detected Java project");
            return String::from("./gradlew check");
        }
        
        if root_path.join("package.json").exists() {
            println!("Detected Node.js project");
            let package_json = std::fs::read_to_string(root_path.join("package.json")).unwrap_or_default();
            let package_data: serde_json::Value = serde_json::from_str(&package_json).unwrap_or_default();
            if let Some(main) = package_data.get("main").and_then(|v| v.as_str().map(String::from)) {
                return format!("node {}", main);
            }
        }

        println!("Unable to detect project type");
        String::new()
    }

    pub fn config_path() -> Result<PathBuf, String> {
        GitTree::get_git_root()
            .map(|root| root.join(Self::CONFIG_FILE))
            .map_err(|e| {
                info!("Unable to determine Git root directory: {}", e);
                format!("Unable to determine Git root directory: {}", e)
            })
    }

    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let config_path = Self::config_path()?;

        let content = fs::read_to_string(config_path)?;
        let config: ProjectConfig = toml::from_str(&content)?;
        info!("Loaded project config: {:?}", config);
        Ok(config)
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config_str = toml::to_string_pretty(self)?;
        let config_path = Self::config_path()?;
        fs::write(config_path, config_str)?;
        info!("Saved project config");
        Ok(())
    }

    pub fn init() -> Result<(), Box<dyn std::error::Error>> {
        let config_dir = match GitTree::get_git_root() {
            Ok(d) => d,
            Err(_) => {
                eprintln!("Could not find git root, please make sure git is initialized.");
                std::process::exit(1);
            }

        };
        let config_file = config_dir.join("pprog.toml");
        if config_file.exists() {
            return Err("Project already initialized".into());
        }

        // Try to get API key from environment first
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .or_else(|_| std::env::var("OPENAI_API_KEY"))
            .unwrap_or_default();

        // Detect appropriate check command
        let check_cmd = Self::detect_check_cmd();

        // Create config with detected values
        let config = ProjectConfig {
            model: String::from("claude-3-5-haiku-latest"),
            check_cmd,
            api_url: String::from("https://api.anthropic.com/v1"),
            api_key,
            max_context: 100000,
            max_output_tokens: 8096,
            provider: String::from("anthropic"),
        };
        config.save()?;

        info!("Initialized project with config: {:?}", config);
        Ok(())
    }
}
