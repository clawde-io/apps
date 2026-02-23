//! Cursor runner — placeholder skeleton.
//!
//! Cursor does not yet expose a stable headless CLI comparable to `claude` or
//! `codex`. This module satisfies the `Runner` interface so the session layer
//! can type-check, but all methods return `PROVIDER_NOT_AVAILABLE` until the
//! Cursor CLI is available.

use super::runner::Runner;
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

pub struct CursorRunner {
    session_id: String,
}

impl CursorRunner {
    pub fn new(session_id: String) -> Arc<Self> {
        Arc::new(Self { session_id })
    }
}

#[async_trait]
impl Runner for CursorRunner {
    async fn run_turn(&self, _content: &str) -> Result<()> {
        anyhow::bail!(
            "PROVIDER_NOT_AVAILABLE: cursor CLI is not yet supported (session: {})",
            self.session_id
        )
    }

    async fn send(&self, _content: &str) -> Result<()> {
        Ok(())
    }

    async fn pause(&self) -> Result<()> {
        Ok(())
    }

    async fn resume(&self) -> Result<()> {
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        Ok(())
    }
}
