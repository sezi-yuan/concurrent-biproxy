
mod packet;
mod server;
mod auth;
mod error;
mod client;
mod online_state;
mod tcp_connection;
#[cfg(test)]
mod test;
use std::{time::Duration, str::FromStr, io};

use acceptor_connect::{extract::{online_state::StateSupportProcessor, heart_beat::HeartBeatProcessor, sync_message::SyncMessageProcessor}, Connector};
use auth::TokenAuthServer;
use online_state::OnlineStateServer;
pub use server::Server;
pub use packet::*;
pub use client::*;
use tcp_connection::TcpConnection;
use tokio::net::TcpListener;
use tracing_log::LogTracer;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};


#[tokio::main]
pub async fn main() -> std::io::Result<()> {
    init_log();
    let state_server = OnlineStateServer::new();
    let im_server = Server::new(state_server.clone());
    let connector = connector_init(
        im_server, state_server, Duration::from_secs(10), Duration::from_secs(30)
    );
    
    let listener = TcpListener::bind("0.0.0.0:6000").await?;
    tracing::info!("im server start at 0.0.0.0:6000");
    loop {
        match listener.accept().await {
            Ok((socket, addr)) => {
                tracing::info!("new connecton coming: {}", addr);
                let connection = TcpConnection::new(socket, 4096);
                // TODO use local_ip + client_ip
                connector.run(addr.to_string(), connection);
            },
            Err(e) => tracing::error!("could't get client: {:?}", e)
        }
    }
}

fn connector_init(inner: Server, state_server: OnlineStateServer, heart_beat: Duration, state_update: Duration) 
    -> Connector<String, TcpConnection, SyncMessageProcessor<HeartBeatProcessor<StateSupportProcessor<u64, String, OnlineStateServer, Server, TokenAuthServer>>>> 
{
    let auth_server = TokenAuthServer::new();
    let processor = StateSupportProcessor::new(state_server, inner, auth_server, state_update);
    let processor = HeartBeatProcessor::new(heart_beat, processor);
    let processor = SyncMessageProcessor::new(processor);
    Connector::new(processor)
}

fn init_log() {
    LogTracer::init().unwrap();
    let log_level = std::env::var("LOG_FILTER").unwrap_or_else(|_| "DEBUG".to_owned());
    let log_level = tracing::Level::from_str(log_level.as_str()).unwrap_or(tracing::Level::INFO);
    let log_path = std::env::var("LOG_PATH").unwrap_or_else(|_| {
        std::env::var("PWD").expect("can not get home dir") + 
       "/log"
    });
    // initialize tracing
    let file_appender = tracing_appender::rolling::hourly(log_path, "log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    let subscriber = tracing_subscriber::registry()
        .with(EnvFilter::from_default_env().add_directive(log_level.into()))
        .with(fmt::Layer::new().with_writer(io::stdout))
        .with(fmt::Layer::new().with_writer(non_blocking));
    tracing::subscriber::set_global_default(subscriber).expect("Unable to set a global collector");
}
