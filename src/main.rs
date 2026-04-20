mod api;
mod cli;
mod notify;
mod state;

use clap::Parser;
use cli::{Cli, Command};
use snafu::{ResultExt, Snafu};
use state::PresenceStatus;

#[derive(Debug, Snafu)]
enum Error {
    #[snafu(display("{source}"))]
    Api { source: api::Error },

    #[snafu(display("{source}"))]
    State { source: state::Error },
}

type Result<T, E = Error> = std::result::Result<T, E>;

fn main() {
    let cli = Cli::parse();
    let exit_code = match cli.command {
        Command::Login { email, password } => run_login(&email, &password),
        Command::Check => run_check(),
    };
    std::process::exit(exit_code);
}

fn run_login(email: &str, password: &str) -> i32 {
    match do_login(email, password) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("Error: {e}");
            let _ = notify::send(
                "HortProChecker Error",
                &e.to_string(),
                notify::Urgency::Critical,
            );
            2
        }
    }
}

fn run_check() -> i32 {
    match do_check() {
        Ok(PresenceStatus::CheckedIn) => 0,
        Ok(PresenceStatus::CheckedOut) => 1,
        Err(e) => {
            eprintln!("Error: {e}");
            let _ = notify::send(
                "HortProChecker Error",
                &e.to_string(),
                notify::Urgency::Critical,
            );
            2
        }
    }
}

fn do_login(email: &str, password: &str) -> Result<()> {
    let client = api::build_client().context(ApiSnafu)?;
    let (session_cookie, _login_data) = api::login(&client, email, password).context(ApiSnafu)?;
    let kid = api::fetch_first_kid(&client, &session_cookie).context(ApiSnafu)?;

    let group_display = kid.kid_group.as_deref().unwrap_or("unknown group");
    println!("Logged in. Child: {} ({})", kid.firstname, group_display);

    let app_state = state::AppState {
        session_cookie,
        kid_id: kid.id,
        kid_name: kid.firstname,
        last_status: None,
        last_status_date: None,
        last_check_at: None,
    };

    let path = state::default_state_path().context(StateSnafu)?;
    state::save_state(&path, &app_state).context(StateSnafu)?;
    println!("State saved to {}", path.display());

    Ok(())
}

fn do_check() -> Result<PresenceStatus> {
    let path = state::default_state_path().context(StateSnafu)?;
    let mut app_state = state::load_state(&path).context(StateSnafu)?;

    let client = api::build_client().context(ApiSnafu)?;
    let records = api::fetch_presences(&client, &app_state.session_cookie, &app_state.kid_id)
        .context(ApiSnafu)?;

    let today = chrono::Local::now().date_naive();
    let current_status = api::determine_status(&records, today).context(ApiSnafu)?;
    let effective_previous = app_state.effective_last_status(today);
    let transition = state::detect_transition(effective_previous, current_status);

    let name = &app_state.kid_name;
    match transition {
        state::Transition::Arrived => {
            let _ = notify::send(
                "HortProChecker",
                &format!("{name} arrived at daycare"),
                notify::Urgency::Normal,
            );
        }
        state::Transition::Left => {
            let _ = notify::send(
                "HortProChecker",
                &format!("{name} left daycare"),
                notify::Urgency::Normal,
            );
        }
        state::Transition::None => {}
    }

    app_state.last_status = Some(current_status);
    app_state.last_status_date = Some(today);
    app_state.last_check_at = Some(chrono::Local::now().fixed_offset());
    state::save_state(&path, &app_state).context(StateSnafu)?;

    Ok(current_status)
}
