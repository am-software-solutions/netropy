use bytes::Bytes;
use std::time::Duration;

use crate::NodeId;

#[derive(Clone, Debug)]
pub struct LinkConfig {
    pub latency: Duration,
    pub jitter: Duration,
}

/// network packet scheduled for delivery
#[derive(Clone, Debug)]
pub struct Packet {
    pub src: NodeId,
    pub dst: NodeId,
    pub data: Bytes,
    pub at: super::time::SimTime,
}
