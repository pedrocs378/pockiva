use gb_core::Frame;

use super::packet::{FRAME_PACKET_BYTE_LENGTH, FRAME_RGBA_BYTE_LENGTH};
use super::{AcknowledgeError, FrameQueue, encode_frame_packet};

fn frame(sequence: u64) -> Frame {
    Frame::new(
        sequence,
        vec![sequence.to_le_bytes()[0]; FRAME_RGBA_BYTE_LENGTH],
    )
    .expect("valid frame")
}

#[test]
fn raw_packet_is_fixed_little_endian_rgba_without_text_encoding() {
    let rgba = (0..FRAME_RGBA_BYTE_LENGTH)
        .map(|index| index.to_le_bytes()[0])
        .collect();
    let frame = Frame::new(0x0102_0304_0506_0708, rgba).expect("valid frame");
    let packet = encode_frame_packet(&frame);
    assert_eq!(packet.len(), FRAME_PACKET_BYTE_LENGTH);
    assert_eq!(&packet[0..12], &[8, 7, 6, 5, 4, 3, 2, 1, 160, 0, 144, 0]);
    assert_eq!(&packet[12..16], &[0, 1, 2, 3]);

    let source = include_str!("packet.rs").to_ascii_lowercase();
    for forbidden in ["base64", "data:image", "png", "serde_json"] {
        assert!(!source.contains(forbidden));
    }
}

#[test]
fn queue_keeps_one_in_flight_and_only_the_latest_pending_frame() {
    let mut queue = FrameQueue::default();
    assert_eq!(queue.offer(frame(1)).expect("publish").sequence(), 1);
    assert!(queue.offer(frame(2)).is_none());
    assert!(queue.offer(frame(3)).is_none());
    assert_eq!(queue.buffered_frame_count(), 2);
    assert_eq!(
        queue
            .acknowledge(1)
            .expect("matching ack")
            .expect("pending")
            .sequence(),
        3
    );
    assert_eq!(queue.buffered_frame_count(), 1);
}

#[test]
fn queue_rejects_stale_or_future_ack_without_losing_frames() {
    let mut queue = FrameQueue::default();
    queue.offer(frame(7));
    assert_eq!(queue.acknowledge(6), Err(AcknowledgeError::NotInFlight));
    assert_eq!(queue.acknowledge(8), Err(AcknowledgeError::NotInFlight));
    assert_eq!(queue.buffered_frame_count(), 1);
}

#[test]
fn queue_clear_and_stress_remain_bounded() {
    let mut queue = FrameQueue::default();
    queue.offer(frame(0));
    for sequence in 1..10_000 {
        queue.offer(frame(sequence));
        assert!(queue.buffered_frame_count() <= 2);
    }
    queue.clear();
    assert_eq!(queue.buffered_frame_count(), 0);
    assert_eq!(queue.acknowledge(0), Err(AcknowledgeError::NotInFlight));
}
