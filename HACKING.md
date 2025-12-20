# HACKING

Developer onboarding for `sd2`.

* End-user CLI usage: `README.md`
* Internal architecture and data flow: `DESIGN.md`

This document is **normative** for contributors.

---

## Project Overview

`sd2` is a stream-oriented text processor written in Rust.

It serves two audiences:

1. **Humans**
   A safer, clearer alternative to `sed`/`awk`, with atomic edits and explicit scope.

2. **AI agents**
   A deterministic refactoring engine with strict JSON schemas, manifests, and event output.
   No shell injection, no heuristics, no hidden behavior.

---

## Core Design Principles

These are not preferences. They are invariants.

* **Atomic transactions**
  Files are never left partially modified. All writes are transactional.

* **Explicit scope**
  `sd2` does not walk directories or infer intent.
  Input files must be named or streamed explicitly.

* **Structured I/O**
  Manifests and JSON output are first-class APIs, not debug features.

* **Pipeline-first**
  Designed to compose with Unix tools (`rg`, `fd`, `find`) cleanly and predictably.

If a change violates one of these, it is incorrect.

---

## Architecture

High-level architecture, module layout, and data-flow diagrams live in `DESIGN.md`.

This file intentionally avoids duplicating that material.

---

## Building and Running

### Prerequisites

* Rust toolchain ≥ **1.86**
* `rg` (ripgrep)

  * Required for some integration tests
  * `tests/ripgrep_workflow.rs` currently invokes `/usr/bin/rg`

---

### Common Commands

**Build**

```bash
cargo build
```

**Run**

```bash
cargo run -- <args>
```

**Test**

```bash
cargo test
```

**Format**

```bash
cargo fmt --all
```

**Lint**

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

**Local install**

```bash
cargo install --path .
```

---

## Development Usage Examples

**Basic replacement**

```bash
cargo run -- "find_pattern" "replace_pattern" src/main.rs
```

**Dry run**

```bash
cargo run -- "foo" "bar" src/main.rs --dry-run
```

**Dump JSON schema (agent tooling)**

```bash
cargo run -- schema
```

**Apply manifest**

```bash
cargo run -- apply --manifest examples/manifest.json
```

**Read file paths from stdin**

```bash
fd -e rs | cargo run -- "foo" "bar"
```

**Consume ripgrep JSON spans**

```bash
rg --json "TODO" | cargo run -- --rg-json "TODO" "FIXME"
```

---

## Development Conventions

### Code Style

* Always run `cargo fmt`
* Treat `cargo clippy -D warnings` as non-negotiable

---

### Safety and Correctness

* Never modify files in place
* Every write must be crash-safe
* Temporary files must be cleaned up on failure
* Behavior must be identical across:

  * file mode
  * stdin-text mode
  * manifest mode

---

### Testing

* Unit tests live alongside implementation where practical

  * e.g. `src/replacer/mod.rs`
* Integration tests belong in `tests/`
* Any behavior change requires tests
* If behavior is hard to test, that is a design smell

---

### Agent-Facing APIs

When adding or modifying behavior:

* Update JSON schemas (`model.rs`)
* Ensure CLI flags map cleanly to manifest fields
* Ensure emitted JSON events remain deterministic
* Never introduce undocumented fields or silent behavior

If an agent cannot rely on it, it is broken.

---

## Contribution Flow

1. Make a focused change
   Small, reviewable diffs only

2. Update tests
   `cargo test` must pass

3. Run tooling
   `cargo fmt`
   `cargo clippy -D warnings`

4. Submit a PR
   Include:

   * what changed
   * why it changed
   * any CLI or JSON contract impact

---

## Non-Goals

These are explicitly out of scope:

* Implicit directory traversal
* Heuristic matching
* “Smart” behavior that hides ambiguity
* Compatibility hacks that break determinism

If a feature needs guessing, it does not belong in `sd2`.
