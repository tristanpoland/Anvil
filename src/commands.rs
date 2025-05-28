use crate::error::{AnvilError, AnvilResult};
use crate::objects::ShellObject;
use std::collections::HashMap;

pub type CommandFn = Box<dyn Fn(&[String]) -> AnvilResult<ShellObject> + Send + Sync>;

pub struct CommandRegistry {
    commands: HashMap<String, CommandInfo>,
}

pub struct CommandInfo {
    pub name: String,
    pub description: String,
    pub usage: String,
    pub handler: CommandFn,
}

impl CommandRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            commands: HashMap::new(),
        };
        
        registry.register_builtin_commands();
        registry
    }

    pub fn register_command(&mut self, info: CommandInfo) {
        self.commands.insert(info.name.clone(), info);
    }

    pub fn has_command(&self, name: &str) -> bool {
        self.commands.contains_key(name)
    }

    pub fn execute_command(&self, name: &str, args: &[String]) -> AnvilResult<ShellObject> {
        if let Some(cmd) = self.commands.get(name) {
            (cmd.handler)(args)
        } else {
            Err(AnvilError::command(format!("Unknown command: {}", name)))
        }
    }

    pub fn list_commands(&self) -> Vec<&CommandInfo> {
        self.commands.values().collect()
    }

    fn register_builtin_commands(&mut self) {
        // File system operations
        self.register_command(CommandInfo {
            name: "cat".to_string(),
            description: "Display file contents".to_string(),
            usage: "cat <file>".to_string(),
            handler: Box::new(|args| {
                if args.is_empty() {
                    return Err(AnvilError::command("cat: missing file argument"));
                }
                
                let content = std::fs::read_to_string(&args[0])
                    .map_err(|e| AnvilError::file_not_found(format!("cat: {}: {}", args[0], e)))?;
                Ok(ShellObject::String(content))
            }),
        });

        self.register_command(CommandInfo {
            name: "head".to_string(),
            description: "Display first lines of a file".to_string(),
            usage: "head [-n lines] <file>".to_string(),
            handler: Box::new(|args| {
                if args.is_empty() {
                    return Err(AnvilError::command("head: missing file argument"));
                }

                let mut lines = 10;
                let mut file_idx = 0;

                // Parse -n option
                if args.len() >= 3 && args[0] == "-n" {
                    lines = args[1].parse::<usize>()
                        .map_err(|_| AnvilError::command("head: invalid line count"))?;
                    file_idx = 2;
                } else if args.len() >= 1 {
                    file_idx = 0;
                }

                let content = std::fs::read_to_string(&args[file_idx])
                    .map_err(|e| AnvilError::file_not_found(format!("head: {}: {}", args[file_idx], e)))?;
                
                let output: Vec<&str> = content.lines().take(lines).collect();
                Ok(ShellObject::String(output.join("\n")))
            }),
        });

        self.register_command(CommandInfo {
            name: "tail".to_string(),
            description: "Display last lines of a file".to_string(),
            usage: "tail [-n lines] <file>".to_string(),
            handler: Box::new(|args| {
                if args.is_empty() {
                    return Err(AnvilError::command("tail: missing file argument"));
                }

                let mut lines = 10;
                let mut file_idx = 0;

                // Parse -n option
                if args.len() >= 3 && args[0] == "-n" {
                    lines = args[1].parse::<usize>()
                        .map_err(|_| AnvilError::command("tail: invalid line count"))?;
                    file_idx = 2;
                } else if args.len() >= 1 {
                    file_idx = 0;
                }

                let content = std::fs::read_to_string(&args[file_idx])
                    .map_err(|e| AnvilError::file_not_found(format!("tail: {}: {}", args[file_idx], e)))?;
                
                let all_lines: Vec<&str> = content.lines().collect();
                let start_idx = if all_lines.len() > lines { all_lines.len() - lines } else { 0 };
                let output: Vec<&str> = all_lines[start_idx..].to_vec();
                Ok(ShellObject::String(output.join("\n")))
            }),
        });

        self.register_command(CommandInfo {
            name: "wc".to_string(),
            description: "Count lines, words, and characters".to_string(),
            usage: "wc <file>".to_string(),
            handler: Box::new(|args| {
                if args.is_empty() {
                    return Err(AnvilError::command("wc: missing file argument"));
                }
                
                let content = std::fs::read_to_string(&args[0])
                    .map_err(|e| AnvilError::file_not_found(format!("wc: {}: {}", args[0], e)))?;
                
                let lines = content.lines().count();
                let words = content.split_whitespace().count();
                let chars = content.chars().count();
                
                let mut result = HashMap::new();
                result.insert("lines".to_string(), ShellObject::Integer(lines as i64));
                result.insert("words".to_string(), ShellObject::Integer(words as i64));
                result.insert("chars".to_string(), ShellObject::Integer(chars as i64));
                result.insert("file".to_string(), ShellObject::String(args[0].clone()));
                
                Ok(ShellObject::Map(result))
            }),
        });

        self.register_command(CommandInfo {
            name: "find".to_string(),
            description: "Find files matching criteria".to_string(),
            usage: "find <path> [-name pattern]".to_string(),
            handler: Box::new(|args| {
                if args.is_empty() {
                    return Err(AnvilError::command("find: missing path argument"));
                }

                let path = &args[0];
                let mut pattern = None;

                // Parse -name option
                if args.len() >= 3 && args[1] == "-name" {
                    pattern = Some(&args[2]);
                }

                let mut results = Vec::new();
                if let Ok(entries) = std::fs::read_dir(path) {
                    for entry in entries.flatten() {
                        let file_name = entry.file_name().to_string_lossy().to_string();
                        
                        if let Some(pat) = pattern {
                            if file_name.contains(pat) {
                                results.push(ShellObject::String(
                                    entry.path().to_string_lossy().to_string()
                                ));
                            }
                        } else {
                            results.push(ShellObject::String(
                                entry.path().to_string_lossy().to_string()
                            ));
                        }
                    }
                }

                Ok(ShellObject::Array(results))
            }),
        });

        // Text processing
        self.register_command(CommandInfo {
            name: "grep".to_string(),
            description: "Search for patterns in text".to_string(),
            usage: "grep <pattern> <file>".to_string(),
            handler: Box::new(|args| {
                if args.len() < 2 {
                    return Err(AnvilError::command("grep: missing pattern or file argument"));
                }

                let pattern = &args[0];
                let file = &args[1];
                
                let content = std::fs::read_to_string(file)
                    .map_err(|e| AnvilError::file_not_found(format!("grep: {}: {}", file, e)))?;
                
                let matching_lines: Vec<ShellObject> = content
                    .lines()
                    .filter(|line| line.contains(pattern))
                    .map(|line| ShellObject::String(line.to_string()))
                    .collect();

                Ok(ShellObject::Array(matching_lines))
            }),
        });

        self.register_command(CommandInfo {
            name: "sort".to_string(),
            description: "Sort lines of text".to_string(),
            usage: "sort <file>".to_string(),
            handler: Box::new(|args| {
                if args.is_empty() {
                    return Err(AnvilError::command("sort: missing file argument"));
                }
                
                let content = std::fs::read_to_string(&args[0])
                    .map_err(|e| AnvilError::file_not_found(format!("sort: {}: {}", args[0], e)))?;
                
                let mut lines: Vec<&str> = content.lines().collect();
                lines.sort();
                
                Ok(ShellObject::String(lines.join("\n")))
            }),
        });

        self.register_command(CommandInfo {
            name: "uniq".to_string(),
            description: "Remove duplicate lines".to_string(),
            usage: "uniq <file>".to_string(),
            handler: Box::new(|args| {
                if args.is_empty() {
                    return Err(AnvilError::command("uniq: missing file argument"));
                }
                
                let content = std::fs::read_to_string(&args[0])
                    .map_err(|e| AnvilError::file_not_found(format!("uniq: {}: {}", args[0], e)))?;
                
                let mut unique_lines = Vec::new();
                let mut last_line = "";
                
                for line in content.lines() {
                    if line != last_line {
                        unique_lines.push(line);
                        last_line = line;
                    }
                }
                
                Ok(ShellObject::String(unique_lines.join("\n")))
            }),
        });

        // System information
        self.register_command(CommandInfo {
            name: "ps".to_string(),
            description: "List running processes".to_string(),
            usage: "ps".to_string(),
            handler: Box::new(|_args| {
                // This is a simplified implementation
                // In a real implementation, you'd use system APIs to get process info
                let mut processes = Vec::new();
                
                // Add a dummy process for demonstration
                let mut proc = HashMap::new();
                proc.insert("pid".to_string(), ShellObject::Integer(std::process::id() as i64));
                proc.insert("name".to_string(), ShellObject::String("anvil".to_string()));
                proc.insert("status".to_string(), ShellObject::String("running".to_string()));
                processes.push(ShellObject::Map(proc));
                
                Ok(ShellObject::Array(processes))
            }),
        });

        self.register_command(CommandInfo {
            name: "df".to_string(),
            description: "Display filesystem disk usage".to_string(),
            usage: "df".to_string(),
            handler: Box::new(|_args| {
                // Simplified implementation
                let mut filesystems = Vec::new();
                
                if let Ok(metadata) = std::fs::metadata(".") {
                    let mut fs = HashMap::new();
                    fs.insert("filesystem".to_string(), ShellObject::String("/".to_string()));
                    fs.insert("type".to_string(), ShellObject::String("ext4".to_string()));
                    fs.insert("available".to_string(), ShellObject::Integer(metadata.len() as i64));
                    filesystems.push(ShellObject::Map(fs));
                }
                
                Ok(ShellObject::Array(filesystems))
            }),
        });

        // Network utilities (basic)
        self.register_command(CommandInfo {
            name: "ping".to_string(),
            description: "Ping a network host".to_string(),
            usage: "ping <host>".to_string(),
            handler: Box::new(|args| {
                if args.is_empty() {
                    return Err(AnvilError::command("ping: missing host argument"));
                }
                
                // Use system ping command
                let output = std::process::Command::new("ping")
                    .arg("-c")
                    .arg("4")
                    .arg(&args[0])
                    .output()
                    .map_err(|e| AnvilError::command(format!("ping: {}", e)))?;
                
                if output.status.success() {
                    Ok(ShellObject::String(String::from_utf8_lossy(&output.stdout).to_string()))
                } else {
                    Ok(ShellObject::String(String::from_utf8_lossy(&output.stderr).to_string()))
                }
            }),
        });

        // Help command
        self.register_command(CommandInfo {
            name: "help".to_string(),
            description: "Show help for built-in commands".to_string(),
            usage: "help [command]".to_string(),
            handler: Box::new(move |args| {
                if args.is_empty() {
                    // List all commands
                    let mut help_text = String::from("Available built-in commands:\n\n");
                    
                    let command_names = [
                        "cat", "head", "tail", "wc", "find", "grep", "sort", "uniq",
                        "ps", "df", "ping", "help"
                    ];
                    
                    for cmd in &command_names {
                        help_text.push_str(&format!("  {}\n", cmd));
                    }
                    
                    help_text.push_str("\nUse 'help <command>' for specific usage information.\n");
                    Ok(ShellObject::String(help_text))
                } else {
                    // Show help for specific command
                    let cmd_name = &args[0];
                    // In a real implementation, we'd look up the command info
                    Ok(ShellObject::String(format!("Help for command: {}", cmd_name)))
                }
            }),
        });
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_registry() {
        let registry = CommandRegistry::new();
        assert!(registry.has_command("cat"));
        assert!(registry.has_command("grep"));
        assert!(!registry.has_command("nonexistent"));
    }

    #[test]
    fn test_help_command() {
        let registry = CommandRegistry::new();
        let result = registry.execute_command("help", &[]).unwrap();
        
        match result {
            ShellObject::String(help_text) => {
                assert!(help_text.contains("Available built-in commands"));
            }
            _ => panic!("Expected string result for help"),
        }
    }
}