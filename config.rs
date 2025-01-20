use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

#[derive(Debug, Serialize, Deserialize)]
pub struct TradingConfig {
    pub rpc_url: String,
    pub ws_url: String,
    pub wallet_path: String,
    pub quote_token: String,
    pub min_liquidity: f64,
    pub max_position_size: f64,
    pub risk_percentage: f64,
    pub profit_target: f64,
    pub stop_loss: f64,
}

impl Default for TradingConfig {
    fn default() -> Self {
        Self {
            rpc_url: "https://api.mainnet-beta.solana.com".to_string(),
            ws_url: "wss://api.mainnet-beta.solana.com".to_string(),
            wallet_path: "wallet.json".to_string(),
            quote_token: "SOL".to_string(),
            min_liquidity: 1000.0,
            max_position_size: 1.0,
            risk_percentage: 1.0,
            profit_target: 2.0,
            stop_loss: 0.5,
        }
    }
}