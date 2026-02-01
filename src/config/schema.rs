use serde::{Deserialize, Serialize};

/// Main configuration structure for xpander
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub settings: Settings,
    #[serde(default)]
    pub snippets: Vec<SnippetNode>,
    
    #[serde(default)]
    pub variables: serde_yaml::Value,
}

/// Global application settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Play sound on expansion
    #[serde(default)]
    pub enable_sound: bool,

    /// Show notification on expansion
    #[serde(default)]
    pub notify_on_expand: bool,

    /// Enable/disable the expander globally
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Backspace behavior - delete trigger characters before expanding
    #[serde(default = "default_true")]
    pub delete_trigger: bool,

    /// Delay in milliseconds between keystrokes when typing
    #[serde(default = "default_keystroke_delay")]
    pub keystroke_delay_ms: u64,

    /// Path to ydotool socket (optional, uses default if not specified)
    #[serde(default)]
    pub ydotool_socket: Option<String>,

    /// Keyboard layout (qwerty, azerty, qwertz)
    #[serde(default = "default_layout")]
    pub layout: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enable_sound: false,
            notify_on_expand: false,
            enabled: true,
            delete_trigger: true,
            keystroke_delay_ms: default_keystroke_delay(),
            ydotool_socket: None,
            layout: default_layout(),
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_keystroke_delay() -> u64 {
    12
}

fn default_layout() -> String {
    "qwerty".to_string()
}

/// A node in the snippet hierarchy (either a snippet or a folder)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SnippetNode {
    Folder(Folder),
    Snippet(Snippet),
}

/// A folder containing snippets or other folders
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    /// Name of the folder
    pub folder: String,
    
    /// Items inside the folder
    #[serde(default)]
    pub items: Vec<SnippetNode>,
    
    /// Whether this folder is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// A single text expansion snippet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snippet {
    /// The trigger text that activates this snippet
    pub trigger: String,

    /// The replacement text
    pub replace: String,

    /// Optional label/description for the snippet
    #[serde(default)]
    pub label: Option<String>,

    /// Whether to propagate case from trigger to replacement
    #[serde(default)]
    pub propagate_case: bool,

    /// Whether to position cursor at $|$ marker after expansion
    #[serde(default)]
    pub cursor_position: bool,

    /// Only trigger on word boundaries (after space, punctuation, etc.)
    #[serde(default)]
    pub word_boundary: bool,

    /// Use regex matching for trigger
    #[serde(default)]
    pub regex: bool,

    /// Only expand in specific applications (by window class)
    #[serde(default)]
    pub applications: Option<Vec<String>>,

    /// Exclude expansion in specific applications
    #[serde(default)]
    pub exclude_applications: Option<Vec<String>>,

    /// Whether this snippet is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Snippet {
    /// Create a new simple snippet
    pub fn new(trigger: impl Into<String>, replace: impl Into<String>) -> Self {
        Self {
            trigger: trigger.into(),
            replace: replace.into(),
            label: None,
            propagate_case: false,
            cursor_position: false,
            word_boundary: false,
            regex: false,
            applications: None,
            exclude_applications: None,
            enabled: true,
        }
    }

    /// Builder method to set label
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Builder method to enable case propagation
    pub fn with_case_propagation(mut self) -> Self {
        self.propagate_case = true;
        self
    }

    /// Builder method to enable cursor positioning
    pub fn with_cursor_position(mut self) -> Self {
        self.cursor_position = true;
        self
    }

    /// Builder method to enable word boundary matching
    pub fn with_word_boundary(mut self) -> Self {
        self.word_boundary = true;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.settings.enabled);
        assert!(!config.settings.enable_sound);
        assert!(config.snippets.is_empty());
    }

    #[test]
    fn test_snippet_builder() {
        let snippet = Snippet::new(";email", "test@example.com")
            .with_label("Email")
            .with_case_propagation();

        assert_eq!(snippet.trigger, ";email");
        assert_eq!(snippet.replace, "test@example.com");
        assert_eq!(snippet.label, Some("Email".to_string()));
        assert!(snippet.propagate_case);
    }

    #[test]
    fn test_deserialize_config() {
        let yaml = r#"
settings:
  enable_sound: true
  notify_on_expand: false
snippets:
  - trigger: ";test"
    replace: "hello world"
    propagate_case: true
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert!(config.settings.enable_sound);
        assert!(!config.settings.notify_on_expand);
        assert_eq!(config.snippets.len(), 1);
        
        match &config.snippets[0] {
            SnippetNode::Snippet(s) => {
                assert_eq!(s.trigger, ";test");
                assert!(s.propagate_case);
            }
            _ => panic!("Expected snippet"),
        }
    }
    
    #[test]
    fn test_deserialize_nested_config() {
        let yaml = r#"
snippets:
  - folder: "Work"
    items:
      - trigger: ";sig"
        replace: "Work Signature"
  - trigger: ";home"
    replace: "Home Address"
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.snippets.len(), 2);
        
        // First item should be a folder
        match &config.snippets[0] {
            SnippetNode::Folder(f) => {
                assert_eq!(f.folder, "Work");
                assert_eq!(f.items.len(), 1);
                match &f.items[0] {
                    SnippetNode::Snippet(s) => assert_eq!(s.trigger, ";sig"),
                    _ => panic!("Expected snippet inside folder"),
                }
            }
            _ => panic!("Expected folder"),
        }
        
        // Second item should be a snippet
        match &config.snippets[1] {
            SnippetNode::Snippet(s) => assert_eq!(s.trigger, ";home"),
            _ => panic!("Expected snippet"),
        }
    }
}
