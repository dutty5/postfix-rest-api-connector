# Postfix REST API Connector - Build and Installation Guide

## Overview

This is a high-performance implementation of the Postfix REST API connector written in Rust. It provides:

- **Zero GC pauses** - No garbage collection, predictable latency
- **Memory safety** - No segfaults, buffer overflows, or memory leaks
- **Async I/O** - Efficient handling of thousands of concurrent connections
- **Built-in HTTP/JSON** - No external dependencies like libcurl/cJSON

## Prerequisites

### Build System (Example for EL8)

```bash
# Install system dependencies
sudo dnf install -y gcc make rpm-build rpmdevtools

# Install Rust (latest stable)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source $HOME/.cargo/env

# Verify installation
rustc --version  # Should be 1.70 or newer
cargo --version
```

## Project Structure

```
postfix-rest-api-connector/
├── Cargo.toml                          # Rust project configuration
├── src/
│   ├── main.rs                         # Entry point
│   ├── config.rs                       # Configuration parser
│   ├── server.rs                       # TCP server
│   └── protocol.rs                     # Protocol handlers
├── postfix-rest-api-connector.spec     # RPM spec file
├── build-rpm-rust.sh                   # Build automation
├── BUILD_AND_INSTALL.md          # This file
└── README.md

```

## Building from Source

### Quick Build

```bash
# Build release binary
cargo build --release

# Binary will be at: target/release/postfix-rest-api-connector

# Test it
./target/release/postfix-rest-api-connector
# Should show: Usage: postfix-rest-api-connector <config-file>

# Run with config
RUST_LOG=info ./target/release/postfix-rest-api-connector sample.json
```

### Install Locally

```bash
# Install to ~/.cargo/bin
cargo install --path .

# Or install system-wide
sudo cargo install --path . --root /usr/local
```

## Creating RPM Packages

### Automated Build

```bash
# Make script executable
chmod +x build-rpm-rust.sh

# Build RPM (this handles everything)
./build-rpm-rust.sh 1.0.0

# This script will:
# 1. Check and install Rust if needed
# 2. Setup RPM build environment
# 3. Create source tarball
# 4. Build RPM with cargo
# 5. Report results
```

### Configure

```bash
# Copy sample config
sudo cp /etc/postfix-rest-api-connector/config.json.sample \
        /etc/postfix-rest-api-connector/config.json

# Edit configuration
sudo vim /etc/postfix-rest-api-connector/config.json
```

### Configuration Example

```json
{
  "user-agent": "Postfix REST API Connector",
  "endpoints": [
    {
      "name": "domain-lookup",
      "mode": "tcp-lookup",
      "target": "https://your-api/api/postfix/domain",
      "bind-address": "127.0.0.1",
      "bind-port": 9001,
      "auth-token": "your-secure-token",
      "request-timeout": 2000
    },
    {
      "name": "mailbox-lookup",
      "mode": "tcp-lookup",
      "target": "https://your-api/api/postfix/mailbox",
      "bind-address": "127.0.0.1",
      "bind-port": 9002,
      "auth-token": "your-secure-token",
      "request-timeout": 2000
    },
    {
      "name": "socketmap",
      "mode": "socketmap-lookup",
      "target": "http://your-api:8080/api/postfix/socketmap",
      "bind-address": "127.0.0.1",
      "bind-port": 9003,
      "auth-token": "your-secure-token",
      "request-timeout": 2000
    },
    {
      "name": "policy-check",
      "mode": "policy",
      "target": "http://your-api:8080/api/postfix/policy",
      "bind-address": "127.0.0.1",
      "bind-port": 9004,
      "auth-token": "your-secure-token",
      "request-timeout": 2000
    }
  ]
}
```

### Start Service

```bash
# Enable and start
sudo systemctl enable --now postfix-rest-api-connector

# Check status
sudo systemctl status postfix-rest-api-connector

# View logs (with color and follow)
sudo journalctl -u postfix-rest-api-connector -f

# Set log level via environment
sudo systemctl edit postfix-rest-api-connector
# Add:
# [Service]
# Environment="RUST_LOG=debug"

sudo systemctl daemon-reload
sudo systemctl restart postfix-rest-api-connector
```

## Postfix Configuration

Add to `/etc/postfix/main.cf`:

```conf
# Virtual domains lookup
virtual_mailbox_domains = tcp:127.0.0.1:9001

# Mailbox lookup
virtual_mailbox_maps = tcp:127.0.0.1:9002

# Using socketmap
virtual_alias_maps = socketmap:inet:127.0.0.1:9003:aliases

# Policy delegation
smtpd_relay_restrictions =
    permit_mynetworks
    check_policy_service inet:127.0.0.1:9004
    reject
```

## Testing

### Basic Tests

```bash
# Test TCP lookup
echo "get test@example.com" | nc 127.0.0.1 9001

# Test Socketmap
printf "18:domain example.com," | nc 127.0.0.1 9003

# Test with telnet
telnet 127.0.0.1 9001
get test@example.com
```

### Monitor Performance

```bash
# Watch logs
sudo journalctl -u postfix-rest-api-connector -f

# Check connections
sudo netstat -tnp | grep postfix-res

# Monitor resource usage
ps aux | grep postfix-rest-api-connector

# Detailed stats
sudo systemctl status postfix-rest-api-connector
```

## Performance Tuning

### Rust-Specific Optimizations

The binary is already optimized with:
```toml
[profile.release]
opt-level = 3        # Maximum optimization
lto = true           # Link-time optimization
codegen-units = 1    # Better optimization (slower compile)
strip = true         # Strip symbols (smaller binary)
```

### Runtime Tuning

```bash
# Set Tokio worker threads (default: CPU cores)
# Environment variable in systemd service:
Environment="TOKIO_WORKER_THREADS=4"

# Log level (options: error, warn, info, debug, trace)
Environment="RUST_LOG=info"

# For production with high load
Environment="RUST_LOG=warn"
Environment="TOKIO_WORKER_THREADS=8"
```

## Debugging

### Enable Debug Logging

```bash
# Temporary (one-time)
sudo RUST_LOG=debug /usr/bin/postfix-rest-api-connector config.json

# Permanent (systemd)
sudo systemctl edit postfix-rest-api-connector
# Add: Environment="RUST_LOG=debug"
sudo systemctl daemon-reload
sudo systemctl restart postfix-rest-api-connector
```

### Backtrace on Panic

```bash
# Enable backtraces
sudo systemctl edit postfix-rest-api-connector
# Add: Environment="RUST_BACKTRACE=1"
```

### Debug Build

```bash
# Build with debug symbols
cargo build

# Run with debugger
rust-gdb ./target/debug/postfix-rest-api-connector
```

## Upgrading

```bash
# Stop service
sudo systemctl stop postfix-rest-api-connector

# Install new version
sudo rpm -Uvh postfix-rest-api-connector-X.Y.Z.rpm

# Start service
sudo systemctl start postfix-rest-api-connector

# Check logs
sudo journalctl -u postfix-rest-api-connector -n 50
```

## Uninstalling

```bash
# Stop and disable
sudo systemctl stop postfix-rest-api-connector
sudo systemctl disable postfix-rest-api-connector

# Remove package
sudo rpm -e postfix-rest-api-connector

# Clean up (optional)
sudo rm -rf /etc/postfix-rest-api-connector
```

## Resources

- [Rust Book](https://doc.rust-lang.org/book/)
- [Tokio Documentation](https://tokio.rs)
- [Postfix TCP Tables](http://www.postfix.org/tcp_table.5.html)
- [Postfix Socketmap](http://www.postfix.org/socketmap_table.5.html)
- [Postfix Policy](http://www.postfix.org/SMTPD_POLICY_README.html)

## License

MIT License
