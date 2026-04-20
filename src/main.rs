mod api;
mod cli;
mod notify;
mod state;

use clap::Parser;
use cli::{Cli, Command};

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::Login { email, password } => {
            println!("Login: {email}");
            drop(password);
        }
        Command::Check => {
            println!("Check");
        }
    }
}
