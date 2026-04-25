# Byte Audit CLI Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Expose the existing `byte_audit_report` library API through `pid_inspect --byte-audit` with human-readable and JSON output.

**Architecture:** Keep parser and byte-audit internals unchanged. `pid_inspect` already parses a `PidPackage` for normal inspect modes, so the CLI can reuse that package and render `ByteAuditReport` as either text or `serde_json`. Tests exercise the binary boundary using synthetic CFB fixtures, matching the existing `--coverage` CLI test style.

**Tech Stack:** Rust 2021, existing `pid_parse::byte_audit_report`, `serde_json`, `std::process::Command` integration tests.

---

### Task 1: Add CLI Regression Tests

**Files:**
- Modify: `tests/inspect_cli.rs`

**Step 1: Write the failing tests**

Add two tests:
- `byte_audit_flag_prints_text_report`
- `byte_audit_json_flag_emits_parseable_report`

Reuse `build_mixed_coverage_fixture`, because it already contains a traced `/DocVersion3` stream and unregistered streams.

Expected text output should include:
- `--- Byte Audit ---`
- `Overall coverage:`
- `/DocVersion3`
- `parse_doc_version3`
- `/MysteryStream`
- `unregistered`

Expected JSON output should parse and include:
- top-level `per_stream`
- top-level `unregistered_paths`
- `/MysteryStream` inside `unregistered_paths`
- `/DocVersion3` with `parser_name == "parse_doc_version3"`

**Step 2: Run tests to verify RED**

Run:

```bash
cargo test --test inspect_cli byte_audit -- --nocapture
```

Expected: FAIL because `pid_inspect` does not recognize `--byte-audit` yet and falls back to the full report path.

### Task 2: Implement `pid_inspect --byte-audit`

**Files:**
- Modify: `src/bin/pid_inspect.rs`

**Step 1: Parse the new flag**

Add:

```rust
let byte_audit = args.iter().any(|a| a == "--byte-audit");
```

Update usage text to include:

```text
[--byte-audit]
```

**Step 2: JSON path**

Inside the existing `if json_mode` block, handle `byte_audit` before full-document JSON:

```rust
if byte_audit {
    match serde_json::to_string_pretty(&pid_parse::byte_audit_report(&pkg)) {
        Ok(json) => println!("{json}"),
        Err(e) => { ... }
    }
    return;
}
```

**Step 3: Text path**

Add `print_byte_audit(&pkg)` and call it when `byte_audit` is set.

Text output should be deterministic and compact:

```text
--- Byte Audit ---
Total stream bytes: ...
Overall consumed: ...
Overall leftover: ...
Overall coverage: ...
Fully consumed traced streams: N
Unregistered streams: N
  [100.0%] /DocVersion3 ... parse_doc_version3
  [  0.0%] /MysteryStream ... unregistered
```

**Step 4: Suppress legacy full report**

Add `&& !byte_audit` to the final "no action flags" condition.

### Task 3: Verify and Format

**Files:**
- Verify: `tests/inspect_cli.rs`
- Verify: `src/bin/pid_inspect.rs`

Run:

```bash
cargo test --test inspect_cli byte_audit -- --nocapture
cargo test --test inspect_cli -- --nocapture
cargo fmt --all
```

Expected: all inspect CLI tests pass and formatting is clean.
