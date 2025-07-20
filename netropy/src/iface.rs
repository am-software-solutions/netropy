use crate::{
    error::SimError,
    link::Packet,
    network::NodeId,
    time::{ScheduledEvent, SimTime},
};
use bytes::Bytes;
use std::{
    collections::BinaryHeap,
    sync::Arc,
    time::Duration,
};
use tokio::sync::{mpsc::UnboundedReceiver, Mutex};
use crate::link::LinkConfig;

pub struct SimInterface {
    me: NodeId,
    rx: UnboundedReceiver<Packet>,
    links: Vec<(NodeId, NodeId, LinkConfig)>,
    queue: Arc<Mutex<BinaryHeap<ScheduledEvent<Packet>>>>,
    now: SimTime,
}

impl SimInterface {
    pub(crate) fn new(
        me: NodeId,
        rx: UnboundedReceiver<Packet>,
        links: Vec<(NodeId, NodeId, LinkConfig)>,
        queue: Arc<Mutex<BinaryHeap<ScheduledEvent<Packet>>>>,
        now: SimTime,
    ) -> Self {
        SimInterface { me, rx, links, queue, now }
    }

    pub async fn send(
        &self,
        dst: &NodeId,
        data: Bytes,
    ) -> Result<(), SimError> {
        let cfg = self
            .links
            .iter()
            .find(|(a, b, _)| &self.me == a && dst == b)
            .map(|(_, _, cfg)| cfg.clone())
            .ok_or_else(|| SimError::UnknownNode(dst.0.clone()))?;

        let jitter_ns = rand::random::<u64>() % cfg.jitter.as_nanos() as u64;
        let at = self
            .now
            .checked_add(cfg.latency)
            .and_then(|t| t.checked_add(Duration::from_nanos(jitter_ns)))
            .unwrap();

        let pkt = Packet {
            src: self.me.clone(),
            dst: dst.clone(),
            data,
            at,
        };

        let mut q = self.queue.lock().await;
        q.push(ScheduledEvent { when: at, payload: pkt });
        Ok(())
    }

    pub async fn recv(&mut self) -> Result<(NodeId, Bytes), SimError> {
        match self.rx.recv().await {
            Some(pkt) => Ok((pkt.src, pkt.data)),
            None => Err(SimError::SimulationEnded),
        }
    }

    pub async fn sleep(&self, dur: Duration) {
        tokio::time::sleep(dur).await;
    }
}
