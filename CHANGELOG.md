# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [v1.0.4] - 2025-11-01

- Debian builds added
- ARM64 builds added
- Better TLS amd HTTP/2 support via rustls
- keepalive and connection pool optimizations


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


[v1.0.4]: https://github.com/dutty5/postfix-rest-api-connector/releases/tag/v1.0.4
