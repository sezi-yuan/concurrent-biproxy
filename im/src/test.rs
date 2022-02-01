use std::{time::Duration, str::FromStr, io};

use crate::client::{ self, Client };
use bytes::{BytesMut, BufMut};
use crypto::{
    aes, blockmodes,
    buffer::{self, ReadBuffer, WriteBuffer},
};
use tracing_log::LogTracer;
use tracing_subscriber::{EnvFilter, fmt, prelude::__tracing_subscriber_SubscriberExt};

fn init_log() {
    LogTracer::init().unwrap();
    let log_level = std::env::var("LOG_FILTER").unwrap_or_else(|_| "DEBUG".to_owned());
    let log_level = tracing::Level::from_str(log_level.as_str()).unwrap_or(tracing::Level::INFO);
    let log_path = std::env::var("LOG_PATH").unwrap_or_else(|_| {
        std::env::var("PWD").expect("can not get home dir") + 
       "/logclient"
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

#[tokio::test]
async fn test_im_send() -> Result<(), Box<dyn std::error::Error>> {
    init_log();
    let total_client = 3u64;

    for id in 1..=total_client {
        let token = encrypt(id);
        let client = Client::new(id, 6000, token).await;
        client::auto_client(client, total_client);
    }

    tokio::time::sleep(Duration::from_secs(300)).await;

    Ok(())
}

static KEY: [u8; 16] = [
    0x0, 0x1, 0x2, 0x3, 0x4, 0x5, 0xC, 0x7, 0xB, 0x9, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15,
];
static IV: [u8; 16] = [
    0x0, 0x1, 0x2, 0x3, 0x4, 0x5, 0xC, 0x7, 0xB, 0x9, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15,
];

fn encrypt(user_id: u64) -> String {
    let mut buf = BytesMut::with_capacity(12);
    buf.put_u64(user_id);
    buf.put_u16(32);
    buf.put_u16(32);
    let data = buf.to_vec();
    let mut encryptor = aes::cbc_encryptor(
        aes::KeySize::KeySize128,
        &KEY[..],
        &IV[..],
        blockmodes::PkcsPadding,
    );

    let mut final_result = Vec::<u8>::new();
    let mut read_buffer = buffer::RefReadBuffer::new(data.as_slice());
    let mut buffer = [0; 128];
    let mut write_buffer = buffer::RefWriteBuffer::new(&mut buffer);

    loop {
        let result = encryptor
            .encrypt(&mut read_buffer, &mut write_buffer, true)
            .unwrap();
        final_result.extend(
            write_buffer
                .take_read_buffer()
                .take_remaining()
                .iter()
                .map(|&i| i),
        );

        match result {
            buffer::BufferResult::BufferUnderflow => break,
            buffer::BufferResult::BufferOverflow => {}
        }
    }

    base64::encode(final_result)
}
