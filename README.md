# blackjackrs

A Rust console application that prints `Hello World!` and exits successfully.

## Package information

| Property | Value |
| --- | --- |
| Package | `blackjackrs` |
| Version | `0.1.0` |
| Type | Binary application |
| Rust edition | 2024 |
| Minimum Rust version | 1.85 |
| Published to crates.io | No |
| Entry point | `src/main.rs` |

## Requirements

- Rust 1.85 or newer with Cargo
- GNU Make or a compatible implementation for optional convenience targets

Install Rust using the [official Rust installation instructions][rust-install].

## Build

Build the optimized executable:

```console
cargo build --release
```

The executable is written to `target/release/blackjackrs` on Unix-like
systems and `target/release/blackjackrs.exe` on Windows.

## Run

Run the application through Cargo:

```console
cargo run --quiet
```

Expected output:

```text
Hello World!
```

The equivalent live Make target is `make demo`.

## Verify

Run the full formatting, linting, test, and release-build gate:

```console
make verify
```

The underlying Cargo commands are:

```console
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build --release
```

The integration test executes the compiled binary and verifies its exit status,
standard output, and standard error.

## Repository layout

| Path | Purpose |
| --- | --- |
| `Cargo.toml` | Package metadata and Cargo configuration |
| `src/main.rs` | Console application entry point |
| `tests/hello_world.rs` | End-to-end binary behavior test |
| `Makefile` | Optional development and verification targets |

[rust-install]: https://doc.rust-lang.org/book/ch01-01-installation.html
