pub mod config;
pub mod error;
pub mod eval;
pub mod objects;
pub mod repl;
pub mod shell;
pub mod commands;
pub mod utils;

pub use error::{AnvilError, AnvilResult};
pub use shell::Shell;
pub use objects::ShellObject;
pub use repl::ReplEngine;

/// Anvil version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Default configuration file name
pub const CONFIG_FILE: &str = "anvil.toml";

/// Default history file name  
pub const HISTORY_FILE: &str = "anvil_history.txt";

/// Maximum history entries to keep
pub const MAX_HISTORY_ENTRIES: usize = 10000;

/// Anvil prompt prefix
pub const PROMPT_PREFIX: &str = "anvil";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }
}