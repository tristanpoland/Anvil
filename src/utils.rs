use crate::error::{AnvilError, AnvilResult};
use crate::objects::ShellObject;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use regex::Regex;

/// Utility functions for the Anvil shell

/// Expand shell patterns like glob, tilde, and environment variables
pub fn expand_shell_pattern(pattern: &str) -> AnvilResult<Vec<PathBuf>> {
    let mut results = Vec::new();
    
    // Handle tilde expansion
    let expanded = expand_tilde(pattern);
    
    // Handle environment variable expansion
    let expanded = expand_env_vars(&expanded)?;
    
    // Handle glob patterns
    if expanded.contains('*') || expanded.contains('?') || expanded.contains('[') {
        results.extend(expand_glob(&expanded)?);
    } else {
        results.push(PathBuf::from(expanded));
    }
    
    Ok(results)
}

/// Expand tilde (~) to home directory
pub fn expand_tilde(path: &str) -> String {
    if path.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
            if path == "~" {
                return home.to_string_lossy().to_string();
            } else if path.starts_with("~/") {
                return home.join(&path[2..]).to_string_lossy().to_string();
            }
        }
    }
    path.to_string()
}

/// Expand environment variables in the form $VAR or ${VAR}
pub fn expand_env_vars(text: &str) -> AnvilResult<String> {
    let mut result = text.to_string();
    
    // Handle ${VAR} format
    let brace_re = Regex::new(r"\$\{([^}]+)\}")
        .map_err(|e| AnvilError::runtime(format!("Regex error: {}", e)))?;
    
    for captures in brace_re.captures_iter(text) {
        if let Some(var_name) = captures.get(1) {
            let var_value = std::env::var(var_name.as_str()).unwrap_or_default();
            result = result.replace(&captures[0], &var_value);
        }
    }
    
    // Handle $VAR format (word boundaries)
    let simple_re = Regex::new(r"\$([A-Za-z_][A-Za-z0-9_]*)")
        .map_err(|e| AnvilError::runtime(format!("Regex error: {}", e)))?;
    
    for captures in simple_re.captures_iter(&result.clone()) {
        if let Some(var_name) = captures.get(1) {
            let var_value = std::env::var(var_name.as_str()).unwrap_or_default();
            result = result.replace(&captures[0], &var_value);
        }
    }
    
    Ok(result)
}

/// Simple glob pattern expansion
pub fn expand_glob(pattern: &str) -> AnvilResult<Vec<PathBuf>> {
    let mut results = Vec::new();
    let path = Path::new(pattern);
    
    if let Some(parent) = path.parent() {
        if let Some(filename) = path.file_name() {
            let filename_str = filename.to_string_lossy();
            
            if let Ok(entries) = std::fs::read_dir(parent) {
                for entry in entries.flatten() {
                    let entry_name = entry.file_name().to_string_lossy().to_string();
                    if glob_match(&filename_str, &entry_name) {
                        results.push(entry.path());
                    }
                }
            }
        }
    }
    
    if results.is_empty() {
        results.push(PathBuf::from(pattern));
    }
    
    Ok(results)
}

/// Simple glob pattern matching
pub fn glob_match(pattern: &str, text: &str) -> bool {
    let pattern_chars: Vec<char> = pattern.chars().collect();
    let text_chars: Vec<char> = text.chars().collect();
    
    glob_match_recursive(&pattern_chars, &text_chars, 0, 0)
}

fn glob_match_recursive(pattern: &[char], text: &[char], p_idx: usize, t_idx: usize) -> bool {
    if p_idx >= pattern.len() {
        return t_idx >= text.len();
    }
    
    match pattern[p_idx] {
        '*' => {
            // Try matching zero or more characters
            for i in t_idx..=text.len() {
                if glob_match_recursive(pattern, text, p_idx + 1, i) {
                    return true;
                }
            }
            false
        }
        '?' => {
            // Match exactly one character
            if t_idx < text.len() {
                glob_match_recursive(pattern, text, p_idx + 1, t_idx + 1)
            } else {
                false
            }
        }
        c => {
            // Match literal character
            if t_idx < text.len() && text[t_idx] == c {
                glob_match_recursive(pattern, text, p_idx + 1, t_idx + 1)
            } else {
                false
            }
        }
    }
}

/// Format file size in human-readable format
pub fn format_file_size(size: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    
    if size == 0 {
        return "0 B".to_string();
    }
    
    let mut size_f = size as f64;
    let mut unit_idx = 0;
    
    while size_f >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size_f /= 1024.0;
        unit_idx += 1;
    }
    
    if unit_idx == 0 {
        format!("{} {}", size, UNITS[unit_idx])
    } else {
        format!("{:.1} {}", size_f, UNITS[unit_idx])
    }
}

/// Format duration in human-readable format
pub fn format_duration(duration: std::time::Duration) -> String {
    let secs = duration.as_secs();
    let millis = duration.subsec_millis();
    
    if secs >= 3600 {
        let hours = secs / 3600;
        let minutes = (secs % 3600) / 60;
        format!("{}h {}m", hours, minutes)
    } else if secs >= 60 {
        let minutes = secs / 60;
        let seconds = secs % 60;
        format!("{}m {}s", minutes, seconds)
    } else if secs > 0 {
        format!("{}.{}s", secs, millis / 100)
    } else {
        format!("{}ms", millis)
    }
}

/// Parse command line arguments with basic quoting support
pub fn parse_command_line(line: &str) -> AnvilResult<Vec<String>> {
    let mut args = Vec::new();
    let mut current_arg = String::new();
    let mut chars = line.chars().peekable();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut escape_next = false;
    
    while let Some(ch) = chars.next() {
        if escape_next {
            current_arg.push(ch);
            escape_next = false;
            continue;
        }
        
        match ch {
            '\\' if !in_single_quote => {
                escape_next = true;
            }
            '\'' if !in_double_quote => {
                in_single_quote = !in_single_quote;
            }
            '"' if !in_single_quote => {
                in_double_quote = !in_double_quote;
            }
            ' ' | '\t' if !in_single_quote && !in_double_quote => {
                if !current_arg.is_empty() {
                    args.push(current_arg);
                    current_arg = String::new();
                }
                // Skip multiple whitespace
                while chars.peek() == Some(&' ') || chars.peek() == Some(&'\t') {
                    chars.next();
                }
            }
            _ => {
                current_arg.push(ch);
            }
        }
    }
    
    if in_single_quote || in_double_quote {
        return Err(AnvilError::parse("Unterminated quote"));
    }
    
    if !current_arg.is_empty() {
        args.push(current_arg);
    }
    
    Ok(args)
}

/// Convert Rust value to ShellObject
pub trait ToShellObject {
    fn to_shell_object(self) -> ShellObject;
}

impl ToShellObject for String {
    fn to_shell_object(self) -> ShellObject {
        ShellObject::String(self)
    }
}

impl ToShellObject for &str {
    fn to_shell_object(self) -> ShellObject {
        ShellObject::String(self.to_string())
    }
}

impl ToShellObject for i32 {
    fn to_shell_object(self) -> ShellObject {
        ShellObject::Integer(self as i64)
    }
}

impl ToShellObject for i64 {
    fn to_shell_object(self) -> ShellObject {
        ShellObject::Integer(self)
    }
}

impl ToShellObject for f64 {
    fn to_shell_object(self) -> ShellObject {
        ShellObject::Float(self)
    }
}

impl ToShellObject for bool {
    fn to_shell_object(self) -> ShellObject {
        ShellObject::Boolean(self)
    }
}

impl ToShellObject for () {
    fn to_shell_object(self) -> ShellObject {
        ShellObject::Unit
    }
}

impl<T: ToShellObject> ToShellObject for Vec<T> {
    fn to_shell_object(self) -> ShellObject {
        let objects: Vec<ShellObject> = self.into_iter().map(|item| item.to_shell_object()).collect();
        ShellObject::Array(objects)
    }
}

impl<T: ToShellObject> ToShellObject for HashMap<String, T> {
    fn to_shell_object(self) -> ShellObject {
        let objects: HashMap<String, ShellObject> = self.into_iter()
            .map(|(k, v)| (k, v.to_shell_object()))
            .collect();
        ShellObject::Map(objects)
    }
}

/// Extract ShellObject value as Rust type
pub trait FromShellObject: Sized {
    fn from_shell_object(obj: ShellObject) -> AnvilResult<Self>;
}

impl FromShellObject for String {
    fn from_shell_object(obj: ShellObject) -> AnvilResult<Self> {
        match obj {
            ShellObject::String(s) => Ok(s),
            other => Ok(other.to_display_string()),
        }
    }
}

impl FromShellObject for i64 {
    fn from_shell_object(obj: ShellObject) -> AnvilResult<Self> {
        match obj {
            ShellObject::Integer(i) => Ok(i),
            ShellObject::Float(f) => Ok(f as i64),
            ShellObject::String(s) => {
                s.parse().map_err(|_| AnvilError::type_error("integer", "string"))
            }
            other => Err(AnvilError::type_error("integer", other.type_name())),
        }
    }
}

impl FromShellObject for f64 {
    fn from_shell_object(obj: ShellObject) -> AnvilResult<Self> {
        match obj {
            ShellObject::Float(f) => Ok(f),
            ShellObject::Integer(i) => Ok(i as f64),
            ShellObject::String(s) => {
                s.parse().map_err(|_| AnvilError::type_error("float", "string"))
            }
            other => Err(AnvilError::type_error("float", other.type_name())),
        }
    }
}

impl FromShellObject for bool {
    fn from_shell_object(obj: ShellObject) -> AnvilResult<Self> {
        match obj {
            ShellObject::Boolean(b) => Ok(b),
            ShellObject::Integer(i) => Ok(i != 0),
            ShellObject::String(s) => match s.to_lowercase().as_str() {
                "true" | "yes" | "1" => Ok(true),
                "false" | "no" | "0" => Ok(false),
                _ => Err(AnvilError::type_error("boolean", "string")),
            },
            other => Err(AnvilError::type_error("boolean", other.type_name())),
        }
    }
}

impl FromShellObject for Vec<ShellObject> {
    fn from_shell_object(obj: ShellObject) -> AnvilResult<Self> {
        match obj {
            ShellObject::Array(arr) => Ok(arr),
            other => Err(AnvilError::type_error("array", other.type_name())),
        }
    }
}

impl FromShellObject for HashMap<String, ShellObject> {
    fn from_shell_object(obj: ShellObject) -> AnvilResult<Self> {
        match obj {
            ShellObject::Map(map) => Ok(map),
            other => Err(AnvilError::type_error("map", other.type_name())),
        }
    }
}

/// Utility for working with paths
pub struct PathUtils;

impl PathUtils {
    /// Check if a path is safe (doesn't contain dangerous patterns)
    pub fn is_safe_path(path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        
        // Check for dangerous patterns
        if path_str.contains("..") {
            return false;
        }
        
        // Check for absolute paths outside of allowed directories
        if path.is_absolute() {
            // In a real implementation, you'd check against allowed directories
            return true;
        }
        
        true
    }
    
    /// Normalize a path (resolve . and .. components)
    pub fn normalize_path(path: &Path) -> PathBuf {
        let mut components = Vec::new();
        
        for component in path.components() {
            match component {
                std::path::Component::Normal(name) => components.push(name),
                std::path::Component::ParentDir => {
                    components.pop();
                }
                std::path::Component::CurDir => {
                    // Skip current directory references
                }
                _ => components.push(component.as_os_str()),
            }
        }
        
        components.iter().collect()
    }
    
    /// Get relative path from base to target
    pub fn relative_path(base: &Path, target: &Path) -> Option<PathBuf> {
        use std::path::Component;
        
        let base_components: Vec<_> = base.components().collect();
        let target_components: Vec<_> = target.components().collect();
        
        // Find common prefix
        let common_len = base_components
            .iter()
            .zip(target_components.iter())
            .take_while(|(a, b)| a == b)
            .count();
        
        // Build relative path
        let mut relative = PathBuf::new();
        
        // Add .. for each remaining base component
        for _ in common_len..base_components.len() {
            relative.push("..");
        }
        
        // Add remaining target components
        for component in &target_components[common_len..] {
            match component {
                Component::Normal(name) => relative.push(name),
                _ => continue,
            }
        }
        
        if relative.as_os_str().is_empty() {
            Some(PathBuf::from("."))
        } else {
            Some(relative)
        }
    }
}

/// Text processing utilities
pub struct TextUtils;

impl TextUtils {
    /// Word wrap text to specified width
    pub fn word_wrap(text: &str, width: usize) -> String {
        let mut result = String::new();
        let mut current_line_len = 0;
        
        for word in text.split_whitespace() {
            if current_line_len + word.len() + 1 > width && current_line_len > 0 {
                result.push('\n');
                current_line_len = 0;
            }
            
            if current_line_len > 0 {
                result.push(' ');
                current_line_len += 1;
            }
            
            result.push_str(word);
            current_line_len += word.len();
        }
        
        result
    }
    
    /// Truncate text to specified length with ellipsis
    pub fn truncate(text: &str, max_len: usize) -> String {
        if text.len() <= max_len {
            text.to_string()
        } else if max_len <= 3 {
            "...".to_string()
        } else {
            format!("{}...", &text[..max_len - 3])
        }
    }
    
    /// Center text within specified width
    pub fn center(text: &str, width: usize) -> String {
        if text.len() >= width {
            return text.to_string();
        }
        
        let padding = width - text.len();
        let left_padding = padding / 2;
        let right_padding = padding - left_padding;
        
        format!("{}{}{}", " ".repeat(left_padding), text, " ".repeat(right_padding))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tilde_expansion() {
        let expanded = expand_tilde("~/test");
        assert!(expanded.contains("test"));
        
        let expanded = expand_tilde("/absolute/path");
        assert_eq!(expanded, "/absolute/path");
    }

    #[test]
    fn test_glob_matching() {
        assert!(glob_match("*.txt", "file.txt"));
        assert!(glob_match("test*", "test123"));
        assert!(glob_match("file?.txt", "file1.txt"));
        assert!(!glob_match("*.txt", "file.rs"));
    }

    #[test]
    fn test_command_line_parsing() {
        let args = parse_command_line("echo \"hello world\" test").unwrap();
        assert_eq!(args, vec!["echo", "hello world", "test"]);
        
        let args = parse_command_line("echo 'single quotes' test").unwrap();
        assert_eq!(args, vec!["echo", "single quotes", "test"]);
    }

    #[test]
    fn test_file_size_formatting() {
        assert_eq!(format_file_size(512), "512 B");
        assert_eq!(format_file_size(1024), "1.0 KB");
        assert_eq!(format_file_size(1048576), "1.0 MB");
    }

    #[test]
    fn test_path_normalization() {
        let path = PathBuf::from("./test/../file.txt");
        let normalized = PathUtils::normalize_path(&path);
        assert_eq!(normalized, PathBuf::from("file.txt"));
    }

    #[test]
    fn test_text_wrapping() {
        let text = "This is a long line that should be wrapped";
        let wrapped = TextUtils::word_wrap(text, 20);
        assert!(wrapped.contains('\n'));
    }

    #[test]
    fn test_text_truncation() {
        let text = "This is a very long string";
        let truncated = TextUtils::truncate(text, 10);
        assert_eq!(truncated, "This is...");
    }

    #[test]
    fn test_to_shell_object() {
        let obj = "test".to_shell_object();
        assert!(matches!(obj, ShellObject::String(s) if s == "test"));
        
        let obj = 42.to_shell_object();
        assert!(matches!(obj, ShellObject::Integer(42)));
        
        let obj = vec![1, 2, 3].to_shell_object();
        assert!(matches!(obj, ShellObject::Array(_)));
    }

    #[test]
    fn test_from_shell_object() {
        let obj = ShellObject::String("42".to_string());
        let value: i64 = FromShellObject::from_shell_object(obj).unwrap();
        assert_eq!(value, 42);
        
        let obj = ShellObject::Boolean(true);
        let value: bool = FromShellObject::from_shell_object(obj).unwrap();
        assert!(value);
    }
}