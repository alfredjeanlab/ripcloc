.PHONY: check build test clean

check:
	cargo fmt --all
	cargo clippy --all-targets -- -D warnings
	cargo test --all

build:
	cargo build --release

test:
	cargo test --all

clean:
	cargo clean
