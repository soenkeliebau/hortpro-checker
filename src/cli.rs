use clap::{Parser, Subcommand};

/// Check daycare attendance status via the HortPro Elternportal
#[derive(Parser)]
#[command(name = "hortpro-checker")]
pub struct Cli {
    /// The subcommand to execute
    #[command(subcommand)]
    pub command: Command,
}

/// Available subcommands for the HortPro checker
#[derive(Subcommand)]
pub enum Command {
    /// Authenticate and store session (prompts interactively if not provided)
    Login {
        /// Account email address
        #[arg(long)]
        email: Option<String>,
        /// Account password
        #[arg(long)]
        password: Option<String>,
    },
    /// Check current attendance status
    Check,
}
