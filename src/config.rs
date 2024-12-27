use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::tree::GitTree;

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub model: String,
    pub check_cmd: String,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        ProjectConfig {
            model: String::from("claude-3-5-sonnet-latest"),
            check_cmd: String::from("echo 'No check command configured'"),
        }
    }
}

impl ProjectConfig {
    const CONFIG_FILE: &'static str = "cmon.toml";

    fn detect_check_cmd() -> String {
        let root_path = match GitTree::get_git_root() {
            Ok(root) => root.join(Self::CONFIG_FILE),
            Err(e) => {
                eprintln!("Unable to determine Git root directory: {}", e);
                std::process::exit(1);
            }
        };

        
        // Check for Rust project (Cargo.toml)
        if root_path.join("Cargo.toml").exists() {
            return String::from("cargo check");
        }
        
        // TODO should run code through node after tsc and check for errors
        // Check for TypeScript project (tsconfig.json)
        if root_path.join("tsconfig.json").exists() {
            return String::from("tsc --noEmit");
        }
        
        // Check for Java project (gradlew)
        if root_path.join("gradlew").exists() {
            return String::from("./gradlew check");
        }
        
        // TODO should run code through node to check for errors
        if root_path.join("package.json").exists() {
            // If we find eslint config, use that for checking
            if root_path.join(".eslintrc.json").exists() || root_path.join(".eslintrc.js").exists() {
                return String::from("eslint .");
            }
            return String::from("npm run lint");
        }
        
        String::from("echo 'No check command configured'")
    }

    pub fn config_path() -> Result<PathBuf, String> {
        GitTree::get_git_root()
            .map(|root| root.join(Self::CONFIG_FILE))
            .map_err(|e| format!("Unable to determine Git root directory: {}", e))
    }

    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let config_path = Self::config_path()?;

        let content = fs::read_to_string(config_path)?;
        let config: ProjectConfig = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config_str = toml::to_string_pretty(self)?;
        let config_path = Self::config_path()?;
        fs::write(config_path, config_str)?;
        Ok(())
    }

    pub fn init() -> Result<(), Box<dyn std::error::Error>> {
        let config_dir = Path::new(".");
        let config_file = config_dir.join("cmon.toml");
        if config_file.exists() {
            return Err("Project already initialized".into());
        }

        // Detect appropriate check command
        let check_cmd = Self::detect_check_cmd();

        // Create config with detected values
        let config = ProjectConfig {
            model: String::from("claude-3-5-sonnet-latest"),
            check_cmd,
        };
        config.save()?;

        Ok(())
    }
}

