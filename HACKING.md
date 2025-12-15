# sd2

## Project Overview

`sd2` is a next-generation stream-oriented text processor written in Rust. It is designed for two primary audiences:
1.  **Humans:** Offering a safer, clearer alternative to `sed` and `awk` with atomic writes and explicit operations.
2.  **AI Agents:** Providing a formal, machine-readable API (via JSON schemas and manifests) for deterministic refactoring without shell injection risks.

**Key Design Principles:**
*   **Atomic Transactions:** Writes are atomic; files are never left partially modified.
*   **No Implicit Traversal:** Does not walk directories by default; delegates to tools like `ripgrep` or `fd`.
*   **Structured I/O:** Supports JSON manifests for defining operations and JSON output for reporting.
*   **Pipeline-First:** Designed to work effectively in Unix pipelines.

## Architecture & Codebase

*   **Language:** Rust (Edition 2024)
*   **Entry Point:** `src/main.rs` - Handles CLI parsing (via `clap`) and dispatching to `Schema` or `Apply` modes.
*   **Core Logic:**
    *   `src/engine.rs`: Orchestrates the execution of the replacement pipeline.
    *   `src/replacer/mod.rs`: Encapsulates the regex replacement logic using the `regex` crate.
    *   `src/model.rs`: Defines the data structures for the Pipeline and Operations (serialized via `serde`).
    *   `src/validate.rs` (under `replacer`): Validates replacement strings and capture groups.
*   **Dependencies:**
    *   `clap`: CLI argument parsing.
    *   `regex`: The regex engine.
    *   `serde`/`serde_json`: JSON serialization/deserialization for manifests and reports.
    *   `schemars`: JSON Schema generation.
    *   `tempfile`: Managing atomic writes.
    *   `ignore` (optional): For directory walking (feature gated).

## Building and Running

### Prerequisites
*   Rust toolchain (v1.86.0+)

### Commands

*   **Build:**
    ```bash
    cargo build
    ```

*   **Run:**
    ```bash
    cargo run -- <args>
    ```

*   **Test:**
    ```bash
    cargo test
    ```

*   **Install (Local):**
    ```bash
    cargo install --path .
    ```

### Usage Examples

*   **Basic Replacement:**
    ```bash
    cargo run -- "find_pattern" "replace_pattern" src/main.rs
    ```

*   **Dry Run:**
    ```bash
    cargo run -- "foo" "bar" src/main.rs --dry-run
    ```

*   **Schema Dump (for Agents):**
    ```bash
    cargo run -- schema
    ```

## Development Conventions

*   **Code Style:** Follow standard Rust formatting (`cargo fmt`) and clippy advice (`cargo clippy`).
*   **Safety:** Prioritize atomic operations. Never modify a file in-place without a strategy to prevent data loss on crash.
*   **Testing:**
    *   Unit tests are co-located in source files (e.g., `src/replacer/mod.rs`).
    *   Integration tests likely exist (implied by `assert_cmd` in dev-dependencies) or should be added to `tests/`.
*   **Agent Interaction:** When adding features, consider how an AI agent would invoke them. Ensure schemas are updated (`model.rs`) and CLI flags have corresponding JSON manifest fields.
