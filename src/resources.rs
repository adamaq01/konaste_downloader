use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ResourceInfo {
    #[serde(rename = "$value", default)]
    pub files: Vec<FileResource>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileResource {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub version: i32,
    #[serde(default)]
    pub size: i32,
    #[serde(default)]
    pub sum: String,
    #[serde(default)]
    pub url: String,
}
