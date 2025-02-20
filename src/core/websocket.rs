use tokio_tungstenite::connect_async;
use futures::{StreamExt, SinkExt};
use serde_json::{json, Value};
use anyhow::{Result, anyhow};
use dashmap::DashMap;
use tracing::{info, error, warn, debug};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicI64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_request::TokenAccountsFilter;
use solana_sdk::program_pack::Pack;
use spl_token::state::Account as TokenAccount;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use solana_account_decoder::UiAccountData;
use tokio::sync::mpsc;

pub struct WebsocketMonitor {
    ws_url: String,
    token_holders_cache: Arc<DashMap<String, u64>>,
    rpc_client: Arc<RpcClient>,
    reconnect_attempts: AtomicU32,
    last_connection_time: AtomicI64,
}

impl WebsocketMonitor {
    pub fn new(ws_url: &str, rpc_client: Arc<RpcClient>) -> Result<Self> {
        Ok(Self {
            ws_url: ws_url.to_string(),
            token_holders_cache: Arc::new(DashMap::new()),
            rpc_client,
            reconnect_attempts: AtomicU32::new(0),
            last_connection_time: AtomicI64::new(0),
        })
    }

    pub async fn subscribe_to_logs(&self) -> Result<()> {
        loop {
            match self.connect_and_monitor().await {
                Ok(_) => {
                    info!("WebSocket connection closed gracefully");
                    self.reconnect_attempts.store(0, Ordering::SeqCst);
                }
                Err(e) => {
                    let attempts = self.reconnect_attempts.fetch_add(1, Ordering::SeqCst);
                    error!("WebSocket error (attempt {}): {}", attempts + 1, e);
                    
                    if attempts >= 5 {
                        return Err(anyhow!("Max reconnection attempts reached"));
                    }
                    
                    let delay = 5000 * (attempts as u64 + 1);
                    warn!("Reconnecting in {} ms...", delay);
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                }
            }
        }
    }

    async fn connect_and_monitor(&self) -> Result<()> {
        info!("Connecting to websocket...");
        
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        self.last_connection_time.store(now, Ordering::SeqCst);

        let (ws_stream, _) = connect_async(&self.ws_url).await?;
        let (write, mut read) = ws_stream.split();
        info!("WebSocket connected successfully");

        let subscribe_msg = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "logsSubscribe",
            "params": [
                {
                    "mentions": [
                        "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
                        "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
                    ]
                },
                {"commitment": "processed"}
            ]
        });

        // Create channel for ping task to communicate with write half
        let (tx, mut rx) = mpsc::channel(32);
        let mut write = write;

        // Send initial subscription
        write.send(tokio_tungstenite::tungstenite::Message::Text(subscribe_msg.to_string())).await?;

        // Spawn heartbeat task
        let ping_tx = tx.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                if ping_tx.send(tokio_tungstenite::tungstenite::Message::Ping(vec![])).await.is_err() {
                    error!("Failed to send ping message");
                    break;
                }
                debug!("Ping sent");
            }
        });

        // Spawn writer task
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if let Err(e) = write.send(msg).await {
                    error!("Failed to send websocket message: {}", e);
                    break;
                }
            }
        });

        // Handle incoming messages
        while let Some(msg) = read.next().await {
            match msg {
                Ok(msg) => {
                    match msg {
                        tokio_tungstenite::tungstenite::Message::Pong(_) => {
                            debug!("Received pong");
                            continue;
                        }
                        tokio_tungstenite::tungstenite::Message::Close(_) => {
                            info!("Received close frame");
                            break;
                        }
                        tokio_tungstenite::tungstenite::Message::Text(text) => {
                            if let Ok(log_data) = serde_json::from_str::<Value>(&text) {
                                if self.is_valid_pool_creation(&log_data) {
                                    if let Ok(token_address) = self.extract_token_address(&log_data) {
                                        if let Ok(holder_count) = self.get_holder_count(&token_address).await {
                                            info!("Valid pool creation detected with {} holders", holder_count);
                                            // TODO: Trigger buy transaction
                                        }
                                    }
                                }
                            }
                        }
                        _ => continue,
                    }
                }
                Err(e) => {
                    error!("WebSocket message error: {}", e);
                    return Err(anyhow!("WebSocket message error: {}", e));
                }
            }
        }

        Ok(())
    }

    fn is_valid_pool_creation(&self, _log_data: &Value) -> bool {
        // TODO: Implement actual pool creation detection logic
        false
    }

    fn extract_token_address(&self, _log_data: &Value) -> Result<String> {
        // TODO: Implement token address extraction logic
        Err(anyhow!("Not implemented"))
    }

    async fn get_holder_count(&self, token_address: &str) -> Result<u64> {
        if let Some(count) = self.token_holders_cache.get(token_address) {
            return Ok(*count);
        }

        let mut total_holders = 0;

        // Check regular SPL Token accounts
        if let Ok(accounts) = self.rpc_client.get_token_accounts_by_owner_with_commitment(
            &token_address.parse()?,
            TokenAccountsFilter::Mint(token_address.parse()?),
            self.rpc_client.commitment(),
        ) {
            total_holders += accounts.value.iter()
                .filter(|account| {
                    match &account.account.data {
                        UiAccountData::Binary(data, _) => {
                            if let Ok(decoded) = BASE64.decode(data) {
                                if let Ok(token_account) = TokenAccount::unpack(&decoded) {
                                    token_account.amount > 0
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        },
                        _ => false
                    }
                })
                .count();
        }

        // Check Token-2022 accounts
        if let Ok(accounts_2022) = self.rpc_client.get_token_accounts_by_owner_with_commitment(
            &token_address.parse()?,
            TokenAccountsFilter::ProgramId("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb".parse()?),
            self.rpc_client.commitment(),
        ) {
            total_holders += accounts_2022.value.iter()
                .filter(|account| {
                    match &account.account.data {
                        UiAccountData::Binary(data, _) => {
                            if let Ok(decoded) = BASE64.decode(data) {
                                if let Ok(token_account) = TokenAccount::unpack(&decoded) {
                                    token_account.amount > 0
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        },
                        _ => false
                    }
                })
                .count();
        }

        let total_holders = total_holders as u64;
        self.token_holders_cache.insert(token_address.to_string(), total_holders);
        
        info!("Token {} has {} total holders (SPL + Token-2022)", token_address, total_holders);
        Ok(total_holders)
    }
} 