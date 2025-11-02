use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EndpointMode {
    TcpLookup,
    SocketmapLookup,
    Policy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Endpoint {
    pub name: String,
    pub mode: EndpointMode,
    pub target: String,
    pub bind_address: String,
    pub bind_port: u16,
    pub auth_token: String,
    pub request_timeout: u64, // milliseconds
    #[serde(skip)]
    pub http_client: Option<Arc<Client>>,
}

impl Endpoint {
    pub fn timeout(&self) -> Duration {
        Duration::from_millis(self.request_timeout)
    }
    
    pub fn with_client(mut self) -> Result<Self> {
        let client = Client::builder()
            .timeout(self.timeout())
            .pool_max_idle_per_host(50)
            .pool_idle_timeout(Duration::from_secs(90))
            .tcp_keepalive(Duration::from_secs(60))
            // http2_adaptive_window is enabled by default in reqwest 0.12+
            .build()
            .context("Failed to create HTTP client")?;
        self.http_client = Some(Arc::new(client));
        Ok(self)
    }
    
    pub fn client(&self) -> &Client {
        self.http_client.as_ref().expect("HTTP client not initialized")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    pub user_agent: String,
    pub endpoints: Vec<Endpoint>,
}

impl Config {
    pub fn from_file(path: &str) -> Result<Self> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path))?;

        let config: Config = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path))?;

        // Validate configuration
        if config.endpoints.is_empty() {
            anyhow::bail!("Configuration must have at least one endpoint");
        }

        Ok(config)
    }
}
