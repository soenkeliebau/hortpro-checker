mod api;
mod cli;
mod notify;
mod state;

use clap::Parser;
use cli::{Cli, Command};
use notify::Urgency;
use state::{detect_transition, load_state, save_state};

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::Login { email, password } => {
            println!("Login: {email}");
            drop(password);
        }
        Command::Check => {
            let path = match state::default_state_path() {
                Ok(p) => p,
                Err(e) => {
                    let _ = notify::send("HortProChecker", &e.to_string(), Urgency::Critical);
                    eprintln!("error: {e}");
                    return;
                }
            };
            let app_state = load_state(&path).ok();
            let effective = app_state
                .as_ref()
                .and_then(|s| s.effective_last_status(chrono::Local::now().date_naive()));
            let _transition = detect_transition(effective, state::PresenceStatus::CheckedIn);
            if let Some(s) = app_state {
                let _ = save_state(&path, &s);
            }
            let _ = notify::send("HortProChecker", "Check complete", Urgency::Normal);
            println!("Check");
        }
    }
}
