use {
    inquire::{Select, Confirm, Text},
    colored::*,
    std::fmt,
    solana_sdk::{pubkey::Pubkey, signature::Keypair, transaction::Transaction},
};

impl BotUI {
    pub fn new(wallet: Keypair, config: Config) -> Self {
        Self {
            wallet,
            config,
            running: false
        }
    }

    pub async fn show_main_menu(&mut self) -> Result<()> {
        println!("{}", "=== Solana Copy Trading Bot ===".bright_green());
        
        // Display wallet and status info
        println!("\nWallet: {}", self.wallet.pubkey());
        
        if self.running {
            println!("Copy Trading: ACTIVE");
            println!("Target Wallet: {}", self.config.target_wallet);
        } else {
            println!("Copy Trading: INACTIVE");
        }
        
        if self.config.fixed_amount > 0.0 {
            println!("Fixed Trading Amount: {} SOL", self.config.fixed_amount);
        }
        
        println!("\n");

        loop {
            let choices = vec![
                "💼 Wallet Info",
                "💰 Check Balance",
                "🎯 Manual Trading",
                "▶️ Start Copy Trading",
                "⚙️ Settings",
                "🚪 Exit"
            ];

            let selection = Select::new("Select an option:", choices).prompt()?;

            match selection {
                "💼 Wallet Info" => self.show_wallet_info().await?,
                "💰 Check Balance" => self.show_balance().await?,
                "🎯 Manual Trading" => self.show_manual_trading_menu().await?,
                "▶️ Start Copy Trading" => self.start_bot().await?,
                "⚙️ Settings" => self.show_settings().await?,
                "🚪 Exit" => break,
                _ => println!("Invalid option")
            }
        }
        Ok(())
    }

    pub async fn show_settings(&mut self) -> Result<()> {
        loop {
            let settings = vec![
                "Set Fixed Trading Amount",
                "Target Wallet",
                "RPC URL",
                "Slippage %",
                "Back"
            ];

            let selection = Select::new("Settings:", settings).prompt()?;
            
            match selection {
                "Set Fixed Trading Amount" => {
                    let amount = Text::new("Enter fixed trading amount (SOL):").prompt()?;
                    self.config.fixed_amount = amount.parse::<f64>()?;
                    println!("Fixed trading amount set to: {} SOL", self.config.fixed_amount);
                },
                "Target Wallet" => {
                    let wallet = Text::new("Enter target wallet:").prompt()?;
                    self.config.target_wallet = Pubkey::from_str(&wallet)?;
                },
                "Back" => break,
                _ => println!("Setting: {}", selection)
            }
        }
        Ok(())
    }

    // Add debug logging
    pub async fn start_bot(&mut self) -> Result<()> {
        println!("Starting bot with configuration:");
        println!("RPC URL: {}", self.config.rpc_url);
        println!("Target Wallet: {}", self.config.target_wallet);
        println!("Fixed Amount: {} SOL", self.config.fixed_amount);
        
        self.test_rpc_connection().await?;
        self.verify_wallet_balance().await?;
        
        self.running = true;
        Ok(())
    }

    async fn test_rpc_connection(&self) -> Result<()> {
        self.rpc_client
            .get_latest_blockhash()
            .map_err(|e| anyhow!("RPC connection failed: {}", e))?;
        println!("RPC connection verified");
        Ok(())
    }

    async fn verify_wallet_balance(&self) -> Result<()> {
        let balance = self.rpc_client
            .get_balance(&self.wallet.pubkey())
            .await?;
        println!("Wallet balance: {} SOL", balance as f64 / 1e9);
        Ok(())
    }

    async fn show_manual_trading_menu(&mut self) -> Result<()> {
        loop {
            println!("\n=== Manual Trading ===");
            let action = Select::new("Select action:", vec![
                "Buy Token",
                "Sell Token",
                "Back"
            ]).prompt()?;

            match action {
                "Buy Token" => {
                    let address = Text::new("Enter token address:").prompt()?;
                    
                    self.execute_manual_buy(
                        Pubkey::from_str(&address)?,
                    ).await?;
                },
                "Back" => break,
                _ => println!("Invalid option")
            }
        }
        Ok(())
    }

    async fn execute_manual_buy(&self, token: Pubkey) -> Result<()> {
        let amount = if self.config.fixed_amount > 0.0 {
            self.config.fixed_amount
        } else {
            let input = Text::new("Enter amount (SOL):").prompt()?;
            input.parse::<f64>()?
        };

        self.execute_trade(token, amount).await
    }

    async fn execute_direct_swap(&self, token: Pubkey, amount: f64) -> Result<()> {
        let ix = self.engine.create_privileged_swap(
            &token,
            amount_to_lamports(amount),
            true // bypass checks flag
        )?;

        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        
        let tx = Transaction::new_signed_with_payer(
            &[
                ComputeBudgetInstruction::set_compute_unit_limit(1_400_000),
                ComputeBudgetInstruction::set_compute_unit_price(
                    self.engine.calculate_optimal_priority_fee()
                ),
                ix
            ],
            Some(&self.wallet.pubkey()),
            &[&self.wallet],
            recent_blockhash
        );

        self.rpc_client.send_transaction_with_config(
            &tx,
            RpcSendTransactionConfig {
                skip_preflight: true,
                preflight_commitment: None,
                encoding: None,
                max_retries: Some(0),
                min_context_slot: None,
            },
        )?;

        Ok(())
    }

    pub async fn show_positions_menu(&mut self) -> Result<()> {
        loop {
            let positions = self.engine.get_active_positions().await?;
            
            println!("\n=== Active Positions ===");
            for pos in &positions {
                println!(
                    "Token: {} | Amount: {} | Entry: ${:.2} | Current: ${:.2} | PnL: ${:.2}",
                    pos.token, pos.amount, pos.entry_price, pos.current_price, pos.pnl
                );
            }

            let choices = vec![
                "Buy More",
                "Sell Partial",
                "Sell All", 
                "Back"
            ];

            match Select::new("Select action:", choices).prompt()? {
                "Buy More" => {
                    let token = Select::new(
                        "Select token:", 
                        positions.iter().map(|p| p.token).collect()
                    ).prompt()?;
                    
                    let amount = if self.config.fixed_amount > 0.0 {
                        self.config.fixed_amount
                    } else {
                        Text::new("Enter amount:").prompt()?.parse()?
                    };

                    self.engine.execute_privileged_swap(&token, amount_to_lamports(amount)).await?;
                },
                "Sell Partial" => {
                    let token = Select::new(
                        "Select token:",
                        positions.iter().map(|p| p.token).collect()
                    ).prompt()?;
                    
                    let percentage = Text::new("Enter percentage to sell (1-100):").prompt()?.parse::<f64>()? / 100.0;
                    
                    self.engine.manage_position(
                        &token,
                        PositionAction::SellPartial(percentage)
                    ).await?;
                },
                "Sell All" => {
                    let token = Select::new(
                        "Select token:",
                        positions.iter().map(|p| p.token).collect()
                    ).prompt()?;
                    
                    self.engine.manage_position(
                        &token,
                        PositionAction::SellAll
                    ).await?;
                },
                "Back" => break,
                _ => continue,
            }
        }
        Ok(())
    }

    pub async fn show_trade_history(&self) -> Result<()> {
        let history = self.engine.get_trade_history();
        
        println!("\n=== Trade History ===");
        for trade in history {
            let status = if trade.success { "✅" } else { "❌" };
            println!(
                "{} {} | {} | Amount: {} | Price: ${:.2} | {}",
                status,
                trade.timestamp.elapsed().as_secs(),
                trade.trade_type,
                trade.amount,
                trade.price,
                trade.error.unwrap_or_default()
            );
        }
        Ok(())
    }
}

fn amount_to_lamports(amount: f64) -> u64 {
    (amount * 1_000_000_000.0) as u64
}