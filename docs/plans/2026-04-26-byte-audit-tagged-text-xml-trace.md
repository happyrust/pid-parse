# Byte Audit Tagged Text XML Trace Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Register `/TaggedTxtData/Drawing` and `/TaggedTxtData/General` in `ByteAuditReport` so already-decoded XML metadata streams no longer appear as unregistered byte-audit inventory.

**Architecture:** Keep the change inside the byte-audit dispatcher. The XML parsers already accept UTF-8 strings and preserve raw XML on the model, so the trace path can validate UTF-8, call the existing parser, and consume the full raw byte range as `Decoded` only when parsing succeeds. Invalid UTF-8 or parser errors remain registered but consume zero bytes, making failures visible as leftover.

**Tech Stack:** Rust 2021, existing `pid_parse::byte_audit`, existing `parsers::drawing_xml` / `parsers::general_xml`, unit tests in `src/byte_audit/aggregate.rs`, CLI tests in `tests/inspect_cli.rs`.

---

### Task 1: Add Failing Byte-Audit Unit Coverage

**Files:**
- Modify: `src/byte_audit/aggregate.rs`

Add tests that build an in-memory `PidPackage` containing:
- `/TaggedTxtData/Drawing` with a small valid XML body.
- `/TaggedTxtData/General` with a small valid XML body.

Expected behavior:
- Each path has `parser_name = Some("parse_drawing_xml")` or `Some("parse_general_xml")`.
- `consumed_bytes == total_bytes`.
- The paths are absent from `unregistered_paths`.

Expected RED: tests fail because both paths are currently unregistered.

### Task 2: Register XML Stream Traces

**Files:**
- Modify: `src/byte_audit/aggregate.rs`

Implementation:
- Add dispatcher arms for `/TaggedTxtData/Drawing` and `/TaggedTxtData/General`.
- Add a private helper that:
  1. Builds a `ParserTraceBuilder`.
  2. Converts bytes with `std::str::from_utf8`.
  3. Calls the matching XML parser.
  4. Consumes `[0..data.len())` with `TraceConfidence::Decoded` only on success.
  5. Returns the builder so invalid payloads still produce a registered trace with leftover bytes.

### Task 3: Update Docs

**Files:**
- Modify: `docs/byte-audit-guide.md`
- Modify: `CHANGELOG.md`

Document that `TaggedTxtData/Drawing` and `TaggedTxtData/General` are now registered byte-audit streams when their XML is UTF-8 parseable.

### Task 4: Verify

Run:

```bash
cargo fmt --check
cargo test byte_audit --lib
cargo test --test inspect_cli byte_audit -- --nocapture
```

Expected: all focused byte-audit tests pass.
