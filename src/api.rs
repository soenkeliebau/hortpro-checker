use chrono::{DateTime, NaiveDate};
use serde::Deserialize;
use snafu::{OptionExt, ResultExt, Snafu, ensure};

use crate::state::PresenceStatus;

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

/// Sends a POST request and deserialises the JSON body into an `ApiResponse<T>`.
///
/// This is the shared HTTP helper used by all API calls. It maps `reqwest::Error`
/// into the typed `Request` and `ParseResponse` error variants.
fn post_json<T: serde::de::DeserializeOwned>(
    client: &reqwest::blocking::Client,
    url: &str,
    body: &serde_json::Value,
) -> Result<ApiResponse<T>> {
    let response = client.post(url).json(body).send().context(RequestSnafu)?;
    response
        .json::<ApiResponse<T>>()
        .context(ParseResponseSnafu)
}

/// Logs in with the given credentials and returns the parsed `LoginData`.
///
/// Full implementation in Task 7 will read `data.firstname`, `data.lastname`,
/// and the session cookie from the response.
pub fn login(email: &str, password: &str) -> Result<LoginData> {
    let client = reqwest::blocking::Client::new();
    let body = serde_json::json!({ "email": email, "password": password });
    let response = post_json::<LoginData>(&client, "http://invalid.placeholder/login", &body)?;
    ensure!(response.success, ApiUnsuccessfulSnafu);
    let data = response.data.context(MissingDataSnafu)?;
    // Read fields to satisfy dead-code lint until Task 7 wires up real credential handling.
    let _ = (&data.firstname, &data.lastname);
    Ok(data)
}

/// Fetches the first kid for the logged-in account.
///
/// Full implementation in Task 7 will call `GET /api/kids` and return the first kid's
/// `id`, `firstname`, and `kid_group`.
pub fn fetch_first_kid(session: &str) -> Result<Kid> {
    let client = reqwest::blocking::Client::new();
    let body = serde_json::json!({ "session": session });
    let response = post_json::<Vec<Kid>>(&client, "http://invalid.placeholder/kids", &body)?;
    let mut kids = response.data.context(MissingDataSnafu)?;
    let kid = kids.drain(..).next().context(NoKidsSnafu)?;
    // Read fields to satisfy dead-code lint until Task 7 wires up real kid selection.
    let _ = (&kid.id, &kid.firstname, &kid.kid_group);
    Ok(kid)
}

/// Fetches the presence records for the given kid.
///
/// Full implementation in Task 7 will call `GET /api/presences` and return a
/// `PresencesData` with the `rows` of presence records.
pub fn fetch_presences(session: &str, kid_id: &str) -> Result<PresencesData> {
    let client = reqwest::blocking::Client::new();
    let body = serde_json::json!({ "session": session, "kid_id": kid_id });
    let response =
        post_json::<PresencesData>(&client, "http://invalid.placeholder/presences", &body)?;
    let data = response.data.context(MissingDataSnafu)?;
    // Read field to satisfy dead-code lint until Task 7 wires up real presence handling.
    let _ = &data.rows;
    Ok(data)
}

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

    let start = DateTime::parse_from_rfc3339(&record.date_start).context(ParseDateSnafu {
        value: record.date_start.clone(),
    })?;

    if start.date_naive() == today {
        Ok(PresenceStatus::CheckedIn)
    } else {
        Ok(PresenceStatus::CheckedOut)
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
    fn test_checked_out_when_start_is_yesterday()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
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
