# List the available recipes
default:
    @just --list

# Format the code
fmt:
    cargo fmt --all

# Everything CI runs: formatting, lints, tests
check:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace

# Run the tests only
test:
    cargo test --workspace
