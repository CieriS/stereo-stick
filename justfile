# List the available recipes
default:
    @just --list

# Install the frontend dependencies
setup:
    cd app && bun install

# Open the desktop app, reloading on changes
dev: setup
    cd app && bun run tauri dev

# Preview the window in a browser, with a pretend backend
preview: setup
    cd app && bun run dev

# Download the tools the app ships with into app/src-tauri/tools
tools: setup
    cd app && bun run fetch-tools

# Build the installer for this system, tools included
bundle: tools
    cd app && bun run tauri build

# Format the code
fmt:
    cargo fmt --all
    cd app && bun run format

# Everything CI runs: formatting, lints, tests, frontend build
check: setup
    cd app && bun run lint
    cd app && bun test
    cd app && bun run build
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace

# Run the tests only
test:
    cd app && bun test
    cargo test --workspace
