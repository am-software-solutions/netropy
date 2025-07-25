Not using SemVer for now, will be used once v1.0.0 is reached...

Upcoming features:
- [x] command interface (for shutdown, simulate failure (like link down etc))
- [ ] e.g. YAML "driver"
- [ ] simulate reorder issues? not really need for TCP...
- [ ] link througput?
- [ ] send/recv buffers?
- [ ] record/replay
- [x] packet tracing (node packet in/out)
- [ ] swappable interface (which would allow importing real net code or export to net code)

Usage (see tester/src/main):

```rust
use netropy::{
    SimNetBuilder, LinkConfig, SimInterface, NodeId
};
use bytes::Bytes;
use tracing::Level;
use std::time::Duration;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        .init();

    let mut builder = SimNetBuilder::new();
    builder
        .add_node("A")
        .add_node("B")
        .add_bi_link(
            "A", "B",
            LinkConfig {
                latency: Duration::from_millis(5),
                jitter: Duration::from_millis(5),
            }, None
        ).await;

    let mut net = builder.build();

    net.spawn_node("A", |mut iface: SimInterface| async move {
        tracing::info!("start A");
        for i in 0..5 {
            let payload = format!("{}", i).into_bytes();
            tracing::debug!("A sent");
            iface.send(&NodeId("B".into()), Bytes::from(payload)).await.unwrap();
            if let Ok((src, data)) = iface.recv().await {
                tracing::debug!("A got from {}: {:?}", src.0, data);
            }
            iface.sleep(Duration::from_secs(1)).await;
        }
        tracing::info!("exit A");
    });

    net.spawn_node("B", |mut iface: SimInterface| async move {
        tracing::info!("start B");
        while let Ok((src, data)) = iface.recv().await {
            tracing::debug!("B got from {}: {:?}", src.0, data);
            if data == "2" {
                break;
            }
            iface.send(&src, data).await.unwrap();
            tracing::debug!("B sent");
        }
        tracing::info!("exit B");
    });



    net.run().await;
}
```