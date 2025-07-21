use crate::{
    iface::{SimCommand, SimInterface},
    link::{LinkConfig, Packet},
    time::{ScheduledEvent, SimTime},
};
use std::{
    collections::{BinaryHeap, HashMap, HashSet},
    future::Future,
    sync::Arc,
};
use tokio::{sync::{mpsc::{self, unbounded_channel, Receiver, Sender, UnboundedReceiver, UnboundedSender}, RwLock}, time::timeout};
use tokio::sync::Mutex;
use tokio::time::Duration as TokioDuration;

/// unique node handle
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct NodeId(pub String);

/// builder for the simulation
pub struct SimNetBuilder {
    nodes: HashSet<NodeId>,
    links: Arc<RwLock<Vec<(NodeId, NodeId, LinkConfig)>>>,
    speed: f64,
}

impl SimNetBuilder {
    pub fn new() -> Self {
        Self {
            nodes: HashSet::new(),
            links: Arc::new(RwLock::new(Vec::new())),
            speed: 1.0,
        }
    }

    pub fn add_node(&mut self, id: impl Into<String>) -> &mut Self {
        self.nodes.insert(NodeId(id.into()));
        self
    }

    // TODO: candidate for async
    pub fn add_link(
        &mut self,
        a: impl Into<String>,
        b: impl Into<String>,
        cfg: LinkConfig,
    ) -> &mut Self {
        let links = self.links.clone();
        let mut w = links.blocking_write();
        w.push((NodeId(a.into()), NodeId(b.into()), cfg));
        self
    }

    pub async fn add_bi_link(
        &mut self,
        a: impl Into<String>,
        b: impl Into<String>,
        cfg: LinkConfig,
        cfg_back: Option<LinkConfig>,
    ) -> &mut Self {
        let links = self.links.clone();
        let a = NodeId(a.into());
        let b = NodeId(b.into());
        let mut w = links.write().await;
        w.push((a.clone(), b.clone(), cfg.clone()));
        w.push((b, a, cfg_back.unwrap_or(cfg)));
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
    links: Arc<RwLock<Vec<(NodeId, NodeId, LinkConfig)>>>,
    queue: Arc<Mutex<BinaryHeap<ScheduledEvent<Packet>>>>,
    command_channel: (Sender<SimCommand>, Receiver<SimCommand>),
    now: SimTime,
    speed: f64,
}

impl SimNet {
    fn new(
        nodes: HashSet<NodeId>,
        links: Arc<RwLock<Vec<(NodeId, NodeId, LinkConfig)>>>,
        speed: f64,
    ) -> Self {
        let mut inbox_senders = HashMap::new();
        let mut inbox_receivers = HashMap::new();

        let command_channel = mpsc::channel(32);

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
            command_channel,
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

    /// returns a sender for enqueuing sim commands
    pub fn command_tx(&self) -> Sender<SimCommand> {
        self.command_channel.0.clone()
    }

    /// endless run the scheduler, until receive of shutdown command
    pub async fn run(mut self) {
        // let tasks enqueue their first sends
        tokio::task::yield_now().await;

        loop {
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
                    Self::packet_log(&format!("rx {payload:?}"));
                    let _ = tx.send(payload);
                }

                // let the node tasks run
                tokio::task::yield_now().await;
            }

            // handle sim commands
            // TODO: whole loop is suboptimal, we should use a notify mechanic if smth is added to the queue
            // combined with tokio select for polling on queue and command channel
            match timeout(TokioDuration::from_millis(1), self.command_channel.1.recv()).await {
                Ok(Some(cmd)) => {
                    match cmd {
                        SimCommand::Shutdown => {
                            tracing::info!("received shutdown command");
                            break;
                        },
                    }
                },
                Ok(None) => {},
                Err(_) => {},
            }
        }

        // Dropping the senders will cause node tasks to observe SimulationEnded
    }

    #[cfg(feature = "packet_tracing")]
    fn packet_log(msg: &str) {
        tracing::debug!("{msg}");
    }

    #[cfg(not(feature = "packet_tracing"))]
    fn packet_log(msg: &str) {
    }
}
