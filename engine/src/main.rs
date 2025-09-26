use anyhow::Result;
use std::env;
use tokio::net::TcpListener;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let port = env::var("PORT").unwrap_or_else(|_| "8081".to_string());
    let addr = format!("0.0.0.0:{}", port);

    info!("Starting Demon Engine on {}", addr);

    let listener = TcpListener::bind(&addr).await?;
    info!("Engine server listening on {}", addr);

    loop {
        match listener.accept().await {
            Ok((mut stream, addr)) => {
                info!("Connection from {}", addr);
                tokio::spawn(async move {
                    use tokio::io::{AsyncReadExt, AsyncWriteExt};

                    let mut buffer = [0; 1024];
                    match stream.read(&mut buffer).await {
                        Ok(0) => {
                            info!("Connection closed by client");
                        }
                        Ok(n) => {
                            let request = String::from_utf8_lossy(&buffer[..n]);
                            let request_line = request.lines().next().unwrap_or("");
                            info!("Received request: {}", request_line);

                            let path = request_line.split_whitespace().nth(1).unwrap_or("/");

                            let response_body = match path {
                                "/health" => r#"{"status":"ok"}"#,
                                "/ready" => r#"{"ready":true}"#,
                                _ => r#"{"service":"demon-engine"}"#,
                            };

                            let response = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                response_body.len(),
                                response_body
                            );

                            if let Err(e) = stream.write_all(response.as_bytes()).await {
                                warn!("Failed to write response: {}", e);
                            }
                            if let Err(e) = stream.flush().await {
                                warn!("Failed to flush response: {}", e);
                            }
                            if let Err(e) = stream.shutdown().await {
                                warn!("Failed to shutdown stream: {}", e);
                            }
                        }
                        Err(e) => {
                            warn!("Failed to read from stream: {}", e);
                        }
                    }
                });
            }
            Err(e) => {
                warn!("Failed to accept connection: {}", e);
            }
        }
    }
}
