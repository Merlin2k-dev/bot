#[derive(Debug, Clone)]
pub struct Position {
    pub token: Pubkey,
    pub amount: u64,
    pub entry_price: f64,
    pub current_price: f64,
    pub pnl: f64,
    pub timestamp: Instant,
}

#[derive(Debug, Clone)]
pub struct TradeHistory {
    pub signature: String,
    pub token: Pubkey,
    pub trade_type: TradeType,
    pub amount: u64,
    pub price: f64,
    pub success: bool,
    pub error: Option<String>,
    pub timestamp: Instant,
}

// filepath: /src/trading/engine.rs
impl TradingEngine {
    // Position Management
    pub async fn get_active_positions(&self) -> Result<Vec<Position>> {
        let mut positions = Vec::new();
        for token in &self.tracked_tokens {
            let amount = self.get_token_balance(token).await?;
            if amount > 0 {
                let current_price = self.get_token_price(token).await?;
                positions.push(Position {
                    token: *token,
                    amount,
                    entry_price: self.get_entry_price(token)?,
                    current_price,
                    pnl: self.calculate_pnl(token)?,
                    timestamp: Instant::now(),
                });
            }
        }
        Ok(positions)
    }

    pub async fn manage_position(&self, token: &Pubkey, action: PositionAction) -> Result<()> {
        match action {
            PositionAction::Buy(amount) => {
                self.execute_privileged_swap(token, amount).await?;
            },
            PositionAction::SellPartial(percentage) => {
                let position = self.get_position(token).await?;
                let sell_amount = (position.amount as f64 * percentage) as u64;
                self.execute_sell(token, sell_amount).await?;
            },
            PositionAction::SellAll => {
                let position = self.get_position(token).await?;
                self.execute_sell(token, position.amount).await?;
            }
        }
        Ok(())
    }

    // Copy Trading Enhancement
    pub async fn copy_trade(&self, tx: &Transaction) -> Result<()> {
        let start = Instant::now();
        let result = self.execute_copy_trade(tx).await;
        
        // Record trade history
        let history = TradeHistory {
            signature: tx.signatures[0].to_string(),
            token: self.extract_token_from_tx(tx)?,
            trade_type: self.determine_trade_type(tx)?,
            amount: self.extract_amount_from_tx(tx)?,
            price: self.get_execution_price(tx)?,
            success: result.is_ok(),
            error: result.err().map(|e| e.to_string()),
            timestamp: start,
        };
        
        self.trade_history.push(history.clone());
        
        // Log errors for analysis
        if let Err(e) = &result {
            self.log_trade_error(e, tx).await?;
        }
        
        result
    }

    // Error Analysis
    async fn log_trade_error(&self, error: &Error, tx: &Transaction) -> Result<()> {
        let error_log = ErrorLog {
            timestamp: Instant::now(),
            error_type: error.to_string(),
            transaction: tx.clone(),
            context: self.get_market_context().await?,
        };
        
        self.error_logs.push(error_log);
        self.analyze_error_patterns()?;
        
        Ok(())
    }

    // Trade History Management
    pub fn get_trade_history(&self) -> Vec<TradeHistory> {
        self.trade_history.clone()
    }

    pub fn get_failed_trades(&self) -> Vec<TradeHistory> {
        self.trade_history.iter()
            .filter(|t| !t.success)
            .cloned()
            .collect()
    }
}