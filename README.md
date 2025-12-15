# `sd2`
<div style="background-color: #1e1e1e; padding: 1em; display: inline-block; border-radius: 8px;">
  <img src=".github/sd2.png" alt="sd2 logo" width="300">
</div>

`sd2` is a next-generation **stream-oriented text processor** designed for two audiences:

* **Humans** who want a safer, clearer CLI than `sed` or `awk`
* **AI agents** that require structured inputs, deterministic behavior, and strict JSON validation

It follows the Unix philosophy strictly:
`sd2` does **not** walk directories, infer context, or guess intent. It consumes streams, applies explicit operations, and performs **atomic, transactional edits**.

---

## ⚡ Quick Start

### Basic Replacement
Simple positional arguments. No regex soup, no flag archaeology.

```bash
# Replace 'lazy_static' with 'once_cell' in main.rs
sd2 "lazy_static" "once_cell" src/main.rs
```

### The "Search & Destroy" Workflow
Let `ripgrep` (`rg`) or `fd` decide **what** to touch, and let `sd2` decide **how** to edit.

```bash
# 1. Find files containing 'unwrap()'
# 2. Replace it safely everywhere
rg -l "unwrap\(\)" | sd2 "unwrap()" "expect(\"checked by safe-mode\")"
```

No directory traversal. No implicit recursion. No surprises.

---

## 📚 CLI Guide

### Synopsis

```bash
# Replace in explicit files
sd2 [OPTIONS] FIND REPLACE [FILES...]

# Replace in files listed on stdin (fd/rg -l output)
fd -e rs | sd2 [OPTIONS] FIND REPLACE
rg -l PATTERN | sd2 [OPTIONS] FIND REPLACE

# Targeted edits using rg JSON matches
rg --json PATTERN | sd2 --rg-json [OPTIONS] FIND REPLACE

# Agent workflow
sd2 schema
sd2 apply --manifest manifest.json [OPTIONS]
```

### Commands

*   **`sd2 FIND REPLACE [FILES...]`**
    Default command. Edits provided files or reads file paths from stdin if no files are passed.

*   **`schema`**
    Print the JSON Schema describing manifests, operations, and output events.
    ```bash
    sd2 schema > tools_schema.json
    ```

*   **`apply --manifest FILE`**
    Apply a manifest (multi-file, multi-op), with full validation and atomic commit.
    ```bash
    sd2 apply --manifest manifest.json
    ```

---

### Input Modes

`sd2` is strict about what stdin means.

*   **Auto (default)**
    If stdin is piped **and** no `FILES...` are given, stdin is treated as a list of paths (newline-delimited).
    ```bash
    rg -l unwrap | sd2 unwrap expect
    ```

*   **`--stdin-paths`**
    Force stdin to be interpreted as newline-delimited paths.

*   **`--files0`**
    Read **NUL-delimited** paths from stdin. Compatible with `fd -0`, `find -print0`, `rg -l0`.
    ```bash
    fd -0 -e rs | sd2 --files0 foo bar
    ```

*   **`--stdin-text`**
    Treat stdin as *content* and write the transformed content to stdout (filter mode). No files are opened.
    ```bash
    printf '%s\n' "hello foo" | sd2 --stdin-text foo bar
    ```

*   **`--rg-json`**
    Consume `rg --json` output from stdin and apply edits **only** to the matched spans.
    *   No re-searching.
    *   Edits are constrained to rg-reported match spans.
    *   Fails if input is not rg JSON.
    ```bash
    rg --json "foo" | sd2 --rg-json foo bar
    ```

*   **`--files`**
    Force positional arguments to be treated as files even if stdin is present.

---

### Match Semantics

*   **Literal by default**
    `FIND` is treated as an exact string.

*   **`--regex`**
    Treat `FIND` as a regex pattern.
    ```bash
    sd2 --regex 'foo\s+bar' 'baz' file.txt
    ```

*   **Case Handling**
    *   `--case-sensitive` (Default)
    *   `--ignore-case`
    *   `--smart-case` (Case-insensitive unless `FIND` contains uppercase)

---

### Scope Controls

*   **`--limit N`**
    Maximum replacements per file.
    ```bash
    sd2 foo bar file.rs --limit 1
    ```

*   **`--range START[:END]`**
    Only apply replacements in a line range (1-based).
    ```bash
    sd2 foo bar file.rs --range 10:200
    ```

*   **`--glob-include GLOB`**
    Apply edits only to files whose *path* matches the glob (post-filter).
    ```bash
    fd . | sd2 foo bar --glob-include '**/*.rs'
    ```

*   **`--glob-exclude GLOB`**
    Exclude matching paths (post-filter).

---

### Safety & Guarantees

*   **`--dry-run`**
    Print a unified diff, perform no writes.

*   **`--no-write`**
    Stronger than `--dry-run`: guarantees zero filesystem writes even if output mode changes.

*   **`--require-match`**
    Fail if **zero** matches are found across all inputs.

*   **`--expect N`**
    Require **exactly N total replacements** across all inputs. If count differs, abort and write nothing.

*   **`--fail-on-change`**
    Exit non-zero if any change would occur (useful for CI assertions).

---

### Transaction Model

*   **`--transaction all|file`**
    *   `all` (default): Stage edits and commit only if **every** file succeeds.
    *   `file`: Commit each file independently (still atomic per file).

---

### Filesystem Behavior

*   **`--symlinks follow|skip|error`**
    *   `follow` (default): Edit target, preserve symlink.
    *   `skip`: Ignore symlinks.
    *   `error`: Abort on symlink.

*   **`--binary skip|error`**
    *   `skip` (default): Skip binary-like files.
    *   `error`: Abort if binary-like file encountered.

*   **`--permissions preserve|fixed`**
    *   `preserve` (default): Preserve mode/owner.
    *   `fixed`: Write with fixed mode.

---

### Output Control

*   **Default Behavior**:
    *   TTY: Unified diff + summary.
    *   Pipe: JSON events.

*   **`--json`**: Force JSON event output.
*   **`--quiet`**: No diff, no summary. Errors still emitted.
*   **`--format diff|summary|json`**: Explicit output formatting.

---

### Agent Mode (Manifests)

Agents submit a **Pipeline Manifest** for complex, multi-file atomic edits.

```json
{
  "files": ["src/lib.rs", "src/config.rs"],
  "operations": [
    {
      "replace": {
        "find": "fn process(data: String)",
        "with": "fn process(data: &str)",
        "limit": 1
      }
    }
  ],
  "transaction": "all"
}
```

**Execute:**
```bash
sd2 apply --manifest manifest.json
```

**Options:**
*   `--validate-only`: Parse + plan, no execution.
*   `--dry-run`: Diff output for planned changes.
*   `--json`: Emit structured events.

---

## 📦 Installation

*(Pending crates.io release)*

```bash
cargo install --path .
```

---

## Exit Codes

*   `0`: Success.
*   `1`: Operational failure (I/O, parse error).
*   `2`: Policy failure (`--require-match`, `--expect`, `--fail-on-change`).
*   `3`: Partial/aborted transaction.

---

## License

MIT

```