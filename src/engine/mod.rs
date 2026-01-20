pub mod expander;
pub mod matcher;
pub mod monitor;
pub mod output;

pub use expander::{expand_match, ExpansionResult};
pub use matcher::{MatchResult, Matcher};
pub use monitor::{KeyboardEvent, KeyboardMonitor};
pub use output::OutputEngine;

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

use crate::config::{Config, Snippet};

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

    /// Update output engine settings
    pub fn update_settings(&mut self, keystroke_delay: u64, socket_path: Option<String>) {
        self.output = OutputEngine::new(keystroke_delay, socket_path);
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
        let config = self.config.read().await;
        let snippets: Vec<Snippet> = config.snippets.clone();
        drop(config);

        if let Some(match_result) = self.matcher.check_match(&snippets) {
            log::info!(
                "Match found: '{}' -> '{}'",
                match_result.typed_trigger,
                match_result.snippet.replace
            );

            // Remove the matched text from the buffer
            self.matcher.remove_last(match_result.chars_to_delete);

            // Expand the match
            let expansion = expand_match(&match_result)?;

            // Output the expansion
            self.output.output_expansion(&expansion).await?;

            log::debug!("Expansion complete");
        }

        Ok(())
    }

    /// Run the engine with a keyboard event receiver
    pub async fn run(mut self, mut event_rx: mpsc::Receiver<KeyboardEvent>) -> Result<()> {
        log::info!("Expansion engine started");

        while let Some(event) = event_rx.recv().await {
            if let Err(e) = self.process_event(event).await {
                log::error!("Error processing event: {}", e);
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
) -> Result<()> {
    // Check prerequisites
    OutputEngine::check_availability().await?;

    // Create the keyboard event channel
    let (event_tx, event_rx) = mpsc::channel::<KeyboardEvent>(256);

    // Create and start the keyboard monitor
    let monitor = KeyboardMonitor::new(event_tx)?;

    // Create the expansion engine
    let engine = ExpansionEngine::new(config, enabled);

    // Run both in parallel
    tokio::select! {
        result = monitor.run() => {
            if let Err(e) = result {
                log::error!("Keyboard monitor error: {}", e);
            }
        }
        result = engine.run(event_rx) => {
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
