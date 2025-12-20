# Integrating `sd2` into a Model Context Protocol (MCP) Server

This document describes how to expose the `sd2` binary as a tool in a Python-based **Model Context Protocol (MCP)** server.

The goal is to allow LLMs (Claude Desktop, Cursor, or custom agents) to perform **safe, atomic, and deterministic** text edits on a codebase by delegating all mutation logic to `sd2`.

This integration treats `sd2` as the **sole authority** for filesystem edits.

---

## Goals

* Allow LLMs to request refactors without writing ad-hoc scripts
* Guarantee atomicity and rollback on failure
* Provide structured, machine-readable feedback
* Avoid shell injection, heuristic matching, or partial writes

---

## Prerequisites

* `sd2` installed and available on `$PATH` (or via absolute path)
* Python **3.10+**
* `uv` (recommended) or `pip`
* An MCP-capable client (Claude Desktop, Cursor, etc.)

---

## Architecture

The integration uses MCP’s **stdio transport**.

The Python process acts as the MCP server and invokes `sd2` via `subprocess`.
All filesystem mutation happens inside `sd2`.

```mermaid
[LLM Client]
    ⇄ MCP (stdio)
        ⇄ Python MCP Server
            ⇄ subprocess
                ⇄ sd2
                    ⇄ filesystem
```

Key properties:

* The LLM never touches the filesystem directly
* The Python layer is a thin adapter, not an editor
* `sd2 --format json` is the API boundary

---

## Project Setup

Create a new Python project for the MCP server:

```bash
uv init sd2-mcp
cd sd2-mcp
uv add "mcp[cli]"
```

This project should contain **no code that edits files directly**.

---

## Python MCP Server

### Overview

The server exposes two tools:

1. **`sd2_replace`**
   Simple, direct replacements on explicit file lists

2. **`sd2_apply`**
   Manifest-based, multi-file atomic operations (agent mode)

All output is derived from `sd2`’s JSON event stream.

---

### `server.py`

```python
import json
import shutil
import subprocess
import tempfile
from typing import Optional, List, Dict, Any
from mcp.server.fastmcp import FastMCP

mcp = FastMCP("sd2-tools")

SD2_BINARY = "sd2"  # must be resolvable via PATH or absolute path


def run_sd2_command(args: List[str], input_data: Optional[str] = None) -> str:
    """
    Run sd2 with forced JSON output and summarize results for the LLM.
    """
    final_args = [SD2_BINARY] + args + ["--format=json"]

    try:
        process = subprocess.Popen(
            final_args,
            stdin=subprocess.PIPE if input_data else None,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        stdout, stderr = process.communicate(input=input_data)
    except FileNotFoundError:
        return f"Error: '{SD2_BINARY}' not found in PATH"

    modified = []
    errors = []

    for line in stdout.splitlines():
        if not line.strip():
            continue

        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue

        if "file" in event:
            f = event["file"]
            path = f.get("path", "<unknown>")

            if f["type"] == "success" and f.get("modified"):
                modified.append(
                    f"{path} ({f.get('replacements', 0)} replacements)"
                )

            elif f["type"] == "error":
                errors.append(f"{path}: {f.get('message')}")

        elif "run_end" in event:
            if event["run_end"].get("policy_violation"):
                errors.append(
                    f"Policy violation: {event['run_end']['policy_violation']}"
                )

    out = []

    if modified:
        out.append("### Modified files")
        out.extend(f"- {m}" for m in modified)
    else:
        out.append("No files were modified")

    if errors:
        out.append("\n### Errors")
        out.extend(f"- {e}" for e in errors)

    if stderr.strip():
        out.append("\n### stderr")
        out.append(stderr)

    return "\n".join(out)


@mcp.tool()
def sd2_replace(
    find: str,
    replace: str,
    files: List[str],
    regex: bool = False,
    dry_run: bool = False,
) -> str:
    """
    Perform a simple search-and-replace on explicit files.
    """
    args = [find, replace] + files

    if regex:
        args.append("--regex")
    if dry_run:
        args.append("--dry-run")

    return run_sd2_command(args)


@mcp.tool()
def sd2_apply(
    manifest: Dict[str, Any],
    dry_run: bool = False,
) -> str:
    """
    Apply a manifest describing multi-file atomic operations.
    """
    with tempfile.NamedTemporaryFile(
        mode="w", suffix=".json", delete=False
    ) as f:
        json.dump(manifest, f)
        manifest_path = f.name

    try:
        args = ["apply", "--manifest", manifest_path]
        if dry_run:
            args.append("--dry-run")

        return run_sd2_command(args)
    finally:
        try:
            shutil.os.remove(manifest_path)
        except OSError:
            pass


if __name__ == "__main__":
    mcp.run()
```

---

## Client Configuration

### Claude Desktop

Add to `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "sd2": {
      "command": "uv",
      "args": [
        "run",
        "/absolute/path/to/sd2-mcp/server.py"
      ]
    }
  }
}
```

---

### Cursor

Add a new MCP server:

* **Transport:** stdio
* **Command:**

  ```text
  uv run /absolute/path/to/sd2-mcp/server.py
  ```

---

## Behavioral Guarantees

This integration relies on guarantees provided by `sd2`:

### Atomicity

* Default `transaction=all`
* If any file fails, **no files are modified**

### Determinism

* No implicit traversal
* No heuristic matching
* Binary, symlink, and permission handling is explicit

### Structured Feedback

* Every action produces structured JSON events
* Policy failures are explicit
* Errors are machine-readable and stable

---

## Non-Goals

This integration intentionally does **not**:

* Allow the LLM to write files directly
* Generate or execute arbitrary shell code
* Guess which files should be edited
* Perform directory traversal

All such logic must be expressed explicitly via `sd2`.

---

## Recommended Agent Usage

* Use **`sd2_replace`** for small, obvious changes
* Use **`sd2_apply`** for:

  * multi-file refactors
  * deletes
  * policy-guarded edits
* Always start with `dry_run` when uncertain
* Use JSON event feedback to refine the next request
