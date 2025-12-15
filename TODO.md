# Project TODOs

This tracks what’s left to finish, stabilize, and ship now that the core CLI + engine features are largely in place.

## ✅ Status recap

You’ve already implemented the heavy hitters:

* CLI flags + input modes (stdin paths / files0 / stdin-text / rg-json / files override)
* matcher semantics (literal + regex), ranges, limit/max-replacements
* include/exclude globs, path collection + post-filtering
* safety policies (no-write, require-match, expect, fail-on-change)
* transactions (file + all), staging/commit behavior
* filesystem behavior (symlink modes, binary modes, permissions preserve/fixed)
* output formats (diff/summary/json/agent) and rg-json span targeting

What’s left is mostly **structured output stability**, **consistency**, and **ship readiness**.

---

## 🚨 Release-blockers

### 1) Stable JSON event model (the big remaining gap)

**Goal:** “`--format json`” becomes a contract you can version and other tools can rely on.

* [x] Define a **single event schema** (serde-serializable) with:

  * run header: tool version, mode (cli/apply), input mode, transaction mode, policy knobs
  * per-file events: changed/skipped/error stats + reason enums
  * policy results: require-match/expect/fail-on-change, validate-only, no-write/dry-run
  * final summary: totals + exit classification
* [x] Ensure **all skip/error paths** emit structured events (binary skip, symlink skip, glob exclude, unreadable file, permission failure, etc)
* [x] Make JSON output ordering deterministic (run_start → file events → run_end)
* [x] Add `schema_version: "1"` (or similar) so you can evolve it safely later

### 2) Exit code + error taxonomy alignment

* [x] Standardize exit categories (examples):

  * success (0)
  * policy violation (2)
  * input error (1)
  * filesystem error (1)
  * internal error (3 for transaction failure)
* [x] Make sure **transaction all** returns the correct exit classification when staging fails vs commit fails
* [x] Add tests that assert exit codes for the major policy flags and failure modes

---

## 🧪 Test completion and hardening

### 3) Expand tests for JSON events

* [x] Golden-style tests for `--format json`:

  * stable keys present
  * per-file arrays contain expected stats
  * skip reasons match expected enum strings
  * validate-only includes `"validate_only": true`
* [x] Tests for “no writes happened” in:

  * `--no-write`
  * `--stdin-text`
  * validate-only
  * transaction all staging failure

### 4) Path + glob matching edge cases

* [x] Confirm and lock down what you match globs against:

  * raw incoming path vs normalized relative-to-cwd
* [x] Add tests for:

  * `./path` vs `path`
  * absolute paths
  * repeated inputs and dedupe ordering (if you dedupe)
  * glob include then exclude precedence (already planned, now verify via tests)

---

## 🔧 UX polish that pays off fast

### 5) Output behavior consistency

* [x] Ensure `--quiet` suppresses human output but **never suppresses JSON errors/events**
* [x] Decide + enforce one contract for mixed streams:

  * human output to stdout and JSON to stderr, or vice versa
  * make it consistent across all modes (stdin-text, rg-json, apply manifest)

### 6) Diff correctness and stability

* [x] Confirm diff formatting is stable and deterministic (path headers, newline handling, no trailing noise)
* [x] Add tests for newline edge cases:

  * files without trailing newline
  * CRLF input (if you support it) and how it’s preserved

---

## ⚙️ Internal cleanup (good “post-v1” but low risk)

### 7) Refactor: one “policy enforcement” chokepoint

* [x] Ensure policies are enforced from exactly the correct lifecycle points:

  * validate-only: after plan, before any stage/write
  * transaction all: after staging and before commit
  * transaction file: after per-file apply or at end (depending on policy)
* [x] Avoid policy logic leaking into reporter and engine separately (single authoritative function)

### 8) Performance sanity checks

* [x] Add a small benchmark or at least stress tests for:

  * large files
  * many files (transaction all staging)
  * rg-json streaming inputs
* [x] Confirm match scanning doesn’t do accidental quadratic work (especially with line/range mapping)

---

## 🔮 Future / planned (keep, but clearly non-blocking)

### 9) New operations in manifest model

* [ ] Extend `Operation` (tagged serde enums) with:

  * replace / delete / insert / regex_replace
* [ ] Decide whether capture expansion (`$1`) exists and gate it behind a flag if added

### 10) Manifest schema upgrades

* [ ] Add optional manifest keys:

  * transaction, glob_include/exclude
* [ ] Define precedence rules (CLI overrides manifest unless a future “respect-manifest” is added)

