use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration, collections::HashMap,
};

use acceptor_connect::Connection;
use tokio::{
    sync::mpsc,
};

use crate::{Payload, tcp_connection::TcpConnection, Packet, Message};


pub fn auto_client(client: Client, max_user_id: u64) {
    tokio::spawn(async move {
        loop {
            let to_account = (rand::random::<u64>() % max_user_id) + 1;
            client
                .send(message_payload(client.identifier, to_account))
                .await;
            tokio::time::sleep(Duration::from_millis(1000)).await;
        }
    });
}

pub struct Client {
    pub identifier: u64,
    channel: mpsc::Sender<Payload>,
    pub id_generator: Arc<AtomicU64>,
}

impl Client {
    pub async fn new(identifier: u64, port: u16, token: String) -> Client {
        let (wx, rx) = mpsc::channel(1);
        let client = Self {
            identifier,
            channel: wx,
            id_generator: Arc::new(AtomicU64::new(1)),
        };
        let stream = tokio::net::TcpStream::connect(("0.0.0.0", port))
            .await
            .unwrap();
        tokio::spawn(Self::event_loop(
            identifier.clone(),
            client.id_generator.clone(),
            TcpConnection::new(stream, 4096),
            rx,
        ));

        client.send(login_payload(token)).await;
        println!("client[{}] successed connected", identifier);

        client
    }

    async fn event_loop(
        identifier: u64,
        id_generator: Arc<AtomicU64>,
        mut stream: TcpConnection,
        mut channel: mpsc::Receiver<Payload>,
    ) {
        let counter = AtomicU64::new(0);
        let rev_counter = AtomicU64::new(0);
        loop {
            tokio::select! {
                send_data = channel.recv() => {
                    let payload = match send_data {
                        Some(payload) => payload,
                        None => break
                    };
                    
                    let packet = Packet::new(id_generator.fetch_add(1, Ordering::SeqCst), HashMap::new(), payload);
                    counter.fetch_add(1, Ordering::SeqCst);
                    stream.write(packet).await.unwrap();
                    println!("message send successed");
                },
                maybe_packet = stream.next() => {
                    let packet = maybe_packet.unwrap();
                    let request_id = packet.request_id;
                    match packet.payload {
                        Payload::Message(message) => {
                            assert_eq!(identifier, message.to, "client[{}] receive packet:{:?}", identifier, message.to);
                            let resp = Packet::new(request_id, HashMap::new(), Payload::MessageCliAck);
                            stream.write(resp).await.unwrap();
                        },
                        Payload::MessageSrvAck => {
                            println!("message send success: {}", request_id);
                        },
                        other => {
                            println!("unknown message: {:?}", other);
                        }
                    }
                }
            }
        }
        println!(
            "client[{}] send: {} receive: {}",
            identifier,
            counter.load(Ordering::SeqCst),
            rev_counter.load(Ordering::SeqCst)
        );
    }

    pub async fn send(&self, data: Payload) {
        self.channel.send(data).await.unwrap();
    }
}

fn login_payload(token: String) -> Payload {
    Payload::Login{
        version: "1".into(),
        timestamp: 32,
        user_agent: "rust".into(),
        token: token.into(),
    }
}

fn message_payload(from: u64, to: u64) -> Payload {
    Payload::Message(Message {
        timestamp: 32,
        seq: 1,
        from,
        to,
        msg_type: "text".into(),
        content: "hello".into(),
    })
}
