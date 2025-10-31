#!/bin/bash
#
# Automated build script for postfix-rest-api-connector (Rust) RPM
# Builds binary first, then packages it into RPM
# Usage: ./build-rpm-rust.sh [version]
#

set -e

VERSION=${1:-1.0.0}
NAME="postfix-rest-api-connector"

echo "=============================================="
echo "Postfix REST API Connector (Rust) RPM Builder"
echo "Version: ${VERSION}"
echo "=============================================="

# Check if we're on a supported system
if [ ! -f /etc/redhat-release ]; then
    echo "Error: This script requires a Red Hat-based system"
    exit 1
fi

# Check for Rust installation
if ! command -v cargo &> /dev/null; then
    echo "Rust is not installed. Installing..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source $HOME/.cargo/env
fi

echo ""
echo "Rust version:"
rustc --version
cargo --version

# Install RPM build dependencies
echo ""
echo "Checking RPM build dependencies..."
DEPS="rpm-build rpmdevtools"
MISSING=""

for dep in $DEPS; do
    if ! rpm -q $dep &>/dev/null; then
        MISSING="$MISSING $dep"
    fi
done

if [ -n "$MISSING" ]; then
    echo "Installing dependencies:$MISSING"
    sudo dnf install -y $MISSING || sudo yum install -y $MISSING
fi

# Setup RPM build tree
echo ""
echo "Setting up RPM build environment..."
rpmdev-setuptree

# Update version in Cargo.toml
echo ""
echo "Updating version in Cargo.toml..."
sed -i "s/^version = .*/version = \"${VERSION}\"/" Cargo.toml

# Build the release binary
echo ""
echo "Building release binary..."
echo "This may take 2-3 minutes on first build..."
cargo build --release

BINARY="target/release/${NAME}"

if [ ! -f "$BINARY" ]; then
    echo "Error: Binary not found at $BINARY after build!"
    exit 1
fi

# Show binary info
echo ""
echo "Binary built successfully:"
ls -lh "$BINARY"
file "$BINARY"

# Show dependencies
echo ""
echo "Binary dependencies:"
ldd "$BINARY"

# Copy binary to SOURCES
echo ""
echo "Copying binary to RPM SOURCES..."
cp "$BINARY" ~/rpmbuild/SOURCES/

# Copy spec file
echo "Copying spec file..."
SPEC_FILE="${NAME}.spec"
if [ ! -f "$SPEC_FILE" ]; then
    echo "Error: $SPEC_FILE not found!"
    exit 1
fi

cp "$SPEC_FILE" ~/rpmbuild/SPECS/

# Update version in spec file
sed -i "s/^Version:.*/Version:        ${VERSION}/" ~/rpmbuild/SPECS/${NAME}.spec

# Build RPM (only packaging, no compilation)
echo ""
echo "Building RPM package..."
rpmbuild -bb ~/rpmbuild/SPECS/${NAME}.spec

# Check if build succeeded
if [ $? -eq 0 ]; then
    echo ""
    echo "=============================================="
    echo "Build completed successfully!"
    echo "=============================================="
    echo ""
    echo "Binary RPM:"
    RPM_FILE=$(ls ~/rpmbuild/RPMS/*/${NAME}-${VERSION}*.rpm 2>/dev/null | head -1)
    if [ -n "$RPM_FILE" ]; then
        ls -lh "$RPM_FILE"
        echo ""
        echo "RPM Contents:"
        rpm -qpl "$RPM_FILE"
        echo ""
        echo "RPM Info:"
        rpm -qpi "$RPM_FILE"
        echo ""
        echo "Installation Instructions:"
        echo "=========================="
        echo ""
        echo "1. Install the RPM:"
        echo "   sudo rpm -ivh $RPM_FILE"
        echo ""
        echo "2. Configure:"
        echo "   sudo cp /etc/${NAME}/config.json{.sample,}"
        echo "   sudo vi /etc/${NAME}/config.json"
        echo ""
        echo "3. Start the service:"
        echo "   sudo systemctl enable --now ${NAME}"
        echo "   sudo systemctl status ${NAME}"
        echo ""
        echo "4. View logs:"
        echo "   sudo journalctl -u ${NAME} -f"
        echo ""
        echo "Or to upgrade existing installation:"
        echo "   sudo rpm -Uvh $RPM_FILE"
        echo ""
    else
        echo "Warning: Could not find built RPM file"
        echo "Check ~/rpmbuild/RPMS/ directory"
    fi
    echo ""
else
    echo ""
    echo "Build failed! Check the output above for errors."
    exit 1
fi
