//! Connectivity module — direct LAN discovery and connection quality monitoring.
//!
//! ## Submodules
//!
//! - `direct` — mDNS browse for `_clawde._tcp.local.` peer discovery
//! - `monitor` — 30-second ping loop that tracks RTT and packet loss
//!
//! ## Usage
//!
//! The `AppContext` holds a `SharedQuality` (from `monitor`) and a
//! `PeerRegistry` (from `direct`). Both are started in `main.rs` if the
//! daemon is not in air-gap mode. The `connectivity.status` RPC handler
//! reads both to build its response.

pub mod direct;
pub mod monitor;

pub use direct::{new_registry, LanPeer, PeerRegistry};
pub use monitor::{new_shared_quality, ConnectionMode, QualitySnapshot, SharedQuality};
