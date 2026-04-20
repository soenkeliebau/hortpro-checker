mod api;
mod cli;
mod notify;
mod state;

use clap::Parser;
use cli::{Cli, Command};
use notify::Urgency;
use state::{default_state_path, detect_transition, load_state, save_state};

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::Login { email, password } => {
            let client = match api::build_client() {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("error: {e}");
                    return;
                }
            };
            match api::login(&client, &email, &password) {
                Ok((_, data)) => println!("Logged in as {} {}", data.firstname, data.lastname),
                Err(e) => eprintln!("error: {e}"),
            }
        }
        Command::Check => {
            let path = match default_state_path() {
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

            let client = match api::build_client() {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("error: {e}");
                    return;
                }
            };

            let session = app_state
                .as_ref()
                .map(|s| s.session_cookie.as_str())
                .unwrap_or("");

            let kid = match api::fetch_first_kid(&client, session) {
                Ok(k) => k,
                Err(e) => {
                    eprintln!("error: {e}");
                    return;
                }
            };

            let group = kid.kid_group.as_deref().unwrap_or("unknown");
            println!("Checking {} (group: {group})", kid.firstname);

            let records = match api::fetch_presences(&client, session, &kid.id) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("error: {e}");
                    return;
                }
            };

            let status = match api::determine_status(&records, chrono::Local::now().date_naive()) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("error: {e}");
                    return;
                }
            };

            let transition = detect_transition(effective, status);

            if let Some(mut s) = app_state {
                s.last_status = Some(status);
                s.last_status_date = Some(chrono::Local::now().date_naive());
                let _ = save_state(&path, &s);
            }

            let message = format!("transition: {transition:?}");
            let _ = notify::send("HortProChecker", &message, Urgency::Normal);
            println!("Check complete (status: {status:?})");
        }
    }
}
