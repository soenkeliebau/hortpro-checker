use std::path::{Path, PathBuf};

use base64::Engine;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use snafu::{OptionExt, ResultExt, Snafu};

/// Errors that can occur during state file operations.
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

    #[snafu(display("failed to decode photo from base64"))]
    DecodePhoto { source: base64::DecodeError },

    #[snafu(display("failed to write photo to {}", path.display()))]
    WritePhoto {
        source: std::io::Error,
        path: PathBuf,
    },

    #[snafu(display("photo data URI has no base64 payload"))]
    PhotoNoPayload,
}

/// A specialized `Result` type for state operations.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Whether the child is currently at daycare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresenceStatus {
    CheckedIn,
    CheckedOut,
}

/// The type of status change detected between two checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transition {
    Arrived,
    Left,
    None,
}

/// Persisted application state, stored as JSON between runs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppState {
    pub session_cookie: String,
    pub kid_id: String,
    pub kid_name: String,
    pub last_status: Option<PresenceStatus>,
    pub last_status_date: Option<NaiveDate>,
    pub last_check_at: Option<chrono::DateTime<chrono::FixedOffset>>,
}

impl AppState {
    /// Returns the last status if it was recorded today, otherwise None.
    pub fn effective_last_status(&self, today: NaiveDate) -> Option<PresenceStatus> {
        match self.last_status_date {
            Some(date) if date == today => self.last_status,
            _ => None,
        }
    }
}

/// Returns the default path for the state file: `~/.local/state/hortpro/state.json`
pub fn default_state_path() -> Result<PathBuf> {
    let base = dirs::state_dir().context(StateDirSnafu)?;
    Ok(base.join("hortpro").join("state.json"))
}

/// Returns the default path for the kid photo: `~/.local/state/hortpro/photo.jpg`
pub fn default_photo_path() -> Result<PathBuf> {
    let base = dirs::state_dir().context(StateDirSnafu)?;
    Ok(base.join("hortpro").join("photo.jpg"))
}

/// Decodes a `data:image/jpeg;base64,...` URI and writes the image to disk.
pub fn save_photo(data_uri: &str, path: &Path) -> Result<()> {
    let payload = data_uri
        .split_once(",")
        .map(|(_, b)| b)
        .context(PhotoNoPayloadSnafu)?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload)
        .context(DecodePhotoSnafu)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context(CreateDirSnafu {
            path: parent.to_path_buf(),
        })?;
    }
    std::fs::write(path, bytes).context(WritePhotoSnafu {
        path: path.to_path_buf(),
    })?;
    Ok(())
}

/// Loads the application state from the given path.
pub fn load_state(path: &Path) -> Result<AppState> {
    let content = std::fs::read_to_string(path).context(ReadStateSnafu {
        path: path.to_path_buf(),
    })?;
    serde_json::from_str(&content).context(ParseStateSnafu)
}

/// Saves the application state to the given path, creating parent directories as needed.
pub fn save_state(path: &Path, state: &AppState) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context(CreateDirSnafu {
            path: parent.to_path_buf(),
        })?;
    }
    let content = serde_json::to_string_pretty(state).context(SerializeStateSnafu)?;
    std::fs::write(path, content).context(WriteStateSnafu {
        path: path.to_path_buf(),
    })?;
    Ok(())
}

/// Determines the notification transition between two statuses.
#[must_use]
pub fn detect_transition(
    effective_previous: Option<PresenceStatus>,
    current: PresenceStatus,
) -> Transition {
    match (effective_previous, current) {
        (None, PresenceStatus::CheckedIn) => Transition::Arrived,
        (Some(PresenceStatus::CheckedOut), PresenceStatus::CheckedIn) => Transition::Arrived,
        (Some(PresenceStatus::CheckedIn), PresenceStatus::CheckedOut) => Transition::Left,
        _ => Transition::None,
    }
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
        assert_eq!(
            state.effective_last_status(today),
            Some(PresenceStatus::CheckedIn)
        );
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
        assert_eq!(
            state.effective_last_status(today),
            ::core::option::Option::None
        );
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
        assert_eq!(
            state.effective_last_status(today),
            ::core::option::Option::None
        );
    }

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
    fn test_load_missing_file_returns_read_error() {
        let result = load_state(Path::new("/tmp/nonexistent-hortpro-test/state.json"));
        assert!(matches!(result, Err(Error::ReadState { .. })));
    }
}
