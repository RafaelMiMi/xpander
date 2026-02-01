use anyhow::{Context, Result};
use chrono::Local;
use rand::Rng;
use regex::Regex;
use std::process::Command;
use std::sync::LazyLock;

/// Regex for matching variable patterns in text
static VARIABLE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\{\{([^}]+)\}\}").expect("Invalid variable regex")
});

/// Expand all variables in the given text
pub fn expand_variables(text: &str, custom_vars: &serde_yaml::Value) -> Result<String> {
    let mut result = text.to_string();
    let mut offset: i64 = 0;

    for cap in VARIABLE_REGEX.captures_iter(text) {
        let full_match = cap.get(0).unwrap();
        let var_content = &cap[1];

        let replacement = expand_single_variable(var_content, custom_vars)?;

        let start = (full_match.start() as i64 + offset) as usize;
        let end = (full_match.end() as i64 + offset) as usize;

        result.replace_range(start..end, &replacement);
        offset += replacement.len() as i64 - full_match.len() as i64;
    }

    Ok(result)
}

/// Expand a single variable (without the {{ }} markers)
fn expand_single_variable(var: &str, custom_vars: &serde_yaml::Value) -> Result<String> {
    let var = var.trim();

    // Check for custom variable first
    if let Some(val) = expand_custom_variable(var, custom_vars) {
        return Ok(val);
    }

    // Handle different variable types
    if var == "date" {
        Ok(expand_date(None))
    } else if let Some(format) = var.strip_prefix("date:") {
        Ok(expand_date(Some(format.trim())))
    } else if var == "time" {
        Ok(expand_time(None))
    } else if let Some(format) = var.strip_prefix("time:") {
        Ok(expand_time(Some(format.trim())))
    } else if var == "datetime" {
        Ok(expand_datetime(None))
    } else if let Some(format) = var.strip_prefix("datetime:") {
        Ok(expand_datetime(Some(format.trim())))
    } else if var == "clipboard" {
        expand_clipboard()
    } else if let Some(n) = var.strip_prefix("random:") {
        expand_random(n.trim())
    } else if let Some(var_name) = var.strip_prefix("env:") {
        expand_env(var_name.trim())
    } else if let Some(cmd) = var.strip_prefix("shell:") {
        expand_shell(cmd.trim())
    } else if var == "uuid" {
        Ok(expand_uuid())
    } else if var == "cursor" || var == "|" {
        // Cursor position marker - keep it for later processing
        Ok("$|$".to_string())
    } else {
        // Unknown variable - keep as-is
        log::warn!("Unknown variable: {}", var);
        Ok(format!("{{{{{}}}}}", var))
    }
}

/// Expand date variable with optional format
fn expand_date(format: Option<&str>) -> String {
    let now = Local::now();
    let fmt = format.unwrap_or("%Y-%m-%d");
    now.format(fmt).to_string()
}

/// Expand time variable with optional format
fn expand_time(format: Option<&str>) -> String {
    let now = Local::now();
    let fmt = format.unwrap_or("%H:%M:%S");
    now.format(fmt).to_string()
}

/// Expand datetime variable with optional format
fn expand_datetime(format: Option<&str>) -> String {
    let now = Local::now();
    let fmt = format.unwrap_or("%Y-%m-%d %H:%M:%S");
    now.format(fmt).to_string()
}

/// Expand clipboard variable
fn expand_clipboard() -> Result<String> {
    let mut clipboard = arboard::Clipboard::new()
        .context("Failed to access clipboard")?;

    clipboard
        .get_text()
        .context("Failed to get clipboard text")
}

/// Expand random number variable
fn expand_random(n: &str) -> Result<String> {
    let digits: usize = n.parse()
        .context("Invalid number of digits for random")?;

    if digits == 0 || digits > 20 {
        anyhow::bail!("Random digits must be between 1 and 20");
    }

    let mut rng = rand::thread_rng();
    let min = if digits == 1 { 0 } else { 10_u64.pow(digits as u32 - 1) };
    let max = 10_u64.pow(digits as u32);

    let num = rng.gen_range(min..max);
    Ok(format!("{:0width$}", num, width = digits))
}

/// Expand environment variable
fn expand_env(var_name: &str) -> Result<String> {
    std::env::var(var_name)
        .with_context(|| format!("Environment variable '{}' not found", var_name))
}

/// Expand shell command variable
fn expand_shell(cmd: &str) -> Result<String> {
    let output = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .output()
        .with_context(|| format!("Failed to execute shell command: {}", cmd))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Shell command failed: {}", stderr);
    }

    let stdout = String::from_utf8_lossy(&output.stdout)
        .trim_end_matches('\n')
        .to_string();

    Ok(stdout)
}

/// Expand UUID variable
fn expand_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Find cursor position marker in text and return (text_without_marker, cursor_offset)
pub fn find_cursor_position(text: &str) -> (String, Option<usize>) {
    const CURSOR_MARKER: &str = "$|$";

    if let Some(pos) = text.find(CURSOR_MARKER) {
        let cleaned = format!("{}{}", &text[..pos], &text[pos + CURSOR_MARKER.len()..]);
        Some((cleaned, Some(pos)))
    } else {
        None
    }
    .unwrap_or_else(|| (text.to_string(), None))
}

/// Expand custom variable using dot notation (e.g. "user.email")
fn expand_custom_variable(var_path: &str, custom_vars: &serde_yaml::Value) -> Option<String> {
    let parts: Vec<&str> = var_path.split('.').collect();
    let mut current = custom_vars;

    for part in parts {
        match current {
            serde_yaml::Value::Mapping(map) => {
                // Try exact match first
                if let Some(val) = map.get(&serde_yaml::Value::String(part.to_string())) {
                    current = val;
                } else {
                    return None;
                }
            }
            _ => return None,
        }
    }

    match current {
        serde_yaml::Value::String(s) => Some(s.clone()),
        serde_yaml::Value::Number(n) => Some(n.to_string()),
        serde_yaml::Value::Bool(b) => Some(b.to_string()),
        serde_yaml::Value::Null => Some("".to_string()),
        _ => None, // Complex types (arrays/objects) not supported as direct replacement
    }
}

/// Apply case propagation from trigger to replacement
pub fn propagate_case(trigger: &str, replacement: &str) -> String {
    if trigger.is_empty() || replacement.is_empty() {
        return replacement.to_string();
    }

    let trigger_chars: Vec<char> = trigger.chars().collect();
    let first_char = trigger_chars[0];

    // Check if trigger is all uppercase
    let is_all_upper = trigger_chars.iter().filter(|c| c.is_alphabetic()).all(|c| c.is_uppercase());

    // Check if trigger starts with uppercase (title case)
    let is_title_case = first_char.is_uppercase()
        && trigger_chars.iter().skip(1).filter(|c| c.is_alphabetic()).all(|c| c.is_lowercase());

    if is_all_upper && trigger_chars.iter().any(|c| c.is_alphabetic()) {
        // ALL CAPS
        replacement.to_uppercase()
    } else if is_title_case {
        // Title Case - capitalize first letter only
        let mut chars = replacement.chars();
        match chars.next() {
            None => String::new(),
            Some(first) => first.to_uppercase().chain(chars).collect(),
        }
    } else {
        // Keep original case
        replacement.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_date() {
        let result = expand_date(None);
        assert!(result.len() == 10); // YYYY-MM-DD
        assert!(result.contains('-'));
    }

    #[test]
    fn test_expand_date_custom_format() {
        let result = expand_date(Some("%d/%m/%Y"));
        assert!(result.len() == 10);
        assert!(result.contains('/'));
    }

    #[test]
    fn test_expand_time() {
        let result = expand_time(None);
        assert!(result.contains(':'));
    }

    #[test]
    fn test_expand_random() {
        let result = expand_random("5").unwrap();
        assert_eq!(result.len(), 5);
        assert!(result.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_expand_env() {
        std::env::set_var("TEST_VAR_XPANDER", "test_value");
        let result = expand_env("TEST_VAR_XPANDER").unwrap();
        assert_eq!(result, "test_value");
    }

    #[test]
    fn test_expand_shell() {
        let result = expand_shell("echo hello").unwrap();
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_expand_uuid() {
        let result = expand_uuid();
        assert_eq!(result.len(), 36);
        assert!(result.contains('-'));
    }

    #[test]
    fn test_expand_variables() {
        std::env::set_var("TEST_USER", "testuser");
        let text = "Hello {{env:TEST_USER}}, today is {{date}}";
        let result = expand_variables(text, &serde_yaml::Value::Null).unwrap();
        assert!(result.contains("testuser"));
        assert!(!result.contains("{{"));
    }

    #[test]
    fn test_custom_variables() {
        let yaml = r#"
        user:
            name: "Rafa"
            contact:
                email: "test@example.com"
            age: 30
        "#;
        let vars: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();

        let text = "Hi {{user.name}}, email: {{user.contact.email}}, age: {{user.age}}";
        let result = expand_variables(text, &vars).unwrap();

        assert!(result.contains("Hi Rafa"));
        assert!(result.contains("email: test@example.com"));
        assert!(result.contains("age: 30"));
    }

    #[test]
    fn test_expand_variables_default() {
        // Test with empty custom variables (should behave like before)
        std::env::set_var("TEST_USER_DEF", "testuser");
        let text = "Hello {{env:TEST_USER_DEF}}, today is {{date}}";
        let vars = serde_yaml::Value::Null;
        let result = expand_variables(text, &vars).unwrap();
        assert!(result.contains("testuser"));
        assert!(!result.contains("{{"));
    }

    #[test]
    fn test_find_cursor_position() {
        let (text, pos) = find_cursor_position("Hello $|$ World");
        assert_eq!(text, "Hello  World");
        assert_eq!(pos, Some(6));

        let (text, pos) = find_cursor_position("No cursor here");
        assert_eq!(text, "No cursor here");
        assert_eq!(pos, None);
    }

    #[test]
    fn test_propagate_case_all_upper() {
        let result = propagate_case("EMAIL", "test@example.com");
        assert_eq!(result, "TEST@EXAMPLE.COM");
    }

    #[test]
    fn test_propagate_case_title() {
        let result = propagate_case("Email", "test@example.com");
        assert_eq!(result, "Test@example.com");
    }

    #[test]
    fn test_propagate_case_lower() {
        let result = propagate_case("email", "Test@Example.com");
        assert_eq!(result, "Test@Example.com");
    }
}
