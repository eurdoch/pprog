
use std::fs;
use std::path::Path;
use std::process::Command;
use anyhow::Result;

use crate::config::ProjectConfig;

pub struct Tools;

impl Tools {
    fn read_file(path: &str) -> Result<String> {
        Ok(fs::read_to_string(path)?)
    }

    fn write_file(path: &str, content: &str) -> Result<()> {
        if let Some(parent) = Path::new(path).parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }
        Ok(fs::write(path, content)?)
    }

    fn execute(statement: &str) -> Result<String> {
        let output = Command::new("bash")
            .arg("-c")
            .arg(statement)
            .output()?;

        Ok(String::from_utf8(output.stdout)? + &String::from_utf8(output.stderr)?)
    }

    fn compile_check() -> Result<String, anyhow::Error> {
        let config = ProjectConfig::load().map_err(|e| anyhow::anyhow!("{}", e))?;

        let output = Command::new("bash")
            .arg("-c")
            .arg(config.check_cmd)
            .output()?;

        Ok(String::from_utf8(output.stdout)? + &String::from_utf8(output.stderr)?)
    }

    pub fn handle_tool_use(name: &String, inputs: &serde_json::Value) -> Result<String, anyhow::Error> {
        match name.as_str() {
            "read_file" => {
                let path = inputs.get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::Error::msg("Missing or invalid 'path' input".to_string()))?;

                Tools::read_file(path)
            },
            "write_file" => {
                let path = inputs
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::Error::msg("Missing or invalid 'path' input".to_string()))?;

                let content = inputs
                    .get("content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::Error::msg("Missing or invalid 'content' input".to_string()))?;

                Tools::write_file(path, content)?;
                Ok("File written successfully".to_string())
            },
            "execute" => {
                let statement = inputs
                    .get("statement")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::Error::msg("Missing or invalid 'statement' input".to_string()))?;

                Tools::execute(statement)
            },
            "compile_check" => {
                match Tools::compile_check() {
                    Ok(output) => {
                        return Ok(output);
                    },
                    Err(e) => return Err(anyhow::Error::msg(format!("Error doing compile check: {}", e.to_string()))),
                };
            },
            _ => Err(anyhow::Error::msg(format!("Invalid tool name: {}", name))),
        }
    }
}

