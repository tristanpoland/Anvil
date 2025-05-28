use crate::error::{AnvilError, AnvilResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub shell: ShellConfig,
    pub repl: ReplConfig,
    pub environment: EnvironmentConfig,
    pub aliases: HashMap<String, String>,
    pub functions: HashMap<String, String>,
    pub keybindings: HashMap<String, String>,
    pub paths: PathsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellConfig {
    pub prompt: String,
    pub continuation_prompt: String,
    pub history_file: PathBuf,
    pub max_history_size: usize,
    pub auto_cd: bool,
    pub case_sensitive: bool,
    pub tab_completion: bool,
    pub syntax_highlighting: bool,
    pub auto_suggestions: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplConfig {
    pub auto_print: bool,
    pub multiline_mode: bool,
    pub indent_size: usize,
    pub compile_timeout_ms: u64,
    pub execution_timeout_ms: u64,
    pub enable_unsafe: bool,
    pub prelude: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentConfig {
    pub inherit_system_env: bool,
    pub default_vars: HashMap<String, String>,
    pub path_separator: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathsConfig {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub temp_dir: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("anvil");
        
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("anvil");

        Self {
            shell: ShellConfig {
                prompt: "anvil> ".to_string(),
                continuation_prompt: "    > ".to_string(),
                history_file: data_dir.join("history.txt"),
                max_history_size: 10000,
                auto_cd: true,
                case_sensitive: false,
                tab_completion: true,
                syntax_highlighting: true,
                auto_suggestions: true,
            },
            repl: ReplConfig {
                auto_print: true,
                multiline_mode: true,
                indent_size: 4,
                compile_timeout_ms: 5000,
                execution_timeout_ms: 30000,
                enable_unsafe: false,
                prelude: vec![
                    "use std::collections::HashMap;".to_string(),
                    "use std::path::PathBuf;".to_string(),
                    "use std::fs;".to_string(),
                    "use std::process::Command;".to_string(),
                ],
            },
            environment: EnvironmentConfig {
                inherit_system_env: true,
                default_vars: HashMap::new(),
                path_separator: if cfg!(windows) { ";" } else { ":" }.to_string(),
            },
            aliases: create_default_aliases(),
            functions: HashMap::new(),
            keybindings: create_default_keybindings(),
            paths: PathsConfig {
                config_dir: config_dir.clone(),
                data_dir: data_dir.clone(),
                cache_dir: dirs::cache_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("anvil"),
                temp_dir: std::env::temp_dir().join("anvil"),
            },
        }
    }
}

impl Config {
    /// Load configuration from file or create default
    pub async fn load(config_path: Option<&Path>) -> AnvilResult<Self> {
        let config_file = if let Some(path) = config_path {
            path.to_path_buf()
        } else {
            let config_dir = dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("anvil");
            config_dir.join("config.toml")
        };

        if config_file.exists() {
            let content = fs::read_to_string(&config_file).await?;
            let config: Config = toml::from_str(&content)
                .map_err(|e| AnvilError::config(format!("Failed to parse config: {}", e)))?;
            Ok(config)
        } else {
            let config = Config::default();
            config.ensure_directories().await?;
            Ok(config)
        }
    }

    /// Save configuration to file
    pub async fn save(&self, config_path: Option<&Path>) -> AnvilResult<()> {
        let config_file = if let Some(path) = config_path {
            path.to_path_buf()
        } else {
            self.paths.config_dir.join("config.toml")
        };

        self.ensure_directories().await?;
        
        let content = toml::to_string_pretty(self)
            .map_err(|e| AnvilError::config(format!("Failed to serialize config: {}", e)))?;
        
        fs::write(config_file, content).await?;
        Ok(())
    }

    /// Initialize configuration (create default config file)
    pub async fn init(&self, force: bool) -> AnvilResult<()> {
        let config_file = self.paths.config_dir.join("config.toml");
        
        if config_file.exists() && !force {
            return Err(AnvilError::config(
                "Configuration file already exists. Use --force to overwrite."
            ));
        }

        self.save(None).await?;
        println!("✓ Configuration initialized at {}", config_file.display());
        Ok(())
    }

    /// Check configuration and system setup
    pub async fn doctor(&self) -> AnvilResult<()> {
        println!("🔧 Anvil Configuration Check");
        println!();

        // Check directories
        self.check_directory("Config", &self.paths.config_dir).await?;
        self.check_directory("Data", &self.paths.data_dir).await?;
        self.check_directory("Cache", &self.paths.cache_dir).await?;
        self.check_directory("Temp", &self.paths.temp_dir).await?;

        // Check history file
        if self.shell.history_file.exists() {
            let metadata = fs::metadata(&self.shell.history_file).await?;
            println!("✓ History file: {} ({} bytes)", 
                self.shell.history_file.display(), metadata.len());
        } else {
            println!("⚠ History file: {} (not found)", self.shell.history_file.display());
        }

        // Check Rust installation
        match which::which("rustc") {
            Ok(rustc_path) => {
                println!("✓ Rust compiler: {}", rustc_path.display());
                
                // Get Rust version
                let output = std::process::Command::new("rustc")
                    .arg("--version")
                    .output()
                    .map_err(|e| AnvilError::command(format!("Failed to get Rust version: {}", e)))?;
                
                if output.status.success() {
                    let version = String::from_utf8_lossy(&output.stdout);
                    println!("  Version: {}", version.trim());
                }
            }
            Err(_) => {
                println!("✗ Rust compiler: not found in PATH");
                println!("  Install Rust from https://rustup.rs/");
            }
        }

        // Check cargo
        match which::which("cargo") {
            Ok(cargo_path) => println!("✓ Cargo: {}", cargo_path.display()),
            Err(_) => println!("✗ Cargo: not found in PATH"),
        }

        // Check aliases
        println!("📝 Aliases: {} configured", self.aliases.len());
        
        // Check functions
        println!("🔧 Functions: {} configured", self.functions.len());

        println!();
        println!("✓ Configuration check complete");
        Ok(())
    }

    /// Clear shell history
    pub async fn clear_history(&self) -> AnvilResult<()> {
        if self.shell.history_file.exists() {
            fs::remove_file(&self.shell.history_file).await?;
        }
        Ok(())
    }

    /// Ensure all necessary directories exist
    async fn ensure_directories(&self) -> AnvilResult<()> {
        fs::create_dir_all(&self.paths.config_dir).await?;
        fs::create_dir_all(&self.paths.data_dir).await?;
        fs::create_dir_all(&self.paths.cache_dir).await?;
        fs::create_dir_all(&self.paths.temp_dir).await?;
        
        // Ensure history file directory exists
        if let Some(parent) = self.shell.history_file.parent() {
            fs::create_dir_all(parent).await?;
        }
        
        Ok(())
    }

    async fn check_directory(&self, name: &str, path: &Path) -> AnvilResult<()> {
        if path.exists() {
            let metadata = fs::metadata(path).await?;
            if metadata.is_dir() {
                println!("✓ {} directory: {}", name, path.display());
            } else {
                println!("✗ {} directory: {} (not a directory)", name, path.display());
            }
        } else {
            println!("⚠ {} directory: {} (will be created)", name, path.display());
        }
        Ok(())
    }

    /// Get the full path to a file in the config directory
    pub fn config_file(&self, filename: &str) -> PathBuf {
        self.paths.config_dir.join(filename)
    }

    /// Get the full path to a file in the data directory
    pub fn data_file(&self, filename: &str) -> PathBuf {
        self.paths.data_dir.join(filename)
    }

    /// Get the full path to a file in the cache directory
    pub fn cache_file(&self, filename: &str) -> PathBuf {
        self.paths.cache_dir.join(filename)
    }

    /// Get the full path to a file in the temp directory
    pub fn temp_file(&self, filename: &str) -> PathBuf {
        self.paths.temp_dir.join(filename)
    }
}

fn create_default_aliases() -> HashMap<String, String> {
    let mut aliases = HashMap::new();
    
    // Common shell aliases
    aliases.insert("ls".to_string(), "std::fs::read_dir(\".\").unwrap().collect::<Result<Vec<_>, _>>().unwrap()".to_string());
    aliases.insert("pwd".to_string(), "std::env::current_dir().unwrap()".to_string());
    aliases.insert("cd".to_string(), "std::env::set_current_dir".to_string());
    aliases.insert("cat".to_string(), "std::fs::read_to_string".to_string());
    aliases.insert("echo".to_string(), "println!".to_string());
    
    // Rust-specific aliases
    aliases.insert("rustc".to_string(), "std::process::Command::new(\"rustc\")".to_string());
    aliases.insert("cargo".to_string(), "std::process::Command::new(\"cargo\")".to_string());
    
    aliases
}

fn create_default_keybindings() -> HashMap<String, String> {
    let mut bindings = HashMap::new();
    
    // Emacs-style bindings (default for most shells)
    bindings.insert("Ctrl+A".to_string(), "move_to_line_start".to_string());
    bindings.insert("Ctrl+E".to_string(), "move_to_line_end".to_string());
    bindings.insert("Ctrl+L".to_string(), "clear_screen".to_string());
    bindings.insert("Ctrl+C".to_string(), "interrupt".to_string());
    bindings.insert("Ctrl+D".to_string(), "exit".to_string());
    bindings.insert("Tab".to_string(), "complete".to_string());
    bindings.insert("Ctrl+R".to_string(), "reverse_search".to_string());
    
    bindings
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_config_creation() {
        let config = Config::default();
        assert_eq!(config.shell.prompt, "anvil> ");
        assert!(!config.aliases.is_empty());
    }

    #[tokio::test]
    async fn test_config_save_load() {
        let temp_dir = tempdir().unwrap();
        let config_file = temp_dir.path().join("config.toml");
        
        let config = Config::default();
        config.save(Some(&config_file)).await.unwrap();
        
        let loaded_config = Config::load(Some(&config_file)).await.unwrap();
        assert_eq!(config.shell.prompt, loaded_config.shell.prompt);
    }
}