use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

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
        let path = Path::new(".");
        
        // Check for Rust project (Cargo.toml)
        if path.join("Cargo.toml").exists() {
            return String::from("cargo check");
        }
        
        // TODO should run code through node after tsc and check for errors
        // Check for TypeScript project (tsconfig.json)
        if path.join("tsconfig.json").exists() {
            return String::from("tsc --noEmit");
        }
        
        // Check for Java project (gradlew)
        if path.join("gradlew").exists() {
            return String::from("./gradlew check");
        }
        
        // TODO should run code through node to check for errors
        if path.join("package.json").exists() {
            // If we find eslint config, use that for checking
            if path.join(".eslintrc.json").exists() || path.join(".eslintrc.js").exists() {
                return String::from("eslint .");
            }
            return String::from("npm run lint");
        }
        
        String::from("echo 'No check command configured'")
    }


    pub fn config_path() -> PathBuf {
        Path::new(".").join(Self::CONFIG_FILE)
    }

    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let config_path = Self::config_path();
        if !config_path.exists() {
            return Err("Project not initialized. Run 'init' first.".into());
        }

        let content = fs::read_to_string(config_path)?;
        let config: ProjectConfig = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config_dir = Path::new(".");
        if !config_dir.exists() {
            fs::create_dir_all(config_dir)?;
        }

        let config_str = toml::to_string_pretty(self)?;
        fs::write(Self::config_path(), config_str)?;
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

