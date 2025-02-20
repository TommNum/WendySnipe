use {
    anyhow::{Result, anyhow},
    futures::{SinkExt, StreamExt},
    serde_json::{json, Value},
    std::collections::HashMap,
    tokio_tungstenite::{connect_async, WebSocketStream},
    tracing::{info, error, warn, debug},
    chrono::Utc,
    solana_sdk::pubkey::Pubkey,
    crate::config::{Config, Environment},
};

#[derive(Debug, Clone)]
pub enum PoolType {
    PumpFun,  // Development
    DaoFun    // Production
}

#[derive(Debug)]
pub struct PoolCreationEvent {
    pub signature: String,
    pub pool_address: Pubkey,
    pub token_address: Pubkey,
    pub holder_count: u64,
    pub buy_count: u64,
    pub timestamp: i64,
    pub slot: u64,
    pub pool_type: PoolType,
}

#[derive(Debug)]
pub struct PumpFunCriteria {
    pub holder_count: u64,
    pub buy_count: u64,
    min_holders: u64,
    min_buys: u64,
    max_buys: u64,
}

impl Default for PumpFunCriteria {
    fn default() -> Self {
        Self {
            holder_count: 0,
            buy_count: 0,
            min_holders: 140,
            min_buys: 140,
            max_buys: 300,
        }
    }
}

pub struct WebsocketMonitor {
    ws_url: String,
    config: Config,
    token_metrics: HashMap<String, PumpFunCriteria>,
}

impl WebsocketMonitor {
    pub fn new(ws_url: &str, config: &Config) -> Result<Self> {
        Ok(Self {
            ws_url: ws_url.to_string(),
            config: config.clone(),
            token_metrics: HashMap::new(),
        })
    }

    pub async fn subscribe_to_logs(&self) -> Result<()> {
        info!("Connecting to websocket...");
        
        let (ws_stream, _) = connect_async(&self.ws_url).await?;
        info!("WebSocket connected successfully");

        let subscribe_msg = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "logsSubscribe",
            "params": [
                {
                    "mentions": [
                        "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA", // pump.fun
                        "5jnapfrAN47UYkLkEf7HnprPPBCQLvkYWGZDeKkaP5hv", // dao.fun
                        "CreateIdempotent"
                    ]
                },
                {"commitment": "processed"}
            ]
        });

        self.process_logs(ws_stream, subscribe_msg).await
    }

    async fn process_logs(&self, mut ws_stream: WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, subscribe_msg: Value) -> Result<()> {
        ws_stream.send(tokio_tungstenite::tungstenite::Message::Text(subscribe_msg.to_string())).await?;
        
        while let Some(msg) = ws_stream.next().await {
            match msg {
                Ok(msg) => {
                    if let Ok(log_data) = serde_json::from_str::<Value>(&msg.to_string()) {
                        if let Some(pool_type) = self.is_create_idempotent(&log_data) {
                            match (pool_type, &self.config.environment.environment) {
                                (PoolType::PumpFun, Environment::Development) => {
                                    info!("Detected pump.fun pool creation in development");
                                    self.handle_pump_fun_creation(&log_data).await?;
                                },
                                (PoolType::DaoFun, Environment::Production) => {
                                    info!("Detected dao.fun pool creation in production");
                                    self.handle_dao_fun_creation(&log_data).await?;
                                },
                                _ => {
                                    debug!("Ignoring pool creation - environment mismatch");
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("WebSocket message error: {:?}", e);
                    return Err(anyhow!("WebSocket error: {:?}", e));
                }
            }
        }
        Ok(())
    }

    fn is_create_idempotent(&self, log_data: &Value) -> Option<PoolType> {
        if let Some(logs) = log_data.get("result").and_then(|r| r.get("logs")) {
            logs.as_array().map_or(None, |log_array| {
                for log in log_array {
                    if let Some(log_str) = log.as_str() {
                        if log_str.contains("CreateIdempotent") {
                            if log_str.contains("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA") {
                                return Some(PoolType::PumpFun);
                            }
                            if log_str.contains("5jnapfrAN47UYkLkEf7HnprPPBCQLvkYWGZDeKkaP5hv") {
                                return Some(PoolType::DaoFun);
                            }
                        }
                    }
                }
                None
            })
        } else {
            None
        }
    }

    async fn handle_pump_fun_creation(&self, log_data: &Value) -> Result<()> {
        if let Some(event) = self.extract_pool_creation_event(&log_data, PoolType::PumpFun).await? {
            let criteria = self.verify_pump_fun_criteria(&event.token_address).await?;
            
            self.log_criteria_check(&event.token_address, &criteria);
            
            if self.is_valid_pump_fun_criteria(&criteria) {
                info!("Valid pump.fun pool creation detected");
                self.handle_valid_pool_creation(event).await?;
            }
        }
        Ok(())
    }

    async fn handle_dao_fun_creation(&self, log_data: &Value) -> Result<()> {
        if let Some(event) = self.extract_pool_creation_event(&log_data, PoolType::DaoFun).await? {
            info!("Valid dao.fun pool creation detected: {}", event.token_address);
            self.handle_valid_pool_creation(event).await?;
        }
        Ok(())
    }

    async fn verify_pump_fun_criteria(&self, token_address: &Pubkey) -> Result<PumpFunCriteria> {
        let holder_count = self.get_holder_count(token_address).await?;
        let buy_count = self.get_buy_count(token_address).await?;

        Ok(PumpFunCriteria {
            holder_count,
            buy_count,
            ..Default::default()
        })
    }

    fn is_valid_pump_fun_criteria(&self, criteria: &PumpFunCriteria) -> bool {
        criteria.holder_count >= criteria.min_holders && 
        criteria.buy_count >= criteria.min_buys && 
        criteria.buy_count <= criteria.max_buys
    }

    fn log_criteria_check(&self, token_address: &Pubkey, criteria: &PumpFunCriteria) {
        let holder_check = criteria.holder_count >= criteria.min_holders;
        let buy_check = criteria.buy_count >= criteria.min_buys && criteria.buy_count <= criteria.max_buys;

        info!("Token {} validation:
            Holders: {} (min: {}) - {}, 
            Buys: {} (min: {}, max: {}) - {}",
            token_address,
            criteria.holder_count,
            criteria.min_holders,
            if holder_check { "✅" } else { "❌" },
            criteria.buy_count,
            criteria.min_buys,
            criteria.max_buys,
            if buy_check { "✅" } else { "❌" }
        );

        if !holder_check || !buy_check {
            warn!("Pool creation ignored for token {}: 
                Insufficient holders ({} < {}) or 
                Invalid buy count ({} not between {} and {})",
                token_address,
                criteria.holder_count,
                criteria.min_holders,
                criteria.buy_count,
                criteria.min_buys,
                criteria.max_buys
            );
        }
    }

    async fn extract_pool_creation_event(&self, log_data: &Value, pool_type: PoolType) -> Result<Option<PoolCreationEvent>> {
        // Extract signature
        let signature = log_data.get("result")
            .and_then(|r| r.get("signature"))
            .and_then(|s| s.as_str())
            .ok_or_else(|| anyhow!("Missing signature"))?
            .to_string();

        // Extract other fields (simplified for example)
        let pool_address = Pubkey::new_unique(); // TODO: Extract from logs
        let token_address = Pubkey::new_unique(); // TODO: Extract from logs
        let slot = log_data.get("result")
            .and_then(|r| r.get("slot"))
            .and_then(|s| s.as_u64())
            .ok_or_else(|| anyhow!("Missing slot"))?;

        let timestamp = chrono::Utc::now().timestamp();

        Ok(Some(PoolCreationEvent {
            signature,
            pool_address,
            token_address,
            holder_count: 0,
            buy_count: 0,
            timestamp,
            slot,
            pool_type,
        }))
    }

    async fn get_holder_count(&self, _token_address: &Pubkey) -> Result<u64> {
        // TODO: Implement actual API call to get holder count
        Ok(150) // Placeholder
    }

    async fn get_buy_count(&self, _token_address: &Pubkey) -> Result<u64> {
        // TODO: Implement actual API call to get buy count
        Ok(200) // Placeholder
    }

    async fn handle_valid_pool_creation(&self, event: PoolCreationEvent) -> Result<()> {
        info!("Processing valid pool creation: {:?}", event);
        // TODO: Implement transaction execution
        Ok(())
    }
}