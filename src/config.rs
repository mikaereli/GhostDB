use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::path::Path;
use anyhow::{Context, Result};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AppConfig {
    pub tables: HashMap<String, TableConfig>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TableConfig {
    pub columns: HashMap<String, ColumnStrategy>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ColumnStrategy {
    FirstName,
    LastName,
    FullName,
    Email,
    Phone,
    Mask,
    Fixed(String),
    Keep,
}

impl AppConfig {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(path).context("Failed to open configuration file")?;
        let config: AppConfig = serde_yaml::from_reader(file)
            .context("Failed to parse YAML configuration")?;
        Ok(config)
    }
}
