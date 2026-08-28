use gb_core::{Frame, SCREEN_HEIGHT, SCREEN_WIDTH};

pub(crate) const FRAME_HEADER_BYTE_LENGTH: usize = 12;
pub(crate) const FRAME_RGBA_BYTE_LENGTH: usize = SCREEN_WIDTH * SCREEN_HEIGHT * 4;
pub(crate) const FRAME_PACKET_BYTE_LENGTH: usize =
    FRAME_HEADER_BYTE_LENGTH + FRAME_RGBA_BYTE_LENGTH;

#[must_use]
pub(crate) fn encode_frame_packet(frame: &Frame) -> Vec<u8> {
    debug_assert_eq!(frame.rgba().len(), FRAME_RGBA_BYTE_LENGTH);
    let mut packet = Vec::with_capacity(FRAME_PACKET_BYTE_LENGTH);
    packet.extend_from_slice(&frame.sequence().to_le_bytes());
    let width = u16::try_from(SCREEN_WIDTH).expect("Game Boy screen width fits in u16");
    let height = u16::try_from(SCREEN_HEIGHT).expect("Game Boy screen height fits in u16");
    packet.extend_from_slice(&width.to_le_bytes());
    packet.extend_from_slice(&height.to_le_bytes());
    packet.extend_from_slice(frame.rgba());
    debug_assert_eq!(packet.len(), FRAME_PACKET_BYTE_LENGTH);
    packet
}
