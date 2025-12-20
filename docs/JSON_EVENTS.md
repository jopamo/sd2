# JSON Event Schema

When `sd2` runs with `--format json` (or when JSON output is selected implicitly), it emits a **newline-delimited JSON (NDJSON)** stream.
Each line is a complete, self-contained event object.

This document is **normative**.
If emitted JSON diverges from this specification, the implementation is wrong.

---

## General Rules

* Output is **NDJSON** (one JSON object per line)
* Events are emitted in **strict order**
* Each event contains **exactly one** top-level key
* Optional fields are **omitted**, not set to `null`, unless explicitly stated
* All strings are valid UTF-8

  * Invalid sequences are replaced with U+FFFD (`�`)
* Unknown reasons or codes must **not** be collapsed or re-labeled

---

## Event Types

There are exactly three event types, wrapped in a single top-level key:

1. `run_start` — emitted once at the beginning
2. `file` — emitted once per processed input item
3. `run_end` — emitted once at the end

---

## Event Order

Events are emitted in this order:

1. Exactly one `run_start`
2. Zero or more `file` events
3. Exactly one `run_end`

---

## 1. Run Start Event

### `run_start`

Emitted once at the beginning of execution.
Describes configuration, modes, and active policies.

```json
{
  "run_start": {
    "schema_version": "1",
    "tool_version": "0.1.0",
    "mode": "cli",
    "input_mode": "args",
    "transaction_mode": "all",
    "dry_run": false,
    "validate_only": false,
    "no_write": false,
    "policies": {
      "require_match": false,
      "expect": null,
      "fail_on_change": false
    }
  }
}
```

### Fields

| Field              | Type    | Description                                                                      |
| ------------------ | ------- | -------------------------------------------------------------------------------- |
| `schema_version`   | string  | JSON event schema version. Currently `"1"`                                       |
| `tool_version`     | string  | `sd2` version string                                                             |
| `mode`             | string  | `"cli"` or `"apply"`                                                             |
| `input_mode`       | string  | `"args"`, `"stdin-paths"`, `"stdin-text"`, `"rg-json"`, `"files0"`, `"manifest"` |
| `transaction_mode` | string  | `"all"` or `"file"`                                                              |
| `dry_run`          | boolean | Dry-run mode enabled                                                             |
| `validate_only`    | boolean | Validation-only mode enabled                                                     |
| `no_write`         | boolean | Filesystem writes disabled                                                       |
| `policies`         | object  | Active policy configuration                                                      |

#### `policies` fields

| Field            | Type           | Description                    |
| ---------------- | -------------- | ------------------------------ |
| `require_match`  | boolean        | Fail if zero matches occur     |
| `expect`         | number or null | Require exactly N replacements |
| `fail_on_change` | boolean        | Fail if any change would occur |

---

## 2. File Event

### `file`

Emitted once per input item that is considered for processing.
The `type` field determines the event shape.

---

### Success

Emitted when a file or virtual input was processed successfully, even if no changes were made.

```json
{
  "file": {
    "type": "success",
    "path": "/abs/path/to/file.txt",
    "modified": true,
    "replacements": 2,
    "diff": "---\n+++ \n@@ -1 +1 @@\n-foo\n+bar\n",
    "diff_is_binary": false,
    "is_virtual": false
  }
}
```

#### Fields

| Field               | Type    | Description                                               |
| ------------------- | ------- | --------------------------------------------------------- |
| `type`              | string  | Always `"success"`                                        |
| `path`              | string  | Absolute path, or a virtual identifier (e.g. `"<stdin>"`) |
| `modified`          | boolean | `true` if changes were made or would be made              |
| `replacements`      | number  | Number of replacements performed                          |
| `diff`              | string  | Unified diff. Omitted if unavailable                      |
| `diff_is_binary`    | boolean | `true` if diff was suppressed due to binary content       |
| `generated_content` | string  | Full transformed content. Omitted unless relevant         |
| `is_virtual`        | boolean | `true` if input does not exist on disk                    |

---

### Skipped

Emitted when an input item was intentionally skipped.

```json
{
  "file": {
    "type": "skipped",
    "path": "/abs/path/to/binary.exe",
    "reason": "binary"
  }
}
```

#### Fields

| Field    | Type   | Description                |
| -------- | ------ | -------------------------- |
| `type`   | string | Always `"skipped"`         |
| `path`   | string | Path or virtual identifier |
| `reason` | string | Reason for skipping        |

`reason` is an open set. Known values include:

* `binary`
* `symlink`
* `glob_exclude`

Unknown reasons must be preserved verbatim.

---

### Error

Emitted when an operational error occurs while processing a specific input.

```json
{
  "file": {
    "type": "error",
    "path": "/abs/path/to/locked.txt",
    "code": "E_ACCES",
    "message": "Permission denied (os error 13)"
  }
}
```

#### Fields

| Field     | Type   | Description                  |
| --------- | ------ | ---------------------------- |
| `type`    | string | Always `"error"`             |
| `path`    | string | Path or virtual identifier   |
| `code`    | string | Machine-readable error code  |
| `message` | string | Human-readable error message |

Error codes are stable and suitable for automation.

---

## 3. Run End Event

### `run_end`

Emitted once after all file events.
Contains aggregate statistics and final status.

```json
{
  "run_end": {
    "total_files": 10,
    "total_processed": 10,
    "total_modified": 2,
    "total_replacements": 5,
    "has_errors": false,
    "policy_violation": null,
    "committed": true,
    "duration_ms": 45,
    "exit_code": 0
  }
}
```

### Fields

| Field                | Type           | Description                        |
| -------------------- | -------------- | ---------------------------------- |
| `total_files`        | number         | Total inputs considered            |
| `total_processed`    | number         | Inputs actually processed          |
| `total_modified`     | number         | Files modified                     |
| `total_replacements` | number         | Total replacements                 |
| `has_errors`         | boolean        | Any file-level errors occurred     |
| `policy_violation`   | string or null | Policy failure description         |
| `committed`          | boolean        | Transaction committed successfully |
| `duration_ms`        | number         | Execution duration                 |
| `exit_code`          | number         | Suggested process exit code        |

`committed` is always `false` for dry-run or validation-only executions.

---

## Stability Guarantees

* Field names are stable within a schema version
* New fields may be added only in a backward-compatible way
* Behavior changes require a schema version bump
* Event order and meanings never depend on output format
