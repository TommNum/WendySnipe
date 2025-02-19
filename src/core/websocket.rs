use tokio_tungstenite::{connect_async, WebSocketStream, MaybeTlsStream};
use tokio::net::TcpStream;
use futures::{StreamExt, SinkExt};
use serde_json::{json, Value};
use anyhow::{Result, anyhow};
use dashmap::DashMap;
use tracing::{info, error};
use std::sync::Arc;
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_request::TokenAccountsFilter;
use solana_sdk::program_pack::Pack;
use spl_token::state::Account as TokenAccount;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use solana_account_decoder::UiAccountData;

pub struct WebsocketMonitor {
    ws_url: String,
    token_holders_cache: Arc<DashMap<String, u64>>,
    rpc_client: Arc<RpcClient>,
}

impl WebsocketMonitor {
    pub fn new(ws_url: &str, rpc_client: Arc<RpcClient>) -> Result<Self> {
        Ok(Self {
            ws_url: ws_url.to_string(),
            token_holders_cache: Arc::new(DashMap::new()),
            rpc_client,
        })
    }

    pub async fn subscribe_to_logs(&self) -> Result<()> {
        info!("Connecting to websocket...");
        
        let (ws_stream, _) = connect_async(&self.ws_url).await?;
        info!("Websocket connected");

        let subscribe_msg = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "logsSubscribe",
            "params": [
                {
                    "mentions": [
                        "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
                        "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"  // Token-2022 Program
                    ]
                },
                {"commitment": "processed"}
            ]
        });

        self.process_logs(ws_stream, subscribe_msg).await
    }

    async fn process_logs(
        &self,
        mut ws_stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
        subscribe_msg: Value,
    ) -> Result<()> {
        ws_stream.send(tokio_tungstenite::tungstenite::Message::Text(subscribe_msg.to_string())).await?;
        
        while let Some(msg) = ws_stream.next().await {
            match msg {
                Ok(msg) => {
                    if let Ok(log_data) = serde_json::from_str::<Value>(&msg.to_string()) {
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
                Err(e) => {
                    error!("WebSocket error: {:?}", e);
                    return Err(anyhow!("WebSocket connection error: {:?}", e));
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