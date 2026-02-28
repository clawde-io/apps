//! `connectivity.*` RPC handlers — Sprint JJ.
//!
//! ## Methods
//!
//! - `connectivity.status` — current connection mode, RTT, packet loss, LAN peers.

use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};

/// `connectivity.status` — return current connection quality and LAN peers.
///
/// Response fields:
/// - `mode` — `"relay"` | `"direct"` | `"vpn"` | `"offline"`
/// - `rtt_ms` — round-trip time in milliseconds (0 if not yet measured)
/// - `packet_loss_pct` — estimated loss percentage over last 10 pings
/// - `degraded` — `true` when RTT > 500ms or loss > 5%
/// - `last_ping_at` — Unix timestamp of last ping attempt
/// - `prefer_direct` — config flag value
/// - `vpn_host` — configured VPN host, or `null`
/// - `air_gap` — config flag value
/// - `lan_peers` — array of discovered LAN peers (empty when no direct browse)
pub async fn status(_params: Value, ctx: &AppContext) -> Result<Value> {
    let snap = ctx.quality.read().await;

    let peers: Vec<Value> = {
        let reg = ctx
            .peer_registry
            .read()
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        reg.values()
            .map(|p| {
                json!({
                    "name": p.name,
                    "address": p.address,
                    "port": p.port,
                    "version": p.version,
                    "daemon_id": p.daemon_id,
                    "last_seen": p.last_seen,
                })
            })
            .collect()
    };

    Ok(json!({
        "mode": snap.mode,
        "rtt_ms": snap.rtt_ms,
        "packet_loss_pct": snap.packet_loss_pct,
        "degraded": snap.degraded,
        "last_ping_at": snap.last_ping_at,
        "prefer_direct": ctx.config.connectivity.prefer_direct,
        "vpn_host": ctx.config.connectivity.vpn_host,
        "air_gap": ctx.config.connectivity.air_gap,
        "lan_peers": peers,
    }))
}
