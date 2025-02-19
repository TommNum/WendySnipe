// src/utils/logging.rs
use tracing_subscriber::{fmt, EnvFilter};
use anyhow::Result;

pub fn setup_logging() -> Result<()> {
    fmt()
        .with_env_filter(EnvFilter::from_default_env()
            .add_directive("solana_mev_bot=debug".parse()?))
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .init();
    
    Ok(())
}