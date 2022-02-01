use std::collections::HashMap;

use serde::{Serialize, Deserialize};
use acceptor_connect::extract::{sync_message::RequestIdPacket, online_state::LoginPacket, heart_beat::PingPongPacket};

/// this message come from im_server using grpc
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Message {
    pub from: u64,
    pub to: u64,
    pub timestamp: u64,
    pub seq: u32,
    pub msg_type: String,
    pub content: String
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "tag", content = "content")]
pub enum Payload {
    Error {
        code: u32,
        message: String,
    },
    Login{
        token: String,
        version: String,
        user_agent: String,
        timestamp: u64
    },
    Logout,
    Ping(u64),
    Pong(u64),
    Message(Message),
    MessageCliAck,
    MessageSrvAck
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Packet {
    pub request_id: u64,
    pub headers: HashMap<String, String>,
    pub payload: Payload
}

impl Packet {
    pub fn new(request_id: u64, headers: HashMap<String, String>, payload: Payload) -> Self {
        Self {
            request_id,
            headers,
            payload
        }
    }
}

impl RequestIdPacket for Packet {
    type ID = u64;

    fn request_id(&self) -> Self::ID {
        self.request_id
    }
}

impl LoginPacket for Packet {
    fn login_packet(&self) -> bool {
        matches!(&self.payload, &Payload::Login {..})
    }

    fn logout_packet(&self) -> bool {
        matches!(&self.payload, &Payload::Logout)
    }

}

impl PingPongPacket for Packet {
    fn ping(&self) -> bool {
        match self.payload {
            Payload::Ping(_) => true,
            _ => false
        }
    }

    fn pong(&self) -> bool {
        match self.payload {
            Payload::Pong(_) => true,
            _ => false
        }
    }

    fn gen_pong(&self) -> Self {
        debug_assert!(self.ping());
        if let Payload::Ping(nonce) = self.payload {
            Packet::new(self.request_id, HashMap::new(), Payload::Pong(nonce))
        } else {
            unreachable!()
        }
    }
}