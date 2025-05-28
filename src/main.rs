use anvil::{shell::Shell, config::Config, error::AnvilResult};
use clap::{Parser, Subcommand};
use log::info;
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(name = "anvil")]
#[command(about = "A type-safe Rust shell and REPL")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Configuration file path
    #[arg(long)]
    config: Option<PathBuf>,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Run a single command and exit
    #[arg(short = 'c', long)]
    command_string: Option<String>,

    /// Execute a script file
    #[arg(short, long)]
    script: Option<PathBuf>,

    /// Start in REPL mode (default)
    #[arg(long)]
    repl: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize anvil configuration
    Init {
        /// Force overwrite existing config
        #[arg(long)]
        force: bool,
    },
    /// Check anvil installation and configuration
    Doctor,
    /// Show configuration information
    Config,
    /// Clear shell history
    ClearHistory,
}

#[tokio::main]
async fn main() -> AnvilResult<()> {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = if cli.verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level))
        .init();

    info!("Starting Anvil shell v{}", env!("CARGO_PKG_VERSION"));

    // Load configuration
    let config = Config::load(cli.config.as_deref()).await?;

    // Handle subcommands
    if let Some(command) = cli.command {
        return handle_command(command, &config).await;
    }

    // Create shell instance
    let mut shell = Shell::new(config).await?;

    // Handle different execution modes
    match (cli.command_string, cli.script, cli.repl) {
        (Some(cmd), None, false) => {
            // Execute single command
            shell.execute_command(&cmd).await?;
        }
        (None, Some(script_path), false) => {
            // Execute script file
            shell.execute_script(&script_path).await?;
        }
        _ => {
            // Start interactive REPL (default)
            shell.run_repl().await?;
        }
    }

    Ok(())
}

async fn handle_command(command: Commands, config: &Config) -> AnvilResult<()> {
    match command {
        Commands::Init { force } => {
            config.init(force).await?;
            println!("✓ Anvil configuration initialized");
        }
        Commands::Doctor => {
            config.doctor().await?;
        }
        Commands::Config => {
            println!("{}", serde_json::to_string_pretty(config)?);
        }
        Commands::ClearHistory => {
            config.clear_history().await?;
            println!("✓ Shell history cleared");
        }
    }
    Ok(())
}