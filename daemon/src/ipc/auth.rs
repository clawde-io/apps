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

    // Create the file with owner-only permissions from the start to eliminate
    // the TOCTOU window that would exist if we wrote first and chmod'd second.
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)?;
        f.write_all(token.as_bytes())?;
    }
    #[cfg(not(unix))]
    std::fs::write(&path, &token)?;

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

/// Check that the auth token file has secure permissions (DC.T42).
///
/// On Unix, warns if the file is not exclusively owner read/write (0o600).
/// No automatic correction is made — the user must run `chmod 0600 <path>`.
pub fn check_token_permissions(data_dir: &Path) {
    let path = data_dir.join("auth_token");
    if !path.exists() {
        return;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(&path) {
            let mode = meta.permissions().mode() & 0o777;
            if mode != 0o600 {
                tracing::warn!(
                    path = %path.display(),
                    mode = format!("{:04o}", mode),
                    "auth_token file has insecure permissions (expected 0600). \
                     Run: chmod 0600 {}",
                    path.display()
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_validate_bearer_valid() {
        assert!(validate_bearer("Bearer secret123", "secret123"));
    }

    #[test]
    fn test_validate_bearer_invalid() {
        assert!(!validate_bearer("Bearer wrong", "secret123"));
        assert!(!validate_bearer("secret123", "secret123"));
        assert!(!validate_bearer("", "secret123"));
    }

    #[test]
    fn test_get_or_create_token_creates_file() {
        let dir = TempDir::new().unwrap();
        let token = get_or_create_token(dir.path()).unwrap();
        assert_eq!(token.len(), 32, "token should be 32 hex chars");
        assert!(dir.path().join("auth_token").exists());
    }

    #[test]
    fn test_get_or_create_token_idempotent() {
        let dir = TempDir::new().unwrap();
        let t1 = get_or_create_token(dir.path()).unwrap();
        let t2 = get_or_create_token(dir.path()).unwrap();
        assert_eq!(t1, t2, "second call should return same token");
    }

    #[cfg(unix)]
    #[test]
    fn test_auth_token_created_with_0600_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let dir = TempDir::new().unwrap();
        get_or_create_token(dir.path()).unwrap();
        let meta = std::fs::metadata(dir.path().join("auth_token")).unwrap();
        let mode = meta.permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o600,
            "auth_token must have mode 0600, got {mode:04o}"
        );
    }
}
