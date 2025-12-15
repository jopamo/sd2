# JSON Event Schema

When running `sd2` with `--format json` (or implicit JSON output), the tool emits a stream of newline-delimited JSON objects. Each line is a self-contained event.

## Event Types

There are three top-level event types, wrapped in a single key:

1.  `run_start`: Emitted once at the beginning.
2.  `file`: Emitted for each file processed.
3.  `run_end`: Emitted once at the end.

### 1. Run Start (`run_start`)

Contains metadata about the execution, configuration, and policies.

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

**Fields:**
*   `schema_version`: Version of this JSON schema (currently "1").
*   `tool_version`: Version of `sd2`.
*   `mode`: Execution mode ("cli" or "apply").
*   `input_mode`: How inputs were provided ("args", "stdin-paths", "stdin-text", "rg-json", "files0").
*   `transaction_mode`: Transaction safety level ("all" or "file").
*   `dry_run`, `validate_only`, `no_write`: Boolean flags indicating safety modes.
*   `policies`: Object containing policy configuration (`require_match`, `expect`, `fail_on_change`).

### 2. File Event (`file`)

Describes the result of processing a single file. The object contains a `type` field indicating the outcome.

#### Success

Emitted when a file was successfully processed (even if no changes were made).

```json
{
  "file": {
    "type": "success",
    "path": "/abs/path/to/file.txt",
    "modified": true,
    "replacements": 2,
    "diff": "---\n+++ \n@@ -1 +1 @@\n-foo\n+bar\n",
    "generated_content": null
  }
}
```

**Fields:**
*   `type`: "success"
*   `path`: Path to the file.
*   `modified`: `true` if changes were made (or would be made in dry-run).
*   `replacements`: Number of replacements performed.
*   `diff`: Unified diff string (optional, usually present in dry-run or validation).
*   `generated_content`: The full transformed content (optional, mostly for `stdin-text` mode).

#### Skipped

Emitted when a file was skipped due to configuration or file type.

```json
{
  "file": {
    "type": "skipped",
    "path": "/abs/path/to/binary.exe",
    "reason": "binary"
  }
}
```

**Fields:**
*   `type`: "skipped"
*   `path`: Path to the file.
*   `reason`: Why it was skipped. Values: "binary", "symlink", "glob_exclude", "not_modified".

#### Error

Emitted when an operational error occurred for a specific file (e.g., permission denied).

```json
{
  "file": {
    "type": "error",
    "path": "/abs/path/to/locked.txt",
    "message": "Permission denied (os error 13)"
  }
}
```

**Fields:**
*   `type`: "error"
*   `path`: Path to the file.
*   `message`: Human-readable error message.

### 3. Run End (`run_end`)

Emitted after all files are processed. Contains aggregate statistics and final status.

```json
{
  "run_end": {
    "total_files": 10,
    "total_modified": 2,
    "total_replacements": 5,
    "has_errors": false,
    "policy_violation": null,
    "exit_code": 0
  }
}
```

**Fields:**
*   `total_files`: Total files scanned/processed.
*   `total_modified`: Number of files modified.
*   `total_replacements`: Total replacements across all files.
*   `has_errors`: `true` if any file-level errors occurred.
*   `policy_violation`: String describing policy failure (e.g., "No matches found"), or `null`.
*   `exit_code`: Suggested process exit code (0=success, 1=error, 2=policy violation).

```