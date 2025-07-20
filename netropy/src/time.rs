use std::cmp::Ordering;
use std::time::Duration;

/// simulation timestamp (nanoseconds since epoch 0).
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct SimTime(pub u64);

impl SimTime {
    /// advance by a duration.
    pub fn checked_add(&self, d: Duration) -> Option<SimTime> {
        self.0.checked_add(d.as_nanos() as u64).map(SimTime)
    }
}

/// single scheduled event for the simulator
pub(crate) struct ScheduledEvent<T> {
    pub when: SimTime,
    pub payload: T,
}

impl<T> PartialEq for ScheduledEvent<T> {
    fn eq(&self, other: &Self) -> bool {
        self.when == other.when
    }
}

impl<T> Eq for ScheduledEvent<T> {}

impl<T> PartialOrd for ScheduledEvent<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        other.when.partial_cmp(&self.when)
    }
}

impl<T> Ord for ScheduledEvent<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        other.when.cmp(&self.when)
    }
}
