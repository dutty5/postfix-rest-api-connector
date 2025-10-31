# GitHub Actions Workflows

This directory contains GitHub Actions workflows for automated building and releasing.

## Workflows

### `rust.yml` - Build and Release

**Triggers:**
- Push to `main` branch (runs tests only)
- Push tags matching `v*` (runs full build and release)
- Pull requests to `main` (runs tests only)

**Jobs:**

1. **test** - Runs on every push/PR
   - Installs Rust toolchain
   - Caches dependencies
   - Builds the project
   - Runs tests
   - Runs clippy linting

2. **build-rpm** - Runs only on version tags
   - Builds RPM packages for:
     - Rocky Linux 8 (EL8)
     - Rocky Linux 9 (EL9)
     - AlmaLinux 9 (EL9)
   - Uses official distribution containers
   - Uploads artifacts

3. **build-deb** - Runs only on version tags
   - Builds DEB packages for:
     - Debian 11
     - Debian 12
     - Ubuntu 22.04 LTS
     - Ubuntu 24.04 LTS
   - Uses official distribution containers
   - Uploads artifacts

4. **create-release** - Runs after build jobs complete
   - Downloads all package artifacts
   - Generates SHA256 checksums
   - Creates GitHub release with:
     - All RPM packages
     - All DEB packages
     - All checksum files
     - Automated release notes

## Usage

### To Trigger a Release

```bash
# 1. Update version and commit
git add Cargo.toml postfix-rest-api-connector.spec CHANGELOG.md
git commit -m "Bump version to 1.0.1"
git push origin main

# 2. Create and push tag
git tag -a v1.0.1 -m "Release version 1.0.1"
git push origin v1.0.1

# 3. Monitor at: https://github.com/YOUR_USERNAME/postfix-rest-api-connector/actions
```

### To Test Without Release

Simply push to main or create a PR - only tests will run, no packages will be built.

## Caching

The workflow caches:
- Cargo registry (`~/.cargo/registry`)
- Cargo git dependencies (`~/.cargo/git`)
- Build artifacts (`target/`)

This significantly speeds up builds on subsequent runs.

## Permissions

The workflow requires:
- **contents: write** - For creating releases and uploading artifacts

This is granted via the `permissions` setting in the workflow.

## Supported Architectures

Currently: **x86_64 (amd64)** only

To add ARM64 support, you would need to:
1. Add cross-compilation setup
2. Use multi-arch containers
3. Update package naming to include architecture

## Container Images

The workflow uses official distribution images:
- `rockylinux:8` and `rockylinux:9`
- `almalinux:9`
- `debian:11` and `debian:12`
- `ubuntu:22.04` and `ubuntu:24.04`

These ensure packages are built in the correct environment with proper dependencies.

## Troubleshooting

### Build Fails

Check the Actions log for specific errors. Common issues:
- Missing dependencies in container
- Version mismatch between Cargo.toml and tag
- Test failures

### Release Not Created

Ensure:
- Tag starts with `v` (e.g., `v1.0.0`)
- Build jobs completed successfully
- GitHub token has proper permissions

### Package Installation Issues

Test packages locally before release:
```bash
# For RPM
rpm -qp --provides package.rpm
rpm -qp --requires package.rpm

# For DEB  
dpkg-deb --info package.deb
dpkg-deb --contents package.deb
```

## Maintenance

Update the workflow when:
- Adding support for new distributions
- Changing build dependencies
- Modifying package structure
- Adding new features requiring CI changes

## Resources

- [GitHub Actions Documentation](https://docs.github.com/en/actions)
- [Rust GitHub Actions](https://github.com/actions-rs)
- [RPM Packaging Guide](https://rpm-packaging-guide.github.io/)
- [Debian Packaging](https://www.debian.org/doc/manuals/maint-guide/)
