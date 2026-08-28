use std::time::Instant;

const TOKEN_SCALE: u128 = 1_000_000_000;

#[derive(Debug, Clone)]
pub struct InputRateLimiter {
    refill_per_second: u128,
    capacity: u128,
    available: u128,
    last_refill: Instant,
}

impl InputRateLimiter {
    #[must_use]
    pub fn new(refill_per_second: u64, capacity: u64, start: Instant) -> Self {
        let capacity = u128::from(capacity).saturating_mul(TOKEN_SCALE);
        Self {
            refill_per_second: u128::from(refill_per_second),
            capacity,
            available: capacity,
            last_refill: start,
        }
    }

    pub fn allow(&mut self, now: Instant) -> bool {
        if let Some(elapsed) = now.checked_duration_since(self.last_refill) {
            let refill = elapsed.as_nanos().saturating_mul(self.refill_per_second);
            self.available = self.available.saturating_add(refill).min(self.capacity);
            self.last_refill = now;
        }

        if self.available < TOKEN_SCALE {
            return false;
        }
        self.available -= TOKEN_SCALE;
        true
    }
}
