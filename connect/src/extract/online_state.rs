use async_trait::async_trait;
use tokio::time::Instant;
use std::{hash::Hash, marker, time::Duration, fmt::Display};

use crate::{Processor, context, Context};

use super::IntoPacket;


pub trait LoginPacket: Sized {
    fn login_packet(&self) -> bool;
    fn logout_packet(&self) -> bool;
}

/// 通过LoginPacket的user_id解析成真实的user_id
/// loginPacket的user_id可能并不是真实user_id
#[async_trait]
pub trait AuthServer<Packet: LoginPacket, ID> {
    type Error: IntoPacket<Packet> + Send;
    async fn auth(&self, packet: &Packet) -> Result<ID, Self::Error>;
}

#[async_trait]
pub trait UserIdContextExt<ID> {
    async fn current_user(&self) -> Option<ID>;
}

#[async_trait]
impl<K, ID, T> UserIdContextExt<ID> for context::Context<K, T> 
where
    K: Send + Sync + 'static,
    ID: Hash + Eq + Clone + Send + 'static,
    T: LoginPacket + Send
{
    async fn current_user(&self) -> Option<ID> {
        self.attr::<ID>("USER_ID").await
    }
}


/// maybe we need a auth server?
#[async_trait]
pub trait StateServer<CID, T>: Send + Sync + Clone 
{
    type K;
    type S;
    async fn put(&self, key: Self::K, state: Self::S);
    async fn put_local(&self, key: Self::K, ctx: Context<CID, T>);
    async fn remove(&self, key: &Self::K) -> Option<Self::S>;
    async fn remove_local(&self, key: &Self::K);
}

#[derive(Clone)]
pub struct StateSupportProcessor<ID, CID, S, Pro, Auth> 
where
CID: Clone + Sync + Send + 'static,
    S: StateServer<CID, Pro::T, K = ID, S = CID>,
    Pro: Processor<K = CID>,
    Pro::T: LoginPacket + Send + Clone + 'static,
    ID: Eq + Hash + Clone + Send,
    Auth: AuthServer<Pro::T, ID> + Send + 'static,
{
    state_server: S,
    inner: Pro,
    auth_server: Auth,
    /// 通知状态服务器该链接在线的间隔，需要和心跳配合使用
    state_update_interval: Duration,
    _id: marker::PhantomData<ID>
}

impl<ID, CID, S, Pro, Auth> StateSupportProcessor<ID, CID, S, Pro, Auth> 
where
    CID: Clone + Sync + Send + 'static,
    S: StateServer<CID, Pro::T, K = ID, S = CID> + 'static,
    Pro: Processor<K = CID>,
    Pro::T: LoginPacket + Send + Clone + 'static,
    ID: Eq + Hash + Clone + Send + 'static,
    Auth: AuthServer<Pro::T, ID> + Send + 'static,
{
    pub fn new(state_server: S, inner: Pro, auth_server: Auth, state_update_interval: Duration) -> Self {
        Self {
            state_server,
            inner,
            auth_server,
            state_update_interval,
            _id: marker::PhantomData
        }
    }

    fn update_state(&self, key: ID, ctx: crate::Context<CID, Pro::T>) {
        let server = self.state_server.clone();
        tokio::spawn(async move {
            let key_cloned = key.clone();
            server.put_local(key_cloned, ctx.clone()).await;
            server.put(key, ctx.id().clone()).await;
            ctx.put_attr("STATE_LAST_SYNC", Instant::now()).await;
        });
    }

    async fn state_last_update(&self, ctx: &crate::Context<CID, Pro::T>) -> Option<Instant> {
        ctx.attr::<Instant>("STATE_LAST_SYNC").await
    }
}

#[async_trait]
impl<ID, CID, S, Pro, Auth> Processor for StateSupportProcessor<ID, CID, S, Pro, Auth> 
where
    CID: Clone + Sync + Send + 'static,
    S: StateServer<CID, Pro::T, K = ID, S = CID> + 'static,
    Pro: Processor<K = CID>,
    Pro::T: LoginPacket + Send + Clone + Sync + 'static,
    ID: Eq + Hash + Clone + Send + Sync + Display + 'static,
    Auth: AuthServer<Pro::T, ID> + Send + Sync + Clone + 'static,
{
    type K = CID;
    type T = Pro::T;

    async fn connected(&self, ctx: crate::Context<Self::K, Self::T>) {
        self.inner.connected(ctx).await
    }

    async fn process(&self, ctx: crate::Context<Self::K, Self::T>, packet: Self::T) {
        if packet.login_packet() {
            match self.auth_server.auth(&packet).await {
                Ok(user_id) => {
                    tracing::debug!("user[{}] login", user_id);
                    // login success
                    ctx.put_attr("USER_ID", user_id.clone()).await;
                    self.update_state(user_id, ctx.clone());
                },
                Err(error) => {
                    let packet = error.into_packet();
                    let _ = ctx.send(packet).await;
                    return
                }
            }
        } else if packet.logout_packet() {
            ctx.close().await;
            return
        } else {
            let user_id = match ctx.current_user().await {
                Some(user_id) => user_id,
                None => {
                    ctx.close().await;
                    return
                }
            };
            match self.state_last_update(&ctx).await {
                Some(last_time) => {
                    // 判断是否超过一定时间，超过则想stateserver重新put一次
                    if last_time.elapsed() > self.state_update_interval {
                        self.update_state(user_id, ctx.clone())
                    }

                },
                None => self.update_state(user_id, ctx.clone())
            }
        }
        self.inner.process(ctx, packet).await
    }

    async fn disconnected(&self, ctx: crate::Context<Self::K, Self::T>) {
        if let Some(user_id) = ctx.current_user().await {
            self.state_server.remove(&user_id).await;
            self.state_server.remove_local(&user_id).await;
        }
        
        self.inner.disconnected(ctx).await
    }
}