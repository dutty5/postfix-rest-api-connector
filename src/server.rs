use anyhow::{Context, Result};
use log::{debug, error, info, warn};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::config::{Endpoint, EndpointMode};
use crate::protocol::{
    encode_netstring, handle_policy_check, handle_socketmap_lookup, handle_tcp_lookup,
    MAX_NETSTRING_LENGTH,
};

const BUFFER_SIZE: usize = 8192;

/// Hard cap on accumulated request bytes per connection.
/// The largest legitimate frame is a socketmap netstring (bounded by
/// MAX_NETSTRING_LENGTH); 256 KiB leaves headroom for all three protocols
/// while bounding per-connection memory against a misbehaving client.
const MAX_REQUEST_BUFFER: usize = 256 * 1024;

/// Bind the endpoint's listener. Separated from serving so that bind
/// failures (port already in use, bad address in config) propagate to
/// main() and fail the whole service fast with a non-zero exit, instead
/// of being logged inside a spawned task while the service keeps running
/// partially configured.
pub async fn bind_endpoint(endpoint: &Endpoint) -> Result<TcpListener> {
    let addr = format!("{}:{}", endpoint.bind_address, endpoint.bind_port);
    let listener = TcpListener::bind(&addr)
        .await
        .with_context(|| format!("Endpoint '{}': failed to bind {}", endpoint.name, addr))?;

    info!(
        "Endpoint '{}' listening on {} (mode: {:?})",
        endpoint.name, addr, endpoint.mode
    );

    Ok(listener)
}

/// Accept loop for a bound endpoint.
pub async fn serve_endpoint(
    listener: TcpListener,
    endpoint: Arc<Endpoint>,
    user_agent: String,
) -> Result<()> {
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
                // Accept can fail in bursts (e.g. EMFILE when out of file
                // descriptors). Without a pause this loop would spin hot,
                // flooding the log and burning CPU; back off briefly.
                error!("Accept error: {}", e);
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
}

/// Result of trying to extract one complete protocol frame from the
/// accumulation buffer.
enum Frame {
    /// One complete request: the frame text and how many buffer bytes it consumed.
    Complete { request: String, consumed: usize },
    /// Frame not complete yet - need more bytes from the socket.
    Incomplete,
    /// Framing irrecoverably broken (protocol desync) - close the connection.
    Invalid(&'static str),
}

/// Extract one complete frame according to the endpoint protocol.
///
/// TCP is a byte stream: a single read() may deliver half a request or
/// several requests glued together. Each protocol defines its own frame
/// boundary, and we must honor it instead of assuming one read == one
/// request:
///   - tcp-lookup:  one line, terminated by '\n'  (tcp_table(5))
///   - policy:      attribute lines terminated by an empty line, i.e. "\n\n"
///   - socketmap:   one netstring "<len>:<data>,"  (socketmap_table(5))
fn extract_frame(buf: &[u8], mode: &EndpointMode) -> Frame {
    match mode {
        EndpointMode::TcpLookup => match buf.iter().position(|&b| b == b'\n') {
            Some(pos) => Frame::Complete {
                request: String::from_utf8_lossy(&buf[..=pos]).into_owned(),
                consumed: pos + 1,
            },
            None => Frame::Incomplete,
        },
        EndpointMode::Policy => match buf.windows(2).position(|w| w == b"\n\n") {
            Some(pos) => Frame::Complete {
                request: String::from_utf8_lossy(&buf[..pos + 2]).into_owned(),
                consumed: pos + 2,
            },
            None => Frame::Incomplete,
        },
        EndpointMode::SocketmapLookup => extract_netstring_frame(buf),
    }
}

/// Netstring framing: "<digits>:<data>,". The declared length tells us
/// exactly how many bytes to wait for, so partial reads are handled
/// precisely rather than guessed at.
fn extract_netstring_frame(buf: &[u8]) -> Frame {
    let colon = match buf.iter().position(|&b| b == b':') {
        Some(p) => p,
        None => {
            // No colon yet: everything so far must be length digits.
            if buf.iter().any(|b| !b.is_ascii_digit()) {
                return Frame::Invalid("netstring: non-digit in length prefix");
            }
            if buf.len() > 7 {
                return Frame::Invalid("netstring: length prefix too long");
            }
            return Frame::Incomplete;
        }
    };

    if colon == 0 || colon > 7 {
        return Frame::Invalid("netstring: bad length prefix");
    }

    let length: usize = match std::str::from_utf8(&buf[..colon])
        .ok()
        .and_then(|s| s.parse().ok())
    {
        Some(l) => l,
        None => return Frame::Invalid("netstring: invalid length prefix"),
    };

    if length > MAX_NETSTRING_LENGTH {
        return Frame::Invalid("netstring: declared length too large");
    }

    // ':' + data + ',' ; bounded above by 7 + 1 + MAX_NETSTRING_LENGTH + 1,
    // so no overflow is possible here.
    let total = colon + 1 + length + 1;
    if buf.len() < total {
        return Frame::Incomplete;
    }
    if buf[total - 1] != b',' {
        return Frame::Invalid("netstring: missing trailing comma");
    }

    Frame::Complete {
        request: String::from_utf8_lossy(&buf[..total]).into_owned(),
        consumed: total,
    }
}

async fn handle_connection(
    socket: &mut tokio::net::TcpStream,
    endpoint: &Endpoint,
    user_agent: &str,
) -> Result<()> {
    // Accumulation buffer: bytes are appended from the socket and consumed
    // frame by frame. This correctly handles both a request split across
    // several reads and several requests arriving in one read.
    let mut buf: Vec<u8> = Vec::with_capacity(BUFFER_SIZE);
    let mut chunk = vec![0u8; BUFFER_SIZE];

    loop {
        // Drain every complete frame currently in the buffer.
        loop {
            match extract_frame(&buf, &endpoint.mode) {
                Frame::Complete { request, consumed } => {
                    buf.drain(..consumed);

                    // char-based truncation: byte-slicing a &str can panic on
                    // a UTF-8 boundary (and lossy conversion changes lengths).
                    debug!(
                        "Received frame ({} bytes): {:?}",
                        consumed,
                        request.chars().take(100).collect::<String>()
                    );

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

                    if let Err(e) = socket.write_all(response.as_bytes()).await {
                        warn!("Write error: {}", e);
                        return Err(e.into());
                    }
                    if let Err(e) = socket.flush().await {
                        warn!("Flush error: {}", e);
                        return Err(e.into());
                    }

                    debug!("Sent response: {}", response.trim());

                    // The connection stays open for ALL modes, including
                    // policy: per SMTPD_POLICY_README the Postfix policy
                    // client keeps the connection open and reuses it for
                    // subsequent requests; closing after each reply would
                    // force a reconnect per SMTP event.
                }
                Frame::Incomplete => break,
                Frame::Invalid(reason) => {
                    warn!("Protocol framing error: {} - closing connection", reason);
                    // Best-effort error reply so Postfix sees a TEMP failure
                    // instead of a bare disconnect (socketmap only; for the
                    // line protocols there is no parsable frame to answer).
                    if matches!(endpoint.mode, EndpointMode::SocketmapLookup) {
                        let _ = socket
                            .write_all(encode_netstring("TEMP Invalid netstring format").as_bytes())
                            .await;
                        let _ = socket.flush().await;
                    }
                    return Ok(());
                }
            }
        }

        if buf.len() > MAX_REQUEST_BUFFER {
            warn!(
                "Request exceeds {} bytes without completing a frame - closing connection",
                MAX_REQUEST_BUFFER
            );
            return Ok(());
        }

        let n = match socket.read(&mut chunk).await {
            Ok(0) => {
                if buf.is_empty() {
                    debug!("Client closed connection");
                } else {
                    debug!(
                        "Client closed connection with {} unconsumed bytes (incomplete frame)",
                        buf.len()
                    );
                }
                return Ok(());
            }
            Ok(n) => n,
            Err(e) => {
                warn!("Read error: {}", e);
                return Err(e.into());
            }
        };

        buf.extend_from_slice(&chunk[..n]);
    }
}
