<div style="background-color:#1e1e1e; padding:1em; display:inline-block; border-radius:8px; text-align:left;">
  <img src=".github/txed.png" alt="txed logo" width="300" style="display:block; margin:0;">
</div>

`txed` is a **stream-oriented text processor** designed for two audiences:

* **Humans** who want a safer, clearer CLI than `sed` or `awk`
* **AI agents** that require structured inputs, deterministic behavior, and strict JSON validation

It follows the Unix philosophy strictly.
`txed` does **not** walk directories, infer context, or guess intent. It consumes streams, applies explicit operations, and performs **atomic, transactional edits**.

---

## Features

*   **Atomic edits by default:** Transactional writes across one file or many (`--transaction all|file`).
*   **Explicit inputs:** Edits only the files you pass (args/stdin); no implicit directory traversal.
*   **Multiple input modes:** Positional files, newline/NUL-delimited stdin paths, stdin text, or `rg --json` spans.
*   **Safe previews:** `--dry-run` diffs, `--no-write`, and validation-only runs.
*   **Structured automation:** JSON event stream (`--format json`) and JSON Schema (`txed schema`) for agent tooling.
*   **Manifest apply mode:** Multi-file pipelines via `txed apply --manifest …`.

## Documentation

*   **Developer guide:** `HACKING.md`
*   **Architecture/design notes:** `DESIGN.md`
*   **JSON output contract:** `docs/JSON_EVENTS.md`
*   **MCP integration example:** `docs/MCP_INTEGRATION.md`

## ⚡ Quick Start

### Basic Replacement

Simple positional arguments. No regex soup, no flag archaeology.

```bash
# Replace 'lazy_static' with 'once_cell' in main.rs
txed "lazy_static" "once_cell" src/main.rs
```

### Search & Destroy Workflow

Let `ripgrep` (`rg`) or `fd` decide **what** to touch, and let `txed` decide **how** to edit.

```bash
# Find files containing unwrap()
# Replace it safely everywhere
rg -l "unwrap()" | txed "unwrap()" "expect(\"checked by safe-mode\")"
```

No directory traversal.
No implicit recursion.
No surprises.

---

## 📚 CLI Guide

### Synopsis

```bash
# Replace in explicit files
txed [OPTIONS] FIND REPLACE [FILES...]

# Replace in files listed on stdin
fd -e rs | txed [OPTIONS] FIND REPLACE
rg -l PATTERN | txed [OPTIONS] FIND REPLACE

# Targeted edits using rg JSON matches
rg --json PATTERN | txed --rg-json [OPTIONS] FIND REPLACE

# Agent workflows
txed schema
txed apply --manifest manifest.json [OPTIONS]
```

---

### Commands

**`txed FIND REPLACE [FILES...]`**
Default command. Edits provided files or reads file paths from stdin when no files are passed.

**`schema`**
Print the JSON Schema describing manifests, operations, and output events.

```bash
txed schema > tools_schema.json
```

**`apply --manifest FILE`**
Apply a manifest (multi-file, multi-operation), with full validation and atomic commit.

```bash
txed apply --manifest manifest.json
```

---

## Input Modes

`txed` is explicit about what stdin represents.

### Auto (default)

If stdin is piped **and** no `FILES...` are provided, stdin is treated as a newline-delimited list of paths.

```bash
rg -l unwrap | txed unwrap expect
```

### `--stdin-paths`

Force stdin to be interpreted as newline-delimited paths.

### `--files0`

Read **NUL-delimited** paths from stdin. Compatible with `fd -0`, `find -print0`, `rg -l0`.

```bash
fd -0 -e rs | txed --files0 foo bar
```

### `--stdin-text`

Treat stdin as *content* and write transformed content to stdout. No files are opened.

```bash
printf '%s\n' "hello foo" | txed --stdin-text foo bar
```

### `--rg-json`

Consume `rg --json` output from stdin and apply edits **only** to reported match spans.

* No re-searching
* No heuristic matching
* Fails if input is not valid `rg` JSON

```bash
rg --json "foo" | txed --rg-json foo bar
```

### `--files`

Force positional arguments to be treated as files even when stdin is present.

---

## Match Semantics

### Literal by default

`FIND` is treated as a literal string.

### `--regex`

Interpret `FIND` as a regular expression.

```bash
txed --regex 'foo\s+bar' baz file.txt
```

### Case Handling

* `--ignore-case`
* `--smart-case`
  Case-insensitive unless `FIND` contains uppercase

(Default behavior is case-sensitive matching.)

---

## Scope Controls

**`--limit N`**
Maximum replacements per file.

```bash
txed foo bar file.rs --limit 1
```

**`--range START[:END]`**
Apply replacements only within a line range (1-based).

```bash
txed foo bar file.rs --range 10:200
```

**`--glob-include GLOB`**
Apply edits only to files whose *paths* match the glob.

```bash
fd . | txed foo bar --glob-include '**/*.rs'
```

**`--glob-exclude GLOB`**
Exclude matching paths.

---

## Safety & Guarantees

**`--dry-run`**
Print unified diffs. Perform no writes.

**`--no-write`**
Guarantee zero filesystem writes even if output mode changes.

**`--require-match`**
Fail if zero matches are found across all inputs.

**`--expect N`**
Require exactly `N` total replacements or abort.

**`--fail-on-change`**
Exit non-zero if any change would occur. Useful for CI.

---

## Transaction Model

**`--transaction all|file`**

* `all` (default): Commit only if **every** file succeeds
* `file`: Commit each file independently (still atomic per file)

---

## Filesystem Behavior

**`--symlinks follow|skip|error`**

* `follow` (default)
* `skip`
* `error`

**`--binary skip|error`**

* `skip` (default)
* `error`

**`--permissions preserve|fixed`**

* `preserve` (default)
* `fixed`

---

## Output Control

**Default behavior**

* TTY: unified diff + summary
* Pipe: structured JSON events

**Flags**

* `--json`: Force JSON output. See [JSON Event Schema](docs/JSON_EVENTS.md).
* `--quiet`
* `--format diff|summary|json|agent`

---

## 🍳 Cookbook (Common Patterns)

### Dry Run with Regex

Preview changes without modifying files.

```bash
# Preview replacing 3 digits with "NUM"
txed --dry-run --regex '\d{3}' 'NUM' data.txt
```

### Targeted Edits via `ripgrep`

Use `rg` to find specific matches (e.g. only in function bodies) and `txed` to replace them using the exact spans found by `rg`.

```bash
# Find "foo" only in lines starting with "fn" and replace with "bar"
rg --json '^fn.*foo' | txed --rg-json foo bar
```

### Bulk Rename via Manifest

Apply complex multi-file edits transactionally.

**manifest.json:**
```json
{
  "files": ["src/main.rs", "src/lib.rs"],
  "transaction": "all",
  "operations": [
    {
      "type": "replace",
      "find": "OldName",
      "with": "NewName"
    },
    {
      "type": "delete",
      "find": "// TODO: remove me"
    }
  ]
}
```

```bash
txed apply --manifest manifest.json
```

### Pipeline Validation

Check if a replacement would change anything without actually doing it.

```bash
# Fail if no changes would be made (ensure your regex matches)
txed --require-match foo bar src/

# Fail if changes WOULD be made (verify cleanliness)
txed --fail-on-change --dry-run foo bar src/
```

---

## 📂 Examples

Check the `examples/` directory for ready-to-use recipes:

*   [`examples/manifest_simple.json`](examples/manifest_simple.json): Basic replacement.
*   [`examples/manifest_advanced.json`](examples/manifest_advanced.json): Regex, deletes, and capture expansion.
*   [`examples/mcp_server.py`](examples/mcp_server.py): Python script to run `txed` as a Model Context Protocol server.

---

## Agent Mode (Manifests)

Agents submit a **pipeline manifest** describing multi-file atomic edits.

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

```bash
txed apply --manifest manifest.json
```

Options:

* `--validate-only`
* `--dry-run`
* `--json`

---

## 📊 Performance (Example Benchmark)

**Workload**
Literal replacement on a large wordlist, streamed to stdout

### Results

* **txed (release, fixed string, stdin-text)**
  ~**1.36 s** mean
  User: ~0.82 s
  System: ~0.53 s

* **sed (streaming replace)**
  ~**3.81 s** mean
  User: ~3.67 s
  System: ~0.11 s

### Summary

* `txed` is ~**2.8× faster** than `sed` for this workload
* `sed` spends most time in user-space regex processing
* `txed` benefits from deterministic parsing and efficient buffered I/O

---

## 📦 Installation

```bash
# From a local checkout
cargo install --path .

# Or, install directly from git (replace with your repo URL)
cargo install --git <REPO_URL>
```

Requires Rust 1.86+.

Prebuilt binaries and crates.io publishing are not set up yet.

---

## Exit Codes

* `0` success
* `1` operational failure
* `2` policy failure
* `3` partial or aborted transaction

---

## License

MIT

```
