//! Direct LAN peer discovery via mDNS/DNS-SD.
//!
//! Browses `_clawde._tcp.local.` using the same `mdns-sd` crate that the
//! advertisement side uses in `mdns.rs`. Discovered peers are stored in a
//! shared in-memory registry that the `connectivity.status` RPC reads.

use mdns_sd::{ServiceDaemon, ServiceEvent};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tracing::{debug, info, warn};

/// A discovered LAN peer running `clawd`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LanPeer {
    /// mDNS instance name, e.g. `clawd-abc12345`.
    pub name: String,
    /// Resolved IP address (first address returned by mDNS).
    pub address: String,
    /// Daemon port (from TXT record or SRV).
    pub port: u16,
    /// Daemon version (from TXT `version` record).
    pub version: String,
    /// Daemon ID (from TXT `daemon_id` record).
    pub daemon_id: String,
    /// Unix timestamp (seconds) of when this peer was last seen.
    pub last_seen: i64,
}

/// Thread-safe registry of currently visible LAN peers.
pub type PeerRegistry = Arc<RwLock<HashMap<String, LanPeer>>>;

/// Create an empty peer registry.
pub fn new_registry() -> PeerRegistry {
    Arc::new(RwLock::new(HashMap::new()))
}

/// Browse `_clawde._tcp.local.` continuously and update `registry`.
///
/// Spawns a blocking thread (mDNS browse is synchronous). Returns immediately.
/// The browse continues until the returned `BrowseGuard` is dropped.
///
/// This is non-fatal — if mDNS is unavailable the function logs a warning
/// and returns `None`.
pub fn start_browse(registry: PeerRegistry) -> Option<BrowseGuard> {
    match ServiceDaemon::new() {
        Ok(mdns) => {
            let mdns2 = mdns.clone();
            let receiver = match mdns.browse("_clawde._tcp.local.") {
                Ok(r) => r,
                Err(e) => {
                    warn!(err = %e, "mDNS browse failed — direct peer discovery unavailable");
                    let _ = mdns2.shutdown();
                    return None;
                }
            };

            let registry_clone = Arc::clone(&registry);
            std::thread::spawn(move || loop {
                match receiver.recv() {
                    Ok(event) => handle_event(event, &registry_clone),
                    Err(_) => {
                        debug!("mDNS browse channel closed — stopping peer discovery");
                        break;
                    }
                }
            });

            info!("mDNS browse started for _clawde._tcp.local.");
            Some(BrowseGuard { _daemon: mdns2 })
        }
        Err(e) => {
            warn!(err = %e, "mDNS daemon unavailable — direct peer discovery disabled");
            None
        }
    }
}

fn handle_event(event: ServiceEvent, registry: &PeerRegistry) {
    match event {
        ServiceEvent::ServiceResolved(info) => {
            let name = info.get_fullname().to_owned();
            let port = info.get_port();
            let addresses = info.get_addresses();
            let address = addresses
                .iter()
                .next()
                .map(|a| a.to_string())
                .unwrap_or_default();

            let props = info.get_properties();
            let version = props
                .get("version")
                .map(|v| v.val_str())
                .unwrap_or("unknown")
                .to_owned();
            let daemon_id = props
                .get("daemon_id")
                .map(|v| v.val_str())
                .unwrap_or("")
                .to_owned();

            let peer = LanPeer {
                name: name.clone(),
                address,
                port,
                version,
                daemon_id,
                last_seen: chrono::Utc::now().timestamp(),
            };

            info!(name = %peer.name, addr = %peer.address, port = peer.port, "mDNS peer discovered");
            if let Ok(mut reg) = registry.write() {
                reg.insert(name, peer);
            }
        }
        ServiceEvent::ServiceRemoved(_, fullname) => {
            info!(name = %fullname, "mDNS peer departed");
            if let Ok(mut reg) = registry.write() {
                reg.remove(&fullname);
            }
        }
        _ => {}
    }
}

/// Keeps the mDNS browse alive. Shutting down when dropped.
pub struct BrowseGuard {
    _daemon: ServiceDaemon,
}
