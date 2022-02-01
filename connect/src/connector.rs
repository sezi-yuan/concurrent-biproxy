use std::fmt::Display;
use std::marker;
use std::sync::Arc;

use serde::{de::DeserializeOwned, Serialize};
use tokio::sync:: mpsc;

use crate::processor::Processor;
use crate::{Connection, Context, Error};
use crate::context::{ Context as InnerCtx, Handle};

pub struct Connector<K, C, Pro>
where 
    K: Clone + Send + Sync + Display + 'static,
    C: Connection + 'static,
    C::Item: DeserializeOwned + Serialize + Send + 'static,
    Pro: Processor<K=K, T=C::Item> + 'static
{
    _conn: marker::PhantomData<C>,
    _key: marker::PhantomData<K>,
    processor: Pro
}

impl<K, C, Pro> Connector<K, C, Pro>
where 
    K: Clone + Send + Sync + Display + 'static,
    C: Connection + 'static,
    C::Item: DeserializeOwned + Serialize + Send + 'static,
    Pro: Processor<K=K, T=C::Item> + 'static
{

    pub fn new(processor: Pro) -> Self {
        Self {
            _conn: marker::PhantomData,
            _key: marker::PhantomData,
            processor
        }
    }

    pub fn run(&self, key: K, conn: C) {
        let (sender, receiver) = mpsc::channel::<C::Item>(256);
        let (shutdown, shutdown_receiver) = mpsc::channel::<()>(1);

        let handle = Handle::new(sender, shutdown);
        let ctx = Arc::new(InnerCtx::new(key.clone(), handle));
        let processor = self.processor.clone();

        tokio::spawn(async move {
            processor.connected(ctx.clone()).await;
            let res = Self::run_connection(receiver, shutdown_receiver, conn, ctx.clone(), processor.clone()).await;
            if let Err(error) = res {
                tracing::debug!("connection[{}] reset by peer: {}", key, error);
            }
            tracing::debug!("connection[{}] finish", key);
            processor.disconnected(ctx).await;
        });
    }

    async fn run_connection(
        mut send_channel: mpsc::Receiver<C::Item>, 
        mut shutdown: mpsc::Receiver<()>, 
        mut conn: C,
        ctx: Context<K, C::Item>,
        processor: Pro
    ) -> Result<(), Error> {
        loop {
            tokio::select! {
                send_data = send_channel.recv() => {
                    let packet = match send_data {
                        Some(packet) => packet,
                        // 不能再发送数据，关闭channel
                        None => return Ok(())
                    };
                    conn.write(packet).await?;
                },
                maybe_packet = conn.next() => {
                    let packet = maybe_packet?;
                    let processor_cloned = processor.clone();
                    let context = ctx.clone();
                    tokio::spawn(async move {
                        processor_cloned.process(context, packet).await;
                    });
                },
                _ = shutdown.recv() => return Ok(())
            };

        }
    }
}



