pub mod expander;
pub mod matcher;
pub mod monitor;
pub mod output;
mod trie;
pub mod keymaps;

pub use expander::expand_match;
pub use matcher::Matcher;
pub use monitor::{KeyboardEvent, KeyboardMonitor};
pub use output::OutputEngine;

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

use crate::config::Config;

/// The main expansion engine that ties together monitoring, matching, and output
pub struct ExpansionEngine {
    config: Arc<RwLock<Config>>,
    matcher: Matcher,
    output: OutputEngine,
    enabled: Arc<RwLock<bool>>,
}

impl ExpansionEngine {
    /// Create a new expansion engine
    pub fn new(config: Arc<RwLock<Config>>, enabled: Arc<RwLock<bool>>) -> Self {
        Self {
            config,
            matcher: Matcher::new(),
            output: OutputEngine::new(12, None),
            enabled,
        }
    }



    /// Process a keyboard event
    pub async fn process_event(&mut self, event: KeyboardEvent) -> Result<()> {
        // Check if expansion is enabled
        if !*self.enabled.read().await {
            return Ok(());
        }

        match event {
            KeyboardEvent::Character(ch) => {
                self.matcher.push_char(ch);
                self.check_and_expand().await?;
            }
            KeyboardEvent::WordBoundary(ch) => {
                self.matcher.push_char(ch);
                self.check_and_expand().await?;
            }
            KeyboardEvent::Backspace => {
                self.matcher.handle_backspace();
            }
            KeyboardEvent::Enter | KeyboardEvent::Tab | KeyboardEvent::Escape => {
                // These keys reset the buffer (word boundary)
                self.matcher.clear();
            }
        }

        Ok(())
    }

    /// Check for matches and expand if found
    async fn check_and_expand(&mut self) -> Result<()> {
        if let Some(match_result) = self.matcher.check_match() {
            log::debug!(
                "Match found: '{}' -> <redacted len={}>",
                match_result.typed_trigger,
                match_result.snippet.replace.len()
            );

            // Remove the matched text from the buffer
            self.matcher.remove_last(match_result.chars_to_delete);

            // Get variables from config
            let variables = {
                let config = self.config.read().await;
                config.variables.clone()
            };

            // Expand the match
            let expansion = expand_match(&match_result, &variables)?;

            // Output the expansion
            self.output.output_expansion(&expansion).await?;

            log::debug!("Expansion complete");
        }

        Ok(())
    }

    /// Run the engine with a keyboard event receiver and reload receiver
    pub async fn run(
        mut self,
        mut event_rx: mpsc::Receiver<KeyboardEvent>,
        mut reload_rx: mpsc::Receiver<()>,
    ) -> Result<()> {
        log::info!("Expansion engine started");

        // Initial load of snippets
        {
            let config = self.config.read().await;
            let flattened_snippets = crate::config::loader::ConfigManager::flatten_snippets(&config.snippets);
            self.matcher.reload(flattened_snippets.clone());
            log::info!("Loaded {} snippets into matcher", flattened_snippets.len());
        }

        loop {
            tokio::select! {
                Some(event) = event_rx.recv() => {
                    if let Err(e) = self.process_event(event).await {
                        log::error!("Error processing event: {}", e);
                    }
                }
                Some(_) = reload_rx.recv() => {
                    log::info!("Reloading engine configuration...");
                    let config = self.config.read().await;
                    let flattened_snippets = crate::config::loader::ConfigManager::flatten_snippets(&config.snippets);
                    self.matcher.reload(flattened_snippets.clone());
                    log::info!("Reloaded {} snippets", flattened_snippets.len());
                }
                else => break,
            }
        }


        log::info!("Expansion engine stopped");
        Ok(())
    }
}

/// Start the full expansion pipeline
pub async fn start_expansion_pipeline(
    config: Arc<RwLock<Config>>,
    enabled: Arc<RwLock<bool>>,
    reload_rx: mpsc::Receiver<()>,
) -> Result<()> {
    // Check prerequisites
    OutputEngine::check_availability().await?;

    // Create the keyboard event channel
    let (event_tx, event_rx) = mpsc::channel::<KeyboardEvent>(256);

    // Create and start the keyboard monitor
    let monitor = KeyboardMonitor::new(event_tx, config.clone())?;

    // Create the expansion engine
    let engine = ExpansionEngine::new(config, enabled);

    // Run both in parallel
    tokio::select! {
        result = monitor.run() => {
            if let Err(e) = result {
                log::error!("Keyboard monitor error: {}", e);
            }
        }
        result = engine.run(event_rx, reload_rx) => {
            if let Err(e) = result {
                log::error!("Expansion engine error: {}", e);
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_expansion_engine_creation() {
        let config = Arc::new(RwLock::new(Config::default()));
        let enabled = Arc::new(RwLock::new(true));
        let _engine = ExpansionEngine::new(config, enabled);
    }
}
