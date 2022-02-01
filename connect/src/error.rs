use thiserror::Error as ThisError;


pub type Result<T> = std::result::Result<T, Error>;

#[derive(ThisError, Debug)]
pub enum Error {
    #[error(transparent)]
    Serde(#[from] acceptor_serde::Error),
    #[error("client disconnected")]
    Disconnected,
    #[error("client response timeout")]
    Timeout
}