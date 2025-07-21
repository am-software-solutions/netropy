use crate::{
    error::SimError,
    link::Packet,
    network::NodeId,
    time::{ScheduledEvent, SimTime},
};
use bytes::Bytes;
use std::{
    collections::BinaryHeap,
    sync::{Arc, atomic::{AtomicU64, Ordering}},
    time::Duration,
};
use tokio::sync::{mpsc::UnboundedReceiver, Mutex, RwLock};
use crate::link::LinkConfig;

static CURRENT_PACKET_ID: AtomicU64 = AtomicU64::new(0);

pub enum SimCommand {
    Shutdown,
}

pub struct SimInterface {
    me: NodeId,
    rx: UnboundedReceiver<Packet>,
    links: Arc<RwLock<Vec<(NodeId, NodeId, LinkConfig)>>>,
    queue: Arc<Mutex<BinaryHeap<ScheduledEvent<Packet>>>>,
    now: SimTime,
}

impl SimInterface {
    pub(crate) fn new(
        me: NodeId,
        rx: UnboundedReceiver<Packet>,
        links: Arc<RwLock<Vec<(NodeId, NodeId, LinkConfig)>>>,
        queue: Arc<Mutex<BinaryHeap<ScheduledEvent<Packet>>>>,
        now: SimTime,
    ) -> Self {
        Self { me, rx, links, queue, now }
    }

    pub async fn send(
        &self,
        dst: &NodeId,
        data: Bytes,
    ) -> Result<(), SimError> {
        let link_read = self.links.read().await;
        let cfg = link_read
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

        CURRENT_PACKET_ID.fetch_add(1, Ordering::SeqCst);

        let pkt = Packet {
            src: self.me.clone(),
            dst: dst.clone(),
            data,
            at,
            id: CURRENT_PACKET_ID.load(Ordering::SeqCst).to_string()
        };

        let mut q = self.queue.lock().await;
        Self::packet_log(&format!("tx {pkt:?}"));
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

    #[cfg(feature = "packet_tracing")]
    fn packet_log(msg: &str) {
        tracing::debug!("{msg}");
    }

    #[cfg(not(feature = "packet_tracing"))]
    fn packet_log(msg: &str) {
    }
}
