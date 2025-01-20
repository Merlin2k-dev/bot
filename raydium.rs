use {
    solana_client::rpc_client::RpcClient,
    solana_sdk::{
        pubkey::Pubkey,
        signature::{Keypair, Signature},
        transaction::Transaction,
        sysvar::rent::Rent,
    },
    anyhow::{Result, anyhow},
    raydium_contract_instructions::amm_instruction,
    serde::{Deserialize, Serialize},
    std::collections::HashMap,
    tokio::time::{Duration, Instant},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct PoolInfo {
    pub liquidity: u64,
    pub base_amount: u64,
    pub quote_amount: u64,
    pub fee_numerator: u64,
    pub fee_denominator: u64,
}

#[derive(Debug, Clone)]
pub struct PoolState {
    pub info: PoolInfo,
    pub last_update: Instant,
    pub price_history: Vec<(Instant, f64)>,
}

#[derive(Debug)]
pub struct TradeSignal {
    pub direction: TradeDirection,
    pub price_change: f64,
    pub volume_change: f64,
    pub confidence: f64,
    pub timestamp: Instant,
}

#[derive(Debug)]
pub enum TradeDirection {
    Buy,
    Sell,
}

pub struct RaydiumDex {
    rpc_client: RpcClient,
    amm_program_id: Pubkey,
    min_liquidity: u64,
    max_slippage: f64,
    payer: Keypair,
    pools: HashMap<Pubkey, PoolState>,
    update_interval: Duration,
}

impl RaydiumDex {
    pub fn new(config: &Config) -> Self {
        Self {
            rpc_client: RpcClient::new(config.rpc_url.clone()),
            amm_program_id: "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8"
                .parse()
                .unwrap(),
            min_liquidity: config.min_liquidity,
            max_slippage: config.max_slippage,
            payer: config.payer.clone(),
            pools: HashMap::new(),
            update_interval: Duration::from_secs(1),
        }
    }

    pub async fn get_pool_info(&self, pool_id: &Pubkey) -> Result<PoolInfo> {
        let account = self.rpc_client.get_account(pool_id)?;
        let pool_info = PoolInfo::deserialize(&account.data)?;
        Ok(pool_info)
    }

    pub async fn validate_liquidity(&self, token: &Pubkey) -> Result<bool> {
        let pool = self.get_pool_info(token).await?;
        Ok(pool.liquidity > self.min_liquidity)
    }

    pub async fn execute_swap(
        &self,
        pool_id: &Pubkey,
        amount_in: u64,
        min_amount_out: u64,
    ) -> Result<Signature> {
        let pool = self.get_pool_info(pool_id).await?;
        
        // Calculate price impact
        let price_impact = self.calculate_price_impact(&pool, amount_in)?;
        if price_impact > self.max_slippage {
            return Err(anyhow!("Price impact too high: {}", price_impact));
        }

        let swap_ix = amm_instruction::swap(
            &self.amm_program_id,
            pool_id,
            amount_in,
            min_amount_out,
        )?;

        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        let tx = Transaction::new_signed_with_payer(
            &[swap_ix],
            Some(&self.payer.pubkey()),
            &[&self.payer],
            recent_blockhash,
        );

        self.rpc_client.send_and_confirm_transaction(&tx)
            .map_err(|e| anyhow!("Swap failed: {}", e))
    }

    fn calculate_price_impact(&self, pool: &PoolInfo, amount_in: u64) -> Result<f64> {
        let price_before = pool.quote_amount as f64 / pool.base_amount as f64;
        let new_base = pool.base_amount + amount_in;
        let new_quote = (pool.base_amount * pool.quote_amount) / new_base;
        let price_after = new_quote as f64 / new_base as f64;
        
        Ok((price_before - price_after).abs() / price_before)
    }

    pub async fn update_pool(&mut self, pool_id: &Pubkey) -> Result<()> {
        let pool_info = self.fetch_pool_info(pool_id).await?;
        let price = self.calculate_price(&pool_info)?;
        
        let state = self.pools.entry(*pool_id).or_insert(PoolState {
            info: pool_info.clone(),
            last_update: Instant::now(),
            price_history: Vec::new(),
        });
        
        state.info = pool_info;
        state.last_update = Instant::now();
        state.price_history.push((Instant::now(), price));
        
        // Keep last 24h of price history
        state.price_history.retain(|(time, _)| 
            time.elapsed() < Duration::from_secs(24 * 60 * 60)
        );
        
        Ok(())
    }

    async fn fetch_pool_info(&self, pool_id: &Pubkey) -> Result<PoolInfo> {
        let account = self.rpc_client.get_account(pool_id)?;
        PoolInfo::try_from_slice(&account.data)
            .map_err(|e| anyhow!("Failed to deserialize pool info: {}", e))
    }

    fn calculate_price(&self, pool: &PoolInfo) -> Result<f64> {
        Ok(pool.quote_amount as f64 / pool.base_amount as f64)
    }

    pub async fn monitor_pool(&mut self, pool_id: &Pubkey) -> Result<()> {
        loop {
            let current_state = self.update_pool_state(pool_id).await?;
            
            if let Some(signal) = self.analyze_pool_state(&current_state).await? {
                if self.validate_trade_conditions(pool_id, &signal).await? {
                    self.execute_trade(pool_id, &signal).await?;
                }
            }
            
            tokio::time::sleep(self.update_interval).await;
        }
    }

    async fn update_pool_state(&mut self, pool_id: &Pubkey) -> Result<PoolState> {
        let info = self.fetch_pool_info(pool_id).await?;
        let price = self.calculate_current_price(&info);
        let state = PoolState {
            info,
            last_update: Instant::now(),
            price_history: vec![(Instant::now(), price)],
        };
        
        self.pools.insert(*pool_id, state.clone());
        Ok(state)
    }

    async fn analyze_pool_state(&self, state: &PoolState) -> Result<Option<TradeSignal>> {
        let price_change = self.calculate_price_change(&state.price_history)?;
        let volume = self.calculate_volume(&state.info)?;
        
        if self.should_trade(price_change, volume) {
            Ok(Some(TradeSignal::new(price_change, volume)))
        } else {
            Ok(None)
        }
    }

    fn calculate_price_change(&self, price_history: &[(Instant, f64)]) -> Result<f64> {
        if price_history.len() < 2 {
            return Ok(0.0);
        }
        
        let (_, current_price) = price_history.last()
            .ok_or_else(|| anyhow!("No price data"))?;
        let (_, previous_price) = price_history.first()
            .ok_or_else(|| anyhow!("No previous price"))?;
            
        Ok((current_price - previous_price) / previous_price)
    }

    fn calculate_volume(&self, pool_info: &PoolInfo) -> Result<f64> {
        Ok((pool_info.base_amount as f64) * 
           (pool_info.quote_amount as f64) / 
           (pool_info.liquidity as f64))
    }

    fn should_trade(&self, price_change: f64, volume: f64) -> bool {
        let significant_price_change = price_change.abs() > 0.02; // 2%
        let sufficient_volume = volume > self.min_liquidity as f64;
        
        significant_price_change && sufficient_volume
    }

    async fn validate_trade_conditions(
        &self, 
        pool_id: &Pubkey, 
        signal: &TradeSignal
    ) -> Result<bool> {
        let pool_state = self.pools.get(pool_id)
            .ok_or_else(|| anyhow!("Pool not found"))?;
            
        // Validate liquidity
        if pool_state.info.liquidity < self.min_liquidity {
            return Ok(false);
        }
        
        // Check signal freshness
        if signal.timestamp.elapsed() > Duration::from_secs(30) {
            return Ok(false);
        }
        
        // Validate confidence
        if signal.confidence < 0.7 {
            return Ok(false);
        }
        
        Ok(true)
    }
}