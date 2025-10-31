Name:           postfix-rest-api-connector
Version:        1.0.1
Release:        1%{?dist}
Summary:        REST API connector for Postfix mail server

License:        MIT
URL:            https://github.com/dutty5/postfix-rest-api-connector

# No sources needed - using pre-built binary

%description
A high-performance TCP server that acts as a remote lookup service for 
Postfix mail server, forwarding requests to REST API endpoints. 
Written in Rust for maximum performance and reliability.
Supports TCP lookup, Socketmap, and Policy delegation protocols.

%prep
# No prep needed - using pre-built binary

%build
# No build needed - using pre-built binary

%install
rm -rf $RPM_BUILD_ROOT

# Install pre-built binary
install -d $RPM_BUILD_ROOT%{_bindir}
install -m 755 %{_sourcedir}/postfix-rest-api-connector $RPM_BUILD_ROOT%{_bindir}/

# Create systemd service
install -d $RPM_BUILD_ROOT%{_unitdir}
cat > $RPM_BUILD_ROOT%{_unitdir}/%{name}.service <<'EOF'
[Unit]
Description=Postfix REST API Connector
After=network.target

[Service]
Type=simple
Environment="RUST_LOG=warn"
ExecStart=%{_bindir}/postfix-rest-api-connector /etc/postfix-rest-api-connector/config.json
Restart=on-failure
RestartSec=5s
User=nobody
Group=nobody

# Security hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/log
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true
RestrictRealtime=true
RestrictAddressFamilies=AF_INET AF_INET6
LockPersonality=true

[Install]
WantedBy=multi-user.target
EOF

# Create config directory with proper permissions
install -d -m 750 $RPM_BUILD_ROOT%{_sysconfdir}/%{name}

# Install sample config (world-readable since it's just a sample)
cat > $RPM_BUILD_ROOT%{_sysconfdir}/%{name}/config.json.sample <<'EOF'
{
  "user-agent": "Postfix REST API Connector",
  "endpoints": [
    {
      "name": "domain-lookup",
      "mode": "tcp-lookup",
      "target": "https://your-api.example.com/api/postfix/domain",
      "bind-address": "127.0.0.1",
      "bind-port": 9001,
      "auth-token": "CHANGE_ME_SECRET_TOKEN_HERE",
      "request-timeout": 2000
    },
    {
      "name": "mailbox-lookup",
      "mode": "tcp-lookup",
      "target": "https://your-api.example.com/api/postfix/mailbox",
      "bind-address": "127.0.0.1",
      "bind-port": 9002,
      "auth-token": "CHANGE_ME_SECRET_TOKEN_HERE",
      "request-timeout": 2000
    },
    {
      "name": "alias-lookup",
      "mode": "tcp-lookup",
      "target": "https://your-api.example.com/api/postfix/aliases",
      "bind-address": "127.0.0.1",
      "bind-port": 9003,
      "auth-token": "CHANGE_ME_SECRET_TOKEN_HERE",
      "request-timeout": 2000
    },
    {
      "name": "policy-check",
      "mode": "policy",
      "target": "https://your-api.example.com/api/postfix/policy",
      "bind-address": "127.0.0.1",
      "bind-port": 9004,
      "auth-token": "CHANGE_ME_SECRET_TOKEN_HERE",
      "request-timeout": 2000
    },
    {
      "name": "socketmap",
      "mode": "socketmap-lookup",
      "target": "https://your-api.example.com/api/postfix",
      "bind-address": "127.0.0.1",
      "bind-port": 9005,
      "auth-token": "CHANGE_ME_SECRET_TOKEN_HERE",
      "request-timeout": 2000
    }
  ]
}
EOF

%clean
rm -rf $RPM_BUILD_ROOT

%files
%{_bindir}/%{name}
%dir %attr(750, nobody, nobody) %{_sysconfdir}/%{name}
%attr(644, root, root) %{_sysconfdir}/%{name}/config.json.sample
%{_unitdir}/%{name}.service

%pre
# Create nobody user if it doesn't exist (usually exists by default)
getent group nobody >/dev/null || groupadd -r nobody
getent passwd nobody >/dev/null || useradd -r -g nobody -s /sbin/nologin -c "Nobody user" nobody

%post
%systemd_post %{name}.service

# Instructions for setting up config file
cat <<'NOTICE'

================================================================================
IMPORTANT: Configuration Setup Required
================================================================================

The config directory is owned by nobody:nobody with 750 permissions.

To configure the service:

1. Copy the sample config as root:
   sudo cp /etc/postfix-rest-api-connector/config.json.sample \
           /etc/postfix-rest-api-connector/config.json

2. Edit the configuration file:
   sudo vi /etc/postfix-rest-api-connector/config.json
   
   Change ALL instances of "CHANGE_ME_SECRET_TOKEN_HERE" to secure tokens
   Update the target URLs to point to your REST API

3. Set proper ownership and permissions:
   sudo chown nobody:nobody /etc/postfix-rest-api-connector/config.json
   sudo chmod 600 /etc/postfix-rest-api-connector/config.json

4. Verify permissions:
   sudo ls -la /etc/postfix-rest-api-connector/

   You should see:
   drwxr-x---  2 nobody nobody  config directory
   -rw-------  1 nobody nobody  config.json (after you create it)
   -rw-r--r--  1 root   root    config.json.sample

5. Start the service:
   sudo systemctl enable --now postfix-rest-api-connector

6. Check status and logs:
   sudo systemctl status postfix-rest-api-connector
   sudo journalctl -u postfix-rest-api-connector -f

WARNING: The service will NOT start until config.json exists with proper
         permissions and valid configuration!

================================================================================
NOTICE

%preun
%systemd_preun %{name}.service

%postun
%systemd_postun_with_restart %{name}.service

%changelog
* Fri Nov 01 2025 dutty5 - 1.0.0-1
- Initial Rust implementation
- High-performance async I/O with Tokio
- Zero GC pauses
- Memory safe implementation
- Connection pooling for optimal performance
- Fixed: Proper file permissions and ownership for security
- Fixed: Config directory owned by nobody:nobody
- Fixed: Service hardening with systemd security options
