use std::fs;
use std::process::Command;
use anyhow::Result;

pub struct Tools;

impl Tools {
    fn read_file(path: &str) -> Result<String> {
        Ok(fs::read_to_string(path)?)
    }

    fn write_file(path: &str, content: &str) -> Result<()> {
        Ok(fs::write(path, content)?)
    }

    fn execute(statement: &str) -> Result<String> {
        let output = Command::new("bash")
            .arg("-c")
            .arg(statement)
            .output()?;

        if output.status.success() {
            Ok(String::from_utf8(output.stdout)?)
        } else {
            let error = String::from_utf8_lossy(&output.stderr).to_string();
            Err(anyhow::Error::msg(error))
        }
    }

    fn compile_check(cmd: &str) -> Result<String> {
        let output = Command::new("bash")
            .arg("-c")
            .arg(cmd)
            .output()?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Ok(String::from_utf8_lossy(&output.stderr).to_string())
        }
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
                let cmd = inputs.get("cmd")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::Error::msg("Missing or invalid 'cmd' input".to_string()))?;

                Tools::compile_check(cmd)
            },
            _ => Err(anyhow::Error::msg(format!("Invalid tool name: {}", name))),
        }
    }
}

