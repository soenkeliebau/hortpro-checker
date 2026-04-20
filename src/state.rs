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
}
