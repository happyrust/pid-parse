# Byte Audit JProperties Trace Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Register `*/JProperties` streams in `ByteAuditReport` without overstating coverage, by tracing only the recoverable ASCII / UTF-16LE text runs that the existing heuristic parser actually uses.

**Architecture:** Add a trace-aware wrapper in `parsers::jproperties` and keep the legacy `parse_jproperties` API as a thin wrapper. The trace variant reuses the same string-run heuristics as the parser, emits `TraceConfidence::Probed` ranges for recovered text bytes, and leaves opaque binary bytes as leftover. `byte_audit::aggregate` dispatches any normalized path ending in `/JProperties` to this parser.

**Tech Stack:** Rust 2021, existing `ParserTraceBuilder`, `ByteRange`, `TraceConfidence`, existing `JProperties` DTO, unit tests in `src/parsers/jproperties.rs` and `src/byte_audit/aggregate.rs`.

---

### Task 1: Add Failing Tests

**Files:**
- Modify: `src/parsers/jproperties.rs`
- Modify: `src/byte_audit/aggregate.rs`

Parser test:
- Build bytes with binary prefix, one ASCII run, one UTF-16LE run, and binary suffix.
- Call `parse_jproperties_with_trace`.
- Assert legacy decoded strings are still present.
- Build the trace and assert only text-run bytes are consumed.

Aggregate test:
- Build a `PidPackage` with `/JSite0001/JProperties`.
- Assert the stream is registered as `parse_jproperties`.
- Assert consumed bytes are greater than zero but less than total bytes.
- Assert the path is not in `unregistered_paths`.

Expected RED: `parse_jproperties_with_trace` does not exist and aggregate does not register `*/JProperties`.

### Task 2: Implement Trace-Aware Parser

**Files:**
- Modify: `src/parsers/jproperties.rs`

Implementation:
- Keep `parse_jproperties(data)` as a wrapper that creates a throwaway trace builder.
- Add `parse_jproperties_with_trace(data, trace)`.
- Factor internal ASCII / UTF-16LE range scanners that mirror the existing string extraction thresholds.
- Consume matching ranges as `TraceConfidence::Probed`.
- Avoid claiming binary prefix / suffix bytes.

### Task 3: Register Aggregate Dispatch

**Files:**
- Modify: `src/byte_audit/aggregate.rs`

Implementation:
- Add a `path.ends_with("/JProperties")` dispatch arm.
- Use parser name `parse_jproperties`.
- Build the trace with the actual normalized stream path.

### Task 4: Update Docs

**Files:**
- Modify: `docs/byte-audit-guide.md`
- Modify: `CHANGELOG.md`

Document that `*/JProperties` streams are registered, but only recovered text-run bytes are consumed; remaining opaque binary bytes intentionally stay leftover.

### Task 5: Verify

Run:

```bash
cargo fmt --check
cargo test jproperties --lib
cargo test byte_audit --lib
```

Expected: focused parser and aggregate tests pass.
