
fmt:
	cargo fmt -- --check

lint:
	cargo clippy

check:
	cargo check

test:
ifeq ($(name),)
	@echo "Running all unit tests"
	cargo test -p txapi
else
	@echo "Running unit tests for $(name)"
	cargo test -p txapi --test $(name)
endif

build:
	cargo build -p txapi

ci: check fmt lint test build

clean:
	cargo clean

run:
	cargo run -p txapi

dev:
	cargo watch -x 'run -- --p txapi'