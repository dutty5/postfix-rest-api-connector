use anyhow::Result;
use log::{debug, error, info, warn};
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
                        error!("Connection error from {}: {}", addr, e);
                    }
                    debug!("Connection closed from {}", addr);
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

    // CRITICAL FIX: Loop to handle multiple requests on the same connection
    // Postfix reuses TCP connections for multiple lookups
    loop {
        // Read request from Postfix
        let n = match socket.read(&mut buffer).await {
            Ok(0) => {
                // Connection closed by client (normal)
                debug!("Client closed connection");
                return Ok(());
            }
            Ok(n) => n,
            Err(e) => {
                warn!("Read error: {}", e);
                return Err(e.into());
            }
        };

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
        if let Err(e) = socket.write_all(response.as_bytes()).await {
            warn!("Write error: {}", e);
            return Err(e.into());
        }
        
        // CRITICAL: Flush the socket to ensure data is sent immediately
        if let Err(e) = socket.flush().await {
            warn!("Flush error: {}", e);
            return Err(e.into());
        }
        
        debug!("Sent response: {}", response.trim());

        // For Policy delegation, connection is typically closed after response
        // as per Postfix policy protocol specification
        if matches!(endpoint.mode, EndpointMode::Policy) {
            debug!("Policy check complete, closing connection");
            return Ok(());
        }

        // Continue loop to handle next request on same connection
    }
}
