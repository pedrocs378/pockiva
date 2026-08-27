pub trait Clock {
    fn unix_seconds(&self) -> u64;
}
