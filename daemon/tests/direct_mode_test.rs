/// Sprint JJ DM.5 — direct mode tests.
///
/// Unit tests verify ConnectivityConfig parsing and monitor defaults.
/// Integration tests (marked `#[ignore]`) require a live daemon.

#[cfg(test)]
mod connectivity_config_tests {
    use clawd::config::ConnectivityConfig;

    #[test]
    fn default_connectivity_config() {
        let cfg = ConnectivityConfig::default();
        assert!(!cfg.prefer_direct, "prefer_direct should default to false");
        assert!(cfg.vpn_host.is_none(), "vpn_host should default to None");
        assert!(!cfg.air_gap, "air_gap should default to false");
    }

    #[test]
    fn connectivity_config_serializes() {
        let cfg = ConnectivityConfig {
            prefer_direct: true,
            vpn_host: Some("10.0.1.5".to_string()),
            air_gap: false,
        };
        let json = serde_json::to_string(&cfg).expect("serialize");
        assert!(json.contains("prefer_direct"));
        assert!(json.contains("10.0.1.5"));
    }

    #[test]
    fn connectivity_config_from_toml() {
        let toml_str = r#"
prefer_direct = true
vpn_host = "10.0.2.20"
air_gap = false
"#;
        let cfg: ConnectivityConfig = toml::from_str(toml_str).expect("parse toml");
        assert!(cfg.prefer_direct);
        assert_eq!(cfg.vpn_host.as_deref(), Some("10.0.2.20"));
    }
}

#[cfg(test)]
mod monitor_tests {
    use clawd::connectivity::monitor::{ConnectionMode, QualitySnapshot, new_shared_quality};

    #[test]
    fn default_quality_snapshot() {
        let q = QualitySnapshot::default();
        assert_eq!(q.rtt_ms, 0);
        assert!((q.packet_loss_pct - 0.0).abs() < f32::EPSILON);
        assert!(!q.degraded);
        assert_eq!(q.mode, ConnectionMode::Relay);
    }

    #[tokio::test]
    async fn shared_quality_is_readable() {
        let shared = new_shared_quality();
        let snap = shared.read().await;
        assert_eq!(snap.rtt_ms, 0);
    }

    #[test]
    fn connection_mode_display() {
        assert_eq!(ConnectionMode::Relay.to_string(), "relay");
        assert_eq!(ConnectionMode::Direct.to_string(), "direct");
        assert_eq!(ConnectionMode::Vpn.to_string(), "vpn");
        assert_eq!(ConnectionMode::Offline.to_string(), "offline");
    }
}

/// Integration test: mDNS browse starts without panicking.
/// Requires multicast networking to be available on the test host.
#[tokio::test]
#[ignore = "requires multicast-capable network interface"]
async fn mdns_browse_starts() {
    let registry = clawd::connectivity::direct::new_registry();
    let guard = clawd::connectivity::direct::start_browse(registry);
    // Non-fatal: None means mDNS unavailable (e.g. CI container)
    drop(guard);
}
