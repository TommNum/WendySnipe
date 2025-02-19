// src/core/mod.rs
use crate::config::Config;
use solana_sdk::signature::{Keypair, Signer};
use solana_client::rpc_client::RpcClient;
use anyhow::Result;
use tracing::{info, error};
use std::sync::Arc;

mod websocket;
use self::websocket::WebsocketMonitor;

pub struct PoolMonitor {
    config: Config,
    wallet: Keypair,
    rpc_client: Arc<RpcClient>,
    websocket_monitor: WebsocketMonitor,
}

impl PoolMonitor {
    pub fn new(config: crate::config::Config, wallet: Keypair) -> Result<Self> {
        let rpc_client = Arc::new(RpcClient::new(&config.network.rpc_url));
        let websocket_monitor = WebsocketMonitor::new(&config.network.ws_url, Arc::clone(&rpc_client))?;
        
        Ok(Self {
            config,
            wallet,
            rpc_client,
            websocket_monitor,
        })
    }

    pub async fn start(&self) -> Result<()> {
        info!("Starting pool monitor...");
        
        // Verify wallet balance
        let balance = self.rpc_client.get_balance(&self.wallet.pubkey())?;
        info!("Wallet balance: {} SOL", balance as f64 / 1_000_000_000.0);

        if balance < self.config.wallet.min_sol_balance {
            error!("Insufficient balance for trading");
            return Ok(());
        }

        // Start monitoring
        self.monitor_pools().await
    }

    async fn monitor_pools(&self) -> Result<()> {
        info!("Monitoring pools for {} program", self.config.programs.main_program);
        self.websocket_monitor.subscribe_to_logs().await
    }
}