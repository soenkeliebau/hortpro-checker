# HortProChecker Design Spec

## Purpose

CLI tool that checks whether a child is currently at daycare by querying the HortPro Elternportal API. Tracks state across runs to detect transitions (arrived / left) and fires desktop notifications via `notify-send`. Designed to be called from cron or a systemd timer.

## CLI Interface

```
hortpro-checker login --email <EMAIL> --password <PASSWORD>
hortpro-checker check
```

### `login`

Authenticates against the HortPro API, fetches the child list, stores the session cookie and kid metadata in a state file. Prints confirmation with the child's name and group.

Exit codes: 0 = success, non-zero = error.

### `check`

Loads the stored session, queries the presences API, compares against the last-known status (day-boundary aware), fires `notify-send` on transitions or errors, updates state.

Exit codes:

| Code | Meaning |
|------|---------|
| 0 | Child is checked in |
| 1 | Child is checked out (or no record today) |
| 2 | Error (network, auth, parse failure, etc.) |

Error conditions also fire a notification so failures are visible without checking logs.

## API Interaction

Base URL: `https://elternportal.hortpro.de`

All requests include the header `client-version: 1.14.1`.

### Login

```
POST /api/user/login?_dc={unix_ms}
Content-Type: application/json
```

Body:
```json
{
  "email": "...",
  "password": "...",
  "keepSession": false
}
```

Response sets the `sid-hep` cookie (`Secure; HttpOnly; SameSite=Lax`). Body:
```json
{
  "success": true,
  "data": {
    "id": "...",
    "firstname": "...",
    "lastname": "...",
    "email": "..."
  }
}
```

### Fetch Kids

```
GET /api/kids
Cookie: sid-hep=<token>
```

Returns an array of kids with `id`, `firstname`, `lastname`, `kid_group`, and permissions. We take the first kid's `id` for subsequent calls.

### Fetch Presences

```
GET /api/kids/{kid_id}/presences?start=0&limit=1
Cookie: sid-hep=<token>
```

Response:
```json
{
  "success": true,
  "data": {
    "count": 635,
    "rows": [
      {
        "id": "...",
        "date_start": "2026-04-20T08:26:38+02:00",
        "date_end": null,
        "duration": null
      }
    ]
  }
}
```

Status determination:
- `date_end` is `null` and `date_start` is today: **checked in**
- `date_end` is non-null, or `date_start` is not today: **checked out / not present**

A 401 response means the session has expired.

## State Management

State file location: `~/.local/state/hortpro/state.json`

```json
{
  "session_cookie": "Fe26.2**...",
  "kid_id": "872d5140-3b20-498d-9e79-858e05788c48",
  "kid_name": "ChildName",
  "last_status": "checked_in",
  "last_status_date": "2026-04-20",
  "last_check_at": "2026-04-20T14:30:00+02:00"
}
```

Fields:
- `session_cookie`: the `sid-hep` value from login
- `kid_id`: full UUID of the child
- `kid_name`: first name, used in notifications
- `last_status`: one of `"checked_in"`, `"checked_out"`, or `null`
- `last_status_date`: calendar date (local) the status applies to
- `last_check_at`: ISO 8601 timestamp of the last successful check

### Day Boundary Logic

When `check` runs and today's local date differs from `last_status_date`, the effective previous status is treated as `null` (unknown). This prevents spurious notifications overnight or on weekends.

### Transition Notifications

| Previous (effective) | Current | Notification |
|---|---|---|
| null / unknown | checked_in | "{name} arrived at daycare" |
| checked_in | checked_out | "{name} left daycare" |
| checked_out | checked_in | "{name} arrived at daycare" |
| null / unknown | checked_out | (no notification) |
| same | same | (no notification) |
| any | error | "HortProChecker: {error description}" |

The "unknown to checked_out" case is the normal state before morning drop-off and produces no notification.

## Dependencies

| Crate | Purpose |
|-------|---------|
| `clap` (derive) | CLI argument parsing |
| `reqwest` (blocking, cookies) | HTTP client |
| `serde` / `serde_json` | JSON serialization |
| `snafu` | Error handling with local error enums |
| `chrono` | Date/time parsing and comparison |
| `dirs` | Resolving `~/.local/state` portably |

Async is not needed for a fire-once CLI tool. The `reqwest` blocking client keeps the dependency tree and complexity minimal.

## Project Structure

```
src/
  main.rs    -- entry point, clap dispatch, exit code mapping
  cli.rs     -- clap structs and subcommand definitions
  api.rs     -- login, fetch_kids, fetch_presences
  state.rs   -- state file read/write, day-boundary logic
  notify.rs  -- notify-send wrapper
```

Each module defines its own `Error` enum using snafu. `main.rs` maps module errors to exit codes and error notifications.

No global error type. Each module owns its failure modes.

## Testing Strategy

### Unit Tests

- **`state.rs`**: Day-boundary transition detection across all status combinations. State file round-trip serialization.
- **`api.rs`**: Response parsing from canned JSON strings (presences → status determination, kids → kid extraction, login → cookie extraction). No HTTP mocking.
- **`notify.rs`**: Verify command arguments constructed for `notify-send` without executing.

### Integration Tests

- Happy-path: write state file, verify read-back and transition detection.

No HTTP mocking. Parsing logic is tested in isolation; `reqwest` is trusted to do HTTP correctly.

## Quality Gate

Every change must pass before it is considered done:

```bash
cargo fmt -- --check && cargo clippy -- -D warnings && cargo test
```

No `.unwrap()`, `.expect()`, `panic!()` outside tests, or `#[allow(clippy::...)]` annotations.
