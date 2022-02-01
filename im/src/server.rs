use std::{collections::HashMap, sync::{atomic::{AtomicU64, Ordering}, Arc}};

use acceptor_connect::{Processor, extract::sync_message::{RequestIdPacket, SyncSend}, Context};
use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::{packet::{Packet, Payload, Message}, online_state::OnlineStateServer, error::ImError};

pub struct StoreError{
    code: u32,
    message: String
}

#[derive(Clone)]
pub struct Server {
    state_server: OnlineStateServer,
    request_counter: Arc<Mutex<HashMap<String, AtomicU64>>>
}

impl Server {
    pub fn new(state: OnlineStateServer) -> Self {
        Server {
            state_server: state,
            request_counter: Arc::new(Mutex::new(HashMap::new()))
        }
    }

    async fn process_message(&self, msg: Message, _: acceptor_connect::Context<String, Packet>) -> Result<(), StoreError> {
        // TODO we should store message first
        // then we send the message
        if let Err(error) = self.send(msg).await {
            // send failed, log it
            tracing::error!("failed to send message to target: {}", error);
        }

        Ok(())
    }

    async fn gen_request_id(&self, ctx: &Context<String, Packet>) -> u64 {
        let counter = self.request_counter.lock().await;
        let request_id = counter.get(ctx.id());
        match request_id {
            Some(counter) => counter.fetch_sub(1, Ordering::SeqCst),
            None => {
                self.request_counter.lock().await.insert(ctx.id().clone(), AtomicU64::new(u64::MAX - 1));
                u64::MAX
            }
        }
    }

    async fn get_context(&self, account: u64) -> Result<Context<String, Packet>, ImError> {
        self.state_server.get_context(account).await.ok_or(ImError::Disconnected)
    }

    pub async fn send_oneway(&self, msg: Message) -> Result<(), ImError> {
        let ctx = self.get_context(msg.to).await?;
        let request_id = self.gen_request_id(&ctx).await;
        let packet = Packet::new(request_id, HashMap::new(), Payload::Message(msg));
        ctx.send(packet).await.map_err(|_| ImError::Disconnected)
    }

    pub async fn send(&self, msg: Message) -> Result<(), ImError> {
        let ctx = self.get_context(msg.to).await?;
        let request_id = self.gen_request_id(&ctx).await;
        let packet = Packet::new(request_id, HashMap::new(), Payload::Message(msg));
        let resp = ctx.sync_send(packet).await.map_err(|_| ImError::Disconnected)?;
        match resp.payload {
            Payload::Error{ code, message } => Err(ImError::Custom{code, message}),
            Payload::MessageCliAck => Ok(()),
            _ => Err(ImError::UndefinedBehavior)
        }
    }
}

#[async_trait]
impl Processor for Server {
    type K = String;

    type T = Packet;

    async fn connected(&self, ctx: Context<Self::K, Self::T>) {
        tracing::info!("a new connection connected");
    }

    async fn process(&self, ctx: acceptor_connect::Context<Self::K, Self::T>, packet: Self::T) {
        let request_id = packet.request_id();
        match packet.payload {
            Payload::Message(msg) => {
                if let Ok(_) = self.process_message(msg, ctx.clone()).await {
                    let res = Packet::new(request_id, HashMap::new(), Payload::MessageSrvAck);
                    let _ = ctx.send(res).await;    
                }
            },
            Payload::Login{..} => {
                // success logined
                self.request_counter.lock().await.insert(ctx.id().clone(), AtomicU64::new(u64::MAX));
            }
            _ => {}
        }
    }

    async fn disconnected(&self, ctx: acceptor_connect::Context<Self::K, Self::T>) {
        let _  = self.request_counter.lock().await.remove(ctx.id()); 
    }

}


