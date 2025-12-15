# Project TODOs

This file tracks outstanding tasks and missing features required to match the v1 CLI documentation (`README.md`, `HACKING.md`, `helptext.txt`), with Rust-specific implementation details

## ✅ Baseline Rust Tech Stack Assumptions

* **CLI parsing:** `clap` derive (`clap = { features = ["derive"] }`)
* **Manifest model:** `serde` + `serde_json`
* **Regex:** `regex` crate
* **Globs:** `globset` (fast, compiled globs)
* **Temp + atomic commit:** `tempfile` + `std::fs::rename`
* **Diff output:** `similar` (or your own unified diff generator)
* **Tests:** `assert_cmd`, `predicates`, `tempfile`

---

## 🚨 Critical Missing CLI Features

These are documented but currently missing from `src/cli.rs` and/or execution logic.

### Commands

* [x] **`apply --validate-only`**
  **Rust wiring**

  * `clap` subcommand variant `Command::Apply { manifest: PathBuf, validate_only: bool, ... }`
  * Execution path should be:

    1. read manifest bytes
    2. `serde_json::from_slice::<Pipeline>()`
    3. validate semantic constraints (files non-empty, ops non-empty, supported op set)
    4. build a `Plan` without writing
    5. emit report/events (diff/summary/json) as if `--dry-run` but with a distinct flag in output events like `"validate_only": true`
  * Plan should include:

    * which files will be touched
    * per-file counts
    * policy violations that would abort a real run

---

### Input Modes

Implement as a mutually exclusive **input-mode state machine** that is explicit and testable.

* [x] **`--stdin-paths`**
  **Rust notes**

  * forces stdin to be interpreted as newline-delimited paths
  * disable auto-detection and treat piped stdin as paths even if it “looks like text”
  * implement as `InputMode::StdinPathsNewline`

* [x] **`--files0`**
  **Rust notes**

  * read NUL-delimited paths from stdin
  * implement by reading all bytes from stdin and splitting on `b'\0'`
  * avoid UTF-8 assumptions on raw path bytes if you want to be fully correct, but on Linux you can usually use `OsStrExt::from_bytes`
  * mode `InputMode::StdinPathsNul`

* [x] **`--stdin-text`**
  **Rust notes**

  * stdin is treated as *content*, not a path list
  * output goes to stdout, never writes files
  * returns counts/diff as stdout content + optional JSON events on stderr (or vice versa) depending on your chosen contract
  * mode `InputMode::StdinText`

* [x] **`--rg-json`**
  **Rust notes**

  * stream parse JSON lines from stdin
  * use `serde_json::Deserializer::from_reader(stdin).into_iter::<RgMessage>()` if you model messages, or read line-by-line and `from_str`
  * you must handle rg’s JSON “text vs bytes” objects for paths and lines

    * strict v1: accept `text` only, error if `bytes`
    * better: accept both by base64 decoding `bytes` into `Vec<u8>` and then attempt UTF-8 for content, while keeping paths as `OsString`
  * mode `InputMode::RipgrepJson`

* [x] **`--files`**
  **Rust notes**

  * when stdin is piped, default behavior might auto-select stdin paths
  * `--files` forces positional args to win
  * represent as a CLI boolean that affects input mode selection logic, not as its own mode

---

### Match Semantics

* [x] **`--regex`**
  **Rust notes**

  * default is literal find, implement literal efficiently with `memchr`/`find` on `&str` if you want
  * `--regex` uses `regex::Regex`
  * define:

    * `Matcher::Literal(String)`
    * `Matcher::Regex(Regex)`
  * replacement rules:

    * v1 simple: replacement is literal text, no `$1`
    * if you want capture expansions later, gate behind a separate flag like `--expand` so it doesn’t surprise humans or agents

---

### Scope Controls

* [x] **`--limit`** alias for `--max-replacements`
  **Rust notes**

  * in `clap`, you can mark `--limit` as `alias = "max-replacements"`
  * store as `Option<usize>`
  * apply per-file unless specified otherwise in manifest

* [x] **`--range START[:END]`**
  **Rust notes**

  * parse into `LineRange { start: usize, end: Option<usize> }`
  * line numbering is 1-based in CLI, convert internally to 0-based indices
  * apply as a filter layer around the replacer:

    * either split file into lines with offsets and only allow matches in allowed line span
    * or compute line number during scan and reject matches outside range

* [x] **`--glob-include GLOB`** and **`--glob-exclude GLOB`**
  **Rust notes**

  * treat as post-filters over the collected input file list
  * do not walk directories
  * compile globs with `globset::GlobSetBuilder`
  * apply include first, then exclude
  * be explicit about matching rules:

    * match against the incoming path string exactly as received
    * or normalize to relative paths from cwd (pick one and stick to it)

---

### Safety & Guarantees

These should be enforced in engine/report as policy checks.

* [x] **`--no-write`**
  **Rust notes**

  * stronger than `--dry-run`
  * enforce in one place: `EngineOptions { allow_writes: bool }`
  * every write path checks `allow_writes`, not “dry-run” scattered logic

* [x] **`--require-match`**
  **Rust notes**

  * after processing all inputs (or planning), if `total_replacements == 0` return `Exit::PolicyViolation`
  * make this identical for manifest and CLI modes

* [x] **`--expect N`**
  **Rust notes**

  * validate `total_replacements == N`
  * return policy error otherwise
  * for `--transaction all`, this should be checked *before* commit

* [x] **`--fail-on-change`**
  **Rust notes**

  * if `total_changes > 0`, exit non-zero even if `--dry-run`
  * useful for CI, so treat as policy layer independent of output mode

---

### Transaction Model

* [x] **`--transaction all|file`**
  **Rust design**

  * `enum TransactionMode { All, File }`
  * `File` mode: current behavior, apply per file with atomic write (temp + rename)
  * `All` mode:

    1. build `Vec<FilePlan>` for all files first
    2. stage all modified outputs into temp files (one per file)
    3. run policy checks on totals
    4. commit phase: rename temps into place
    5. rollback on any failure: delete temps, do not rename

  **Implementation note**

  * do not rename progressively in `All` mode
  * commit loop should be “all renames”, but you still can’t make rename atomic across multiple files, so the contract should be:

    * “no file is modified unless staging succeeded for all files”
    * then “commit attempts for all files”

---

### Filesystem Behavior

* [x] **`--symlinks follow|skip|error`**
  **Rust notes**

  * use `std::fs::symlink_metadata` to detect symlink without following
  * if `follow`, open the target with normal `read_to_string`
  * if `skip`, count as skipped in report
  * if `error`, abort with an error event
  * represent as `enum SymlinkMode`

* [ ] **`--binary skip|error`**
  **Rust notes**

  * early detection before parsing as UTF-8
  * simplest heuristic: if bytes contain `0x00`, treat as binary
  * better: use `content_inspector` for a more nuanced guess
  * represent as `enum BinaryMode`

* [ ] **`--permissions preserve|fixed`**
  **Rust notes**

  * preserve: capture `std::fs::metadata().permissions()` and re-apply after write if needed
  * fixed: accept `--mode 644` or `--mode 755` as a separate option, parsed as octal
  * on Linux, set perms with `std::fs::set_permissions`
  * represent as:

    * `enum PermissionsMode { Preserve, Fixed(u32) }`

---

### Output Control

* [ ] **`--quiet`**
  **Rust notes**

  * suppress human output, still emit JSON errors if `--json`
  * implement as output policy in reporter, not sprinkled `println!`

* [ ] **`--format diff|summary|json`**
  **Rust notes**

  * `enum OutputFormat { Diff, Summary, Json }`
  * `--json` can be an alias for `--format json`
  * reporter chooses format based on:

    * explicit `--format`
    * else if stdout is tty, diff
    * else json

* [x] **`--format agent` (Agent-friendly output)**
  **Rust notes**

  * use `BufferedAgentSink` from `src/rgjson.rs`
  * emit XML-style `<file path="...">` blocks
  * group matches by file to avoid interleaving

---

## 🛠 Feature Implementation Details

### Core Engine (`src/engine.rs`, `src/replacer/mod.rs`)

* [x] **Input Mode State Machine**

  * create `src/input.rs` with:

    * `enum InputMode`
    * `fn resolve_input_mode(cli: &Cli) -> Result<InputMode>`
  * enforce mutual exclusion at CLI parse time where possible using `clap` arg groups:

    * `--stdin-text` conflicts with `--files0` and `--rg-json`
  * engine takes an iterator of `InputItem`:

    * `InputItem::Path(PathBuf)`
    * `InputItem::RgSpan { path, line, byte_offset, match_len }`
    * `InputItem::StdinText(String)`

* [ ] **`--rg-json` span targeting**
  **Rust notes**

  * Currently `src/input.rs` only extracts file paths from `rg --json`.
  * Need to parse `submatches` and `absolute_offset` from `RgMessage`.
  * Create `InputItem::RgSpan` and feed to engine to limit replacement scope.

* [ ] **Transaction Manager**

  * create `src/transaction.rs`
  * implement:

    * `stage_file(plan: &FilePlan) -> Result<StagedFile>`
    * `commit_all(staged: Vec<StagedFile>) -> Result<()>`
  * store temp file handles in `StagedFile` so they stay alive until commit

* [x] **Range limiting**

  * implement at match-collection stage, not at write stage
  * recommended flow:

    1. read file
    2. find matches (literal/regex)
    3. map match byte offsets to line numbers (precompute line start offsets)
    4. filter matches by range
    5. apply replacements

* [x] **Post-filtering**

  * after collecting paths from stdin/args/manifest:

    * normalize (optional)
    * dedupe (optional but recommended, preserve first-seen order)
    * apply glob include/exclude
  * track why something was excluded for JSON events

* [ ] **Binary Detection**

  * do it before `String::from_utf8` conversions
  * if binary and mode is skip:

    * record in report and continue
  * if binary and mode is error:

    * abort run

---

### Reporting (`src/report.rs`)

* [ ] **Policy Checks**

  * implement a single function:

    * `fn enforce_policies(report: &Report, opts: &PolicyOptions) -> Result<(), PolicyError>`
  * policies should run:

    * after plan (for validate-only)
    * after staging (for transaction all)
    * after run (for transaction file)

* [ ] **Events for JSON output**

  * define stable event structs with `serde::Serialize`
  * always include:

    * mode (cli/apply)
    * transaction mode
    * per-file stats
    * skipped reasons
    * policy violations

---

## 🔮 Future / Planned

* [ ] **New Operations**

  * extend `src/model.rs` with tagged enums:

    * `Operation::Replace { find, with, limit }`
    * `Operation::Delete { find, limit }`
    * `Operation::Insert { at, text }`
    * `Operation::RegexReplace { pattern, with, limit }`
  * use `#[serde(tag = "op", rename_all = "snake_case")]` or similar to keep schema stable

* [ ] **Manifest Updates**

  * add to `Pipeline`:

    * `transaction: Option<TransactionMode>`
    * `glob_include: Option<Vec<String>>`
    * `glob_exclude: Option<Vec<String>>`
  * keep CLI flags as overrides with clear precedence rules:

    * CLI overrides manifest unless `--respect-manifest` is set (if you ever add it)

---

## Suggested Rust Module Layout

* `src/cli.rs` clap structs + arg groups
* `src/input.rs` stdin/paths/rg-json ingestion
* `src/rgjson.rs` rg message types + decoder
* `src/engine.rs` planning + apply logic
* `src/replacer/` literal + regex replacers
* `src/transaction.rs` staging + commit
* `src/report.rs` summaries + diff + json events
* `src/model.rs` manifest structs + operations
