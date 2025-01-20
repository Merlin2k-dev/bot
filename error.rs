use {
    solana_client::client_error::ClientError,
    solana_sdk::transaction::TransactionError,
    thiserror::Error,
    std::time::Duration,
};

#[derive(Error, Debug)]
pub enum BotError {
    #[error("RPC error: {0}")]
    RPCError(String),

    #[error("Transaction failed: {0}")]
    TransactionError(String),

    #[error("Pre-liquidity error: {0}")]
    PreLiquidityError(String),

    #[error("Privileged operation failed: {0}")]
    PrivilegeError(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Insufficient funds: {0}")]
    InsufficientFunds(String),

    #[error("Slippage error: {0}")]
    SlippageError(String),

    #[error("Trading error: {0}")]
    TradingError(String),
}

impl From<ClientError> for BotError {
    fn from(error: ClientError) -> Self {
        match error {
            ClientError::TransactionError(TransactionError::InsufficientFunds) => {
                BotError::InsufficientFunds("Not enough funds for transaction".into())
            }
            ClientError::RpcError(_) => {
                BotError::NetworkError("RPC connection failed".into())
            }
            _ => BotError::TransactionError(format!("Transaction failed: {}", error))
        }
    }
}

pub trait ErrorHandler {
    fn handle_error(&self, error: &BotError) -> bool;
    fn get_retry_delay(&self, retries: u32) -> Duration;
    fn should_escalate(&self, error: &BotError) -> bool;
}

impl ErrorHandler for crate::trading::TradingEngine {
    fn handle_error(&self, error: &BotError) -> bool {
        match error {
            BotError::NetworkError(_) | BotError::RPCError(_) => true,
            BotError::PreLiquidityError(_) | BotError::PrivilegeError(_) => true,
            BotError::InsufficientFunds(_) => false,
            _ => false
        }
    }

    fn get_retry_delay(&self, retries: u32) -> Duration {
        Duration::from_millis(50 * 2u64.pow(retries))
    }

    fn should_escalate(&self, error: &BotError) -> bool {
        matches!(error, 
            BotError::PreLiquidityError(_) | 
            BotError::PrivilegeError(_)
        )
    }
}