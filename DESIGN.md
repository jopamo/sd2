# DESIGN

This document describes how `txed` works internally.

* End-user CLI semantics: `README.md`
* Contributor workflow and norms: `HACKING.md`

This file is **architectural truth**.
If code diverges from it, **the code is wrong**.

---

## High-Level Architecture

At a high level, `txed`:

1. Parses CLI arguments or a manifest into a unified execution configuration (`Pipeline`)
2. Resolves **exactly one** explicit input mode
3. Executes a deterministic, stream-oriented replacement pipeline
4. Enforces safety and policy constraints
5. Commits writes atomically (or not at all)
6. Emits structured, machine-readable reports and exit codes

```mermaid
flowchart TD
  CLI[CLI args\nsrc/cli.rs] --> MAIN[src/main.rs]
  MAIN --> INPUT[Input collection\nsrc/input.rs + src/rgjson.rs]
  MAIN --> MODEL[Pipeline / Operations\nsrc/model.rs]
  MAIN --> ENGINE[Execution engine\nsrc/engine.rs]
  ENGINE --> REPLACER[Literal / regex replacer\nsrc/replacer/]
  ENGINE --> POLICY[Policy enforcement\nsrc/policy.rs]
  ENGINE --> WRITE[Atomic writes\nsrc/write.rs]
  WRITE --> TXN[Transaction manager\nsrc/transaction.rs]
  ENGINE --> REPORT[Reporting + events\nsrc/reporter.rs + src/events.rs]
  REPORT --> OUT[stdout / stderr\n(diff | summary | json | agent)]
```

The engine is the single authority over execution.
All modes—CLI, stdin, manifests, and agent workflows—flow through the same core logic.

---

## Architectural Invariants

These rules are enforced by design and must remain true:

* No implicit filesystem traversal
* No in-place modification of files
* No heuristic matching or re-searching
* No silent behavior changes between modes
* JSON output must be complete, deterministic, and non-lossy
* Identical inputs must produce identical outputs

If a feature proposal violates any invariant, it must be redesigned or rejected.

---

## Codebase Map

### Language and Edition

* **Language:** Rust
* **Edition:** 2024

The codebase relies on strong typing, explicit state transitions, and exhaustive error handling.

---

### Entry Point and CLI

#### `src/main.rs`

Program entry point. Responsibilities:

* Parse CLI arguments
* Resolve the input mode
* Load or construct a `Pipeline`
* Dispatch execution to the engine
* Map execution results to exit codes

`main.rs` contains **no business logic**.

---

#### `src/cli.rs`

Defines the CLI surface using `clap`.

Supported commands and modes:

* Default replace mode (`FIND REPLACE [FILES...]`)
* `schema`
* `apply --manifest FILE`

Responsibilities:

* Argument parsing
* Flag validation
* CLI → `Pipeline` translation

No filesystem access or execution occurs here.

---

## Data Model

### `src/model.rs`

Defines the complete, serializable execution model.

Key types:

* `Pipeline`
* `Operation`
* `TransactionMode`
* `InputMode`
* Policy enums

These types are shared by:

* CLI execution
* Manifest execution
* JSON Schema generation
* Agent integrations

There is exactly **one** execution model in the system.

---

### Schema Generation

`txed schema`:

* Emits a JSON Schema for `Pipeline`
* Uses `schemars`
* Schema is treated as a public API

Breaking schema changes require a major version bump.

---

## Input Collection

Input resolution is **explicit, exclusive, and deterministic**.

### `src/input.rs`

Resolves stdin and arguments into a concrete input mode.

Possible interpretations of stdin:

* newline-delimited file paths
* NUL-delimited file paths
* raw text (`--stdin-text`)

Rules:

* Only one interpretation is valid
* No guessing
* No fallback heuristics

Outputs `Vec<InputItem>`.

---

### `src/rgjson.rs`

Consumes `rg --json` output.

Responsibilities:

* Parse ripgrep events
* Extract byte-accurate spans
* Convert into `InputItem::RipgrepMatch`

Key guarantees:

* No re-searching
* No fuzzy matching
* Only spans explicitly provided by ripgrep are edited

This mode is designed for agent-safe, span-precise edits.

---

## Execution Engine

### `src/engine.rs`

The engine orchestrates the full execution lifecycle.

Responsibilities:

* Validate pipeline and inputs
* Enforce pre-execution policies
* Apply include/exclude filters
* Process each `InputItem`:

  * Load bytes or accept stdin text
  * Apply symlink and binary policies
  * Apply operations sequentially
  * Track replacements, diffs, and counts
  * Stage or write output
* Enforce post-execution policies
* Commit or roll back writes
* Produce a final `Report`

The engine:

* Has **no CLI knowledge**
* Performs **no input guessing**
* Is deterministic by construction

---

## Replacement Logic

### `src/replacer/mod.rs`

Builds a configured `Replacer`.

Capabilities:

* Literal replacement
* Regex replacement (explicit opt-in)
* Deterministic iteration
* Precise replacement counting

Replacement semantics are identical across all modes.

---

### `src/replacer/validate.rs`

Validates replacement configuration.

Responsibilities:

* Validate capture group references
* Enforce expansion rules
* Reject ambiguous or invalid replacements

Invalid replacement specifications fail **before execution**.

---

## Atomic Writes and Transactions

### `src/write.rs`

Defines safe filesystem write primitives.

Functions:

* `write_file`

  * Write to a temp file
  * Atomically rename into place
* `stage_file`

  * Prepare staged output for transactional commit

Files are **never** modified in place.

---

### `src/transaction.rs`

Defines transactional semantics.

* `TransactionManager`
* Active when `transaction = all`
* Tracks staged writes
* Commits only if all operations succeed
* Rolls back completely on any failure

Partial success is impossible in transactional mode.

---

## Policy Enforcement

### `src/policy.rs`

Centralizes all policy checks.

Policies include:

* symlink handling
* binary file handling
* required match counts
* expected replacement counts
* fail-on-change behavior

Policies are enforced both:

* before execution
* after execution

Policy violations are explicit failures, never warnings.

---

## Reporting and Exit Codes

### `src/reporter.rs`

Aggregates execution results.

Outputs:

* Unified diffs
* Human-readable summaries
* JSON / agent output

Output format is selected **after** execution completes.

---

### `src/events.rs`

Defines newline-delimited JSON event types.

Characteristics:

* Streamable
* Deterministic
* Lossless
* Machine-readable

This is the primary interface for agents and automation.

---

### `src/exit_codes.rs`

Defines canonical exit codes:

* `0` success
* `1` operational failure
* `2` policy failure
* `3` transactional failure

Exit codes are stable and documented.

---

### `src/error.rs`

Structured error system.

Features:

* Typed error categories
* Machine-readable codes
* Human-readable messages
* Shared across engine and reporting

Errors are never silently swallowed.

---

## Data Flow

### CLI Replace Mode (`txed FIND REPLACE …`)

1. Parse CLI arguments
2. Resolve input mode
3. Collect `Vec<InputItem>`
4. Construct `Pipeline`
5. Execute engine:

   * validate
   * process inputs
   * stage/write outputs
   * commit or roll back
6. Emit report and exit code

---

### Manifest Apply Mode (`txed apply --manifest …`)

* Same execution engine
* `Pipeline` deserialized from JSON
* CLI flags override manifest values

Precedence:

```
CLI flags > manifest values > defaults
```

No alternate execution paths exist.

---

### Ripgrep Span Mode (`--rg-json`)

In this mode, `txed` does **not** search.

* ripgrep determines exact spans
* spans are trusted verbatim
* engine edits only those spans

Guarantees:

* no re-searching
* no heuristic matching
* exact, agent-safe edits

---

## Determinism Guarantees

`txed` guarantees:

* Stable ordering of operations
* Stable replacement counts
* Stable JSON output
* Identical behavior across modes

Any nondeterminism is considered a bug.

---

## Decision Log

Key architectural decisions and rationale.

* **Atomic writes via temp + rename**
  Prevents partial writes and enables rollback

* **No implicit traversal**
  File discovery is delegated to external tools

* **Literal matching by default**
  Safer, faster, and easier to reason about

* **NDJSON event stream**
  Enables streaming, pipelines, and agent control

* **ripgrep JSON span mode**
  Enables exact, non-heuristic replacements

---

## Non-Goals

`txed` explicitly does **not**:

* Walk directories
* Guess user intent
* Perform fuzzy matching
* Modify files in place
* Hide failures
* Auto-fix invalid input

These are deliberate exclusions.

---

## Summary

`txed` is designed as a **deterministic, transactional edit engine**.

It prioritizes:

* Explicitness over convenience
* Safety over cleverness
* Determinism over heuristics
* Structured automation over ad-hoc scripting

This design is what makes `txed` suitable for both humans **and** autonomous agents.
