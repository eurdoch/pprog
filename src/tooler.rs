use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::config::ProjectConfig;

#[derive(Debug, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub input_schema: InputSchema,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InputSchema {
    #[serde(rename = "type")]
    pub schema_type: String,
    pub properties: HashMap<String, PropertySchema>,
    pub required: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PropertySchema {
    #[serde(rename = "type")]
    pub property_type: String,
    pub description: String,
}

pub struct Tooler {
    tools: Vec<Tool>,
}

impl Tooler {
    pub fn new() -> Self {
        let config = match ProjectConfig::load() {
            Ok(c) => c,
            Err(_) => ProjectConfig::default()
        };
        let mut tools = vec![
            Tool {
                name: "read_file".to_string(),
                description: "Read file as string using path relative to root directory of project.".to_string(),
                input_schema: InputSchema {
                    schema_type: "object".to_string(),
                    properties: {
                        let mut map = HashMap::new();
                        map.insert(
                            "path".to_string(),
                            PropertySchema {
                                property_type: "string".to_string(),
                                description: "The file path relative to the project root directory".to_string(),
                            },
                        );
                        map
                    },
                    required: vec!["path".to_string()],
                },
            },
            Tool {
                name: "write_file".to_string(),
                description: "Write string to file at path relative to root directory of project.".to_string(),
                input_schema: InputSchema {
                    schema_type: "object".to_string(),
                    properties: {
                        let mut map = HashMap::new();
                        map.insert(
                            "path".to_string(),
                            PropertySchema {
                                property_type: "string".to_string(),
                                description: "The file path relative to the project root directory".to_string(),
                            },
                        );
                        map.insert(
                            "content".to_string(),
                            PropertySchema {
                                property_type: "string".to_string(),
                                description: "The content to write to the file".to_string(),
                            },
                        );
                        map
                    },
                    required: vec!["path".to_string(), "content".to_string()],
                },
            },
            Tool {
                name: "execute".to_string(),
                description: "Execute bash statements as a single string..".to_string(),
                input_schema: InputSchema {
                    schema_type: "object".to_string(),
                    properties: {
                        let mut map = HashMap::new();
                        map.insert(
                            "statement".to_string(),
                            PropertySchema {
                                property_type: "string".to_string(),
                                description: "The bash statement to be executed.".to_string(),
                            },
                        );
                        map
                    },
                    required: vec!["statement".to_string()],
                },
            },
        ];

        if config.check_cmd != "" {
            tools.push(
                Tool {
                    name: "compile_check".to_string(),
                    description: "Check if project compiles or runs without error.".to_string(),
                    input_schema: InputSchema {
                        schema_type: "object".to_string(),
                        properties: {
                            let mut map = HashMap::new();
                            map.insert(
                                "cmd".to_string(),
                                PropertySchema {
                                    property_type: "string".to_string(),
                                    description: "The command to check for compiler/interpreter errors.".to_string(),
                                },
                            );
                            map
                        },
                        required: vec!["cmd".to_string()],
                    },
                },
            );
        }

        Tooler { tools }
    }

    pub fn get_tools_json(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(&self.tools)
    }
}
