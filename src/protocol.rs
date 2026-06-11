use anyhow::Result;
use log::{debug, error, warn};
use serde_json::Value;
use url::Url;

use crate::config::Endpoint;

// Postfix protocol constants
const TCP_MAXIMUM_RESPONSE_LENGTH: usize = 4096;
const SOCKETMAP_MAXIMUM_RESPONSE_LENGTH: usize = 100000;
const END_CHAR: char = '\n';

/// Upper bound for a netstring payload we are willing to parse (shared with
/// the framing layer in server.rs). Mirrors the socketmap response limit.
pub const MAX_NETSTRING_LENGTH: usize = SOCKETMAP_MAXIMUM_RESPONSE_LENGTH;

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

/// Percent-encode a single application/x-www-form-urlencoded token
/// (everything except RFC 3986 unreserved characters gets encoded).
fn form_encode(data: &str) -> String {
    use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};

    const FORM: &percent_encoding::AsciiSet = &NON_ALPHANUMERIC
        .remove(b'-')
        .remove(b'.')
        .remove(b'_')
        .remove(b'~');

    utf8_percent_encode(data, FORM).to_string()
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
/// (pub: also used by the framing layer to report framing errors)
pub fn encode_netstring(data: &str) -> String {
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

    // Defense in depth: the framing layer already enforces this bound, but
    // this function must stay safe on any input. The cap also makes the
    // arithmetic below overflow-free.
    if length > MAX_NETSTRING_LENGTH {
        warn!("Netstring: declared length {} exceeds limit {}", length, MAX_NETSTRING_LENGTH);
        return None;
    }

    // Check if we have enough data (checked arithmetic: a hostile length
    // close to usize::MAX must not panic in debug builds)
    let data_start = colon_pos.checked_add(1)?;
    let data_end = data_start.checked_add(length)?;

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
    // split_whitespace() already trims, so no need to call trim() first
    let parts: Vec<&str> = request.split_whitespace().collect();
    if parts.len() < 2 || parts[0] != "get" {
        return format_tcp_response(500, "Invalid request");
    }

    // Per tcp_table(5) the key arrives %XX-quoted (space and non-printable
    // characters are encoded by Postfix). Decode it before building the URL,
    // otherwise query_pairs_mut() would re-encode the '%' itself and the API
    // would receive a double-encoded key.
    let key = percent_encoding::percent_decode_str(parts[1]).decode_utf8_lossy();
    debug!("TCP lookup for key: {}", key);

    // Build URL
    let mut url = Url::parse(&endpoint.target)?;
    url.query_pairs_mut().append_pair("key", &key);

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
                            .map(encode_response)
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
                // 500 in tcp_table speak means "not found / try later" - the
                // normal negative-lookup answer, not a hard failure.
                format_tcp_response(500, "Not found")
            } else if status.is_client_error() {
                // HTTP 4xx from the API (bad token, bad request shape) is a
                // configuration problem: retrying cannot help -> permanent.
                format_tcp_response(400, "Client error")
            } else if status.is_server_error() {
                // HTTP 5xx from the API is transient (DB restart, API
                // redeploy): Postfix should defer and retry -> temporary.
                format_tcp_response(500, "Server error")
            } else {
                format_tcp_response(500, "Unknown error")
            }
        }
        Err(e) => {
            // Network-level failure (refused / timeout / TLS): the API or
            // the path to it is down right now - classic transient state,
            // so signal temporary and let Postfix retry.
            error!("HTTP request failed: {}", e);
            format_tcp_response(500, "Connection failed")
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
                            .map(encode_response)
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
    //
    // Names and values MUST be form-encoded individually: attribute values
    // are raw client-controlled strings (helo_name, sender, ...) and may
    // contain '&', '=', '%' or '+', which would otherwise corrupt every
    // following pair in the form body.
    let body = request
        .lines()
        .filter(|line| !line.is_empty())  // Remove empty lines
        .map(|line| match line.split_once('=') {
            Some((name, value)) => format!("{}={}", form_encode(name), form_encode(value)),
            // A line without '=' violates the protocol; pass it through
            // encoded as a bare name so the API can see (and log) it.
            None => form_encode(line),
        })
        .collect::<Vec<String>>()
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
                // Even a configuration error must not bounce mail at the
                // policy stage: DEFER_IF_PERMIT keeps it soft.
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
