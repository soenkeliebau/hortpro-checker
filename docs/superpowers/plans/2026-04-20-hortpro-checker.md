# HortProChecker Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a CLI tool that logs into the HortPro Elternportal, checks daycare attendance, fires desktop notifications on status transitions, and exits with status-indicating exit codes.

**Architecture:** Single binary with two subcommands (`login`, `check`). State persisted in a JSON file at `~/.local/state/hortpro/state.json`. Day-boundary-aware transition detection prevents spurious overnight notifications. `notify-send` for desktop notifications.

**Tech Stack:** Rust (edition 2024), clap (derive), reqwest (blocking), serde/serde_json, snafu, chrono, dirs

---

## File Structure

| File | Responsibility |
|------|---------------|
| `src/main.rs` | Entry point, clap dispatch, exit code mapping, error→notification bridging |
| `src/cli.rs` | Clap `Parser` and `Subcommand` derive structs |
| `src/state.rs` | `AppState` struct, `PresenceStatus`/`Transition` enums, transition detection, day-boundary logic, state file I/O |
| `src/api.rs` | HTTP calls (login, fetch kids, fetch presences), response types, `determine_status` parser |
| `src/notify.rs` | `notify-send` command builder and executor |

Each module defines its own `Error` enum via snafu. No global error type.

---

### Task 1: Project Scaffolding

**Files:**
- Modify: `Cargo.toml`
- Create: `src/cli.rs`, `src/state.rs`, `src/api.rs`, `src/notify.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Add dependencies**

```bash
cargo add clap --features derive
cargo add reqwest --features "blocking,json,cookies"
cargo add serde --features derive
cargo add serde_json
cargo add snafu
cargo add chrono --features serde
cargo add dirs
cargo add --dev tempfile
```

- [ ] **Step 2: Rename package for CLI convention**

In `Cargo.toml`, change the package name:

```toml
[package]
name = "hortpro-checker"
version = "0.1.0"
edition = "2024"
```

- [ ] **Step 3: Create empty module files**

Create `src/cli.rs`:
```rust
```

Create `src/state.rs`:
```rust
```

Create `src/api.rs`:
```rust
```

Create `src/notify.rs`:
```rust
```

- [ ] **Step 4: Wire modules in main.rs**

Replace `src/main.rs` with:

```rust
mod api;
mod cli;
mod notify;
mod state;

fn main() {
    println!("Hello, world!");
}
```

- [ ] **Step 5: Verify it compiles**

```bash
cargo build
```

Expected: compiles with warnings about unused modules (acceptable at this stage).

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock src/main.rs src/cli.rs src/state.rs src/api.rs src/notify.rs
git commit -m "scaffold project with dependencies and empty modules"
```

---

### Task 2: CLI Module

**Files:**
- Modify: `src/cli.rs`

- [ ] **Step 1: Define CLI structs**

Write `src/cli.rs`:

```rust
use clap::{Parser, Subcommand};

/// Check daycare attendance status via the HortPro Elternportal
#[derive(Parser)]
#[command(name = "hortpro-checker")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

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
```

- [ ] **Step 2: Use Cli in main.rs to verify parsing works**

Replace `src/main.rs`:

```rust
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
```

- [ ] **Step 3: Verify it compiles and --help works**

```bash
cargo run -- --help
```

Expected: prints help text with `login` and `check` subcommands.

- [ ] **Step 4: Run quality gate**

```bash
cargo fmt && cargo clippy -- -D warnings
```

Fix any issues.

- [ ] **Step 5: Commit**

```bash
git add src/cli.rs src/main.rs
git commit -m "add CLI argument parsing with login and check subcommands"
```

---

### Task 3: State Types and Transition Detection

**Files:**
- Modify: `src/state.rs`
- Test: `src/state.rs` (inline `#[cfg(test)] mod tests`)

- [ ] **Step 1: Write failing tests for transition detection**

Write `src/state.rs`:

```rust
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresenceStatus {
    CheckedIn,
    CheckedOut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transition {
    Arrived,
    Left,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppState {
    pub session_cookie: String,
    pub kid_id: String,
    pub kid_name: String,
    pub last_status: Option<PresenceStatus>,
    pub last_status_date: Option<NaiveDate>,
    pub last_check_at: Option<chrono::DateTime<chrono::FixedOffset>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        match NaiveDate::from_ymd_opt(y, m, d) {
            Some(d) => d,
            ::core::option::Option::None => panic!("invalid date: {y}-{m}-{d}"),
        }
    }

    #[test]
    fn test_unknown_to_checked_in_is_arrived() {
        assert_eq!(
            detect_transition(::core::option::Option::None, PresenceStatus::CheckedIn),
            Transition::Arrived,
        );
    }

    #[test]
    fn test_unknown_to_checked_out_is_no_transition() {
        assert_eq!(
            detect_transition(::core::option::Option::None, PresenceStatus::CheckedOut),
            Transition::None,
        );
    }

    #[test]
    fn test_checked_in_to_checked_out_is_left() {
        assert_eq!(
            detect_transition(Some(PresenceStatus::CheckedIn), PresenceStatus::CheckedOut),
            Transition::Left,
        );
    }

    #[test]
    fn test_checked_out_to_checked_in_is_arrived() {
        assert_eq!(
            detect_transition(Some(PresenceStatus::CheckedOut), PresenceStatus::CheckedIn),
            Transition::Arrived,
        );
    }

    #[test]
    fn test_same_status_is_no_transition() {
        assert_eq!(
            detect_transition(Some(PresenceStatus::CheckedIn), PresenceStatus::CheckedIn),
            Transition::None,
        );
        assert_eq!(
            detect_transition(Some(PresenceStatus::CheckedOut), PresenceStatus::CheckedOut),
            Transition::None,
        );
    }

    #[test]
    fn test_effective_status_same_day() {
        let today = date(2026, 4, 20);
        let state = AppState {
            session_cookie: String::new(),
            kid_id: String::new(),
            kid_name: String::new(),
            last_status: Some(PresenceStatus::CheckedIn),
            last_status_date: Some(today),
            last_check_at: ::core::option::Option::None,
        };
        assert_eq!(state.effective_last_status(today), Some(PresenceStatus::CheckedIn));
    }

    #[test]
    fn test_effective_status_different_day_resets_to_none() {
        let yesterday = date(2026, 4, 19);
        let today = date(2026, 4, 20);
        let state = AppState {
            session_cookie: String::new(),
            kid_id: String::new(),
            kid_name: String::new(),
            last_status: Some(PresenceStatus::CheckedIn),
            last_status_date: Some(yesterday),
            last_check_at: ::core::option::Option::None,
        };
        assert_eq!(state.effective_last_status(today), ::core::option::Option::None);
    }

    #[test]
    fn test_effective_status_no_date_resets_to_none() {
        let today = date(2026, 4, 20);
        let state = AppState {
            session_cookie: String::new(),
            kid_id: String::new(),
            kid_name: String::new(),
            last_status: Some(PresenceStatus::CheckedIn),
            last_status_date: ::core::option::Option::None,
            last_check_at: ::core::option::Option::None,
        };
        assert_eq!(state.effective_last_status(today), ::core::option::Option::None);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test --lib state
```

Expected: compilation errors — `detect_transition` and `effective_last_status` don't exist yet.

- [ ] **Step 3: Implement transition detection and effective status**

Add these functions and the impl block to `src/state.rs`, above the `#[cfg(test)]` block:

```rust
impl AppState {
    /// Returns the last status if it was recorded today, otherwise None.
    pub fn effective_last_status(&self, today: NaiveDate) -> Option<PresenceStatus> {
        match self.last_status_date {
            Some(date) if date == today => self.last_status,
            _ => ::core::option::Option::None,
        }
    }
}

/// Determines the notification transition between two statuses.
#[must_use]
pub fn detect_transition(
    effective_previous: Option<PresenceStatus>,
    current: PresenceStatus,
) -> Transition {
    match (effective_previous, current) {
        (::core::option::Option::None, PresenceStatus::CheckedIn) => Transition::Arrived,
        (Some(PresenceStatus::CheckedOut), PresenceStatus::CheckedIn) => Transition::Arrived,
        (Some(PresenceStatus::CheckedIn), PresenceStatus::CheckedOut) => Transition::Left,
        _ => Transition::None,
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test --lib state
```

Expected: all 7 tests pass.

- [ ] **Step 5: Run quality gate**

```bash
cargo fmt && cargo clippy -- -D warnings
```

Fix any issues.

- [ ] **Step 6: Commit**

```bash
git add src/state.rs
git commit -m "add state types and transition detection with day-boundary logic"
```

---

### Task 4: State File Persistence

**Files:**
- Modify: `src/state.rs`

- [ ] **Step 1: Write failing tests for state I/O**

Add these error types and imports to the top of `src/state.rs`:

```rust
use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use snafu::{OptionExt, ResultExt, Snafu};

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to determine state directory"))]
    StateDir,

    #[snafu(display("failed to create state directory at {}", path.display()))]
    CreateDir {
        source: std::io::Error,
        path: PathBuf,
    },

    #[snafu(display("failed to read state file at {}", path.display()))]
    ReadState {
        source: std::io::Error,
        path: PathBuf,
    },

    #[snafu(display("failed to write state file at {}", path.display()))]
    WriteState {
        source: std::io::Error,
        path: PathBuf,
    },

    #[snafu(display("failed to parse state file"))]
    ParseState { source: serde_json::Error },

    #[snafu(display("failed to serialize state"))]
    SerializeState { source: serde_json::Error },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
```

Add these tests to the `mod tests` block:

```rust
    #[test]
    fn test_state_round_trip() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("state.json");

        let state = AppState {
            session_cookie: "Fe26.2**abc123".to_string(),
            kid_id: "872d5140-3b20-498d-9e79-858e05788c48".to_string(),
            kid_name: "TestKid".to_string(),
            last_status: Some(PresenceStatus::CheckedIn),
            last_status_date: Some(date(2026, 4, 20)),
            last_check_at: ::core::option::Option::None,
        };

        save_state(&path, &state)?;
        let loaded = load_state(&path)?;
        assert_eq!(state, loaded);
        Ok(())
    }

    #[test]
    fn test_load_missing_file_returns_error() {
        let result = load_state(Path::new("/tmp/nonexistent-hortpro-test/state.json"));
        assert!(result.is_err());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test --lib state
```

Expected: compilation errors — `save_state`, `load_state` don't exist yet.

- [ ] **Step 3: Implement state file I/O**

Add these functions to `src/state.rs`, below the `impl AppState` block:

```rust
/// Returns the default path for the state file: `~/.local/state/hortpro/state.json`
pub fn default_state_path() -> Result<PathBuf> {
    let base = dirs::state_dir().context(StateDirSnafu)?;
    Ok(base.join("hortpro").join("state.json"))
}

/// Loads the application state from the given path.
pub fn load_state(path: &Path) -> Result<AppState> {
    let content =
        std::fs::read_to_string(path).context(ReadStateSnafu { path: path.to_path_buf() })?;
    serde_json::from_str(&content).context(ParseStateSnafu)
}

/// Saves the application state to the given path, creating parent directories as needed.
pub fn save_state(path: &Path, state: &AppState) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .context(CreateDirSnafu { path: parent.to_path_buf() })?;
    }
    let content = serde_json::to_string_pretty(state).context(SerializeStateSnafu)?;
    std::fs::write(path, content).context(WriteStateSnafu { path: path.to_path_buf() })?;
    Ok(())
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test --lib state
```

Expected: all 9 tests pass.

- [ ] **Step 5: Run quality gate**

```bash
cargo fmt && cargo clippy -- -D warnings
```

Fix any issues.

- [ ] **Step 6: Commit**

```bash
git add src/state.rs
git commit -m "add state file persistence with snafu error handling"
```

---

### Task 5: Notification Module

**Files:**
- Modify: `src/notify.rs`

- [ ] **Step 1: Write failing test for command building**

Write `src/notify.rs`:

```rust
use std::ffi::OsStr;
use std::process::Command;

use snafu::{ResultExt, Snafu};

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to execute notify-send"))]
    Execute { source: std::io::Error },

    #[snafu(display("notify-send exited with status {status}"))]
    NonZeroExit { status: i32 },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Urgency level for desktop notifications.
#[derive(Debug, Clone, Copy)]
pub enum Urgency {
    Normal,
    Critical,
}

impl Urgency {
    fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Critical => "critical",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_command_normal() {
        let cmd = build_command("HortProChecker", "Kid arrived", Urgency::Normal);
        assert_eq!(cmd.get_program(), "notify-send");
        let args: Vec<&OsStr> = cmd.get_args().collect();
        assert_eq!(
            args,
            [
                "--urgency",
                "normal",
                "--app-name",
                "HortProChecker",
                "HortProChecker",
                "Kid arrived",
            ]
        );
    }

    #[test]
    fn test_build_command_critical() {
        let cmd = build_command("Error", "something broke", Urgency::Critical);
        let args: Vec<&OsStr> = cmd.get_args().collect();
        assert_eq!(args[1], "critical");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test --lib notify
```

Expected: compilation error — `build_command` doesn't exist.

- [ ] **Step 3: Implement build_command and send**

Add these functions to `src/notify.rs`, above the `#[cfg(test)]` block:

```rust
/// Builds the `notify-send` command without executing it.
#[must_use]
pub fn build_command(summary: &str, body: &str, urgency: Urgency) -> Command {
    let mut cmd = Command::new("notify-send");
    cmd.arg("--urgency")
        .arg(urgency.as_str())
        .arg("--app-name")
        .arg("HortProChecker")
        .arg(summary)
        .arg(body);
    cmd
}

/// Sends a desktop notification via `notify-send`.
pub fn send(summary: &str, body: &str, urgency: Urgency) -> Result<()> {
    let status = build_command(summary, body, urgency).status().context(ExecuteSnafu)?;
    if !status.success() {
        let code = status.code();
        return NonZeroExitSnafu {
            status: code.unwrap_or(-1),
        }
        .fail();
    }
    Ok(())
}
```

Note: `status.code().unwrap_or(-1)` is acceptable — `unwrap_or` is a safe fallback, not the banned `.unwrap()`.

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test --lib notify
```

Expected: 2 tests pass.

- [ ] **Step 5: Run quality gate**

```bash
cargo fmt && cargo clippy -- -D warnings
```

Fix any issues.

- [ ] **Step 6: Commit**

```bash
git add src/notify.rs
git commit -m "add notify-send wrapper with command builder and urgency levels"
```

---

### Task 6: API Response Types and Status Determination

**Files:**
- Modify: `src/api.rs`

- [ ] **Step 1: Write failing tests for determine_status**

Write `src/api.rs`:

```rust
use chrono::NaiveDate;
use serde::Deserialize;

use crate::state::PresenceStatus;

/// Wrapper for all API responses.
#[derive(Debug, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
}

/// User data returned from the login endpoint.
#[derive(Debug, Deserialize)]
pub struct LoginData {
    pub firstname: String,
    pub lastname: String,
}

/// A child record from the `/api/kids` endpoint.
#[derive(Debug, Deserialize)]
pub struct Kid {
    pub id: String,
    pub firstname: String,
    pub kid_group: Option<String>,
}

/// Paginated presences wrapper.
#[derive(Debug, Deserialize)]
pub struct PresencesData {
    pub rows: Vec<PresenceRecord>,
}

/// A single presence (check-in/check-out) record.
#[derive(Debug, Deserialize)]
pub struct PresenceRecord {
    pub date_start: String,
    pub date_end: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        match NaiveDate::from_ymd_opt(y, m, d) {
            Some(d) => d,
            ::core::option::Option::None => panic!("invalid date: {y}-{m}-{d}"),
        }
    }

    #[test]
    fn test_checked_in_today() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let records = vec![PresenceRecord {
            date_start: "2026-04-20T08:26:38+02:00".to_string(),
            date_end: ::core::option::Option::None,
        }];
        let status = determine_status(&records, date(2026, 4, 20))?;
        assert_eq!(status, PresenceStatus::CheckedIn);
        Ok(())
    }

    #[test]
    fn test_checked_out_with_end_time() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let records = vec![PresenceRecord {
            date_start: "2026-04-20T08:26:38+02:00".to_string(),
            date_end: Some("2026-04-20T14:30:00+02:00".to_string()),
        }];
        let status = determine_status(&records, date(2026, 4, 20))?;
        assert_eq!(status, PresenceStatus::CheckedOut);
        Ok(())
    }

    #[test]
    fn test_checked_out_when_start_is_yesterday() -> std::result::Result<(), Box<dyn std::error::Error>>
    {
        let records = vec![PresenceRecord {
            date_start: "2026-04-19T08:00:00+02:00".to_string(),
            date_end: ::core::option::Option::None,
        }];
        let status = determine_status(&records, date(2026, 4, 20))?;
        assert_eq!(status, PresenceStatus::CheckedOut);
        Ok(())
    }

    #[test]
    fn test_checked_out_when_no_records() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let records: Vec<PresenceRecord> = vec![];
        let status = determine_status(&records, date(2026, 4, 20))?;
        assert_eq!(status, PresenceStatus::CheckedOut);
        Ok(())
    }

    #[test]
    fn test_parse_kids_response() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let json = r#"{
            "success": true,
            "data": [
                {
                    "id": "872d5140-3b20-498d-9e79-858e05788c48",
                    "firstname": "TestKid",
                    "lastname": "Doe",
                    "kid_group": "Eisbaeren",
                    "extra_field": "ignored"
                }
            ]
        }"#;
        let response: ApiResponse<Vec<Kid>> = serde_json::from_str(json)?;
        assert!(response.success);
        let kids = response.data.ok_or("missing data")?;
        assert_eq!(kids.len(), 1);
        assert_eq!(kids[0].firstname, "TestKid");
        assert_eq!(kids[0].kid_group.as_deref(), Some("Eisbaeren"));
        Ok(())
    }

    #[test]
    fn test_parse_presences_response() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let json = r#"{
            "success": true,
            "data": {
                "count": 635,
                "rows": [
                    {
                        "id": "5fac3696-c7e0-428a-afb1-3dd221845217",
                        "date_start": "2026-04-20T08:26:38+02:00",
                        "date_end": null,
                        "duration": null
                    }
                ]
            }
        }"#;
        let response: ApiResponse<PresencesData> = serde_json::from_str(json)?;
        assert!(response.success);
        let data = response.data.ok_or("missing data")?;
        assert_eq!(data.rows.len(), 1);
        assert!(data.rows[0].date_end.is_none());
        Ok(())
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test --lib api
```

Expected: compilation error — `determine_status` doesn't exist.

- [ ] **Step 3: Implement determine_status**

Add these items to `src/api.rs`, above the `#[cfg(test)]` block:

```rust
use chrono::DateTime;
use snafu::{ResultExt, Snafu};

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to parse presence date: {value}"))]
    ParseDate {
        source: chrono::ParseError,
        value: String,
    },

    #[snafu(display("HTTP request failed"))]
    Request { source: reqwest::Error },

    #[snafu(display("failed to parse response body"))]
    ParseResponse { source: reqwest::Error },

    #[snafu(display("API returned success: false"))]
    ApiUnsuccessful,

    #[snafu(display("API response missing data field"))]
    MissingData,

    #[snafu(display("no kids found in account"))]
    NoKids,

    #[snafu(display("no session cookie in login response"))]
    NoSessionCookie,

    #[snafu(display("session expired (HTTP 401)"))]
    SessionExpired,

    #[snafu(display("unexpected HTTP status: {status}"))]
    UnexpectedStatus { status: u16 },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Determines the current presence status from the most recent presence record.
///
/// Returns `CheckedIn` if the newest record has no `date_end` and `date_start` is today.
/// Returns `CheckedOut` in all other cases (ended record, wrong date, no records).
pub fn determine_status(records: &[PresenceRecord], today: NaiveDate) -> Result<PresenceStatus> {
    let record = match records.first() {
        Some(r) => r,
        ::core::option::Option::None => return Ok(PresenceStatus::CheckedOut),
    };

    if record.date_end.is_some() {
        return Ok(PresenceStatus::CheckedOut);
    }

    let start = DateTime::parse_from_rfc3339(&record.date_start)
        .context(ParseDateSnafu { value: record.date_start.clone() })?;

    if start.date_naive() == today {
        Ok(PresenceStatus::CheckedIn)
    } else {
        Ok(PresenceStatus::CheckedOut)
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test --lib api
```

Expected: all 6 tests pass.

- [ ] **Step 5: Run quality gate**

```bash
cargo fmt && cargo clippy -- -D warnings
```

Fix any issues.

- [ ] **Step 6: Commit**

```bash
git add src/api.rs
git commit -m "add API response types and presence status determination"
```

---

### Task 7: API HTTP Functions

**Files:**
- Modify: `src/api.rs`

These functions perform real HTTP requests and are not unit-tested. The parsing logic they depend on is tested in Task 6.

- [ ] **Step 1: Add HTTP constants and client builder**

Add to the top of `src/api.rs` (below existing imports):

```rust
use reqwest::blocking::Client;
use snafu::OptionExt;

const BASE_URL: &str = "https://elternportal.hortpro.de";
const CLIENT_VERSION: &str = "1.14.1";

/// Builds a reqwest blocking client with the `client-version` header pre-configured.
pub fn build_client() -> Result<Client> {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        "client-version",
        reqwest::header::HeaderValue::from_static(CLIENT_VERSION),
    );
    reqwest::blocking::Client::builder()
        .default_headers(headers)
        .build()
        .context(RequestSnafu)
}
```

- [ ] **Step 2: Add auth status checker helper**

Add this private function:

```rust
fn check_auth_status(status: reqwest::StatusCode) -> Result<()> {
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return SessionExpiredSnafu.fail();
    }
    if !status.is_success() {
        return UnexpectedStatusSnafu { status: status.as_u16() }.fail();
    }
    Ok(())
}

fn extract_data<T>(response: ApiResponse<T>) -> Result<T> {
    if !response.success {
        return ApiUnsuccessfulSnafu.fail();
    }
    response.data.context(MissingDataSnafu)
}
```

- [ ] **Step 3: Implement login**

```rust
/// Authenticates with the HortPro API. Returns the session cookie value and user data.
pub fn login(client: &Client, email: &str, password: &str) -> Result<(String, LoginData)> {
    let timestamp = chrono::Utc::now().timestamp_millis();
    let url = format!("{BASE_URL}/api/user/login?_dc={timestamp}");

    let response = client
        .post(&url)
        .json(&serde_json::json!({
            "email": email,
            "password": password,
            "keepSession": false
        }))
        .send()
        .context(RequestSnafu)?;

    let session_cookie = response
        .cookies()
        .find(|c| c.name() == "sid-hep")
        .map(|c| c.value().to_string())
        .context(NoSessionCookieSnafu)?;

    let body: ApiResponse<LoginData> = response.json().context(ParseResponseSnafu)?;
    let data = extract_data(body)?;

    Ok((session_cookie, data))
}
```

- [ ] **Step 4: Implement fetch_first_kid**

```rust
/// Fetches the list of kids and returns the first one.
pub fn fetch_first_kid(client: &Client, session_cookie: &str) -> Result<Kid> {
    let url = format!("{BASE_URL}/api/kids");

    let response = client
        .get(&url)
        .header("Cookie", format!("sid-hep={session_cookie}"))
        .send()
        .context(RequestSnafu)?;

    let status = response.status();
    check_auth_status(status)?;

    let body: ApiResponse<Vec<Kid>> = response.json().context(ParseResponseSnafu)?;
    let kids = extract_data(body)?;

    kids.into_iter().next().context(NoKidsSnafu)
}
```

- [ ] **Step 5: Implement fetch_presences**

```rust
/// Fetches the most recent presence record for a kid.
pub fn fetch_presences(
    client: &Client,
    session_cookie: &str,
    kid_id: &str,
) -> Result<Vec<PresenceRecord>> {
    let url = format!("{BASE_URL}/api/kids/{kid_id}/presences?start=0&limit=1");

    let response = client
        .get(&url)
        .header("Cookie", format!("sid-hep={session_cookie}"))
        .send()
        .context(RequestSnafu)?;

    let status = response.status();
    check_auth_status(status)?;

    let body: ApiResponse<PresencesData> = response.json().context(ParseResponseSnafu)?;
    let data = extract_data(body)?;

    Ok(data.rows)
}
```

- [ ] **Step 6: Verify it compiles and existing tests still pass**

```bash
cargo build && cargo test
```

Expected: compiles, all previous tests pass.

- [ ] **Step 7: Run quality gate**

```bash
cargo fmt && cargo clippy -- -D warnings
```

Fix any issues.

- [ ] **Step 8: Commit**

```bash
git add src/api.rs
git commit -m "add HTTP functions for login, kid fetch, and presence queries"
```

---

### Task 8: Main Orchestration

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Write the full main.rs**

Replace `src/main.rs` with:

```rust
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
```

- [ ] **Step 2: Verify it compiles**

```bash
cargo build
```

Expected: compiles with no errors.

- [ ] **Step 3: Run all tests**

```bash
cargo test
```

Expected: all tests pass (state and API parsing tests from previous tasks).

- [ ] **Step 4: Run quality gate**

```bash
cargo fmt && cargo clippy -- -D warnings
```

Fix any issues.

- [ ] **Step 5: Commit**

```bash
git add src/main.rs
git commit -m "wire up main with login/check subcommands, exit codes, and notifications"
```

---

### Task 9: Final Quality Gate

**Files:** all

- [ ] **Step 1: Run the full quality gate**

```bash
cargo fmt -- --check && cargo clippy -- -D warnings && cargo test
```

Expected: all three pass with zero warnings and zero failures.

- [ ] **Step 2: Verify --help output**

```bash
cargo run -- --help
cargo run -- login --help
cargo run -- check --help
```

Expected: meaningful help text for each subcommand.

- [ ] **Step 3: Build release binary**

```bash
cargo build --release
```

Expected: builds successfully. Binary at `target/release/hortpro-checker`.

- [ ] **Step 4: Commit if any fixes were needed**

Only if changes were made in steps 1-3:

```bash
git add -A
git commit -m "fix quality gate issues"
```
