pub const RAYDIUM_V4_PROGRAM_ID: &str = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8";

pub fn get_raydium_program_id() -> Pubkey {
    Pubkey::from_str(RAYDIUM_V4_PROGRAM_ID).unwrap()
}

// Dex module placeholder
pub struct Dex;

impl Dex {
    pub fn new() -> Self {
        Dex
    }
}