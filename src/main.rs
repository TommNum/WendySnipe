mod config;
mod core;
mod utils;

use std::path::Path;
use anyhow::{Result, Context};
use tracing::info;
use solana_sdk::signature::Signer;

#[tokio::main]
async fn main() -> Result<()> {
    // Setup logging
    utils::logging::setup_logging()?;
    
    info!("Starting MEV bot...");

    // Load config
    let config_path = Path::new("config/development.toml");
    let config = config::Config::load(config_path.to_str().unwrap())
        .with_context(|| format!("Failed to load config from {:?}", config_path))?;
    info!("Loaded config: {:?}", config);

    // Load wallet
    let wallet_path = Path::new("dev_wallet.json");
    let wallet = utils::wallet::load_wallet(wallet_path.to_str().unwrap())
        .with_context(|| format!("Failed to load wallet from {:?}", wallet_path))?;
    info!("Loaded wallet: {}", wallet.pubkey());

    // Initialize pool monitor
    let monitor = core::PoolMonitor::new(config, wallet)?;
    
    // Start monitoring
    monitor.start().await
}