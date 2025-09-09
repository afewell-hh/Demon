use anyhow::{anyhow, Result};
use async_nats::jetstream;
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let subject = args.next().ok_or_else(|| anyhow!("subject required"))?;
    let json = args.next().ok_or_else(|| anyhow!("json required"))?;
    let msg_id = args.next(); // optional

    let url = env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string());
    let client = async_nats::connect(&url).await?;
    let js = jetstream::new(client);

    let headers = if let Some(id) = msg_id {
        let mut h = async_nats::HeaderMap::new();
        h.insert("Nats-Msg-Id", id.as_str());
        Some(h)
    } else {
        None
    };

    // Ensure a stream exists for ritual events (default RITUAL_EVENTS)
    let stream = env::var("RITUAL_STREAM_NAME").unwrap_or_else(|_| "RITUAL_EVENTS".to_string());
    let _ = js
        .get_or_create_stream(jetstream::stream::Config {
            name: stream,
            subjects: vec!["demon.ritual.v1.>".to_string()],
            ..Default::default()
        })
        .await?;

    if let Some(h) = headers {
        js.publish_with_headers(subject, h, json.into()).await?.await?;
    } else {
        js.publish(subject, json.into()).await?.await?;
    }
    Ok(())
}

