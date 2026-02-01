use anyhow::{Context, Result};
use notify::{Config as NotifyConfig, Event, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

use super::schema::Config;

/// Configuration manager with hot-reload support
pub struct ConfigManager {
    config: Arc<RwLock<Config>>,
    config_path: PathBuf,
    _watcher: Option<RecommendedWatcher>,
}

impl ConfigManager {
    /// Create a new ConfigManager and load the configuration
    pub async fn new() -> Result<(Self, mpsc::Receiver<Config>)> {
        let config_path = Self::get_config_path()?;

        // Create config directory if it doesn't exist
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)
                .context("Failed to create config directory")?;
        }

        // Load or create initial config
        let config = if config_path.exists() {
            Self::load_config(&config_path)?
        } else {
            let default_config = Config::default();
            Self::save_config(&config_path, &default_config)?;
            default_config
        };

        let config = Arc::new(RwLock::new(config));
        let (tx, rx) = mpsc::channel(16);

        // Set up file watcher
        let watcher = Self::setup_watcher(&config_path, config.clone(), tx)?;

        Ok((
            Self {
                config,
                config_path,
                _watcher: Some(watcher),
            },
            rx,
        ))
    }

    /// Get the default config file path
    pub fn get_config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("Could not determine config directory")?;
        Ok(config_dir.join("xpander").join("config.yaml"))
    }

    /// Load configuration from a file
    pub fn load_config(path: &Path) -> Result<Config> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        let config: Config = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;

        log::info!("Loaded configuration from {}", path.display());
        Ok(config)
    }

    /// Save configuration to a file
    pub fn save_config(path: &Path, config: &Config) -> Result<()> {
        let content = serde_yaml::to_string(config)
            .context("Failed to serialize config")?;

        std::fs::write(path, content)
            .with_context(|| format!("Failed to write config file: {}", path.display()))?;

        log::info!("Saved configuration to {}", path.display());
        Ok(())
    }

    /// Set up file watcher for hot-reload
    fn setup_watcher(
        config_path: &Path,
        config: Arc<RwLock<Config>>,
        tx: mpsc::Sender<Config>,
    ) -> Result<RecommendedWatcher> {
        let path = config_path.to_path_buf();
        let handle = tokio::runtime::Handle::current();

        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    if event.kind.is_modify() || event.kind.is_create() {
                        log::debug!("Config file changed, reloading...");

                        match Self::load_config(&path) {
                            Ok(new_config) => {
                                let config = config.clone();
                                let tx = tx.clone();
                                let new_config_clone = new_config.clone();

                                // Update config in a blocking way since we're in the notify callback
                                handle.spawn(async move {
                                    let mut cfg = config.write().await;
                                    *cfg = new_config_clone.clone();
                                    if tx.send(new_config_clone).await.is_err() {
                                        log::warn!("Failed to send config update notification");
                                    }
                                    log::info!("Configuration reloaded successfully");
                                });
                            }
                            Err(e) => {
                                log::error!("Failed to reload config: {}", e);
                            }
                        }
                    }
                }
            },
            NotifyConfig::default(),
        )?;

        // Watch the config file's parent directory
        if let Some(parent) = config_path.parent() {
            watcher.watch(parent, RecursiveMode::NonRecursive)?;
        }

        log::info!("Watching config file for changes: {}", config_path.display());
        Ok(watcher)
    }

    /// Get a read lock on the current configuration
    pub async fn get_config(&self) -> tokio::sync::RwLockReadGuard<'_, Config> {
        self.config.read().await
    }

    /// Update and save the configuration
    pub async fn update_config(&self, config: Config) -> Result<()> {
        Self::save_config(&self.config_path, &config)?;
        let mut cfg = self.config.write().await;
        *cfg = config;
        Ok(())
    }

    /// Get the config file path
    pub fn path(&self) -> &Path {
        &self.config_path
    }

    /// Add a new snippet to the configuration (at the top level)
    pub async fn add_snippet(&self, snippet: super::schema::Snippet) -> Result<()> {
        let mut config = self.config.write().await;
        config.snippets.push(super::schema::SnippetNode::Snippet(snippet));
        Self::save_config(&self.config_path, &config)?;
        Ok(())
    }

    /// Remove a snippet by index from the flattened list (for simple management)
    /// Note: This is checking the top level only for now as basic management
    pub async fn remove_snippet(&self, index: usize) -> Result<()> {
        let mut config = self.config.write().await;
        if index < config.snippets.len() {
            config.snippets.remove(index);
            Self::save_config(&self.config_path, &config)?;
        }
        Ok(())
    }

    /// Update a snippet at a specific index (top level only for now)
    pub async fn update_snippet(&self, index: usize, snippet: super::schema::Snippet) -> Result<()> {
        let mut config = self.config.write().await;
        if index < config.snippets.len() {
            config.snippets[index] = super::schema::SnippetNode::Snippet(snippet);
            Self::save_config(&self.config_path, &config)?;
        }
        Ok(())
    }

    /// Toggle the global enabled state
    pub async fn toggle_enabled(&self) -> Result<bool> {
        let mut config = self.config.write().await;
        config.settings.enabled = !config.settings.enabled;
        Self::save_config(&self.config_path, &config)?;
        Ok(config.settings.enabled)
    }

    /// Flatten snippets from the hierarchy into a single list
    pub fn flatten_snippets(nodes: &[super::schema::SnippetNode]) -> Vec<super::schema::Snippet> {
        let mut result = Vec::new();
        Self::flatten_recursive(nodes, &mut result);
        result
    }

    fn flatten_recursive(nodes: &[super::schema::SnippetNode], result: &mut Vec<super::schema::Snippet>) {
        for node in nodes {
            match node {
                super::schema::SnippetNode::Snippet(s) => {
                    if s.enabled {
                        result.push(s.clone());
                    }
                }
                super::schema::SnippetNode::Folder(f) => {
                    if f.enabled {
                        Self::flatten_recursive(&f.items, result);
                    }
                }
            }
        }
    }
}

/// Export snippets to a YAML file
pub fn export_snippets(snippets: &[super::schema::SnippetNode], path: &Path) -> Result<()> {
    let content = serde_yaml::to_string(snippets)
        .context("Failed to serialize snippets")?;
    std::fs::write(path, content)
        .with_context(|| format!("Failed to write export file: {}", path.display()))?;
    Ok(())
}


/// Structure for exporting custom entries (snippets and variables)
#[derive(Debug, Serialize, Deserialize)]
pub struct ExportData {
    pub snippets: Vec<super::schema::SnippetNode>,
    pub variables: serde_yaml::Value,
}

/// Export snippets and variables to a YAML file
pub fn export_custom_entries(snippets: &[super::schema::SnippetNode], variables: &serde_yaml::Value, path: &Path) -> Result<()> {
    let data = ExportData {
        snippets: snippets.to_vec(),
        variables: variables.clone(),
    };
    let content = serde_yaml::to_string(&data)
        .context("Failed to serialize custom entries")?;
    std::fs::write(path, content)
        .with_context(|| format!("Failed to write export file: {}", path.display()))?;
    Ok(())
}

/// Import snippets and variables from a YAML file
pub fn import_custom_entries(path: &Path) -> Result<ExportData> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read import file: {}", path.display()))?;
    let data: ExportData = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse import file: {}", path.display()))?;
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_save_and_load_config() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.yaml");

        let mut config = Config::default();
        config.snippets.push(super::super::schema::SnippetNode::Snippet(
            super::super::schema::Snippet::new(";test", "hello")
        ));

        ConfigManager::save_config(&path, &config).unwrap();
        let loaded = ConfigManager::load_config(&path).unwrap();

        assert_eq!(loaded.snippets.len(), 1);
        match &loaded.snippets[0] {
            super::super::schema::SnippetNode::Snippet(s) => assert_eq!(s.trigger, ";test"),
            _ => panic!("Expected snippet"),
        }
    }
}
