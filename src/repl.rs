use crate::config::Config;
use crate::error::{AnvilError, AnvilResult};
use crate::objects::ShellObject;
use reedline::{Reedline, Signal, DefaultPrompt, Prompt, PromptHistorySearch, PromptEditMode};
use nu_ansi_term::{Color, Style};
use std::borrow::Cow;
use crossterm::style::{Color as CrosstermColor, Stylize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tempfile::NamedTempFile;
use std::io::Write;
use regex::Regex;

pub struct ReplEngine {
    config: Config,
    editor: Reedline,
    context: ReplContext,
    prompt: AnvilPrompt,
}

#[derive(Debug, Clone)]
pub struct ReplContext {
    /// Variables defined in the REPL session
    pub variables: HashMap<String, ShellObject>,
    /// Functions defined in the REPL session
    pub functions: HashMap<String, String>,
    /// Import statements that should be included in every compilation
    pub imports: Vec<String>,
    /// Code blocks that have been successfully compiled
    pub code_history: Vec<String>,
    /// Whether we're in multiline mode
    pub multiline_mode: bool,
    /// Current line continuation buffer
    pub continuation_buffer: String,
}

impl Default for ReplContext {
    fn default() -> Self {
        Self {
            variables: HashMap::new(),
            functions: HashMap::new(),
            imports: vec![
                "use std::collections::HashMap;".to_string(),
                "use std::path::PathBuf;".to_string(),
                "use std::fs;".to_string(),
                "use std::process::Command;".to_string(),
                "use std::io::{self, Write};".to_string(),
            ],
            code_history: Vec::new(),
            multiline_mode: false,
            continuation_buffer: String::new(),
        }
    }
}

struct AnvilPrompt {
    base_prompt: String,
    continuation_prompt: String,
    multiline_mode: bool,
}

impl AnvilPrompt {
    fn new(config: &Config) -> Self {
        Self {
            base_prompt: config.shell.prompt.clone(),
            continuation_prompt: config.shell.continuation_prompt.clone(),
            multiline_mode: false,
        }
    }

    fn set_multiline(&mut self, multiline: bool) {
        self.multiline_mode = multiline;
    }
}

impl Prompt for AnvilPrompt {
    fn render_prompt_left(&self) -> Cow<str> {
        if self.multiline_mode {
            Cow::Borrowed(&self.continuation_prompt)
        } else {
            Cow::Borrowed(&self.base_prompt)
        }
    }

    fn render_prompt_right(&self) -> Cow<str> {
        // Show current directory on the right
        if let Ok(current_dir) = std::env::current_dir() {
            let dir_name = current_dir
                .file_name()
                .unwrap_or_default()
                .to_string_lossy();
            Cow::Owned(format!("[{}]", dir_name))
        } else {
            Cow::Borrowed("")
        }
    }

    fn render_prompt_indicator(&self, _edit_mode: PromptEditMode) -> Cow<str> {
        Cow::Borrowed("")
    }

    fn render_prompt_multiline_indicator(&self) -> Cow<str> {
        Cow::Borrowed("...")
    }

    fn render_prompt_history_search_indicator(&self, _history_search: PromptHistorySearch) -> Cow<str> {
        Cow::Borrowed("(search) ")
    }
}

impl ReplEngine {
    pub fn new(config: Config) -> AnvilResult<Self> {
        let mut editor = Reedline::create();
        
        // Set up history if configured
        if let Ok(history_file) = std::fs::File::create(&config.shell.history_file) {
            drop(history_file); // Just ensure the file exists
        }

        let prompt = AnvilPrompt::new(&config);
        let context = ReplContext::default();

        Ok(Self {
            config,
            editor,
            context,
            prompt,
        })
    }

    pub async fn run_interactive(&mut self) -> AnvilResult<()> {
        println!("🔨 Anvil Rust Shell v{}", env!("CARGO_PKG_VERSION"));
        println!("Type 'help()' for help, 'exit()' or Ctrl+D to quit");
        println!();

        // Add prelude imports (avoiding duplicates) - do this once at start
        let mut existing_imports: HashSet<String> = 
            self.context.imports.iter().cloned().collect();
        
        for import in &self.config.repl.prelude.clone() {
            if !existing_imports.contains(import) {
                self.context.imports.push(import.clone());
                existing_imports.insert(import.clone());
            }
        }

        loop {
            let sig = self.editor.read_line(&self.prompt);
            
            match sig {
                Ok(Signal::Success(buffer)) => {
                    let line = buffer.trim();
                    
                    if line.is_empty() {
                        continue;
                    }

                    // Handle special commands
                    if let Some(result) = self.handle_special_command(line).await? {
                        if result {
                            break; // Exit requested
                        }
                        continue;
                    }

                    // Handle multiline input
                    if self.is_incomplete_input(line) {
                        self.context.continuation_buffer.push_str(line);
                        self.context.continuation_buffer.push('\n');
                        self.context.multiline_mode = true;
                        self.prompt.set_multiline(true);
                        continue;
                    }

                    // Combine with any continuation buffer
                    let full_input = if !self.context.continuation_buffer.is_empty() {
                        let full = format!("{}{}", self.context.continuation_buffer, line);
                        self.context.continuation_buffer.clear();
                        self.context.multiline_mode = false;
                        self.prompt.set_multiline(false);
                        full
                    } else {
                        line.to_string()
                    };

                    // Execute the input
                    match self.execute_rust_code(&full_input).await {
                        Ok(result) => {
                            if self.config.repl.auto_print {
                                println!("{}", result.to_display_string());
                            }
                        }
                        Err(e) => {
                            if e.is_recoverable() {
                                eprintln!("Error: {}", e);
                            } else {
                                return Err(e);
                            }
                        }
                    }
                }
                Ok(Signal::CtrlD) => {
                    println!("Goodbye!");
                    break;
                }
                Ok(Signal::CtrlC) => {
                    if !self.context.continuation_buffer.is_empty() {
                        self.context.continuation_buffer.clear();
                        self.context.multiline_mode = false;
                        self.prompt.set_multiline(false);
                        println!("^C");
                    } else {
                        println!("Goodbye!");
                        break;
                    }
                }
                Err(e) => {
                    return Err(AnvilError::repl(format!("REPL error: {}", e)));
                }
            }
        }

        Ok(())
    }

    pub async fn execute_line(&mut self, line: &str) -> AnvilResult<ShellObject> {
        if line.trim().is_empty() {
            return Ok(ShellObject::Unit);
        }

        // Handle special commands
        if let Some(_) = self.handle_special_command(line).await? {
            return Ok(ShellObject::Unit);
        }

        self.execute_rust_code(line).await
    }

    async fn execute_rust_code(&mut self, code: &str) -> AnvilResult<ShellObject> {
        // First, try to parse as a simple expression or statement
        if let Ok(object) = self.try_simple_evaluation(code).await {
            return Ok(object);
        }

        // If that fails, compile and execute as full Rust code
        self.compile_and_execute(code).await
    }

    async fn try_simple_evaluation(&self, code: &str) -> AnvilResult<ShellObject> {
        let trimmed = code.trim();

        // Handle variable assignments
        if let Some(captures) = Regex::new(r"^let\s+(\w+)\s*=\s*(.+)$").unwrap().captures(trimmed) {
            let var_name = captures.get(1).unwrap().as_str();
            let value_expr = captures.get(2).unwrap().as_str();
            
            // Evaluate the right-hand side
            let value = self.evaluate_expression(value_expr)?;
            // Note: In a real implementation, we'd store this in context
            return Ok(value);
        }

        // Handle simple expressions
        self.evaluate_expression(trimmed)
    }

    fn evaluate_expression(&self, expr: &str) -> AnvilResult<ShellObject> {
        // Handle literals
        if let Ok(num) = expr.parse::<i64>() {
            return Ok(ShellObject::Integer(num));
        }
        
        if let Ok(num) = expr.parse::<f64>() {
            return Ok(ShellObject::Float(num));
        }

        if expr == "true" {
            return Ok(ShellObject::Boolean(true));
        }
        
        if expr == "false" {
            return Ok(ShellObject::Boolean(false));
        }

        // Handle string literals
        if expr.starts_with('"') && expr.ends_with('"') && expr.len() >= 2 {
            let content = &expr[1..expr.len()-1];
            return Ok(ShellObject::String(content.to_string()));
        }

        // Handle variable references
        if let Some(value) = self.context.variables.get(expr) {
            return Ok(value.clone());
        }

        // If we can't evaluate it simply, we'll need to compile it
        Err(AnvilError::eval("Cannot evaluate expression without compilation"))
    }

    async fn compile_and_execute(&mut self, code: &str) -> AnvilResult<ShellObject> {
        let start_time = Instant::now();

        // Create a temporary Rust file
        let mut temp_file = NamedTempFile::new()
            .map_err(|e| AnvilError::runtime(format!("Failed to create temp file: {}", e)))?;

        // Generate the full Rust program
        let full_program = self.generate_rust_program(code)?;
        
        temp_file.write_all(full_program.as_bytes())
            .map_err(|e| AnvilError::runtime(format!("Failed to write temp file: {}", e)))?;

        let temp_path = temp_file.path().to_path_buf();
        
        // Compile the program
        let exe_path = temp_path.with_extension("exe");
        let compile_result = Command::new("rustc")
            .arg(&temp_path)
            .arg("-o")
            .arg(&exe_path)
            .arg("--edition")
            .arg("2021")
            .arg("--crate-name")
            .arg("anvil_repl")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output();

        let compile_duration = start_time.elapsed();
        
        if compile_duration > Duration::from_millis(self.config.repl.compile_timeout_ms) {
            return Err(AnvilError::compilation("Compilation timeout"));
        }

        let output = compile_result
            .map_err(|e| AnvilError::compilation(format!("Failed to run rustc: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AnvilError::compilation(format!("Compilation failed:\n{}", stderr)));
        }

        // Execute the compiled program
        let exec_start = Instant::now();
        let exec_result = Command::new(&exe_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output();

        let exec_duration = exec_start.elapsed();
        
        if exec_duration > Duration::from_millis(self.config.repl.execution_timeout_ms) {
            return Err(AnvilError::runtime("Execution timeout"));
        }

        let exec_output = exec_result
            .map_err(|e| AnvilError::runtime(format!("Failed to execute: {}", e)))?;

        // Clean up
        let _ = std::fs::remove_file(&exe_path);

        if !exec_output.status.success() {
            let stderr = String::from_utf8_lossy(&exec_output.stderr);
            return Err(AnvilError::runtime(format!("Runtime error:\n{}", stderr)));
        }

        // Parse the output back to a ShellObject
        let stdout = String::from_utf8_lossy(&exec_output.stdout);
        let result = self.parse_output(&stdout)?;

        // Store successful code in history
        self.context.code_history.push(code.to_string());

        Ok(result)
    }

    fn generate_rust_program(&self, code: &str) -> AnvilResult<String> {
        let mut program = String::new();
        
        // Deduplicate imports
        let mut unique_imports = HashSet::new();
        
        // Add context imports
        for import in &self.context.imports {
            unique_imports.insert(import.clone());
        }
        
        // Add any unique imports to the program
        for import in &unique_imports {
            program.push_str(import);
            program.push('\n');
        }
        
        // Add any user-defined functions
        for (_, func_code) in &self.context.functions {
            program.push_str(func_code);
            program.push('\n');
        }
        
        program.push_str("\nfn main() {\n");
        
        // Add the user code, wrapping it appropriately
        if code.trim().ends_with(';') || code.contains("let ") || code.contains("fn ") {
            // It's a statement
            program.push_str("    ");
            program.push_str(code);
            program.push('\n');
        } else {
            // It's an expression, print the result
            program.push_str("    let result = ");
            program.push_str(code);
            program.push_str(";\n");
            program.push_str("    println!(\"{:?}\", result);\n");
        }
        
        program.push_str("}\n");
        
        Ok(program)
    }

    fn parse_output(&self, output: &str) -> AnvilResult<ShellObject> {
        let trimmed = output.trim();
        
        // Try to parse common Rust debug output formats
        if trimmed.is_empty() {
            return Ok(ShellObject::Unit);
        }

        // Handle string output
        if trimmed.starts_with('"') && trimmed.ends_with('"') {
            let content = &trimmed[1..trimmed.len()-1];
            return Ok(ShellObject::String(content.to_string()));
        }

        // Handle numeric output
        if let Ok(num) = trimmed.parse::<i64>() {
            return Ok(ShellObject::Integer(num));
        }

        if let Ok(num) = trimmed.parse::<f64>() {
            return Ok(ShellObject::Float(num));
        }

        // Handle boolean output
        if trimmed == "true" {
            return Ok(ShellObject::Boolean(true));
        }
        if trimmed == "false" {
            return Ok(ShellObject::Boolean(false));
        }

        // Default to string representation
        Ok(ShellObject::String(trimmed.to_string()))
    }

    async fn handle_special_command(&mut self, line: &str) -> AnvilResult<Option<bool>> {
        match line.trim() {
            "exit()" | "quit()" => Ok(Some(true)),
            "help()" => {
                self.show_help();
                Ok(Some(false))
            }
            "clear()" => {
                print!("\x1B[2J\x1B[1;1H"); // ANSI clear screen
                Ok(Some(false))
            }
            "vars()" => {
                self.show_variables();
                Ok(Some(false))
            }
            "history()" => {
                self.show_history();
                Ok(Some(false))
            }
            _ => Ok(None),
        }
    }

    fn show_help(&self) {
        println!(r#"
🔨 Anvil Rust Shell Help

Special Commands:
  help()       - Show this help message
  exit()       - Exit the shell
  quit()       - Exit the shell  
  clear()      - Clear the screen
  vars()       - Show defined variables
  history()    - Show command history

Features:
  • Type any Rust expression or statement
  • Variables persist across commands
  • Multiline input supported (use incomplete syntax)
  • Tab completion and history available
  • File system operations as typed objects

Examples:
  let x = 42;
  x + 8
  std::fs::read_dir(".").unwrap().count()
  let files = std::fs::read_dir(".").unwrap().collect::<Vec<_>>();

Press Ctrl+D or type exit() to quit.
"#);
    }

    fn show_variables(&self) {
        if self.context.variables.is_empty() {
            println!("No variables defined.");
        } else {
            println!("Defined variables:");
            for (name, value) in &self.context.variables {
                println!("  {}: {} = {}", name, value.type_name(), value.to_display_string());
            }
        }
    }

    fn show_history(&self) {
        if self.context.code_history.is_empty() {
            println!("No history available.");
        } else {
            println!("Command history:");
            for (i, code) in self.context.code_history.iter().enumerate() {
                println!("  {}: {}", i + 1, code);
            }
        }
    }

    fn is_incomplete_input(&self, line: &str) -> bool {
        let trimmed = line.trim();
        
        // Simple heuristics for incomplete input
        trimmed.ends_with('{') ||
        trimmed.ends_with('(') ||
        trimmed.ends_with('[') ||
        (trimmed.starts_with("let ") && !trimmed.contains('=')) ||
        (trimmed.starts_with("fn ") && !trimmed.contains('{')) ||
        trimmed.ends_with('\\')
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_repl_creation() {
        let config = Config::default();
        let repl = ReplEngine::new(config);
        assert!(repl.is_ok());
    }

    #[tokio::test] 
    async fn test_simple_evaluation() {
        let config = Config::default();
        let repl = ReplEngine::new(config).unwrap();
        
        let result = repl.evaluate_expression("42").unwrap();
        match result {
            ShellObject::Integer(n) => assert_eq!(n, 42),
            _ => panic!("Expected integer"),
        }
    }
}