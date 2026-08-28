use gb_core::Frame;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AcknowledgeError {
    NotInFlight,
}

#[derive(Debug, Default)]
pub(crate) struct FrameQueue {
    in_flight_sequence: Option<u64>,
    latest_pending: Option<Frame>,
}

impl FrameQueue {
    pub(crate) fn offer(&mut self, frame: Frame) -> Option<Frame> {
        if self.in_flight_sequence.is_none() {
            self.in_flight_sequence = Some(frame.sequence());
            return Some(frame);
        }
        self.latest_pending = Some(frame);
        None
    }

    pub(crate) fn acknowledge(&mut self, sequence: u64) -> Result<Option<Frame>, AcknowledgeError> {
        if self.in_flight_sequence != Some(sequence) {
            return Err(AcknowledgeError::NotInFlight);
        }
        self.in_flight_sequence = None;
        let next = self.latest_pending.take();
        if let Some(frame) = &next {
            self.in_flight_sequence = Some(frame.sequence());
        }
        Ok(next)
    }

    pub(crate) fn clear(&mut self) {
        self.in_flight_sequence = None;
        self.latest_pending = None;
    }

    #[cfg(test)]
    pub(crate) const fn buffered_frame_count(&self) -> usize {
        self.in_flight_sequence.is_some() as usize + self.latest_pending.is_some() as usize
    }
}
