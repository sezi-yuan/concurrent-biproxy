mod error;
mod context;
mod connector;
mod connection;
mod processor;
pub mod extract;

use std::sync::Arc;

pub use error::{Error, Result};
pub use connection::Connection;
pub use connector::Connector;
pub use processor::Processor;

pub type Context<K, T> = Arc<context::Context<K, T>>;
