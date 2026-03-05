#!/usr/bin/env bash
set -euo pipefail

APP="hl-evm-rpc"
VERSION="$(git describe --always --dirty)"

# Build locally and capture image ID
IMAGE_ID="$(podman build -q --build-arg "BUILD_VERSION=${VERSION}" .)"
TAG="${IMAGE_ID:0:12}"
IMAGE="registry.fly.io/${APP}:${TAG}"
podman tag "$IMAGE_ID" "$IMAGE"

# Refresh registry credentials and push
fly auth docker
podman push "$IMAGE"

# Deploy
fly deploy --image "$IMAGE"
