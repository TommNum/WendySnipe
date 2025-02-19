use anyhow::Result;
use solana_sdk::signature::Keypair;
use std::fs::File;
use std::io::Read;
use serde_json;

pub fn load_wallet(path: &str) -> Result<Keypair> {
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    
    let keypair_bytes: Vec<u8> = serde_json::from_str(&contents)?;
    let keypair = Keypair::from_bytes(&keypair_bytes)?;
    
    Ok(keypair)
}