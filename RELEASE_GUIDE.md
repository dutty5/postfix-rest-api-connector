# Release Guide

This guide explains how to create a new release of the Postfix REST API Connector.

## Automated Release Process

The GitHub Actions workflow automatically builds and releases packages when you push a version tag.

### Step-by-Step Release Process

1. **Update Version Numbers**
   ```bash
   # Update version in Cargo.toml
   sed -i 's/^version = .*/version = "1.0.1"/' Cargo.toml
   
   # Update version in spec file
   sed -i 's/^Version:.*/Version:        1.0.1/' postfix-rest-api-connector.spec
   ```

2. **Update CHANGELOG.md**
   ```bash
   # Add your changes under a new version section
   vi CHANGELOG.md
   ```

3. **Commit Changes**
   ```bash
   git add Cargo.toml postfix-rest-api-connector.spec CHANGELOG.md
   git commit -m "Bump version to 1.0.1"
   git push origin main
   ```

4. **Create and Push Tag**
   ```bash
   # Create annotated tag
   git tag -a v1.0.1 -m "Release version 1.0.1"
   
   # Push the tag to trigger the release workflow
   git push origin v1.0.1
   ```

5. **Monitor the Build**
   - Go to https://github.com/YOUR_USERNAME/postfix-rest-api-connector/actions
   - Watch the "Build and Release" workflow
   - It will:
     - Run tests
     - Build RPMs for EL8, EL9
     - Build DEBs for Debian 11, 12 and Ubuntu 22.04, 24.04
     - Create checksums for all packages
     - Create a GitHub release with all artifacts

6. **Verify the Release**
   - Go to https://github.com/YOUR_USERNAME/postfix-rest-api-connector/releases
   - Check that all packages are attached
   - Download and test packages if needed

## What Gets Built

The workflow creates packages for:

### RPM Packages
- **Rocky Linux 8** (EL8) - `postfix-rest-api-connector-X.Y.Z-1.el8.x86_64.rpm`
- **Rocky Linux 9** (EL9) - `postfix-rest-api-connector-X.Y.Z-1.el9.x86_64.rpm`
- **AlmaLinux 9** (EL9) - `postfix-rest-api-connector-X.Y.Z-1.el9.x86_64.rpm`

### DEB Packages
- **Debian 11** - `postfix-rest-api-connector_X.Y.Z_amd64.deb`
- **Debian 12** - `postfix-rest-api-connector_X.Y.Z_amd64.deb`
- **Ubuntu 22.04** - `postfix-rest-api-connector_X.Y.Z_amd64.deb`
- **Ubuntu 24.04** - `postfix-rest-api-connector_X.Y.Z_amd64.deb`

### Checksums
Each package gets a `.sha256` checksum file for verification.

## Manual Release (Alternative)

If you need to create a release manually:

```bash
# 1. Build locally for your platform
cargo build --release

# 2. For RPM (on EL8/9)
./build-rpm-rust.sh 1.0.1

# 3. For DEB (on Debian/Ubuntu)
# Follow the DEB creation steps from the GitHub Actions workflow
```

## Troubleshooting

### Build Fails on Tag Push

1. Check the Actions log for specific errors
2. Common issues:
   - Cargo.toml version doesn't match tag
   - Spec file version doesn't match tag
   - Build dependencies missing
   - Tests failing

### Fix: Delete and Recreate Tag

```bash
# Delete local tag
git tag -d v1.0.1

# Delete remote tag
git push --delete origin v1.0.1

# Fix issues, commit, then recreate tag
git tag -a v1.0.1 -m "Release version 1.0.1"
git push origin v1.0.1
```

## Pre-release Testing

Before tagging, you can test the build process:

```bash
# Run tests locally
cargo test

# Build release binary
cargo build --release

# Test the binary
RUST_LOG=info ./target/release/postfix-rest-api-connector config.json.sample
```

## Version Numbering

Follow [Semantic Versioning](https://semver.org/):

- **MAJOR** version (X.0.0): Incompatible API changes
- **MINOR** version (0.X.0): New functionality (backwards compatible)
- **PATCH** version (0.0.X): Bug fixes (backwards compatible)

Examples:
- `v1.0.0` - Initial release
- `v1.0.1` - Bug fix
- `v1.1.0` - New feature (e.g., add metrics endpoint)
- `v2.0.0` - Breaking change (e.g., config format change)

## Release Checklist

- [ ] Version updated in `Cargo.toml`
- [ ] Version updated in `postfix-rest-api-connector.spec`
- [ ] CHANGELOG.md updated with changes
- [ ] All tests passing locally (`cargo test`)
- [ ] Changes committed and pushed to main
- [ ] Tag created and pushed
- [ ] GitHub Actions workflow succeeded
- [ ] Release artifacts verified
- [ ] Release notes reviewed
- [ ] Packages tested on at least one platform

## Post-Release

After releasing:

1. **Announce** - Update any documentation or notify users
2. **Monitor** - Watch for issues reported by users
3. **Update main** - Bump version to next development version

```bash
# After releasing v1.0.1, update to v1.0.2-dev
sed -i 's/^version = .*/version = "1.0.2-dev"/' Cargo.toml
git add Cargo.toml
git commit -m "Bump version to 1.0.2-dev"
git push origin main
```
