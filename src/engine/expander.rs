use anyhow::Result;
use regex::Regex;
use std::sync::LazyLock;

use crate::config::Snippet;
use crate::variables::{expand_variables, find_cursor_position, propagate_case};

use super::matcher::MatchResult;

/// Result of expanding a snippet
#[derive(Debug, Clone)]
pub struct ExpansionResult {
    /// The final expanded text to output
    pub text: String,
    /// Number of characters to delete before outputting
    pub delete_count: usize,
    /// Cursor offset from end of text (how many chars to move back)
    pub cursor_offset: Option<usize>,
}

/// Regex for replacing capture group references ($1, $2, etc.)
static CAPTURE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\$(\d+)").expect("Invalid capture regex")
});

/// Process a match result and produce the final expansion
pub fn expand_match(match_result: &MatchResult, variables: &serde_yaml::Value) -> Result<ExpansionResult> {
    let snippet = &match_result.snippet;
    let mut text = snippet.replace.clone();

    // Step 1: Replace regex capture groups if present
    if let Some(captures) = &match_result.captures {
        text = replace_captures(&text, captures);
    }

    // Step 2: Expand variables ({{date}}, {{clipboard}}, etc.)
    text = expand_variables(&text, variables)?;

    // Step 3: Apply case propagation if enabled
    if snippet.propagate_case {
        text = propagate_case(&match_result.typed_trigger, &text);
    }

    // Step 4: Find and process cursor position marker
    let (final_text, cursor_pos) = find_cursor_position(&text);

    // Calculate cursor offset from end
    let cursor_offset = if snippet.cursor_position {
        cursor_pos.map(|pos| final_text.len() - pos)
    } else {
        None
    };

    Ok(ExpansionResult {
        text: final_text,
        delete_count: match_result.chars_to_delete,
        cursor_offset,
    })
}

/// Replace capture group references ($1, $2, etc.) with actual captured values
fn replace_captures(text: &str, captures: &[String]) -> String {
    let mut result = text.to_string();

    for cap in CAPTURE_REGEX.captures_iter(text) {
        let full_match = cap.get(0).unwrap().as_str();
        let index: usize = cap[1].parse().unwrap_or(0);

        if index > 0 && index <= captures.len() {
            result = result.replace(full_match, &captures[index - 1]);
        }
    }

    result
}

/// Expand a snippet directly (without a match result)
pub fn expand_snippet(snippet: &Snippet, variables: &serde_yaml::Value) -> Result<ExpansionResult> {
    let match_result = MatchResult {
        snippet: snippet.clone(),
        typed_trigger: snippet.trigger.clone(),
        chars_to_delete: snippet.trigger.len(),
        captures: None,
    };
    expand_match(&match_result, variables)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Snippet;

    #[test]
    fn test_basic_expansion() {
        let snippet = Snippet::new(";test", "hello world");
        let match_result = MatchResult {
            snippet: snippet.clone(),
            typed_trigger: ";test".to_string(),
            chars_to_delete: 5,
            captures: None,
        };

        let result = expand_match(&match_result, &serde_yaml::Value::Null).unwrap();
        assert_eq!(result.text, "hello world");
        assert_eq!(result.delete_count, 5);
        assert!(result.cursor_offset.is_none());
    }

    #[test]
    fn test_capture_replacement() {
        let text = "Number: $1, Code: $2";
        let captures = vec!["123".to_string(), "ABC".to_string()];

        let result = replace_captures(text, &captures);
        assert_eq!(result, "Number: 123, Code: ABC");
    }

    #[test]
    fn test_cursor_position() {
        let mut snippet = Snippet::new(";sig", "Hello $|$ World");
        snippet.cursor_position = true;

        let match_result = MatchResult {
            snippet,
            typed_trigger: ";sig".to_string(),
            chars_to_delete: 4,
            captures: None,
        };

        let result = expand_match(&match_result, &serde_yaml::Value::Null).unwrap();
        assert_eq!(result.text, "Hello  World");
        assert_eq!(result.cursor_offset, Some(6)); // 6 chars from end to cursor
    }

    #[test]
    fn test_case_propagation() {
        let mut snippet = Snippet::new(";email", "test@example.com");
        snippet.propagate_case = true;

        // Test uppercase trigger
        let match_result = MatchResult {
            snippet: snippet.clone(),
            typed_trigger: ";EMAIL".to_string(),
            chars_to_delete: 6,
            captures: None,
        };

        let result = expand_match(&match_result, &serde_yaml::Value::Null).unwrap();
        assert_eq!(result.text, "TEST@EXAMPLE.COM");
    }

    #[test]
    fn test_variable_expansion() {
        std::env::set_var("TEST_EXPAND_VAR", "expanded");
        let snippet = Snippet::new(";test", "Value: {{env:TEST_EXPAND_VAR}}");

        let match_result = MatchResult {
            snippet,
            typed_trigger: ";test".to_string(),
            chars_to_delete: 5,
            captures: None,
        };

        let result = expand_match(&match_result, &serde_yaml::Value::Null).unwrap();
        assert_eq!(result.text, "Value: expanded");
    }

    #[test]
    fn test_regex_capture_expansion() {
        let mut snippet = Snippet::new(r";d(\d+)", "Number is $1");
        snippet.regex = true;

        let match_result = MatchResult {
            snippet,
            typed_trigger: ";d456".to_string(),
            chars_to_delete: 5,
            captures: Some(vec!["456".to_string()]),
        };

        let result = expand_match(&match_result, &serde_yaml::Value::Null).unwrap();
        assert_eq!(result.text, "Number is 456");
    }
}
