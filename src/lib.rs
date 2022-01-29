use std::{
    collections::HashMap,
    io::Cursor,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use async_trait::async_trait;
use bytes::{Buf, BytesMut};
use serde::{Serialize, Deserialize};
use std::future::Future;
use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
    sync::{broadcast, oneshot, Mutex},
};


mod channel;
mod error;
mod ser;
mod de;


pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("channel disconnected")]
    Disconnect,
}

#[derive(Error, Debug)]
pub enum PacketError {
    #[error("")]
    Incomplete,
    #[error("{0}")]
    Other(String),
}

#[async_trait]
pub trait IdentifierPacket: Sized + Send + 'static {
    fn id(&self) -> &str;
    /// return Ok(None) if need more bytes
    async fn from_slice(src: &mut Cursor<&[u8]>) -> Result<Option<Self>>;
    fn into_bytes(self) -> Vec<u8>;
}

#[derive(Debug, Clone)]
pub struct WriteConnection {
    inner: Arc<Mutex<OwnedWriteHalf>>,
}

impl WriteConnection {
    pub fn new(write: OwnedWriteHalf) -> Self {
        Self {
            inner: Arc::new(Mutex::new(write)),
        }
    }

    pub async fn write<P: IdentifierPacket>(&self, packet: P) -> Result<()> {
        let bytes: Vec<u8> = packet.into_bytes();
        self.inner.lock().await.write_all(bytes.as_slice()).await?;
        Ok(())
    }
}

#[async_trait]
pub trait PacketProcessor<P>: Sync + Send + Clone + 'static
where
    P: IdentifierPacket,
{
    async fn process(&self, packet: P);
}

pub struct ReadConnection<P, PP>
where
    P: IdentifierPacket,
    PP: PacketProcessor<P>,
{
    identifer: String,
    inner: OwnedReadHalf,
    response_notify: Arc<Mutex<HashMap<String, oneshot::Sender<P>>>>,
    processor: PP,
    buffer: BytesMut,
}

impl<P, PP> ReadConnection<P, PP>
where
    P: IdentifierPacket,
    PP: PacketProcessor<P>,
{
    pub fn new(
        identifer: &str,
        connection: OwnedReadHalf,
        processor: PP,
        notify: Arc<Mutex<HashMap<String, oneshot::Sender<P>>>>,
        capacity: usize,
    ) -> Self {
        Self {
            identifer: identifer.to_string(),
            inner: connection,
            response_notify: notify,
            processor,
            buffer: BytesMut::with_capacity(capacity),
        }
    }

    pub async fn run(&mut self, mut shutdown: broadcast::Receiver<()>) -> Result<()> {
        loop {
            let maybe_packet = tokio::select! {
                res = self.read() => res?,
                _ = shutdown.recv() => return Ok(())
            };

            let packet = match maybe_packet {
                Some(packet) => packet,
                None => return Ok(()),
            };

            let id = packet.id();
            let notify = self.response_notify.lock().await.remove(id);
            match notify {
                Some(notify) => {
                    if notify.send(packet).is_err() {
                        // TODO maybe we should log the message's content
                        logs::warn!("notify channel has closed, can not notify sender");
                    }
                }
                None => self.processor.process(packet).await,
            }
        }
    }

    async fn read(&mut self) -> Result<Option<P>> {
        loop {
            let mut buf = Cursor::new(&self.buffer[..]);
            if let Some(packet) = <P>::from_slice(&mut buf).await? {
                let len = buf.position() as usize;
                self.buffer.advance(len);
                return Ok(Some(packet));
            }

            let size = self.inner.read_buf(&mut self.buffer).await?;
            if size == 0 {
                if self.buffer.is_empty() {
                    return Ok(None);
                } else {
                    return Err(Error::Disconnect);
                }
            }
            println!("server[{}] receive bytes: {}", self.identifer, size);
        }
    }
}

type NotifyMap<P> = Arc<Mutex<HashMap<String, Arc<Mutex<HashMap<String, oneshot::Sender<P>>>>>>>;
#[derive(Clone)]
pub struct BiProxy<P>
where
    P: IdentifierPacket,
{
    read_buffer_size: usize,
    write_conns: Arc<Mutex<HashMap<String, WriteConnection>>>,
    notify: NotifyMap<P>,
    shutdown: broadcast::Sender<()>,
    _a: std::marker::PhantomData<P>,
}

impl<P> BiProxy<P>
where
    P: IdentifierPacket,
{
    pub fn new(read_buffer_size: usize) -> Self {
        Self {
            read_buffer_size,
            write_conns: Arc::new(Mutex::new(HashMap::new())),
            notify: Arc::new(Mutex::new(HashMap::new())),
            shutdown: broadcast::channel(1).0,
            _a: std::marker::PhantomData::default(),
        }
    }

    pub async fn accept_tcpstream<PP: PacketProcessor<P> + Clone>(
        &self,
        key: &str,
        stream: TcpStream,
        processor: PP,
    ) {
        let (read, write) = stream.into_split();
        self.write_conns
            .lock()
            .await
            .insert(key.to_string(), WriteConnection::new(write));

        let notify_map = Arc::new(Mutex::new(HashMap::new()));
        self.notify
            .lock()
            .await
            .insert(key.to_string(), notify_map.clone());
        self.spawn_read_task(key, notify_map, read, processor).await;
    }

    pub async fn send_oneway(&self, key: &str, data: P) -> Result<()> {
        self.get_connection(key)
            .await
            .ok_or(Error::Disconnect)?
            .write(data)
            .await
    }

    pub async fn send(&self, key: &str, data: P) -> Result<P> {
        let id = data.id().to_string();
        let (sender, receiver) = oneshot::channel();
        self.register_request_notify(key, id.as_str(), sender).await;

        if let Err(error) = self.send_oneway(key, data).await {
            self.deregister_request_notify(key, id.as_str()).await;
            return Err(error);
        }

        let res = Response::new(receiver).await;
        Ok(res)
    }

    async fn spawn_read_task<PP: PacketProcessor<P> + Clone>(
        &self,
        key: &str,
        notify: Arc<Mutex<HashMap<String, oneshot::Sender<P>>>>,
        conn: OwnedReadHalf,
        processor: PP,
    ) {
        let mut read_conn =
            ReadConnection::new(key, conn, processor, notify, self.read_buffer_size);
        let shutdown = self.shutdown.subscribe();
        tokio::spawn(async move {
            if let Err(error) = read_conn.run(shutdown).await {
                logs::error!("connection reset by peer, closed: {}", error);
            }
        });
    }

    async fn deregister_request_notify(&self, key: &str, packet_id: &str) {
        let notify_map = self.notify.lock().await;
        if let Some(notify_map) = notify_map.get(key) {
            // ignore return
            let _ = notify_map.lock().await.remove(packet_id);
        }
    }

    async fn register_request_notify(
        &self,
        key: &str,
        packet_id: &str,
        sender: oneshot::Sender<P>,
    ) {
        let mut notify_map = self.notify.lock().await;

        if !notify_map.contains_key(key) {
            notify_map.insert(key.to_string(), Arc::new(Mutex::new(HashMap::new())));
        }

        let inner_notifys = match notify_map.get(key) {
            Some(read_notify) => read_notify,
            None => unreachable!(),
        };

        inner_notifys
            .lock()
            .await
            .insert(packet_id.to_string(), sender);
    }

    async fn get_connection(&self, key: &str) -> Option<WriteConnection> {
        self.write_conns.lock().await.get(key).cloned()
    }
}

struct Response<P> {
    notify: oneshot::Receiver<P>,
}

impl<P> Response<P> {
    fn new(notify: oneshot::Receiver<P>) -> Self {
        Self { notify }
    }
}

impl<P> Future for Response<P> {
    type Output = P;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.notify).poll(cx) {
            Poll::Ready(Ok(value)) => Poll::Ready(value),
            Poll::Pending => Poll::Pending,
            Poll::Ready(Err(_)) => unreachable!(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct Inner {
    code: u32,
    message: String
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "tag", content = "tag_content")]
enum Tag {
    A,
    B(u32),
    C(String)
}

#[derive(Serialize, Deserialize, Debug)]
struct Data<T> {
    signed: i16,
    unsigned: u16,
    message: String,
    list: Vec<String>,
    ports: Vec<u16>,
    map: HashMap<String, String>,
    inner: T,
    tag: Tag

}

#[test]
pub fn test_serde() {
    let mut map = HashMap::new();
    map.insert("helo".to_string(), "world".to_string());
    map.insert("test".to_string(), "32".to_string());
    let data = Data {
        signed: -78,
        unsigned: 50000,
        message: "hello".into(),
        list: vec!["this".into(), "is".into(), "a".into()],
        ports: vec![8848, 8849],
        map,
        inner: Inner {
            code: 32,
            message: "ok".into()
        },
        tag: Tag::C("heihei".into())
    };

    let bytes = ser::to_vec(&data).unwrap();
    println!("bytes => {:?}", bytes);
    let x = de::from_bytes::<Data<Inner>>(bytes.as_slice()).unwrap();
    println!("dat=> {:?}", x);

}