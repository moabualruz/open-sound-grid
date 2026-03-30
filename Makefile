.PHONY: check fix check-rust fix-rust check-web fix-web build

# Run all checks (CI equivalent)
check: check-rust check-web

# Fix all auto-fixable issues
fix: fix-rust fix-web

# --- Rust ---

check-rust:
	cargo fmt --all -- --check
	cargo clippy -- -D warnings
	cargo test

fix-rust:
	cargo fmt --all
	cargo clippy --fix --allow-dirty --allow-staged -- -D warnings

# --- Web ---

check-web:
	cd web && npm run check

fix-web:
	cd web && npm run lint:fix && npm run format

# --- Build ---

build:
	cargo build
	cd web && npm run build
