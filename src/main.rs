mod api;
mod cli;
mod notify;
mod state;

use clap::Parser;
use cli::{Cli, Command};
use state::{AppState, detect_transition};

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::Login { email, password } => {
            println!("Login: {email}");
            drop(password);
        }
        Command::Check => {
            let _state: Option<AppState> = None;
            let _transition = detect_transition(None, state::PresenceStatus::CheckedIn);
            let _ = _transition;
            if let Some(s) = _state {
                let _ = s.effective_last_status(chrono::Local::now().date_naive());
            }
            println!("Check");
        }
    }
}
