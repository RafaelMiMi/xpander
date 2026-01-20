use anyhow::{Context, Result};
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::{sleep, Duration};

use super::expander::ExpansionResult;

/// Text output engine using ydotool
pub struct OutputEngine {
    /// Delay between keystrokes in milliseconds
    keystroke_delay: u64,
    /// Optional custom socket path for ydotoold
    socket_path: Option<String>,
}

impl OutputEngine {
    /// Create a new output engine
    pub fn new(keystroke_delay: u64, socket_path: Option<String>) -> Self {
        Self {
            keystroke_delay,
            socket_path,
        }
    }

    /// Check if ydotool is available
    pub async fn check_availability() -> Result<()> {
        let output = Command::new("which")
            .arg("ydotool")
            .output()
            .await
            .context("Failed to check for ydotool")?;

        if !output.status.success() {
            anyhow::bail!(
                "ydotool not found. Please install it with: sudo apt install ydotool\n\
                 Then enable the daemon: sudo systemctl enable --now ydotool"
            );
        }

        // Check if ydotoold binary exists - if not, we're on 0.1.x which doesn't need a daemon
        let ydotoold_exists = Command::new("which")
            .arg("ydotoold")
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false);

        if ydotoold_exists {
            // Newer ydotool (1.x+) requires daemon to be running
            let output = Command::new("pgrep")
                .arg("ydotoold")
                .output()
                .await?;

            if !output.status.success() {
                anyhow::bail!(
                    "ydotoold daemon is not running. Start it with:\n\
                     sudo systemctl start ydotool\n\
                     Or run: sudo ydotoold &"
                );
            }
        }
        // ydotool 0.1.x works without a daemon

        Ok(())
    }

    /// Output an expansion result
    pub async fn output_expansion(&self, expansion: &ExpansionResult) -> Result<()> {
        // Step 1: Delete the trigger characters
        if expansion.delete_count > 0 {
            self.send_backspaces(expansion.delete_count).await?;
            // Small delay after backspaces
            sleep(Duration::from_millis(10)).await;
        }

        // Step 2: Type the replacement text
        self.type_text(&expansion.text).await?;

        // Step 3: Move cursor back if needed
        if let Some(offset) = expansion.cursor_offset {
            if offset > 0 {
                sleep(Duration::from_millis(10)).await;
                self.move_cursor_left(offset).await?;
            }
        }

        Ok(())
    }

    /// Send backspace keys to delete characters
    async fn send_backspaces(&self, count: usize) -> Result<()> {
        if count == 0 {
            return Ok(());
        }

        // Use key name format for ydotool 0.1.x compatibility
        // ydotool key --repeat N Backspace
        let args = vec![
            "key".to_string(),
            "--repeat".to_string(),
            count.to_string(),
            "BackSpace".to_string(),
        ];

        self.run_ydotool(&args).await?;
        Ok(())
    }

    /// Type text using ydotool
    async fn type_text(&self, text: &str) -> Result<()> {
        if text.is_empty() {
            return Ok(());
        }

        // Use ydotool type command with --key-delay for 0.1.x compatibility
        let args = vec![
            "type".to_string(),
            "--key-delay".to_string(),
            self.keystroke_delay.to_string(),
            "--".to_string(),
            text.to_string(),
        ];

        self.run_ydotool(&args).await?;
        Ok(())
    }

    /// Move cursor left by N positions
    async fn move_cursor_left(&self, count: usize) -> Result<()> {
        if count == 0 {
            return Ok(());
        }

        // Use key name format for ydotool 0.1.x compatibility
        let args = vec![
            "key".to_string(),
            "--repeat".to_string(),
            count.to_string(),
            "Left".to_string(),
        ];

        self.run_ydotool(&args).await?;
        Ok(())
    }

    /// Run ydotool with the given arguments
    async fn run_ydotool(&self, args: &[String]) -> Result<()> {
        let mut cmd = Command::new("ydotool");
        cmd.args(args);

        // Set socket path if configured
        if let Some(socket) = &self.socket_path {
            cmd.env("YDOTOOL_SOCKET", socket);
        }

        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::piped());

        let output = cmd.output().await
            .context("Failed to run ydotool")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("ydotool failed: {}", stderr);
        }

        Ok(())
    }

    /// Type text character by character with delay (alternative method)
    #[allow(dead_code)]
    async fn type_text_slow(&self, text: &str) -> Result<()> {
        for ch in text.chars() {
            let mut cmd = Command::new("ydotool");
            cmd.args(["type", "--", &ch.to_string()]);

            if let Some(socket) = &self.socket_path {
                cmd.env("YDOTOOL_SOCKET", socket);
            }

            cmd.output().await?;
            sleep(Duration::from_millis(self.keystroke_delay)).await;
        }
        Ok(())
    }
}

/// Alternative output method using stdin pipe (more reliable for special characters)
pub struct PipeOutputEngine {
    keystroke_delay: u64,
    socket_path: Option<String>,
}

impl PipeOutputEngine {
    pub fn new(keystroke_delay: u64, socket_path: Option<String>) -> Self {
        Self {
            keystroke_delay,
            socket_path,
        }
    }

    /// Type text by piping to ydotool's stdin
    pub async fn type_text(&self, text: &str) -> Result<()> {
        let mut cmd = Command::new("ydotool");
        cmd.args([
            "type",
            "--delay",
            &self.keystroke_delay.to_string(),
            "--file",
            "-", // Read from stdin
        ]);

        if let Some(socket) = &self.socket_path {
            cmd.env("YDOTOOL_SOCKET", socket);
        }

        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::piped());

        let mut child = cmd.spawn()
            .context("Failed to spawn ydotool")?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(text.as_bytes()).await?;
            stdin.shutdown().await?;
        }

        let output = child.wait_with_output().await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("ydotool failed: {}", stderr);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires ydotool to be installed
    async fn test_check_availability() {
        // This test will fail if ydotool is not installed
        OutputEngine::check_availability().await.unwrap();
    }

    #[test]
    fn test_output_engine_creation() {
        let engine = OutputEngine::new(12, None);
        assert_eq!(engine.keystroke_delay, 12);
        assert!(engine.socket_path.is_none());

        let engine = OutputEngine::new(20, Some("/tmp/ydotool.sock".to_string()));
        assert_eq!(engine.keystroke_delay, 20);
        assert_eq!(engine.socket_path, Some("/tmp/ydotool.sock".to_string()));
    }
}
