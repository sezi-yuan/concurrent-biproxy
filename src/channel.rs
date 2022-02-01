// use async_trait::async_trait;
// use thiserror::Error;
// use tokio::net::TcpStream;


// #[derive(Debug, Error)]
// pub enum Error {

// }

// pub type Result<T> = std::result::Result<T, Error>;


// #[async_trait]
// pub trait Channel<T>: Send + Sync + Clone + 'static {

//     async fn send(&self, data: T) -> Result<()>;

//     async fn batch_send(&self, data: &[T]) -> Result<()>;

//     async fn recv(&self) -> Result<T>;

//     async fn close(&self);

// }

// #[async_trait]
// pub trait Connection<T> {

//     async fn read() -> Result<T>;

//     async fn write(data: T) -> Result<()>;

// }

// #[async_trait]
// impl<T: Send + 'static> Connection<T> for TcpStream {
//     async fn read() -> Result<T> {
//         todo!()
//     }

//     async fn write(data: T) -> Result<()> {
//         todo!()
//     }
// }

