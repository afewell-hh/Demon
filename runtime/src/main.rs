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

    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr = format!("0.0.0.0:{}", port);

    info!("Starting Demon Runtime on {}", addr);

    let listener = TcpListener::bind(&addr).await?;
    info!("Runtime server listening on {}", addr);

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
                            info!("Received request: {}", request.lines().next().unwrap_or(""));

                            // Simple HTTP response for health checks
                            let response = if request.contains("GET /health") {
                                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 15\r\n\r\n{\"status\":\"ok\"}"
                            } else if request.contains("GET /ready") {
                                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 17\r\n\r\n{\"ready\":true}"
                            } else {
                                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 29\r\n\r\n{\"service\":\"demon-runtime\"}"
                            };

                            if let Err(e) = stream.write_all(response.as_bytes()).await {
                                warn!("Failed to write response: {}", e);
                            }
                            // Flush and shutdown the stream properly
                            let _ = stream.flush().await;
                            let _ = stream.shutdown().await;
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
