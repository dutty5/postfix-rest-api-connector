use anyhow::Result;
use log::{debug, error, warn};
use serde_json::Value;
use url::Url;

use crate::config::Endpoint;

// Postfix protocol constants
const TCP_MAXIMUM_RESPONSE_LENGTH: usize = 4096;
const SOCKETMAP_MAXIMUM_RESPONSE_LENGTH: usize = 100000;
const END_CHAR: char = '\n';

/// URL-encode response data per Postfix specification
/// Uses path segment encoding (encodes /, space, but NOT @ or -)
fn encode_response(data: &str) -> String {
    use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
    
    // Define characters that should NOT be encoded
    // Based on RFC 3986 path segment: unreserved + @ + :
    const ALLOWED: &percent_encoding::AsciiSet = &NON_ALPHANUMERIC
        .remove(b'-')
        .remove(b'.')
        .remove(b'_')
        .remove(b'~')
        .remove(b'@')  // Don't encode @
        .remove(b':')  // Don't encode :
        .remove(b'!');
    
    utf8_percent_encode(data, ALLOWED).to_string()
}

/// Format Postfix TCP response - ALL text is encoded per spec
fn format_tcp_response(code: u16, data: &str) -> Result<String> {
    let encoded = encode_response(data);
    let response = format!("{} {}{}", code, encoded, END_CHAR);
    
    // Check length limit (4096 bytes including newline)
    if response.len() > TCP_MAXIMUM_RESPONSE_LENGTH {
        warn!("Response exceeds maximum length: {} > {}", 
              response.len(), TCP_MAXIMUM_RESPONSE_LENGTH);
        // Return error response
        Ok(format!("500 Response%20too%20long{}", END_CHAR))
    } else {
        Ok(response)
    }
}

/// Encode response as netstring for socketmap protocol
/// Format: <length>:<data>,
fn encode_netstring(data: &str) -> String {
    format!("{}:{},", data.len(), data)
}

/// Decode netstring from socketmap request
/// Format: <length>:<data>,
fn decode_netstring(input: &[u8]) -> Option<String> {
    // Find the colon separator
    let colon_pos = input.iter().position(|&b| b == b':')?;
    
    // Parse length
    let length_str = std::str::from_utf8(&input[..colon_pos]).ok()?;
    let length: usize = length_str.parse().ok()?;
    
    // Check if we have enough data
    let data_start = colon_pos + 1;
    let data_end = data_start + length;
    
    // Debug logging
    debug!("Netstring parse: length={}, data_start={}, data_end={}, input.len()={}", 
           length, data_start, data_end, input.len());
    
    if data_end >= input.len() {
        warn!("Netstring: data_end ({}) >= input.len() ({})", data_end, input.len());
        return None;
    }
    
    if input[data_end] != b',' {
        warn!("Netstring: expected comma at position {}, found: {:?}", 
              data_end, input[data_end] as char);
        return None;
    }
    
    // Extract data
    let data = std::str::from_utf8(&input[data_start..data_end]).ok()?;
    debug!("Netstring decoded successfully: '{}'", data);
    Some(data.to_string())
}

/// Handle TCP lookup protocol
pub async fn handle_tcp_lookup(
    endpoint: &Endpoint,
    request: &str,
    user_agent: &str,
) -> Result<String> {
    // Parse: "get SPACE key NEWLINE"
    let parts: Vec<&str> = request.trim().split_whitespace().collect();
    if parts.len() < 2 || parts[0] != "get" {
        return format_tcp_response(500, "Invalid request");
    }

    let key = parts[1];
    debug!("TCP lookup for key: {}", key);

    // Build URL
    let mut url = Url::parse(&endpoint.target)?;
    url.query_pairs_mut().append_pair("key", key);

    // Use the pre-created HTTP client (connection pooling!)
    let response = endpoint.client()
        .get(url)
        .header("X-Auth-Token", &endpoint.auth_token)
        .header("User-Agent", user_agent)
        .send()
        .await;

    match response {
        Ok(resp) => {
            let status = resp.status();
            debug!("HTTP response code: {}", status);

            if status.is_success() {
                // Parse JSON array response
                match resp.json::<Value>().await {
                    Ok(Value::Array(arr)) if !arr.is_empty() => {
                        // Encode each value and join with commas
                        let encoded_values: Vec<String> = arr
                            .iter()
                            .filter_map(|v| v.as_str())
                            .map(|s| encode_response(s))
                            .collect();
                        
                        if encoded_values.is_empty() {
                            format_tcp_response(500, "Empty result")
                        } else {
                            // Join encoded values with literal commas
                            let joined = encoded_values.join(",");
                            let response = format!("200 {}{}", joined, END_CHAR);
                            
                            if response.len() > TCP_MAXIMUM_RESPONSE_LENGTH {
                                warn!("Response exceeds maximum length: {} > {}", 
                                      response.len(), TCP_MAXIMUM_RESPONSE_LENGTH);
                                Ok(format!("500 Response%20too%20long{}", END_CHAR))
                            } else {
                                Ok(response)
                            }
                        }
                    }
                    Ok(_) => format_tcp_response(500, "Empty result"),
                    Err(e) => {
                        error!("JSON parse error: {}", e);
                        format_tcp_response(500, "Invalid JSON")
                    }
                }
            } else if status.as_u16() == 404 {
                format_tcp_response(500, "Not found")
            } else if status.is_client_error() {
                format_tcp_response(400, "Client error")
            } else if status.is_server_error() {
                format_tcp_response(400, "Server error")
            } else {
                format_tcp_response(500, "Unknown error")
            }
        }
        Err(e) => {
            error!("HTTP request failed: {}", e);
            format_tcp_response(400, "Connection failed")
        }
    }
}

/// Handle socketmap lookup protocol (uses netstring format!)
pub async fn handle_socketmap_lookup(
    endpoint: &Endpoint,
    request: &str,
    user_agent: &str,
) -> Result<String> {
    // Socketmap uses netstring protocol
    debug!("Received socketmap request: {} bytes", request.len());
    
    // Decode the netstring request
    let decoded = match decode_netstring(request.as_bytes()) {
        Some(data) => data,
        None => {
            warn!("Invalid netstring format. Received: {:?}", 
                  String::from_utf8_lossy(request.as_bytes()));
            return Ok(encode_netstring("TEMP Invalid netstring format"));
        }
    };
    
    // Parse: "name SPACE key"
    let parts: Vec<&str> = decoded.splitn(2, ' ').collect();
    
    if parts.len() != 2 {
        return Ok(encode_netstring("TEMP Invalid request"));
    }

    let mapname = parts[0];
    let key = parts[1];
    
    debug!("Socketmap lookup - map: {}, key: {}", mapname, key);

    // Build URL
    let mut url = Url::parse(&endpoint.target)?;
    url.query_pairs_mut()
        .append_pair("name", mapname)
        .append_pair("key", key);

    // Use the pre-created HTTP client
    let response = endpoint.client()
        .get(url)
        .header("X-Auth-Token", &endpoint.auth_token)
        .header("User-Agent", user_agent)
        .send()
        .await;

    match response {
        Ok(resp) => {
            let status = resp.status();
            debug!("HTTP response code: {}", status);

            if status.is_success() {
                match resp.json::<Value>().await {
                    Ok(Value::Array(arr)) if !arr.is_empty() => {
                        // Encode each value and join with commas
                        let encoded_values: Vec<String> = arr
                            .iter()
                            .filter_map(|v| v.as_str())
                            .map(|s| encode_response(s))
                            .collect();
                        
                        if encoded_values.is_empty() {
                            Ok(encode_netstring("NOTFOUND "))
                        } else {
                            let joined = encoded_values.join(",");
                            let response_text = format!("OK {}", joined);
                            
                            if response_text.len() > SOCKETMAP_MAXIMUM_RESPONSE_LENGTH {
                                warn!("Socketmap response too long: {} bytes", response_text.len());
                                Ok(encode_netstring("TEMP Response too long"))
                            } else {
                                Ok(encode_netstring(&response_text))
                            }
                        }
                    }
                    Ok(_) => Ok(encode_netstring("NOTFOUND ")),
                    Err(e) => {
                        error!("JSON parse error: {}", e);
                        Ok(encode_netstring("TEMP Invalid JSON"))
                    }
                }
            } else if status.as_u16() == 404 {
                Ok(encode_netstring("NOTFOUND "))
            } else if status.is_client_error() {
                Ok(encode_netstring("PERM Configuration error"))
            } else if status.is_server_error() {
                Ok(encode_netstring("TEMP Server error"))
            } else {
                Ok(encode_netstring("TEMP Unknown error"))
            }
        }
        Err(e) => {
            error!("HTTP request failed: {}", e);
            Ok(encode_netstring("TEMP Connection failed"))
        }
    }
}

/// Handle policy check protocol
pub async fn handle_policy_check(
    endpoint: &Endpoint,
    request: &str,
    user_agent: &str,
) -> Result<String> {
    debug!("Policy check request");

    // Convert Postfix policy format (newline-separated) to URL-encoded format
    // Postfix sends: "name=value\nname2=value2\n\n"
    // REST API expects: "name=value&name2=value2"
    let body = request
        .lines()
        .filter(|line| !line.is_empty())  // Remove empty lines
        .collect::<Vec<&str>>()
        .join("&");  // Join with & instead of newlines

    debug!("Converted policy request body: {}", body);

    // Use the pre-created HTTP client
    let response = endpoint.client()
        .post(&endpoint.target)
        .header("X-Auth-Token", &endpoint.auth_token)
        .header("User-Agent", user_agent)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await;

    match response {
        Ok(resp) => {
            let status = resp.status();
            debug!("HTTP response code: {}", status);

            if status.is_success() {
                match resp.text().await {
                    Ok(text) => {
                        let trimmed = text.trim();
                        
                        // Validate response format (should start with "action=")
                        if !trimmed.starts_with("action=") {
                            warn!("Invalid policy response format: {}", trimmed);
                            return Ok("action=DEFER_IF_PERMIT Invalid response format\n\n".to_string());
                        }
                        
                        // Policy response format: "action=DUNNO\n\n" (double newline required)
                        let response = format!("{}\n\n", trimmed);
                        
                        if response.len() > TCP_MAXIMUM_RESPONSE_LENGTH {
                            warn!("Policy response too long: {} bytes", response.len());
                            Ok("action=DEFER_IF_PERMIT Response too long\n\n".to_string())
                        } else {
                            Ok(response)
                        }
                    }
                    Err(e) => {
                        error!("Failed to read response: {}", e);
                        Ok("action=DEFER_IF_PERMIT Service error\n\n".to_string())
                    }
                }
            } else if status.is_client_error() {
                Ok("action=DEFER_IF_PERMIT Configuration error\n\n".to_string())
            } else if status.is_server_error() {
                Ok("action=DEFER_IF_PERMIT Server error\n\n".to_string())
            } else {
                Ok("action=DEFER_IF_PERMIT Unknown error\n\n".to_string())
            }
        }
        Err(e) => {
            error!("HTTP request failed: {}", e);
            Ok("action=DEFER_IF_PERMIT Service unavailable\n\n".to_string())
        }
    }
}
