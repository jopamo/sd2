# DESIGN

This document describes how `sd2` works internally.

* End-user CLI semantics: `README.md`
* Contributor workflow and norms: `HACKING.md`

This file is **architectural truth**. If code diverges from it, the code is wrong.

---

## High-Level Architecture

At a high level, `sd2`:

1. Parses CLI arguments or a manifest into a unified execution configuration (`Pipeline`)
2. Resolves a single, explicit input mode
3. Executes a deterministic, stream-oriented replacement pipeline
4. Enforces policies
5. Commits writes atomically
6. Emits structured reports and exit codes

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

---

## Architectural Invariants

These rules are enforced by design and must remain true:

* No implicit filesystem traversal
* No in-place modification of files
* No heuristic matching or re-searching
* No silent behavior changes between modes
* JSON output must be complete, deterministic, and non-lossy

---

## Codebase Map

### Language and Edition

* **Language:** Rust
* **Edition:** 2024

---

### Entry Point and CLI

* `src/main.rs`
  Program entry point. Parses CLI input, resolves the input mode, builds a `Pipeline`, and dispatches execution.

* `src/cli.rs`
  `clap` configuration for flags, defaults, and subcommands:

  * default replace mode (`FIND REPLACE`)
  * `schema`
  * `apply --manifest …`

---

### Data Model

* `src/model.rs`
  Defines all serializable execution state:

  * `Pipeline`
  * `Operation`
  * policy modes
  * filesystem modes

These types are used uniformly by:

* CLI execution

* manifest execution

* JSON Schema generation

* `sd2 schema`
  Emits a JSON Schema for the `Pipeline` type via `schemars`.

---

### Input Collection

Input resolution is explicit and mutually exclusive.

* `src/input.rs`

  * Determines whether stdin represents:

    * file paths
    * NUL-delimited paths
    * raw text
  * Produces `InputItem` values without guessing

* `src/rgjson.rs`

  * Parses `rg --json` output
  * Converts match events into deterministic byte and line spans
  * Produces `InputItem::RipgrepMatch`

No input mode ever re-interprets or re-searches data.

---

### Execution Engine

* `src/engine.rs`

The engine orchestrates the entire execution lifecycle:

* validate pipeline and inputs
* apply glob include/exclude filters
* enforce pre-execution policies
* process each input item:

  * read file bytes or accept stdin text
  * apply symlink and binary handling modes
  * apply each `Operation` sequentially
  * track replacements and diffs
  * stage or write output according to transaction mode
* enforce post-execution policies
* commit or roll back writes
* produce a final `Report`

The engine contains **no CLI logic** and **no I/O heuristics**.

---

### Replacement Logic

* `src/replacer/mod.rs`

  * Builds a configured `Replacer`
  * Supports literal and regex modes
  * Applies replacements deterministically
  * Counts replacements precisely

* `src/replacer/validate.rs`

  * Validates replacement strings
  * Enforces capture expansion rules when enabled
  * Rejects ambiguous or invalid expansions

Replacement behavior must be identical across:

* file mode
* stdin-text mode
* manifest mode
* `rg --json` span mode

---

### Atomic Writes and Transactions

* `src/write.rs`

  * `write_file`
    Write to a temporary file and atomically rename
  * `stage_file`
    Prepare a staged write for later commit

* `src/transaction.rs`

  * `TransactionManager`
  * Used when `transaction = all`
  * Commits all staged writes only if every file succeeds
  * Rolls back on any failure

Files are never modified in place.

---

### Reporting and Exit Codes

* `src/reporter.rs`

  * Aggregates per-input results
  * Produces:

    * unified diffs
    * summaries
    * JSON / agent output

* `src/events.rs`

  * Defines newline-delimited JSON event types
  * Used for streaming, machine-readable output

* `src/exit_codes.rs`

  * Centralized mapping for:

    * success
    * operational failure
    * policy failure
    * transactional failure

* `src/error.rs`

  * Structured error types
  * Machine-readable error codes
  * Shared across engine and reporting

---

## Data Flow

### CLI Replace Mode (`sd2 FIND REPLACE …`)

1. `main.rs` parses CLI arguments
2. Input mode is resolved
3. Inputs are collected into `Vec<InputItem>`
4. A `Pipeline` is constructed from CLI flags
5. `engine::execute(pipeline, inputs)`:

   * validates configuration
   * enforces pre-execution policies
   * processes each input item
   * stages or writes outputs
   * enforces post-execution policies
   * commits or rolls back writes
6. Reporter prints output and sets exit code

---

### Manifest Apply Mode (`sd2 apply --manifest …`)

* The same engine and data flow
* `Pipeline` is deserialized from JSON
* CLI flags act as overrides

Precedence order:

```
CLI flags > manifest values > defaults
```

---

### `rg --json` Span Mode (`--rg-json`)

In this mode, `sd2` does **not** search.

* `rg --json` determines exact match spans
* `src/rgjson.rs` parses and records spans
* `engine.rs` restricts replacements to those spans only

This guarantees:

* no re-searching
* no heuristic matching
* deterministic, agent-safe edits

---

## Decision Log

Architectural decisions and their rationale.

* **Atomic writes via temp + rename**
  Prevents partial writes and enables transactional commits

* **No implicit traversal**
  File selection is delegated to external tools

* **Literal matching by default**
  Safer and faster; regex requires explicit opt-in

* **NDJSON event output**
  Streamable, pipeline-friendly, agent-compatible

* **`rg --json` span mode**
  Enables “replace exactly these matches” workflows without ambiguity
