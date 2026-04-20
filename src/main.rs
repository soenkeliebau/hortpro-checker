mod api;
mod cli;
mod notify;
mod state;

use std::io::Write;

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

    #[snafu(display("failed to read email from terminal"))]
    ReadEmail { source: std::io::Error },

    #[snafu(display("failed to read password from terminal"))]
    ReadPassword { source: std::io::Error },
}

type Result<T, E = Error> = std::result::Result<T, E>;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    let exit_code = match cli.command {
        Command::Login { email, password } => run_login(email, password),
        Command::Check => run_check(),
    };
    std::process::exit(exit_code);
}

fn prompt_email() -> Result<String> {
    print!("Email: ");
    std::io::stdout().flush().context(ReadEmailSnafu)?;
    let mut email = String::new();
    std::io::stdin()
        .read_line(&mut email)
        .context(ReadEmailSnafu)?;
    Ok(email.trim().to_string())
}

fn prompt_password() -> Result<String> {
    rpassword::prompt_password("Password: ").context(ReadPasswordSnafu)
}

fn icon_path() -> Option<std::path::PathBuf> {
    state::default_photo_path().ok().filter(|p| p.exists())
}

fn run_login(email: Option<String>, password: Option<String>) -> i32 {
    let result = (|| -> Result<()> {
        let email = match email {
            Some(e) => e,
            None => prompt_email()?,
        };
        let password = match password {
            Some(p) => p,
            None => prompt_password()?,
        };
        do_login(&email, &password)
    })();

    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("Error: {e}");
            let _ = notify::send(
                "HortProChecker Error",
                &e.to_string(),
                notify::Urgency::Critical,
                None,
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
            let icon = icon_path();
            let _ = notify::send(
                "HortProChecker Error",
                &e.to_string(),
                notify::Urgency::Critical,
                icon.as_deref(),
            );
            2
        }
    }
}

fn do_login(email: &str, password: &str) -> Result<()> {
    let (client, jar) = api::build_client().context(ApiSnafu)?;
    let login_data = api::login(&client, email, password).context(ApiSnafu)?;
    let session_cookie = api::extract_cookies(&jar).context(ApiSnafu)?;
    let kid = api::fetch_first_kid(&client).context(ApiSnafu)?;

    let group_display = kid.kid_group.as_deref().unwrap_or("unknown group");
    println!(
        "Logged in as {} {}. Child: {} ({})",
        login_data.firstname, login_data.lastname, kid.firstname, group_display
    );

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

    if let Some(data_uri) = &kid.picture {
        let photo_path = state::default_photo_path().context(StateSnafu)?;
        state::save_photo(data_uri, &photo_path).context(StateSnafu)?;
        println!("Photo saved to {}", photo_path.display());
    }

    Ok(())
}

fn print_status(name: &str, status: PresenceStatus, record: Option<&api::PresenceRecord>) {
    match (status, record) {
        (PresenceStatus::CheckedIn, Some(r)) => {
            let time = r
                .date_start
                .split('T')
                .nth(1)
                .and_then(|t| t.split('+').next());
            match time {
                Some(t) => println!("{name}: checked in since {t}"),
                None => println!("{name}: checked in"),
            }
        }
        (PresenceStatus::CheckedOut, Some(r)) if r.date_end.is_some() => {
            let start = r
                .date_start
                .split('T')
                .nth(1)
                .and_then(|t| t.split('+').next());
            let end = r
                .date_end
                .as_deref()
                .and_then(|s| s.split('T').nth(1))
                .and_then(|t| t.split('+').next());
            match (start, end) {
                (Some(s), Some(e)) => println!("{name}: checked out ({s} - {e})"),
                _ => println!("{name}: checked out"),
            }
        }
        _ => println!("{name}: not at daycare"),
    }
}

fn do_check() -> Result<PresenceStatus> {
    let path = state::default_state_path().context(StateSnafu)?;
    let mut app_state = state::load_state(&path).context(StateSnafu)?;

    let client = api::build_authenticated_client(&app_state.session_cookie).context(ApiSnafu)?;
    let records = api::fetch_presences(&client, &app_state.kid_id).context(ApiSnafu)?;

    let today = chrono::Local::now().date_naive();
    let current_status = api::determine_status(&records, today).context(ApiSnafu)?;
    let effective_previous = app_state.effective_last_status(today);
    let transition = state::detect_transition(effective_previous, current_status);

    let name = &app_state.kid_name;
    let icon = icon_path();
    print_status(name, current_status, records.first());
    match transition {
        state::Transition::Arrived => {
            let _ = notify::send(
                "HortProChecker",
                &format!("{name} arrived at daycare"),
                notify::Urgency::Normal,
                icon.as_deref(),
            );
        }
        state::Transition::Left => {
            let _ = notify::send(
                "HortProChecker",
                &format!("{name} left daycare"),
                notify::Urgency::Normal,
                icon.as_deref(),
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
