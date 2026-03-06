app := "hl-evm-rpc"
version := `git describe --always --dirty`

# Run all checks (clippy + tests)
check:
    cargo clippy --all-targets -- -D warnings
    cargo test

# Build release binary
build:
    cargo build --release

# Deploy to Fly.io via podman
deploy:
    #!/usr/bin/env bash
    set -euo pipefail
    IMAGE_ID="$(podman build -q --build-arg "BUILD_VERSION={{version}}" .)"
    TAG="${IMAGE_ID:0:12}"
    IMAGE="registry.fly.io/{{app}}:${TAG}"
    podman tag "$IMAGE_ID" "$IMAGE"
    fly auth docker
    podman push "$IMAGE"
    fly deploy --image "$IMAGE"
