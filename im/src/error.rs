use thiserror::Error;


#[derive(Error, Debug)]
pub enum ImError {
    #[error("error from client: code[{code}]; message: {message}")]
    Custom{
        code: u32,
        message: String
    },
    #[error("client disconnected")]
    Disconnected,
    #[error("client response unknown message")]
    UndefinedBehavior
}