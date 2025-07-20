use crate::{
    iface::SimInterface,
    link::{LinkConfig, Packet},
    time::{ScheduledEvent, SimTime},
};
use std::{
    collections::{BinaryHeap, HashMap, HashSet},
    future::Future,
    sync::Arc, time::Duration,
};
use tokio::{sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender}, time::sleep};
use tokio::sync::Mutex;
use tokio::time::Duration as TokioDuration;

/// unique node handle
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct NodeId(pub String);

/// builder for the simulation
pub struct SimNetBuilder {
    nodes: HashSet<NodeId>,
    links: Vec<(NodeId, NodeId, LinkConfig)>,
    speed: f64,
}

impl SimNetBuilder {
    pub fn new() -> Self {
        Self {
            nodes: HashSet::new(),
            links: Vec::new(),
            speed: 1.0,
        }
    }

    pub fn add_node(&mut self, id: impl Into<String>) -> &mut Self {
        self.nodes.insert(NodeId(id.into()));
        self
    }

    pub fn add_link(
        &mut self,
        a: impl Into<String>,
        b: impl Into<String>,
        cfg: LinkConfig,
    ) -> &mut Self {
        self.links.push((NodeId(a.into()), NodeId(b.into()), cfg));
        self
    }

    pub fn build(self) -> SimNet {
        SimNet::new(self.nodes, self.links, self.speed)
    }

    /// Set simulation speed:
    ///   0.0 = as fast as possible,
    ///   1.0 = real-time,
    ///  >1.0 = N× real-time.
    pub fn with_speed(mut self, speed: f64) -> Self {
        assert!(speed >= 0.0, "speed must be non-negative");
        self.speed = speed;
        self
    }
}

/// running simulation
pub struct SimNet {
    inbox_senders: HashMap<NodeId, UnboundedSender<Packet>>,
    inbox_receivers: HashMap<NodeId, UnboundedReceiver<Packet>>,
    links: Vec<(NodeId, NodeId, LinkConfig)>,
    queue: Arc<Mutex<BinaryHeap<ScheduledEvent<Packet>>>>,
    now: SimTime,
    speed: f64,
}

impl SimNet {
    fn new(
        nodes: HashSet<NodeId>,
        links: Vec<(NodeId, NodeId, LinkConfig)>,
        speed: f64,
    ) -> Self {
        let mut inbox_senders = HashMap::new();
        let mut inbox_receivers = HashMap::new();

        for node in nodes.into_iter() {
            let (tx, rx) = unbounded_channel();
            inbox_senders.insert(node.clone(), tx);
            inbox_receivers.insert(node, rx);
        }

        SimNet {
            inbox_senders,
            inbox_receivers,
            links,
            queue: Arc::new(Mutex::new(BinaryHeap::new())),
            now: SimTime(0),
            speed,
        }
    }

    /// spawn a tokio task that drives your node logic
    pub fn spawn_node<Fut>(
        &mut self,
        id: impl Into<String>,
        handler: impl FnOnce(SimInterface) -> Fut + Send + 'static,
    ) where
        Fut: Future<Output = ()> + Send + 'static,
    {
        let node = NodeId(id.into());

        let rx = self
            .inbox_receivers
            .remove(&node)
            .expect("unknown node");

        let iface = SimInterface::new(
            node,
            rx,
            self.links.clone(),
            Arc::clone(&self.queue),
            self.now,
        );

        tokio::spawn(async move {
            handler(iface).await;
        });
    }

    /// endless run the scheduler
    pub async fn run(mut self) {
        // let tasks enqueue their first sends
        tokio::task::yield_now().await;

        loop {
            sleep(Duration::from_millis(1)).await;

            while let Some(ScheduledEvent { when, payload }) = {
                let mut q = self.queue.lock().await;
                q.pop()
            } {
                // compute wall-clock wait = (when - now) / speed
                let delta_ns = when.0.saturating_sub(self.now.0);
                self.now = when;

                if self.speed > 0.0 {
                    // real-time or accelerated
                    let wait = (delta_ns as f64 / self.speed) as u64;
                    if wait > 0 {
                        tokio::time::sleep(TokioDuration::from_nanos(wait)).await;
                    }
                }
                // else speed == 0.0 → no sleep, max speed

                // deliver packet
                if let Some(tx) = self.inbox_senders.get(&payload.dst) {
                    let _ = tx.send(payload);
                }

                // let the node tasks run
                tokio::task::yield_now().await;
            }
        }

        // Dropping the senders will cause node tasks to observe SimulationEnded
    }
}
