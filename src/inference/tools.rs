use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AnthropicTool {
    pub name: String,
    pub description: String,
    #[serde(rename = "input_schema")]
    pub input_schema: InputSchema,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OpenAITool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: OpenAIToolFunction,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OpenAIToolFunction {
    pub description: String,
    pub name: String,
    pub parameters: InputSchema,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InputSchema {
    #[serde(rename = "type")]
    pub schema_type: String,
    pub properties: HashMap<String, PropertySchema>,
    pub required: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PropertySchema {
    #[serde(rename = "type")]
    pub property_type: String,
    pub description: String,
}
