
use std::collections::HashMap;

use acceptor_connect::extract::{online_state::AuthServer, IntoPacket, sync_message::RequestIdPacket};
use async_trait::async_trait;
use bytes::{BytesMut, Buf};
use crypto::{aes, blockmodes, buffer, symmetriccipher};

use crate::packet::{Packet, Payload};

static KEY: [u8; 16] = [
    0x0, 0x1, 0x2, 0x3, 0x4, 0x5, 0xC, 0x7, 0xB, 0x9, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15,
];
static IV: [u8; 16] = [
    0x0, 0x1, 0x2, 0x3, 0x4, 0x5, 0xC, 0x7, 0xB, 0x9, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15,
];


pub enum AuthError {
    // request_id
    InvalidToken{request_id: u64},
}

#[derive(Clone)]
pub struct TokenAuthServer {

}

impl TokenAuthServer {
    pub fn new() -> Self {
        TokenAuthServer {}
    }
    fn parse_token(request_id: u64, token: &str) -> Result<u64, AuthError> {
        let bytes = base64::decode(token)
            .map_err(|_| AuthError::InvalidToken{request_id})?;
        let decrypt_data = Self::decrypt(bytes.as_slice())
            .map_err(|_|AuthError::InvalidToken{request_id})?;   
        
        let mut bytes = BytesMut::with_capacity(12);
        bytes.extend_from_slice(decrypt_data.as_slice());
        Ok(bytes.get_u64())
    }

    
    fn decrypt(encrypted_data: &[u8]) -> Result<Vec<u8>, symmetriccipher::SymmetricCipherError> {
        let mut decryptor = aes::cbc_decryptor(
            aes::KeySize::KeySize128,
            &KEY[..],
            &IV[..],
            blockmodes::PkcsPadding,
        );

        let mut read_buffer = buffer::RefReadBuffer::new(encrypted_data);
        let mut buffer = vec![0; encrypted_data.len()];
        let mut write_buffer = buffer::RefWriteBuffer::new(&mut buffer);
        decryptor.decrypt(&mut read_buffer, &mut write_buffer, true)?;
        Ok(buffer)
    }

}

#[async_trait]
impl AuthServer<Packet, u64> for TokenAuthServer {
    type Error = AuthError;
  
    async fn auth(&self, packet: &Packet) -> Result<u64, Self::Error> {
        let request_id = packet.request_id();
        match &packet.payload {
            Payload::Login{token, ..} => {
                // TODO check token is still in redis
                Self::parse_token(request_id, token.as_str())
            },
            _ => unreachable!()
        }
  }
}

impl IntoPacket<Packet> for AuthError {
    fn into_packet(self) -> Packet {
        match self {
            AuthError::InvalidToken{request_id} => {
                //todo code
                Packet::new(request_id, HashMap::new(), Payload::Error{code: 32, message: "invalid token".into()})
            }
        }
    }
} 