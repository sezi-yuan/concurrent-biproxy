use acceptor_connect::Connection;
use async_trait::async_trait;
use bytes::{BytesMut, Buf};
use tokio::{net::TcpStream, io::{AsyncReadExt, AsyncWriteExt}};

use crate::packet::Packet;


pub struct TcpConnection {
    stream: TcpStream,
    buffer: BytesMut
}

impl TcpConnection {
    pub fn new(stream: TcpStream, capacity: usize) -> Self {
        Self {
            stream,
            buffer: BytesMut::with_capacity(capacity)
        }
    }
}

#[async_trait]
impl Connection for TcpConnection {
    type Item = Packet;

    async fn read_buf(&mut self) -> acceptor_connect::Result<&[u8]> {
        match self.stream.read_buf(&mut self.buffer).await {
            Ok(size) => {
                if size == 0 {
                    println!("can not read more bytes; connection closed; buffer size: {}:{}", self.buffer.len(), self.buffer.capacity());
                    Err(acceptor_connect::Error::Disconnected)
                } else {
                    Ok(&self.buffer[..])
                }
            },
            Err(err) => {
                tracing::debug!("stream closed: {}", err);
                Err(acceptor_connect::Error::Disconnected)    
            }
        }
    }

    async fn write_buf(&mut self, data: &[u8]) -> acceptor_connect::Result<()> {
        self.stream.write_all(data)
            .await
            .map_err(|error| {
                tracing::debug!("stream closed: {}", error);
                acceptor_connect::Error::Disconnected
            })
    }

    async fn advance(&mut self, len: usize) {
        self.buffer.advance(len)
    }
}