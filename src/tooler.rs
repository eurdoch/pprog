use std::collections::HashMap;
use serde::{Deserialize, Serialize};

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

struct Tooler {
    tools: Vec<Tool>,
}

impl Tooler {
    fn new() -> Self {
        Self {
            tools: vec![
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
            ],
        }
    }

    fn to_string(&self) -> String {
        serde_json::to_string(&self.tools).unwrap()
    }
}

