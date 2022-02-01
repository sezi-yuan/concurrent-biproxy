use async_trait::async_trait;
use bytes::Buf;
use serde::{Serialize, de::DeserializeOwned};
use super::error::Result;

#[async_trait]
pub trait Connection: Send {
    type Item: DeserializeOwned + Serialize + Send + 'static;

    async fn advance(&mut self, len: usize);
    /// await for more bytes readed, and return all bytes in buffer
    /// if return empty buffer means that client is disconnected
    async fn read_buf(&mut self) -> Result<&[u8]>;

    /// caller should loop to call next to get stream message
    /// return Ok(Some(T)) if there is a new message coming
    /// return Ok(None) if need more bytes
    async fn next(&mut self) -> Result<Self::Item> {
        loop {
            let mut buffer = self.read_buf().await?;
            if buffer.len() <= 4 {
                continue
            }

            let total_len = buffer.get_u32() as usize;
            if buffer.len() < total_len + 4 {
                continue
            }

            let input = &buffer[..total_len];
            tracing::debug!("total: {}; input: {:?}", total_len, input);
            let res = acceptor_serde::from_bytes::<Self::Item>(input)?;
            self.advance(4 + total_len).await;
            return Ok(res)
        }
    }

    async fn write_buf(&mut self, data: &[u8]) -> Result<()>;

    async fn write(&mut self, val: Self::Item) -> Result<()> {
        let bytes = acceptor_serde::to_bytes(&val)?;
        let len_bytes = (bytes.len() as u32).to_be_bytes();
        let len_bytest = (bytes.len() as u32).to_le_bytes();
        println!("len_bytes: {:?}; le_bytes: {:?}", len_bytes, len_bytest);
        self.write_buf(&len_bytes).await?;
        self.write_buf(bytes.as_slice()).await
    }

}