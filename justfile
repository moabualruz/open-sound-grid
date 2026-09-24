# Mirrors .github/workflows/ci.yml (Test job).
ci:
    cargo fmt --all --check
    cargo clippy -- -D warnings
    cargo test
