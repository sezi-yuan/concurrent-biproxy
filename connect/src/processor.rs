use async_trait::async_trait;

use crate::Context;

#[async_trait]
pub trait Processor: Clone + Sync + Send {
    type K: Send + Sync + 'static;
    type T: Send + 'static;

    async fn connected(&self, _ctx: Context<Self::K, Self::T>) {}

    async fn process(&self, ctx: Context<Self::K, Self::T>, packet: Self::T);

    async fn disconnected(&self, _ctx: Context<Self::K, Self::T>) {}
}
