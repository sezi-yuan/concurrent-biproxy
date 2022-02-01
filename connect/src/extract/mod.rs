pub mod sync_message;
pub mod online_state;
pub mod heart_beat;

pub trait IntoPacket<T> {
    fn into_packet(self) -> T;
}