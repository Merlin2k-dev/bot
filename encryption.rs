use {
    aes_gcm::{
        aead::{Aead, KeyInit},
        Aes256Gcm, Nonce,
    },
    rand::Rng,
    std::env,
};

pub struct Security {
    cipher: Aes256Gcm,
}

impl Security {
    pub fn new() -> Result<Self> {
        // Generate random key or get from secure environment
        let key = env::var("ENCRYPTION_KEY")
            .unwrap_or_else(|_| generate_secure_key());
            
        let cipher = Aes256Gcm::new_from_slice(key.as_bytes())?;
        
        Ok(Self { cipher })
    }

    pub fn encrypt_sensitive_data(&self, data: &[u8]) -> Result<Vec<u8>> {
        let nonce = Nonce::from_slice(b"unique nonce"); // Use random nonce in production
        self.cipher
            .encrypt(nonce, data)
            .map_err(|e| anyhow!("Encryption failed: {}", e))
    }

    pub fn decrypt_sensitive_data(&self, encrypted: &[u8]) -> Result<Vec<u8>> {
        let nonce = Nonce::from_slice(b"unique nonce"); 
        self.cipher
            .decrypt(nonce, encrypted)
            .map_err(|e| anyhow!("Decryption failed: {}", e))
    }
}