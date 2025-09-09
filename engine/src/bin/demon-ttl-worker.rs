use engine::rituals::worker::ttl_worker::{run_loop, TtlWorkerConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let enabled = std::env::var("TTL_WORKER_ENABLED").unwrap_or_else(|_| "0".to_string()) == "1";
    if !enabled {
        println!("TTL worker disabled (set TTL_WORKER_ENABLED=1 to start)");
        return Ok(());
    }
    let cfg = TtlWorkerConfig::default();
    run_loop(cfg).await
}
