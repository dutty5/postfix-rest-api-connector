# Postfix REST API Connector

A high-performance, memory-safe TCP server that bridges Postfix mail server with REST API endpoints. Written in Rust for maximum reliability and performance.

## 🚀 Features

- **Zero GC pauses** - No garbage collection, predictable latency
- **Memory safe** - Impossible to have buffer overflows, use-after-free, or memory leaks
- **High performance** - Async I/O handles thousands of concurrent connections efficiently
- **No external dependencies** - Single static binary, no libcurl or cJSON needed
- **Production ready** - Comprehensive error handling and logging

## 🎯 Supported Protocols

1. **TCP Lookup** - Simple key-value lookups
2. **Socketmap** - Named map lookups (netstring protocol) 
3. **Policy Delegation** - SMTP policy checks

## 📦 Quick Start

### Installation

```bash
# 1. Install Rust (if not installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# 2. Build and create RPM
chmod +x build-rpm-rust.sh
./build-rpm-rust.sh 1.0.0

# 3. Install
sudo rpm -ivh ~/rpmbuild/RPMS/x86_64/postfix-rest-api-connector-*.rpm

# 4. Configure
sudo cp /etc/postfix-rest-api-connector/config.json{.sample,}
sudo vim /etc/postfix-rest-api-connector/config.json

# 5. Start
sudo systemctl enable --now postfix-rest-api-connector
```

### Quick Build (without RPM)

```bash
# Build release binary
cargo build --release

# Run directly
RUST_LOG=info ./target/release/postfix-rest-api-connector config.json
```

## 🔧 Configuration

Create `/etc/postfix-rest-api-connector/config.json`:

```json
{
  "user-agent": "Postfix REST API Connector",
  "endpoints": [
    {
      "name": "domain-lookup",
      "mode": "tcp-lookup",
      "target": "https://api.example.com/api/postfix/domain",
      "bind-address": "127.0.0.1",
      "bind-port": 9001,
      "auth-token": "your-secure-token",
      "request-timeout": 2000
    },
    {
      "name": "mailbox-lookup",
      "mode": "tcp-lookup",
      "target": "https://api.example.com/api/postfix/mailbox",
      "bind-address": "127.0.0.1",
      "bind-port": 9002,
      "auth-token": "your-secure-token",
      "request-timeout": 2000
    },
    {
      "name": "socketmap",
      "mode": "socketmap-lookup",
      "target": "https://api.example.com/api/postfix/socketmap",
      "bind-address": "127.0.0.1",
      "bind-port": 9003,
      "auth-token": "your-secure-token",
      "request-timeout": 2000
    },
    {
      "name": "policy-check",
      "mode": "policy",
      "target": "https://api.example.com/api/postfix/policy",
      "bind-address": "127.0.0.1",
      "bind-port": 9004,
      "auth-token": "your-secure-token",
      "request-timeout": 2000
    }
  ]
}
```

## 🔌 Postfix Integration

Add to `/etc/postfix/main.cf`:

```conf
# TCP lookup tables
virtual_mailbox_domains = tcp:127.0.0.1:9001
virtual_mailbox_maps = tcp:127.0.0.1:9002

# Socketmap
virtual_alias_maps = socketmap:inet:127.0.0.1:9003:aliases

# Policy delegation
smtpd_relay_restrictions =
    permit_mynetworks
    check_policy_service inet:127.0.0.1:9004
    reject

# Make lookup failures temporary if API
# endpoint or connector fails (optional)
unknown_virtual_alias_reject_code = 450
unknown_local_recipient_reject_code = 450
unknown_virtual_mailbox_reject_code = 450
...
```


## 🏗️ Project Structure

```
postfix-rest-api-connector/
├── Cargo.toml              # Dependencies: tokio, serde, reqwest, anyhow
└── src/
    ├── main.rs             # Entry point and signal handling
    ├── config.rs           # Configuration parser
    ├── server.rs           # Async TCP server
    └── protocol.rs         # Postfix protocol handlers

```

## 🔬 REST API Requirements

### TCP Lookup

**Request:**
```
GET /api/endpoint?key={lookup-key}
X-Auth-Token: {auth-token}
```

**Success Response (200):**
```json
["result1", "result2"]
```

**Error Responses:**
- `404` → Returns "Not found" to Postfix
- `4xx` → Permanent error to Postfix  
- `5xx` → Temporary error to Postfix

### Socketmap

**Request:**
```
GET /api/endpoint?name={map name}&key={lookup-key}
X-Auth-Token: {auth-token}
```

**Success Response (200):**
```json
["result1", "result2"]
```

### Policy Check

**Request:**
```
POST /api/policy
X-Auth-Token: {auth-token}
Content-Type: application/x-www-form-urlencoded

request=smtpd_access_policy
protocol_state=RCPT
client_address=1.2.3.4
...
```

**Response (200):**
```
action=DUNNO
```

Or: `OK`, `REJECT`, `DEFER`, `DEFER_IF_PERMIT`, etc.

## 📈 Monitoring

```bash
# View logs (with log level)
sudo journalctl -u postfix-rest-api-connector -f

# Check status
sudo systemctl status postfix-rest-api-connector

# Monitor connections
sudo netstat -tnp | grep postfix-res

# Resource usage
ps aux | grep postfix-rest-api-connector
```

### Log Levels

Set via environment variable in systemd:

```bash
sudo systemctl edit postfix-rest-api-connector
```

Add:
```ini
[Service]
Environment="RUST_LOG=info"
```

Levels: `error`, `warn`, `info`, `debug`, `trace`

## 🧪 Testing

```bash
# Test TCP lookup
echo "get test@example.com" | nc 127.0.0.1 9001

# Test Socketmap
printf "18:domain example.com," | nc 127.0.0.1 9003

# Watch Postfix logs
sudo tail -f /var/log/maillog
```

## 🔒 Security

- **Memory safe** - No buffer overflows, use-after-free, or null pointers
- **Thread safe** - Compiler prevents data races
- **Bind to localhost** - Default configuration uses 127.0.0.1
- **TLS support** - HTTPS targets supported by default
- **Input validation** - All inputs validated before processing
- Runs as **Unprivileged user** 

## 📚 Documentation

- **[BUILD_AND_INSTALL.md](BUILD_AND_INSTALL.md)** - Complete build guide

## 🎯 Performance Tips

1. **Log level** - Use `warn` or `error` in production
2. **Worker threads** - Set `TOKIO_WORKER_THREADS` for high load
3. **Timeouts** - Tune `request-timeout` based on your API
4. **Connection pooling** - Reqwest handles this automatically

## 📄 License

MIT License

## 🙏 Acknowledgments

- Built with [Tokio](https://tokio.rs) - Async runtime
- Uses [Serde](https://serde.rs) - Serialization
- HTTP client: [Reqwest](https://docs.rs/reqwest)
- Inspired by [pschichtel](https://github.com/pschichtel/postfix-rest-connector)

## 📞 Support

- Postfix docs: http://www.postfix.org/
