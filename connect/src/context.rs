use std::{any::Any, collections::HashMap, sync::Arc};

use tokio::sync::{Mutex, mpsc::{self, error::SendError}};

pub(crate) struct Handle<T> {
    channel: mpsc::Sender<T>,
    shutdown: mpsc::Sender<()>,
}

impl<T> Handle<T> {

    pub fn new(send_channel: mpsc::Sender<T>, shutdown: mpsc::Sender<()>) -> Self {
        Self {
            channel: send_channel,
            shutdown
        }
    }

    pub async fn send(&self, packet: T) -> Result<(), T> {
        self.channel
            .send(packet)
            .await
            .map_err(|SendError(val)| val)
    }

    pub async fn close(&self) {
        let _ = self.shutdown.send(()).await;
    }

}

pub struct Context<K, T> {
    connection_id: K,
    handle: Handle<T>,
    attrs: Arc<Mutex<HashMap<String, Box<dyn Any + Send>>>>,
}

impl<K, T> Context<K, T> {

    pub(crate) fn new(id: K, handle: Handle<T>) -> Self {
        Self {
            connection_id: id,
            handle,
            attrs: Arc::new(Mutex::new(HashMap::new()))
        }
    }

    pub fn id(&self) -> &K {
        &self.connection_id
    }

    pub async fn close(&self) {
        self.handle.close().await
    }

    pub async fn send(&self, data: T) -> Result<(), T> {
        self.handle.send(data).await
    }

    pub async fn attr<V: Clone + 'static>(&self, key: &str) -> Option<V> {
        let attrs = self.attrs.lock().await;
        attrs
            .get(key)
            .and_then(|any| any.downcast_ref::<V>())
            .cloned()
    }

    pub async fn put_attr<V: Send + Clone + 'static>(
        &self,
        key: &str,
        value: V,
    ) -> Option<Box<dyn Any + Send>> {
        let mut attrs = self.attrs.lock().await;
        attrs.insert(key.to_string(), Box::new(value))
    }

}