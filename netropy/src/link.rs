use bytes::Bytes;
use std::{fmt, time::Duration};

use crate::NodeId;

#[derive(Clone, Debug)]
pub struct LinkConfig {
    pub latency: Duration,
    pub jitter: Duration,
}

/// network packet scheduled for delivery
#[derive(Clone)]
pub struct Packet {
    pub src: NodeId,
    pub dst: NodeId,
    pub data: Bytes,
    pub at: super::time::SimTime,
    pub id: String,
}

impl fmt::Debug for Packet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}->{} [#{}]", self.src.0, self.dst.0, self.id)
    }
}
