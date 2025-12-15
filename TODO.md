# Project TODOs

This file tracks outstanding tasks and missing features required to match the v1 CLI documentation (`README.md`, `helptext.txt`).

## 🚨 Critical: Missing CLI Features

These features are documented but currently **missing** from `src/cli.rs` and the execution logic.

### Commands
- [ ] **`apply --validate-only`**: Parse manifest, validate, plan, but do not execute.

### Input Modes
- [ ] **`--stdin-paths`**: Flag to strictly interpret stdin as newline-delimited paths (disable auto-detection).
- [ ] **`--files0`**: Read NUL-delimited paths from stdin (interop with `find -print0`, `fd -0`).
- [ ] **`--stdin-text`**: Filter mode: Read content from stdin, transform, write to stdout.
- [ ] **`--rg-json`**: Consume `ripgrep --json` output and limit edits to matched spans.
- [ ] **`--files`**: Force positional args to be treated as files (even if stdin is piped).

### Match Semantics
- [ ] **`--regex`**: Explicit flag to treat FIND as regex (alias/default).

### Scope Controls
- [ ] **`--limit`**: Alias for `--max-replacements`.
- [ ] **`--range START[:END]`**: Apply replacements only within 1-based line ranges.
- [ ] **`--glob-include GLOB`**: Post-filter input files by glob whitelist.
- [ ] **`--glob-exclude GLOB`**: Post-filter input files by glob blacklist.

### Safety & Guarantees
- [ ] **`--no-write`**: Stronger dry-run: guarantees zero writes, even if output mode changes.
- [ ] **`--require-match`**: Error if zero matches found across all inputs (Policy).
- [ ] **`--expect N`**: Error if total replacement count != N (Policy).
- [ ] **`--fail-on-change`**: Exit non-zero if changes would occur (CI check).

### Transaction Model
- [ ] **`--transaction all|file`**:
    - `file` (default behavior currently): Atomic per-file.
    - `all`: Stage *all* edits in memory/temp files, verify success, then commit all (or rollback).

### Filesystem Behavior
- [ ] **`--symlinks`**:
    - `follow` (Implemented).
    - `skip`: Ignore symlinks.
    - `error`: Abort on symlink.
- [ ] **`--binary`**:
    - `skip`: Detect and skip binary files.
    - `error`: Abort on binary file.
- [ ] **`--permissions`**:
    - `preserve` (Implemented).
    - `fixed`: Write with fixed permissions (e.g., 644/755).

### Output Control
- [ ] **`--quiet`**: Suppress summary and diffs.
- [ ] **`--format`**: Explicit control (`diff`, `summary`, `json`).

## 🛠 Feature Implementation

### Core Engine (`src/engine.rs`, `src/replacer/mod.rs`)
- [ ] **Input Mode State Machine**: Refactor `apply` in `main.rs` to handle mutually exclusive input modes (`stdin-text` vs `files0` vs `rg-json` vs `files`).
- [ ] **Transaction Manager**: Implement the "stage all" logic for `--transaction all`.
- [ ] **Range limiting**: Implement line-range filtering in `Replacer` or `engine`.
- [ ] **Post-filtering**: Implement glob matching on the collected file list.
- [ ] **Binary Detection**: Add `content_inspector` or similar check before reading/processing.

### Reporting
- [ ] **Policy Checks**: Implement `require-match` and `expect N` validation in `Report` or `engine`.

## 🔮 Future / Planned
- [ ] **New Operations**: Support `Delete`, `Insert`, `RegexReplace` in `src/model.rs`.
- [ ] **Manifest Updates**: Add `transaction`, `glob_include`, `glob_exclude` to `Pipeline` struct in `model.rs`.