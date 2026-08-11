CARGO ?= cargo

.PHONY: help demo run build test format format-check lint verify clean

help: ## Show available targets
	@printf '%s\n' \
		'help         Show available targets' \
		'demo         Run the application and print its live output' \
		'run          Run the application' \
		'build        Build the optimized release binary' \
		'test         Run all tests' \
		'format       Format Rust sources' \
		'format-check Check Rust source formatting' \
		'lint         Run Clippy with warnings denied' \
		'verify       Run formatting, linting, tests, and release build' \
		'clean        Remove Cargo build artifacts'

demo: ## Run the application and print its live output
	@printf '100\n\n\n\n10\n0\nstand\nquit\n' | $(CARGO) run --quiet

run: ## Run the application
	$(CARGO) run

build: ## Build the optimized release binary
	$(CARGO) build --release

test: ## Run all tests
	$(CARGO) test --all-targets --all-features

format: ## Format Rust sources
	$(CARGO) fmt --all

format-check: ## Check Rust source formatting
	$(CARGO) fmt --all -- --check

lint: ## Run Clippy with warnings denied
	$(CARGO) clippy --all-targets --all-features -- -D warnings

verify: format-check lint test build ## Run all quality gates

clean: ## Remove Cargo build artifacts
	$(CARGO) clean
