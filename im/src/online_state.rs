use std::{collections::HashMap, sync::Arc};

use acceptor_connect::{extract::online_state::StateServer, Context};
use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::packet::Packet;

// todo use redis
#[derive(Clone)]
pub struct OnlineStateServer {
    state: Arc<Mutex<HashMap<u64, String>>>,
    contexts: Arc<Mutex<HashMap<u64, Context<String, Packet>>>>,
}

impl OnlineStateServer {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(HashMap::new())),
            contexts: Arc::new(Mutex::new(HashMap::new()))
        }
    }

    pub async fn get_context(&self, user_id: u64) -> Option<Context<String, Packet>> {
        self.contexts.lock().await.get(&user_id).cloned()
    }
}

#[async_trait]
impl StateServer<String, Packet> for OnlineStateServer
{
    type K = u64;
    /// address + connection_id
    type S = String;

    async fn put(&self, key: Self::K, state: Self::S) {
        self.state.lock().await.insert(key, state);
    }

    async fn remove(&self, key: &Self::K) -> Option<Self::S> {
        self.state.lock().await.remove(key)
    }

    async fn put_local(&self, key: Self::K, ctx: acceptor_connect::Context<String, Packet>) {
        self.contexts.lock().await.insert(key, ctx);
    }

    async fn remove_local(&self, key: &Self::K) {
        self.contexts.lock().await.remove(key);
    }
}