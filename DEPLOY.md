# Fly.io Deployment

## First-time setup

```bash
# Install flyctl
curl -L https://fly.io/install.sh | sh

# Login
fly auth login

# Auth podman with Fly registry (one-time, persists in ~/.docker/config.json)
fly auth docker

# Create app
fly apps create hl-evm-rpc

# First deploy (initializes the registry repo)
fly deploy --dockerfile Dockerfile

# Scale to 1 machine (Fly defaults to 2 for HA)
fly scale count 1
```

## Subsequent deploys

```bash
./deploy.sh
```

This runs: podman build → podman push → fly deploy.

## Quirks and gotchas

- **Registry auth**: `fly auth docker` must be run before first push. Writes to `~/.docker/config.json` which podman reads. If push fails with "app repository not found", re-run `fly auth docker`.
- **Pending app state**: After `fly apps create`, the app is in "pending" state. The registry repo doesn't exist yet. Either do `fly deploy --dockerfile Dockerfile` for the first deploy, or destroy and recreate if stuck.
- **2 machines by default**: Fly creates 2 machines on first deploy for HA, even with `min_machines_running = 0`. Fix with `fly scale count 1`.
- **`fly deploy --local-only` doesn't work with podman**: That flag shells out to Docker specifically. Use manual podman build/push/deploy instead.
- **No ca-certificates needed**: reqwest uses rustls-tls → webpki-roots (bundled Mozilla CAs at compile time). The runtime image is just debian:bookworm-slim + the binary.
- **LISTEN_ADDR**: Must be `0.0.0.0:8080` in fly.toml env (default is `127.0.0.1:8545` which won't accept external traffic).
- **Region**: `arn` (Stockholm).
