use crate::SortBy;
use dirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

/// Configuration structure that mirrors the command-line arguments
#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    /// Sort processes by CPU usage, memory usage, or PID
    pub sort_by: Option<SortBy>,
    
    /// Refresh rate in seconds
    pub refresh_rate: Option<f64>,
    
    /// Show only the top N processes
    pub top: Option<usize>,
    
    /// Filter processes by name (case-insensitive)
    pub filter: Option<String>,
    
    /// Show only processes owned by the specified user
    pub user: Option<String>,
    
    /// Hide kernel processes
    pub no_kernel: Option<bool>,
    
    /// Display memory in human-readable format (KB, MB, GB)
    pub human_readable: Option<bool>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            sort_by: Some(SortBy::Cpu),
            refresh_rate: Some(1.0),
            top: None,
            filter: None,
            user: None,
            no_kernel: None,
            human_readable: None,
        }
    }
}

impl Config {
    /// Load configuration from the default config file location
    pub fn load() -> Result<Self, io::Error> {
        if let Some(config_path) = get_config_path() {
            Self::load_from_file(&config_path)
        } else {
            Ok(Self::default())
        }
    }

    /// Load configuration from a specific file path
    pub fn load_from_file(path: &Path) -> Result<Self, io::Error> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(path)?;
        
        match toml::from_str::<Config>(&content) {
            Ok(config) => Ok(config),
            Err(e) => Err(io::Error::new(
                ErrorKind::InvalidData,
                format!("Failed to parse config file: {}", e),
            )),
        }
    }

    /// Save configuration to the default config file location
    pub fn save(&self) -> Result<(), io::Error> {
        if let Some(config_path) = get_config_path() {
            self.save_to_file(&config_path)
        } else {
            Err(io::Error::new(
                ErrorKind::NotFound,
                "Could not determine config directory",
            ))
        }
    }

    /// Save configuration to a specific file path
    pub fn save_to_file(&self, path: &Path) -> Result<(), io::Error> {
        // Ensure the parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self).map_err(|e| {
            io::Error::new(
                ErrorKind::InvalidData,
                format!("Failed to serialize config: {}", e),
            )
        })?;

        fs::write(path, content)
    }
}

/// Get the path to the config file
pub fn get_config_path() -> Option<PathBuf> {
    // Use ~/.rustop/config.toml
    let home_dir = dirs::home_dir()?;
    let config_dir = home_dir.join(".rustop");
    Some(config_dir.join("config.toml"))
}

/// Create a default config file if it doesn't exist
pub fn ensure_config_file_exists() -> Result<(), io::Error> {
    if let Some(config_path) = get_config_path() {
        if !config_path.exists() {
            let default_config = Config::default();
            default_config.save_to_file(&config_path)?;
            println!("Created default config file at: {:?}", config_path);
        }
    }
    Ok(())
} 