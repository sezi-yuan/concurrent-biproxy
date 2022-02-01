use std::{future::Future, pin::Pin, task::{Poll, Context as FutureContext}, sync::Arc, collections::HashMap, hash::Hash, time::Duration};

use async_trait::async_trait;
use tokio::sync::{oneshot, Mutex};

use crate::{processor::Processor, Context, Error, context};

pub trait RequestIdPacket {
    type ID: Eq + Hash + Clone + Send + 'static;
    fn request_id(&self) -> Self::ID;
}


#[async_trait]
pub trait SyncSend<T> 
where T: Send + RequestIdPacket
{
    async fn sync_send(&self, data: T) -> Result<T, Error>;
}

#[async_trait]
impl<K, T: Send + RequestIdPacket> SyncSend<T> for context::Context<K, T> 
where
    K: Send + Sync + 'static,
    T: 'static
{
    async fn sync_send(&self, data: T) -> Result<T, Error> {
        let notify_map = match self.attr::<Arc<Mutex<HashMap<<T as RequestIdPacket>::ID, oneshot::Sender<T>>>>>("SYNC_NOTIFY").await {
            Some(notify) => notify,
            None => {
                self.close().await;
                return Err(Error::Disconnected);
            }
        };

        let (res, sender) = Response::new();
        notify_map.lock().await.insert(data.request_id().clone(), sender);
        let resp = match self.send(data).await {
            Ok(_) => tokio::time::timeout(Duration::from_secs(3), res).await,
            Err(data) => {
                notify_map.lock().await.remove(&data.request_id());
                return Err(Error::Disconnected)
            }
        };

        match resp {
            Ok(Some(data)) => Ok(data),
            Ok(None) => Err(Error::Disconnected),
            Err(_) => Err(Error::Timeout)
        }



    }
}
#[derive(Clone)]
pub struct SyncMessageProcessor<Pro> 
where
    Pro: Processor,
    Pro::K: Send + 'static,
    Pro::T: RequestIdPacket + Send + Clone + 'static,
{
    inner: Pro,
}

impl<Pro> SyncMessageProcessor<Pro>
where
    Pro: Processor,
    Pro::K: Send + 'static,
    Pro::T: RequestIdPacket + Send + Clone+ 'static,
{
    pub fn new(inner: Pro) -> Self {
        Self {inner}
    }
}


#[async_trait]
impl<Pro> Processor for SyncMessageProcessor<Pro>
where
    Pro: Processor,
    Pro::K: Send + 'static,
    Pro::T: RequestIdPacket + Send + Clone+ 'static,
{
    type K = Pro::K;
    type T = Pro::T;
    

    async fn connected(&self, ctx: Context<Self::K, Self::T>) {
        let notify_map: Arc<Mutex<HashMap<<Pro::T as RequestIdPacket>::ID, oneshot::Sender<Pro::T>>>> = Arc::new(Mutex::new(HashMap::new()));
        ctx.put_attr("SYNC_NOTIFY", notify_map).await;
        self.inner.connected(ctx).await;
    }

    async fn process(&self, ctx: Context<Self::K, Self::T>, packet: Self::T) {
        let request_id = packet.request_id();
        let notify_map = match ctx.attr::<Arc<Mutex<HashMap<<Pro::T as RequestIdPacket>::ID, oneshot::Sender<Pro::T>>>>>("SYNC_NOTIFY").await {
            Some(notify) => notify,
            None => {
                // notify_map must be there
                ctx.close().await;
                return;
            }
        };
        let notify = notify_map.lock().await.remove(&request_id);
        match notify {
            Some(notify) => {
                // just ignore result of send
                let _ = notify.send(packet);
            },
            None => self.inner.process(ctx, packet).await
        }
    }

    async fn disconnected(&self, ctx: Context<Self::K, Self::T>) {
        // when connection lose, context will drop automic, so notify_map is also drop
        self.inner.disconnected(ctx).await
    }
}


struct Response<P> {
    notify: oneshot::Receiver<P>,
}

impl<P> Response<P> {
    fn new() -> (Self, oneshot::Sender<P>) {
        let (wx, notify) = oneshot::channel();
        let resp = Self { notify };
        (resp, wx)
    }
}

impl<P> Future for Response<P> {
    type Output = Option<P>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut FutureContext<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.notify).poll(cx) {
            Poll::Ready(Ok(value)) => Poll::Ready(Some(value)),
            Poll::Pending => Poll::Pending,
            Poll::Ready(Err(_)) => Poll::Ready(None),
        }
    }
}
