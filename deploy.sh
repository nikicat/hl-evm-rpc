#!/usr/bin/env bash
set -euo pipefail

APP="hl-evm-rpc"
IMAGE="registry.fly.io/${APP}:latest"

# Build
podman build -t "$IMAGE" .

# Push to Fly registry
podman push "$IMAGE"

# Deploy
fly deploy
