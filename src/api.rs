use chrono::{DateTime, NaiveDate};
use reqwest::blocking::Client;
use serde::Deserialize;
use snafu::{OptionExt, ResultExt, Snafu};

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

fn check_auth_status(status: reqwest::StatusCode) -> Result<()> {
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return SessionExpiredSnafu.fail();
    }
    if !status.is_success() {
        return UnexpectedStatusSnafu {
            status: status.as_u16(),
        }
        .fail();
    }
    Ok(())
}

fn extract_data<T>(response: ApiResponse<T>) -> Result<T> {
    if !response.success {
        return ApiUnsuccessfulSnafu.fail();
    }
    response.data.context(MissingDataSnafu)
}

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
