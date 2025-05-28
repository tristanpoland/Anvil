use crate::config::Config;
use crate::error::{AnvilError, AnvilResult};
use crate::objects::ShellObject;
use crate::repl::ReplEngine;
use crate::commands::CommandRegistry;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tokio::fs;
use regex::Regex;

pub struct Shell {
    config: Config,
    repl: ReplEngine,
    commands: CommandRegistry,
    env: HashMap<String, String>,
    current_dir: PathBuf,
    aliases: HashMap<String, String>,
}

impl Shell {
    pub async fn new(config: Config) -> AnvilResult<Self> {
        let repl = ReplEngine::new(config.clone())?;
        let commands = CommandRegistry::new();
        
        // Initialize environment
        let mut env = HashMap::new();
        if config.environment.inherit_system_env {
            for (key, value) in std::env::vars() {
                env.insert(key, value);
            }
        }
        
        // Add default environment variables
        for (key, value) in &config.environment.default_vars {
            env.insert(key.clone(), value.clone());
        }

        let current_dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."));

        let aliases = config.aliases.clone();

        Ok(Self {
            config,
            repl,
            commands,
            env,
            current_dir,
            aliases,
        })
    }

    pub async fn run_repl(&mut self) -> AnvilResult<()> {
        self.repl.run_interactive().await
    }

    pub async fn execute_command(&mut self, command: &str) -> AnvilResult<ShellObject> {
        let command = command.trim();
        
        if command.is_empty() {
            return Ok(ShellObject::Unit);
        }

        // Check for shell built-ins first
        if let Some(result) = self.try_builtin_command(command).await? {
            return Ok(result);
        }

        // Check for aliases
        if let Some(alias_command) = self.resolve_alias(command) {
            return Box::pin(self.execute_command(&alias_command)).await;
        }

        // Try to execute as Rust code in the REPL
        match self.repl.execute_line(command).await {
            Ok(result) => Ok(result),
            Err(_) => {
                // If REPL execution fails, try as external command
                self.execute_external_command(command).await
            }
        }
    }

    pub async fn execute_script(&mut self, script_path: &Path) -> AnvilResult<()> {
        let content = fs::read_to_string(script_path).await?;
        let lines = content.lines();

        for (line_num, line) in lines.enumerate() {
            let line = line.trim();
            
            // Skip empty lines and comments
            if line.is_empty() || line.starts_with("//") || line.starts_with('#') {
                continue;
            }

            match self.execute_command(line).await {
                Ok(result) => {
                    if self.config.repl.auto_print {
                        println!("{}", result.to_display_string());
                    }
                }
                Err(e) => {
                    eprintln!("Error on line {}: {}", line_num + 1, e);
                    if !e.is_recoverable() {
                        return Err(e);
                    }
                }
            }
        }

        Ok(())
    }

    async fn try_builtin_command(&mut self, command: &str) -> AnvilResult<Option<ShellObject>> {
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(None);
        }

        let cmd = parts[0];
        let args = &parts[1..];

        match cmd {
            "cd" => {
                let target = if args.is_empty() {
                    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
                } else {
                    self.expand_path(args[0])
                };
                
                self.change_directory(&target).await?;
                Ok(Some(ShellObject::String(format!("Changed to {}", target.display()))))
            }
            "pwd" => {
                Ok(Some(ShellObject::String(self.current_dir.to_string_lossy().to_string())))
            }
            "ls" => {
                let path = if args.is_empty() {
                    &self.current_dir
                } else {
                    &self.expand_path(args[0])
                };
                
                let entries = self.list_directory(path).await?;
                Ok(Some(ShellObject::Array(entries)))
            }
            "echo" => {
                let output = args.join(" ");
                println!("{}", output);
                Ok(Some(ShellObject::String(output)))
            }
            "env" => {
                if args.is_empty() {
                    let env_vars: HashMap<String, ShellObject> = self.env.iter()
                        .map(|(k, v)| (k.clone(), ShellObject::String(v.clone())))
                        .collect();
                    Ok(Some(ShellObject::Map(env_vars)))
                } else {
                    // Set environment variable
                    if let Some(eq_pos) = args[0].find('=') {
                        let key = &args[0][..eq_pos];
                        let value = &args[0][eq_pos + 1..];
                        self.env.insert(key.to_string(), value.to_string());
                        std::env::set_var(key, value);
                        Ok(Some(ShellObject::String(format!("Set {}={}", key, value))))
                    } else {
                        // Get environment variable
                        let value = self.env.get(args[0])
                            .cloned()
                            .unwrap_or_else(|| "".to_string());
                        Ok(Some(ShellObject::String(value)))
                    }
                }
            }
            "alias" => {
                if args.is_empty() {
                    // List all aliases
                    let aliases: HashMap<String, ShellObject> = self.aliases.iter()
                        .map(|(k, v)| (k.clone(), ShellObject::String(v.clone())))
                        .collect();
                    Ok(Some(ShellObject::Map(aliases)))
                } else if args.len() == 1 && args[0].contains('=') {
                    // Set alias
                    let eq_pos = args[0].find('=').unwrap();
                    let key = &args[0][..eq_pos];
                    let value = &args[0][eq_pos + 1..];
                    self.aliases.insert(key.to_string(), value.to_string());
                    Ok(Some(ShellObject::String(format!("Set alias {}={}", key, value))))
                } else {
                    // Get alias
                    let value = self.aliases.get(args[0])
                        .cloned()
                        .unwrap_or_else(|| "".to_string());
                    Ok(Some(ShellObject::String(value)))
                }
            }
            "which" => {
                if args.is_empty() {
                    return Err(AnvilError::command("which: missing argument"));
                }
                
                let program = args[0];
                match which::which(program) {
                    Ok(path) => Ok(Some(ShellObject::String(path.to_string_lossy().to_string()))),
                    Err(_) => Ok(Some(ShellObject::String(format!("{}: not found", program)))),
                }
            }
            "type" => {
                if args.is_empty() {
                    return Err(AnvilError::command("type: missing argument"));
                }
                
                let name = args[0];
                if self.aliases.contains_key(name) {
                    Ok(Some(ShellObject::String(format!("{} is an alias", name))))
                } else if self.commands.has_command(name) {
                    Ok(Some(ShellObject::String(format!("{} is a builtin command", name))))
                } else {
                    match which::which(name) {
                        Ok(path) => Ok(Some(ShellObject::String(format!("{} is {}", name, path.display())))),
                        Err(_) => Ok(Some(ShellObject::String(format!("{}: not found", name)))),
                    }
                }
            }
            "exit" | "quit" => {
                std::process::exit(0);
            }
            _ => Ok(None), // Not a builtin command
        }
    }

    async fn execute_external_command(&mut self, command: &str) -> AnvilResult<ShellObject> {
        let parts = self.parse_command_line(command)?;
        if parts.is_empty() {
            return Ok(ShellObject::Unit);
        }

        let program = &parts[0];
        let args = &parts[1..];

        // Check if it's an executable in PATH or relative/absolute path
        let program_path = if program.contains('/') || program.contains('\\') {
            self.expand_path(program)
        } else {
            match which::which(program) {
                Ok(path) => path,
                Err(_) => {
                    return Err(AnvilError::command(format!("Command not found: {}", program)));
                }
            }
        };

        let mut cmd = Command::new(&program_path);
        cmd.args(args)
            .current_dir(&self.current_dir)
            .env_clear();

        // Set environment variables
        for (key, value) in &self.env {
            cmd.env(key, value);
        }

        let output = cmd.output()
            .map_err(|e| AnvilError::command(format!("Failed to execute {}: {}", program, e)))?;

        if !output.status.success() {
            let code = output.status.code().unwrap_or(-1);
            let stderr = String::from_utf8_lossy(&output.stderr);
            
            if !stderr.is_empty() {
                eprintln!("{}", stderr);
            }
            
            return Err(AnvilError::external_command(program.to_string(), code));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(ShellObject::String(stdout.to_string()))
    }

    async fn change_directory(&mut self, path: &Path) -> AnvilResult<()> {
        let new_path = if path.is_relative() {
            self.current_dir.join(path)
        } else {
            path.to_path_buf()
        };

        let canonical_path = fs::canonicalize(&new_path).await
            .map_err(|_| AnvilError::file_not_found(new_path.to_string_lossy().to_string()))?;

        if !canonical_path.is_dir() {
            return Err(AnvilError::command(format!("Not a directory: {}", canonical_path.display())));
        }

        std::env::set_current_dir(&canonical_path)
            .map_err(|e| AnvilError::command(format!("Failed to change directory: {}", e)))?;

        self.current_dir = canonical_path;
        self.env.insert("PWD".to_string(), self.current_dir.to_string_lossy().to_string());

        Ok(())
    }

    async fn list_directory(&self, path: &Path) -> AnvilResult<Vec<ShellObject>> {
        let mut entries = Vec::new();
        let mut dir = fs::read_dir(path).await
            .map_err(|e| AnvilError::file_not_found(format!("Cannot read directory {}: {}", path.display(), e)))?;

        while let Some(entry) = dir.next_entry().await? {
            let file_name = entry.file_name().to_string_lossy().to_string();
            let metadata = entry.metadata().await?;
            
            let entry_type = if metadata.is_dir() {
                "directory"
            } else if metadata.is_file() {
                "file"
            } else {
                "other"
            };

            let mut entry_map = HashMap::new();
            entry_map.insert("name".to_string(), ShellObject::String(file_name));
            entry_map.insert("type".to_string(), ShellObject::String(entry_type.to_string()));
            entry_map.insert("size".to_string(), ShellObject::Integer(metadata.len() as i64));
            
            entries.push(ShellObject::Map(entry_map));
        }

        Ok(entries)
    }

    fn resolve_alias(&self, command: &str) -> Option<String> {
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return None;
        }

        let cmd = parts[0];
        if let Some(alias) = self.aliases.get(cmd) {
            if parts.len() > 1 {
                let args = parts[1..].join(" ");
                Some(format!("{} {}", alias, args))
            } else {
                Some(alias.clone())
            }
        } else {
            None
        }
    }

    fn expand_path(&self, path: &str) -> PathBuf {
        if path.starts_with('~') {
            if let Some(home) = dirs::home_dir() {
                if path == "~" {
                    return home;
                } else if path.starts_with("~/") {
                    return home.join(&path[2..]);
                }
            }
        }

        if path.starts_with('.') {
            self.current_dir.join(path)
        } else {
            PathBuf::from(path)
        }
    }

    fn parse_command_line(&self, command: &str) -> AnvilResult<Vec<String>> {
        let mut parts = Vec::new();
        let mut current = String::new();
        let mut in_quotes = false;
        let mut escape_next = false;

        for ch in command.chars() {
            if escape_next {
                current.push(ch);
                escape_next = false;
            } else if ch == '\\' {
                escape_next = true;
            } else if ch == '"' || ch == '\'' {
                in_quotes = !in_quotes;
            } else if ch.is_whitespace() && !in_quotes {
                if !current.is_empty() {
                    parts.push(current);
                    current = String::new();
                }
            } else {
                current.push(ch);
            }
        }

        if !current.is_empty() {
            parts.push(current);
        }

        if in_quotes {
            return Err(AnvilError::parse("Unterminated quote"));
        }

        Ok(parts)
    }

    /// Get current working directory
    pub fn current_dir(&self) -> &Path {
        &self.current_dir
    }

    /// Get environment variables
    pub fn env(&self) -> &HashMap<String, String> {
        &self.env
    }

    /// Set environment variable
    pub fn set_env(&mut self, key: String, value: String) {
        std::env::set_var(&key, &value);
        self.env.insert(key, value);
    }

    /// Get shell configuration
    pub fn config(&self) -> &Config {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_shell_creation() {
        let config = Config::default();
        let shell = Shell::new(config).await;
        assert!(shell.is_ok());
    }

    #[tokio::test]
    async fn test_pwd_command() {
        let config = Config::default();
        let mut shell = Shell::new(config).await.unwrap();
        
        let result = shell.execute_command("pwd").await.unwrap();
        match result {
            ShellObject::String(path) => {
                assert!(!path.is_empty());
            }
            _ => panic!("Expected string result for pwd"),
        }
    }

    #[tokio::test]
    async fn test_echo_command() {
        let config = Config::default();
        let mut shell = Shell::new(config).await.unwrap();
        
        let result = shell.execute_command("echo hello world").await.unwrap();
        match result {
            ShellObject::String(output) => {
                assert_eq!(output, "hello world");
            }
            _ => panic!("Expected string result for echo"),
        }
    }

    #[tokio::test]
    async fn test_command_parsing() {
        let config = Config::default();
        let shell = Shell::new(config).await.unwrap();
        
        let parts = shell.parse_command_line("echo \"hello world\"").unwrap();
        assert_eq!(parts, vec!["echo", "hello world"]);
    }
}