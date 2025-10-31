use anyhow::Result;
use log::{error, info};
use std::env;
use std::sync::Arc;
use tokio::signal;
use tokio::sync::broadcast;

mod config;
mod protocol;
mod server;

use config::Config;
use server::start_endpoint;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <config-file>", args[0]);
        std::process::exit(1);
    }

    info!("Starting Postfix REST API Connector...");

    // Load configuration
    let config = Config::from_file(&args[1])?;
    info!("Configuration loaded: {} endpoints", config.endpoints.len());

    let config = Arc::new(config);

    // Create shutdown channel
    let (shutdown_tx, _) = broadcast::channel(1);

    // Start all endpoint servers
    let mut handles = Vec::new();

    for endpoint in &config.endpoints {
        let endpoint = Arc::new(endpoint.clone().with_client()?);
        let user_agent = config.user_agent.clone();
        let mut shutdown_rx = shutdown_tx.subscribe();

        let handle = tokio::spawn(async move {
            tokio::select! {
                result = start_endpoint(endpoint, user_agent) => {
                    if let Err(e) = result {
                        error!("Endpoint error: {}", e);
                    }
                }
                _ = shutdown_rx.recv() => {
                    info!("Endpoint received shutdown signal");
                }
            }
        });

        handles.push(handle);
    }

    // Wait for shutdown signal
    info!("All endpoints started. Press Ctrl+C to shutdown.");
    
    match signal::ctrl_c().await {
        Ok(()) => {
            info!("Shutdown signal received, stopping...");
        }
        Err(err) => {
            error!("Unable to listen for shutdown signal: {}", err);
        }
    }

    // Send shutdown signal to all tasks
    let _ = shutdown_tx.send(());

    // Give tasks time to shutdown gracefully
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Abort remaining tasks
    for handle in handles {
        handle.abort();
    }

    info!("Shutdown complete");
    Ok(())
}
