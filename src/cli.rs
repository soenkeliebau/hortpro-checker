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
    /// Authenticate and store session
    Login {
        /// Account email address
        #[arg(long)]
        email: String,
        /// Account password
        #[arg(long)]
        password: String,
    },
    /// Check current attendance status
    Check,
}
