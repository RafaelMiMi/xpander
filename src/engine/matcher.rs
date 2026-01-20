use regex::Regex;
use std::collections::HashMap;

use crate::config::Snippet;
use crate::engine::trie::Trie;

/// Result of a trigger match
#[derive(Debug, Clone)]
pub struct MatchResult {
    /// The matched snippet
    pub snippet: Snippet,
    /// The actual text that was typed (for case propagation)
    pub typed_trigger: String,
    /// Number of characters to delete (backspaces needed)
    pub chars_to_delete: usize,
    /// Regex capture groups (if regex trigger)
    pub captures: Option<Vec<String>>,
}

/// Maintains a buffer of typed text and matches against triggers
pub struct Matcher {
    /// Buffer of recently typed characters
    buffer: String,
    /// Maximum buffer size (longest trigger + some margin)
    max_buffer_size: usize,
    /// Trie for efficient literal matching
    trie: Trie,
    /// List of regex snippets (checked linearly)
    regex_snippets: Vec<Snippet>,
    /// Cache for compiled regex patterns
    regex_cache: HashMap<String, Regex>,
    /// Whether we're at a word boundary (for word_boundary triggers)
    at_word_boundary: bool,
}

impl Matcher {
    /// Create a new matcher
    pub fn new() -> Self {
        Self {
            buffer: String::with_capacity(256),
            max_buffer_size: 256,
            trie: Trie::new(),
            regex_snippets: Vec::new(),
            regex_cache: HashMap::new(),
            at_word_boundary: true, // Start of input is a word boundary
        }
    }

    /// Add a character to the buffer
    pub fn push_char(&mut self, ch: char) {
        self.buffer.push(ch);

        // Update word boundary status
        self.at_word_boundary = ch.is_whitespace() || ch.is_ascii_punctuation();

        // Trim buffer if too long
        if self.buffer.len() > self.max_buffer_size {
            let drain_to = self.buffer.len() - self.max_buffer_size / 2;
            self.buffer.drain(..drain_to);
        }
    }

    /// Handle backspace - remove last character from buffer
    pub fn handle_backspace(&mut self) {
        self.buffer.pop();
    }

    /// Clear the buffer (called after expansion or on word boundary reset)
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.at_word_boundary = true;
    }

    /// Remove the last N characters from the buffer (after a match)
    pub fn remove_last(&mut self, n: usize) {
        let new_len = self.buffer.len().saturating_sub(n);
        self.buffer.truncate(new_len);
    }

    /// Reload snippets into the Trie and regex list
    pub fn reload(&mut self, snippets: Vec<Snippet>) {
        self.trie = Trie::new();
        self.regex_snippets.clear();
        self.regex_cache.clear();

        for snippet in snippets {
            if !snippet.enabled {
                continue;
            }

            if snippet.regex {
                self.regex_snippets.push(snippet);
            } else {
                self.trie.insert(snippet);
            }
        }
    }

    /// Check if any snippet matches the current buffer
    pub fn check_match(&mut self) -> Option<MatchResult> {
        // 1. Check Trie (O(L))
        if let Some((snippet, len)) = self.trie.find_match(&self.buffer) {
            // Verify word boundary if required
            let valid = if snippet.word_boundary {
                let buffer_len = self.buffer.len();
                if buffer_len > len {
                    let char_before_start = self.buffer.chars().nth(buffer_len - len - 1);
                    if let Some(ch) = char_before_start {
                        ch.is_whitespace() || ch.is_ascii_punctuation()
                    } else {
                        true
                    }
                } else {
                    true // Start of buffer
                }
            } else {
                true
            };

            if valid {
                return Some(MatchResult {
                    snippet: snippet.clone(),
                    typed_trigger: snippet.trigger.clone(),
                    chars_to_delete: len,
                    captures: None,
                });
            }
        }

        // 2. Check Regex snippets (O(N) but only for regex ones)
        // We need to clone the snippets to iterate because check_regex_match borrows self mutably
        // This is a bit annoying. Alternatively, we can inline check_regex_match logic or use RefCell.
        // Or, we iterate indices.
        // Actually, check_regex_match only needs &self for buffer and &mut self for cache.
        // If we split the cache out, it would be easier.
        // Let's just clone the regex snippets for now, or use a loop with manual indexing?
        // Cloning Vec<Snippet> is expensive? No, we just need to iterate.
        // Let's copy the needed logic here or refactor check_regex_match to split borrows.
        
        let regex_snippets = self.regex_snippets.clone();
        for snippet in &regex_snippets {
             if let Some(result) = self.check_regex_match(snippet) {
                 return Some(result);
             }
        }
        
        None
    }

    /// Check for a regex trigger match
    fn check_regex_match(&mut self, snippet: &Snippet) -> Option<MatchResult> {
        // Get or compile the regex
        let regex = if let Some(regex) = self.regex_cache.get(&snippet.trigger) {
            regex
        } else {
            // Compile and cache the regex
            let pattern = format!("(?:{})$", snippet.trigger);

            match Regex::new(&pattern) {
                Ok(regex) => {
                    self.regex_cache.insert(snippet.trigger.clone(), regex);
                    self.regex_cache.get(&snippet.trigger).unwrap()
                }
                Err(e) => {
                    log::error!("Invalid regex pattern '{}': {}", snippet.trigger, e);
                    return None;
                }
            }
        };

        // Check for match at end of buffer
        if let Some(caps) = regex.captures(&self.buffer) {
            let full_match = caps.get(0)?;

            // If word boundary required, check position
            if snippet.word_boundary && full_match.start() > 0 {
                let char_before = self.buffer.chars().nth(full_match.start() - 1);
                if let Some(ch) = char_before {
                    if !ch.is_whitespace() && !ch.is_ascii_punctuation() {
                        return None;
                    }
                }
            }

            // Collect capture groups
            let captures: Vec<String> = caps
                .iter()
                .skip(1) // Skip the full match
                .filter_map(|m| m.map(|m| m.as_str().to_string()))
                .collect();

            Some(MatchResult {
                snippet: snippet.clone(),
                typed_trigger: full_match.as_str().to_string(),
                chars_to_delete: full_match.len(),
                captures: if captures.is_empty() { None } else { Some(captures) },
            })
        } else {
            None
        }
    }

    /// Get the current buffer content (for debugging)
    pub fn buffer(&self) -> &str {
        &self.buffer
    }
}

impl Default for Matcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_snippet(trigger: &str, replace: &str) -> Snippet {
        Snippet::new(trigger, replace)
    }

    #[test]
    fn test_basic_match() {
        let mut matcher = Matcher::new();
        let snippets = vec![make_snippet(";email", "test@example.com")];
        matcher.reload(snippets);

        // Type the trigger
        for ch in ";email".chars() {
            matcher.push_char(ch);
        }

        let result = matcher.check_match();
        assert!(result.is_some());

        let result = result.unwrap();
        assert_eq!(result.typed_trigger, ";email");
        assert_eq!(result.chars_to_delete, 6);
    }

    #[test]
    fn test_no_match() {
        let mut matcher = Matcher::new();
        let snippets = vec![make_snippet(";email", "test@example.com")];
        matcher.reload(snippets);

        // Type something that doesn't match
        for ch in ";emai".chars() {
            matcher.push_char(ch);
        }

        let result = matcher.check_match();
        assert!(result.is_none());
    }

    #[test]
    fn test_word_boundary() {
        let mut matcher = Matcher::new();
        let mut snippet = make_snippet("btw", "by the way");
        snippet.word_boundary = true;
        let snippets = vec![snippet];
        matcher.reload(snippets);

        // Type "btw" without word boundary - should not match
        for ch in "hellobtw".chars() {
            matcher.push_char(ch);
        }
        assert!(matcher.check_match().is_none());

        // Clear and try with word boundary
        matcher.clear();
        for ch in "hello btw".chars() {
            matcher.push_char(ch);
        }
        assert!(matcher.check_match().is_some());
    }

    #[test]
    fn test_backspace() {
        let mut matcher = Matcher::new();
        let snippets = vec![make_snippet(";test", "replacement")];
        matcher.reload(snippets);

        // Type and then backspace
        for ch in ";tess".chars() {
            matcher.push_char(ch);
        }
        matcher.handle_backspace();
        matcher.push_char('t');

        let result = matcher.check_match();
        assert!(result.is_some());
    }

    #[test]
    fn test_regex_match() {
        let mut matcher = Matcher::new();
        let mut snippet = make_snippet(r";d(\d+)", "Number: $1");
        snippet.regex = true;
        let snippets = vec![snippet];
        matcher.reload(snippets);

        for ch in ";d123".chars() {
            matcher.push_char(ch);
        }

        let result = matcher.check_match();
        assert!(result.is_some());

        let result = result.unwrap();
        assert_eq!(result.captures, Some(vec!["123".to_string()]));
    }

    #[test]
    fn test_disabled_snippet() {
        let mut matcher = Matcher::new();
        let mut snippet = make_snippet(";test", "replacement");
        snippet.enabled = false;
        let snippets = vec![snippet];
        matcher.reload(snippets);

        for ch in ";test".chars() {
            matcher.push_char(ch);
        }

        assert!(matcher.check_match().is_none());
    }
}
