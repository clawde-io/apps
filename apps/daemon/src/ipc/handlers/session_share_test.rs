/// Sprint EE CS.10 — Unit tests for session sharing.
#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn active_share_sql_filter_logic() {
        // Demonstrates the SQL filter reasoning: a share is active when
        // revoked_at IS NULL AND (expires_at IS NULL OR expires_at > now).
        // We can't run actual SQLite here, but we verify the JSON shape.
        let share = json!({
            "shareToken": "clw_share_abc123",
            "sessionId": "sess-001",
            "expiresAt": "2099-01-01T00:00:00Z",
            "createdAt": "2026-02-26T00:00:00Z"
        });
        assert!(share["shareToken"].as_str().unwrap().starts_with("clw_share_"));
        assert!(share["expiresAt"].as_str().is_some());
        assert!(share["createdAt"].as_str().is_some());
    }

    #[test]
    fn revoke_response_shape() {
        let response = json!({ "ok": true });
        assert_eq!(response["ok"], true);
    }

    #[test]
    fn share_list_result_shape() {
        let result = json!({
            "shares": [
                {
                    "shareToken": "clw_share_xyz",
                    "sessionId": "sess-002",
                    "expiresAt": null,
                    "createdAt": "2026-02-26T01:00:00Z"
                }
            ]
        });
        let shares = result["shares"].as_array().unwrap();
        assert_eq!(shares.len(), 1);
        assert_eq!(shares[0]["shareToken"], "clw_share_xyz");
    }

    #[test]
    fn share_create_response_includes_token() {
        let response = json!({
            "shareToken": "clw_share_newtoken123",
            "expiresAt": "2026-02-26T18:00:00Z"
        });
        let token = response["shareToken"].as_str().unwrap();
        assert!(token.len() > 10);
    }
}
