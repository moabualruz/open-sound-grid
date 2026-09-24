# Mirrors .github/workflows/ci.yml (Test job). PipeWire/PulseAudio are Linux-only,
# so non-Linux hosts run the same gate inside a Linux container.
ci:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ "$(uname -s)" = Linux ]; then
        cargo fmt --all --check
        cargo clippy -- -D warnings
        cargo test
    else
        docker run --rm -v "$PWD":/src -w /src rust:1.88 bash -euc '
            apt-get update -qq && apt-get install -y -qq libpipewire-0.3-dev libpulse-dev libdbus-1-dev libclang-dev clang pkg-config >/dev/null
            rustup component add rustfmt clippy
            export CARGO_TARGET_DIR=/tmp/target
            cargo fmt --all --check
            cargo clippy -- -D warnings
            cargo test'
    fi
