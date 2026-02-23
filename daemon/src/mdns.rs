//! mDNS/DNS-SD service advertisement for LAN discovery.
//!
//! Advertises `_clawde._tcp.local.` on port 4300 so that ClawDE clients
//! on the same LAN can auto-discover the daemon without manual IP entry.

use mdns_sd::{ServiceDaemon, ServiceInfo};
use tracing::{info, warn};

/// Holds the mDNS daemon and service name.
/// The service is unregistered when this guard is dropped.
pub struct MdnsGuard {
    daemon: ServiceDaemon,
    fullname: String,
}

impl Drop for MdnsGuard {
    fn drop(&mut self) {
        if let Err(e) = self.daemon.unregister(&self.fullname) {
            warn!(err = %e, "mDNS unregister failed on shutdown");
        }
        if let Err(e) = self.daemon.shutdown() {
            warn!(err = %e, "mDNS daemon shutdown failed");
        }
        info!("mDNS advertisement unregistered");
    }
}

/// Start advertising `_clawde._tcp.local.` on `port`.
/// Returns `None` if mDNS is unavailable (non-fatal).
///
/// mDNS failure is explicitly logged at WARN level and the daemon continues
/// starting without LAN discovery. This is expected on headless servers,
/// containers, or systems without multicast networking.
pub fn advertise(daemon_id: &str, port: u16) -> Option<MdnsGuard> {
    match try_advertise(daemon_id, port) {
        Ok(guard) => {
            info!(
                port = port,
                "mDNS advertisement registered (_clawde._tcp.local)"
            );
            Some(guard)
        }
        Err(e) => {
            warn!(
                err = %e,
                port = port,
                "mDNS advertisement failed — LAN discovery will not be available. \
                 The daemon will continue starting without mDNS. \
                 Clients must connect via explicit address (localhost:{}).",
                port,
            );
            None
        }
    }
}

fn try_advertise(daemon_id: &str, port: u16) -> anyhow::Result<MdnsGuard> {
    let mdns =
        ServiceDaemon::new().map_err(|e| anyhow::anyhow!("failed to start mDNS daemon: {e}"))?;

    // Instance name: clawd-{first 8 chars of daemon_id}
    let id_len = daemon_id.len();
    let truncated_len = 8.min(id_len);
    if id_len > truncated_len {
        info!(
            full_len = id_len,
            used_len = truncated_len,
            "mDNS instance name uses first {} chars of daemon_id",
            truncated_len
        );
    }
    let short_id = &daemon_id[..truncated_len];
    let instance_name = format!("clawd-{short_id}");

    // Build TXT properties
    let mut props = std::collections::HashMap::new();
    props.insert("version".to_owned(), env!("CARGO_PKG_VERSION").to_owned());
    props.insert("daemon_id".to_owned(), daemon_id.to_owned());

    let service_info = ServiceInfo::new(
        "_clawde._tcp.local.",
        &instance_name,
        "localhost.local.",
        "", // empty = mdns-sd auto-detects local IP(s)
        port,
        Some(props),
    )?;

    let fullname = service_info.get_fullname().to_owned();
    mdns.register(service_info)?;

    Ok(MdnsGuard {
        daemon: mdns,
        fullname,
    })
}
