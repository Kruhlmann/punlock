# Punlock

PasswordUNLOCKer - A tool to unlock secrets from [Bitwarden](https://bitwarden.com/) and mount them to the filesystem.

## Overview

Punlock authenticates with Bitwarden, fetches secrets based on `JMESPath` queries, and writes them to a secure runtime directory. It can also create symlinks to make secrets accessible at convenient locations.

## Features

- Authenticate with Bitwarden (including self-hosted instances)
- Fetch secrets using `JMESPath` queries on Bitwarden items
- Store secrets in a secure runtime directory (Linux: `/run/user/<uid>/punlock/`)
- Create symlinks to secrets at custom locations
- Automatic file permissions (readonly, 400 for private secrets)

## Prerequisites

- Rust (for building)
- Bitwarden CLI (`bw`) installed and available in PATH

## Building

```bash
cargo build --release
```

The binary will be available at `target/release/punlock`.

## Configuration

Create a configuration file at one of these locations:
- Current directory: `config.toml`
- User config: `~/.config/punlock/config.toml`
- System config: `/etc/punlock/config.toml`

Or specify a custom path with `--config`:

```bash
punlock --config /path/to/config.toml
```

### Configuration Format

```toml
domain = "https://your-bitwarden-instance.com"
email = "user@example.com"

# Secrets to unlock
[[entries]]
id = "bitwarden-item-uuid"
query = "details.fields[?@.name=='ssh_private_key'].value"
path = "ssh/id_rsa"
public = false
links = ["~/.ssh/id_rsa"]

[[entries]]
id = "another-item-uuid"
query = "details.login.password"
path = "api/tokens/github"
public = true
links = ["~/.config/github/token"]
```

### Configuration Fields

- `domain` (optional): Bitwarden server URL for self-hosted instances
- `email` (optional): Bitwarden account email
- `entries`: Array of secrets to unlock
  - `id`: Bitwarden item UUID
  - `query`: `JMESPath` expression to extract the secret from the item
  - `path`: Relative path in the runtime directory to write the secret
  - `links` (optional): Array of symlink paths to create (supports `~` for home directory)
  - `public` (default: `false`): If true, file permissions are 0444; otherwise 0400

## Usage

```sh
punlock
# Or
punlock --config /path/to/config.toml
```

On first run, if email is not configured, you'll be prompted to enter it. You'll also need to enter your Bitwarden password.

## Runtime Directory

On Linux, secrets are stored in: `/run/user/<uid>/punlock/`

This is a tmpfs directory that is automatically cleaned on reboot to ensure that secrets do not stick around.
