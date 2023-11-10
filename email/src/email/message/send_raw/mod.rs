use async_trait::async_trait;

use crate::Result;

#[cfg(feature = "smtp-sender")]
pub mod smtp;

#[async_trait]
pub trait SendRawMessage: Send + Sync {
    /// Send the given raw email message.
    async fn send_raw_message(&self, raw_msg: &[u8]) -> Result<()>;
}