//! Policy violation scanners.
//!
//! Each scanner detects a specific class of violation in a patch or content:
//!  - `placeholders` — detects unimplemented stubs (TODO, FIXME, etc.)
//!  - `secrets`      — detects leaked credentials in diffs
//!  - `forbidden`    — detects tool calls that exceed granted permissions

pub mod forbidden;
pub mod placeholders;
pub mod secrets;
