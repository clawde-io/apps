use anyhow::Result;
use std::path::Path;
use uuid::Uuid;

/// Return the auth token for this daemon instance.
///
/// On first call, generates a random 32-character hex token and writes it to
/// `{data_dir}/auth_token` with user-only read/write permissions (mode 0600
/// on Unix). On subsequent calls, reads and returns the existing token.
///
/// The token file must be kept secret — it is the only credential protecting
/// the local WebSocket port from unauthorized access by other processes on
/// the same machine.
pub fn get_or_create_token(data_dir: &Path) -> Result<String> {
    let path = data_dir.join("auth_token");

    if path.exists() {
        let token = std::fs::read_to_string(&path)?.trim().to_string();
        if !token.is_empty() {
            return Ok(token);
        }
    }

    // Generate a new token (UUID v4, hex without dashes = 32 chars)
    let token = Uuid::new_v4().to_string().replace('-', "");

    std::fs::create_dir_all(data_dir)?;
    std::fs::write(&path, &token)?;

    // Restrict to owner read/write only on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }

    Ok(token)
}

/// Validate a `Bearer <token>` authorization string against the expected token.
/// Returns `true` if the header value is exactly `"Bearer {expected_token}"`.
pub fn validate_bearer(header_value: &str, expected_token: &str) -> bool {
    header_value
        .strip_prefix("Bearer ")
        .map(|t| t == expected_token)
        .unwrap_or(false)
}
