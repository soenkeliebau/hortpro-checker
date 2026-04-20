use std::process::Command;

use snafu::{ResultExt, Snafu};

/// Errors that can occur when sending desktop notifications.
#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to execute notify-send"))]
    Execute { source: std::io::Error },

    #[snafu(display("notify-send exited with status {status}"))]
    NonZeroExit { status: i32 },
}

/// A specialized `Result` type for notification operations.
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
    let status = build_command(summary, body, urgency)
        .status()
        .context(ExecuteSnafu)?;
    if !status.success() {
        let code = status.code();
        return NonZeroExitSnafu {
            status: code.unwrap_or(-1),
        }
        .fail();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

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
