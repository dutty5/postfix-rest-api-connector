# Changelog

All notable changes to this project will be documented in this file.

## [v1.0.8] - 2026-06-11

### Fixed edge cases
- Proper protocol framing for all three modes
- tcp-lookup keys are %XX-decoded per tcp_table(5) ENCODING before being
  passed to the API
- HTTP 5xx and network failures now map to temporary (500) tcp-lookup
  replies instead of permanent (400)
- Policy attribute names/values are form-encoded individually
- Policy connections are kept open between requests
- UTF-8-safe truncation in debug logging
- Hardened netstring length parsing
- Accept-loop backoff on errors
- Bind failures now fail the whole service at startup instead of running
  partially configured

## [v1.0.7] - 2026-06-08

- Updated dependencies
- Bump to rust edition 2024

## [v1.0.6] - 2026-01-17

- Updated dependencies


## [v1.0.5] - 2025-11-02

- Debian builds added
- ARM64 builds added
- Better TLS amd HTTP/2 support via rustls
- keepalive and connection pool optimizations
- Updated dependencies


## [1.0.0] - 2025-11-01

### Added
- Initial Rust implementation of Postfix REST API Connector
- Support for TCP lookup protocol
- Support for Socketmap protocol
- Support for Policy delegation protocol
- Async I/O with Tokio for high performance
- Connection pooling for HTTP requests
- Comprehensive error handling and logging
- Systemd service integration
- RPM packaging for EL8, EL9
- DEB packaging for Debian and Ubuntu
- Automated GitHub Actions workflow for releases

### Features
- Zero GC pauses (no garbage collection)
- Memory safe implementation (Rust)
- High performance async I/O
- Simple configuration via JSON
- Built-in HTTP/JSON support
- Production-ready with comprehensive logging


[v1.0.8]: https://github.com/dutty5/postfix-rest-api-connector/releases/tag/v1.0.8
