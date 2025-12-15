#Project TODOs: Release & Roadmap

##🚀 Phase 1: Pre-Release Polish (Docs & Perf)*The code works. Now, prove it performs and explain how to use it.*

###1) Performance Benchmarking (Carried Over)* [ ] **Stress Test:** Run against a massive repo (e.g., Linux kernel, chromium) to check memory usage during `transaction: all`.
* [ ] **Streaming Latency:** Verify `rg-json` input doesn't introduce blocking buffers (ensure output flows as matches are found).
* [ ] **Quadratic Check:** Verify lines with 10k+ characters or files with 100k+ matches don't stall the engine.

###2) Documentation & Contracts* [x] **JSON Schema Reference:** Document the "Stable JSON event model" you just finalized.
* *Must include:* Example success payload, example error payload, enum definitions.


* [ ] **CLI Help Text Audit:** Ensure `--help` output groups flags logically (e.g., separating "Safety Policies" from "Input Formatting").
* [x] **Recipe/Cookbook:** Add examples for common patterns:
* *“Dry run a regex replacement”*
* *“Pipe ripgrep JSON into tool”*
* *“Apply a bulk edit via manifest”*

---

##📦 Phase 2: Packaging & Distribution*Turn the binary into a release artifact.*

###3) Build Pipeline (CI)* [ ] **Release Profile:** Ensure `release` builds have LTO (Link Time Optimization) enabled and symbols stripped (if applicable) for size/speed.
* [ ] **Cross-Compilation:** Verify builds for:
* Linux (x86_64, aarch64)
* macOS (Intel, Apple Silicon)
* Windows (msvc)


* [ ] **Versioning:** Tag the repo with `v0.1.0` (or `v1.0.0`) and ensure the CLI `--version` output matches the git tag.

---

##🔮 Phase 3: The "Manifest" Evolution (Next Feature Set)*Now that the CLI is safe, turn the tool into a generalized refactoring engine via the Manifest.*

###4) Expanded Operation Primitives* [x] **Schema Expansion:** Update `Operation` enum to support:
* `replace` (current behavior)
* `delete` (remove match entirely)
* `insert_before` / `insert_after`


* [x] **Regex Capture Groups:** Investigate support for `$1` / `${1}` capture expansion in replacement strings.
* *Decision:* Gate behind a `--enable-captures` flag for safety? (Implemented `expand` flag)



###5) Manifest Logic upgrades* [x] **Manifest-level Configuration:** Allow the manifest JSON/YAML to specify:
* `"transaction_mode": "all"` (Override CLI default)
* `"glob_include": [...]`


* [x] **Precedence Logic:** Implement the logic: *CLI Flags > Manifest Config > Defaults*.

---

##🧪 Phase 4: Extended QA (The "Real World")*Beyond unit tests.*

###6) Dogfooding* [ ] **The "Self-Edit":** Use the tool to run a refactor on its own codebase (e.g., renaming a variable project-wide).
* [ ] **Fuzzing:** (Optional) Throw random bytes at the `--format json` parser or input streams to ensure no panics occur.
