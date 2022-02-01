use std::{time::Duration, fmt::Display};

use async_trait::async_trait;
use tokio::{sync::mpsc, time::Instant};

use crate::{Processor, Context};

const MUST_BE_LOGOUT: Duration = Duration::from_secs(3600 * 24);
pub trait PingPongPacket {

    fn ping(&self) -> bool;
    fn pong(&self) -> bool;

    fn gen_pong(&self) -> Self;

}

struct Heartbeat<K, T> {
    interval: Duration,
    ctx: Context<K, T>,
    shutdown: mpsc::Receiver<()>
}

impl<K, T> Heartbeat<K, T> {
    
    pub fn new(interval: Duration, ctx: Context<K, T>) -> (Self, mpsc::Sender<()>) {
        let (sender, receiver) = mpsc::channel(1);
        let reactor = Self {
            interval,
            ctx,
            shutdown: receiver
        };
        (reactor, sender)
    }

    pub async fn run(&mut self) {
        loop {
            // calculate next tick duration
            let elapsed = self.last_read().await;
            tracing::debug!("heart_beat time elapsed: {:?}", elapsed);
            if elapsed > self.interval * 3 {
                return
            }

            let next_tick = if self.interval > elapsed {
                self.interval - elapsed
            } else {
                self.interval
            };

            tokio::select! {
                _ = tokio::time::sleep(next_tick) => continue,
                _ = self.shutdown.recv() => {
                    tracing::debug!("shutdown signal received");
                    return
                }
            }
        }
    }

    async fn last_read(&self) -> Duration {
        let last_read = self.ctx.attr::<Instant>("LAST_READ").await;
        match last_read {
            None => MUST_BE_LOGOUT,
            Some(last_read) => last_read.elapsed()
        }
    }
}

#[derive(Clone)]
pub struct HeartBeatProcessor<Pro> 
where
    Pro: Processor,
    Pro::K: Send + 'static,
    Pro::T: PingPongPacket + Send + Clone + 'static,
{
    interval: Duration,
    inner: Pro,
}

impl<Pro> HeartBeatProcessor<Pro>
where
    Pro: Processor,
    Pro::K: Send + 'static,
    Pro::T: PingPongPacket + Send + Clone+ 'static,
{
    pub fn new(interval: Duration, inner: Pro) -> Self {
        Self {
            interval,
            inner
        }
    }

    pub async fn kick_read(&self, ctx: &crate::Context<Pro::K, Pro::T>) {
        let _ = ctx.put_attr("LAST_READ", Instant::now()).await;
    }
}

#[async_trait]
impl<Pro> Processor for HeartBeatProcessor<Pro>
where
    Pro: Processor,
    Pro::K: Send + Display + 'static,
    Pro::T: PingPongPacket + Send + Sync + Clone + 'static,
{
    type K = Pro::K;
    type T = Pro::T;

    async fn connected(&self, ctx: Context<Self::K, Self::T>) {
        let (mut reactor, shutdown) = Heartbeat::new(self.interval, ctx.clone());
        ctx.put_attr("HEART_BEAT_REACTOR_SHUTDOWN", shutdown).await;
        self.kick_read(&ctx).await;
        tokio::spawn(async move {
            reactor.run().await;
            tracing::debug!("connection lose, heart beat reactor shutdown");
        });
        self.inner.connected(ctx).await;
    }

    async fn process(&self, ctx: crate::Context<Self::K, Self::T>, packet: Self::T) {
        self.kick_read(&ctx).await;
        if packet.ping() {
            if let Err(_) = ctx.send(packet.gen_pong()).await {
                //如果发送失败，可以等待下次重试，所以不需要针对是失败情况做特殊处理
                //如果多次失败，那么心跳应该失败，并且关闭链接
                tracing::debug!("failed to send data to client:{}", ctx.id());
            }
        }
        // 仍然把packet传输给内部进行处理
        self.inner.process(ctx, packet).await
    }

    async fn disconnected(&self, ctx: Context<Self::K, Self::T>) {
        let sender = ctx.attr::<mpsc::Sender<()>>("HEART_BEAT_REACTOR_SHUTDOWN").await;
        if let Some(sender) = sender {
            // 不需要特别关注关闭信号是否已经传达成功，因为心跳reactor在超时后会自动结束
            // 这里发送关闭信号，可以尽早结束心跳，提升一点服务器的性能
            // 如果是reactor主动关闭，这里收到后会发送失败， 忽略即可
            let _ = sender.send(()).await;
        }
    }

}