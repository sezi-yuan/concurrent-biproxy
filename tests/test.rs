// use std::{
//     fmt::Write,
//     io::{BufRead, Cursor},
//     sync::atomic::AtomicU64,
//     time::Duration,
// };

// use async_trait::async_trait;
// use biproxy::{BiProxy, Error, IdentifierPacket, PacketProcessor, Result};
// use bytes::{Buf, BytesMut};
// use concurrent_biproxy as biproxy;
// use tokio::{
//     io::{AsyncReadExt, AsyncWriteExt},
//     net::TcpStream,
//     sync::mpsc,
// };

// #[derive(Debug, Clone)]
// struct DefaultPacket {
//     id: String,
//     from: String,
//     target: String,
//     data: String,
// }

// #[async_trait]
// impl IdentifierPacket for DefaultPacket {
//     fn id(&self) -> &str {
//         self.id.as_str()
//     }
//     /// return Ok(None) if need more bytes
//     async fn from_slice(src: &mut Cursor<&[u8]>) -> Result<Option<Self>> {
//         if src.remaining() < 26 {
//             return Ok(None);
//         }
//         let mut id = String::new();
//         src.read_line(&mut id)?;

//         let mut from = String::new();
//         src.read_line(&mut from)?;

//         let mut target = String::new();
//         src.read_line(&mut target)?;

//         let mut data = String::new();
//         src.read_line(&mut data)?;

//         Ok(Some(Self {
//             id: id.as_str()[..id.len() - 1].to_string(),
//             from: from.as_str()[..from.len() - 1].to_string(),
//             target: target.as_str()[..target.len() - 1].to_string(),
//             data: data.as_str()[..data.len() - 1].to_string(),
//         }))
//     }
//     fn into_bytes(self) -> Vec<u8> {
//         let mut bytes = BytesMut::new();
//         bytes.write_str(self.id.as_str()).unwrap();
//         bytes.write_str("\n").unwrap();

//         bytes.write_str(self.from.as_str()).unwrap();
//         bytes.write_str("\n").unwrap();

//         bytes.write_str(self.target.as_str()).unwrap();
//         bytes.write_str("\n").unwrap();

//         bytes.write_str(self.data.as_str()).unwrap();
//         bytes.write_str("\n").unwrap();

//         bytes.to_vec()
//     }
// }

// #[derive(Clone)]
// struct DefaultProcessor;

// #[async_trait]
// impl PacketProcessor<DefaultPacket> for DefaultProcessor {
//     async fn process(&self, _: DefaultPacket) {}
// }

// struct Client {
//     identifier: String,
//     channel: mpsc::Sender<DefaultPacket>,
// }

// impl Client {
//     async fn new(identifier: &str, port: u16) -> Client {
//         let (wx, rx) = mpsc::channel(1);
//         let client = Self {
//             identifier: identifier.to_string(),
//             channel: wx,
//         };
//         let stream = tokio::net::TcpStream::connect(("0.0.0.0", port))
//             .await
//             .unwrap();
//         tokio::spawn(Self::event_loop(client.identifier.clone(), stream, rx));
//         client
//     }

//     async fn event_loop(
//         id: String,
//         mut stream: TcpStream,
//         mut channel: mpsc::Receiver<DefaultPacket>,
//     ) {
//         let mut buffer = BytesMut::new();

//         loop {
//             tokio::select! {
//                 send_data = channel.recv() => {
//                     let packet = match send_data {
//                         Some(packet) => packet,
//                         None => break
//                     };
//                     let bytes = packet.into_bytes();
//                     println!("client[{}] send total bytes: {}", id, bytes.len());
//                     stream.write_all(bytes.as_slice()).await.unwrap();
//                 },
//                 packet = Self::read_packet(id.as_str(), &mut stream, &mut buffer) => {
//                     // let resp = DefaultPacket {
//                     //     id: packet.id.clone(),
//                     //     from: packet.target.clone(),
//                     //     target: packet.from.clone(),
//                     //     data: "world".to_string()
//                     // };
//                     // let bytes = resp.into_bytes();
//                     // stream.write_all(bytes.as_slice()).await.unwrap();
//                     assert_eq!(id, packet.target, "client_id:{} compare packet's target: {}", id, packet.target);
//                 }
//             }
//         }
//     }

//     async fn send(&self, data: DefaultPacket) {
//         self.channel.send(data).await.unwrap();
//     }

//     async fn read_packet(id: &str, stream: &mut TcpStream, buffer: &mut BytesMut) -> DefaultPacket {
//         match Self::read(id, stream, buffer).await {
//             Ok(Some(packet)) => return packet,
//             _ => {
//                 panic!("connection reset by peer")
//             }
//         }
//     }

//     async fn read(
//         id: &str,
//         stream: &mut TcpStream,
//         buffer: &mut BytesMut,
//     ) -> Result<Option<DefaultPacket>> {
//         loop {
//             let mut buf = Cursor::new(&buffer[..]);
//             if let Some(packet) = DefaultPacket::from_slice(&mut buf).await? {
//                 let len = buf.position() as usize;
//                 buffer.advance(len);
//                 return Ok(Some(packet));
//             }

//             let size = stream.read_buf(buffer).await?;
//             if 0 == size {
//                 if buffer.is_empty() {
//                     return Ok(None);
//                 } else {
//                     return Err(Error::Disconnect);
//                 }
//             }
//             println!("client[{}] receive bytes: {}", id, size)
//         }
//     }
// }

// async fn init_proxy() -> BiProxy<DefaultPacket> {
//     let proxy = BiProxy::new(2048);
//     let proxy_clone = proxy.clone();
//     let processor = LocalRouterProcessor {
//         proxy: proxy.clone(),
//     };
//     tokio::spawn(async move {
//         let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

//         let mut counter = 1;
//         loop {
//             if let Ok((socket, _)) = listener.accept().await {
//                 proxy_clone
//                     .accept_tcpstream(
//                         format!("user{}", counter).as_str(),
//                         socket,
//                         processor.clone(),
//                     )
//                     .await;
//                 counter += 1;
//             }
//         }
//     });
//     tokio::time::sleep(Duration::from_millis(100)).await;
//     proxy
// }

// #[derive(Clone)]
// struct LocalRouterProcessor {
//     proxy: BiProxy<DefaultPacket>,
// }

// #[async_trait]
// impl PacketProcessor<DefaultPacket> for LocalRouterProcessor {
//     async fn process(&self, packet: DefaultPacket) {
//         let target = packet.target.clone();
//         let _ = self.proxy.send_oneway(target.as_str(), packet).await;
//     }
// }

// #[tokio::test]
// pub async fn tset_c2c() -> Result<()> {
//     let _proxy = init_proxy().await;
//     let client1 = Client::new(format!("user{}", 1).as_str(), 3000).await;
//     let client2 = Client::new(format!("user{}", 2).as_str(), 3000).await;

//     let request = DefaultPacket {
//         id: "xasdfa1".to_string(),
//         from: "user1".to_string(),
//         target: "user2".to_string(),
//         data: "hello".to_string(),
//     };

//     client1.send(request).await;
//     let request = DefaultPacket {
//         id: "xasdfa2".to_string(),
//         from: "user2".to_string(),
//         target: "user1".to_string(),
//         data: "hello".to_string(),
//     };
//     client2.send(request).await;
//     tokio::time::sleep(Duration::from_millis(10)).await;
//     Ok(())
// }

// #[tokio::test]
// pub async fn test_sync_send() -> Result<()> {
//     let proxy = init_proxy().await;
//     let _client = Client::new(format!("user{}", 1).as_str(), 3000).await;
//     let request = DefaultPacket {
//         id: "xasdfas".to_string(),
//         from: "user2".to_string(),
//         target: "user1".to_string(),
//         data: "hello".to_string(),
//     };
//     tokio::time::sleep(Duration::from_millis(100)).await;
//     let res = proxy.send("user1", request).await.unwrap();
//     println!("{:?}", res);
//     assert_eq!(res.target, "user2".to_string());
//     assert_eq!(res.data, "world".to_string());
//     Ok(())
// }

// #[tokio::test]
// pub async fn test_async_send() -> Result<()> {
//     let proxy = init_proxy().await;
//     let _client = Client::new(format!("user{}", 1).as_str(), 3000).await;
//     let request = DefaultPacket {
//         id: "xasdfas".to_string(),
//         from: "user2".to_string(),
//         target: "user1".to_string(),
//         data: "hello".to_string(),
//     };
//     tokio::time::sleep(Duration::from_millis(100)).await;
//     let _ = proxy.send_oneway("user1", request).await.unwrap();
//     tokio::time::sleep(Duration::from_millis(100)).await;
//     Ok(())
// }

// #[tokio::test]
// pub async fn test_bench_send() -> Result<()> {
//     let proxy = init_proxy().await;

//     let mut clients = vec![];
//     let client_num = 1000;
//     for id in 1..=client_num {
//         clients.push(Client::new(format!("user{}", id).as_str(), 3000).await)
//     }

//     let counter = AtomicU64::new(0);

//     let mut message_counter = 1_000_000;
//     while message_counter > 0 {
//         let target = format!("user{}", rand::random::<u32>() % client_num + 1);
//         let packet = DefaultPacket {
//             id: format!(
//                 "{}",
//                 counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
//             ),
//             from: format!("user{}", rand::random::<u32>() % 999 + 1),
//             target: target.clone(),
//             data: "hello".to_string(),
//         };

//         let new_proxy = proxy.clone();
//         tokio::spawn(async move {
//             if let Err(error) = new_proxy.send_oneway(target.as_str(), packet).await {
//                 println!("can not send to[{}]:{}", target, error);
//             }
//         });

//         message_counter -= 1;
//     }
//     tokio::time::sleep(Duration::from_secs(60)).await;
//     Ok(())
// }
