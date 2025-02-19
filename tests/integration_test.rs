#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_wallet_loading() {
        let wallet = utils::wallet::load_wallet("dev_wallet.json").unwrap();
        assert!(wallet.pubkey().to_string().len() > 0);
    }
}