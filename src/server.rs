use anyhow::Result;
use log::{debug, error, info};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::config::{Endpoint, EndpointMode};
use crate::protocol::{handle_policy_check, handle_socketmap_lookup, handle_tcp_lookup};

const BUFFER_SIZE: usize = 8192;

pub async fn start_endpoint(endpoint: Arc<Endpoint>, user_agent: String) -> Result<()> {
    let addr = format!("{}:{}", endpoint.bind_address, endpoint.bind_port);
    let listener = TcpListener::bind(&addr).await?;

    info!(
        "Endpoint '{}' listening on {} (mode: {:?})",
        endpoint.name, addr, endpoint.mode
    );

    loop {
        match listener.accept().await {
            Ok((mut socket, addr)) => {
                debug!("New connection from {}", addr);

                let endpoint = Arc::clone(&endpoint);
                let user_agent = user_agent.clone();

                tokio::spawn(async move {
                    if let Err(e) = handle_connection(&mut socket, &endpoint, &user_agent).await {
                        error!("Connection error: {}", e);
                    }
                });
            }
            Err(e) => {
                error!("Accept error: {}", e);
            }
        }
    }
}

async fn handle_connection(
    socket: &mut tokio::net::TcpStream,
    endpoint: &Endpoint,
    user_agent: &str,
) -> Result<()> {
    let mut buffer = vec![0u8; BUFFER_SIZE];

    // Read request from Postfix
    let n = socket.read(&mut buffer).await?;
    if n == 0 {
        return Ok(());
    }

    let request = String::from_utf8_lossy(&buffer[..n]);
    debug!("Received {} bytes: {:?}", n, &request[..n.min(100)]);

    // Process based on mode
    let response = match endpoint.mode {
        EndpointMode::TcpLookup => {
            handle_tcp_lookup(endpoint, &request, user_agent).await?
        }
        EndpointMode::SocketmapLookup => {
            handle_socketmap_lookup(endpoint, &request, user_agent).await?
        }
        EndpointMode::Policy => {
            handle_policy_check(endpoint, &request, user_agent).await?
        }
    };

    // Send response back to Postfix
    socket.write_all(response.as_bytes()).await?;
    
    // Flush to ensure all data is sent immediately
    socket.flush().await?;
    
    debug!("Sent response: {} bytes", response.len());

    Ok(())
}
