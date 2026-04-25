# Byte Audit Sheet Text Trace Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Register top-level `/Sheet*` streams in `ByteAuditReport` by tracing only the printable text-run evidence already produced by `sheet_probe`, without claiming unknown geometry or record-layout bytes.

**Architecture:** Reuse `parsers::sheet_probe::probe_sheet_stream` as an evidence source. The aggregate dispatcher will recognize top-level sheet stream names, run the probe with default options, and consume non-overlapping `SheetProbeReport::text_runs` as `TraceConfidence::Probed`. Candidate chunk boundaries, record-type counts, and coordinate hints remain evidence only and do not count as consumed bytes.

**Tech Stack:** Rust 2021, existing `sheet_probe`, existing `byte_audit::aggregate`, unit tests in `src/byte_audit/aggregate.rs`.

---

### Task 1: Add Failing Aggregate Test

**Files:**
- Modify: `src/byte_audit/aggregate.rs`

Build an in-memory package with `/Sheet6` containing:
- binary prefix bytes,
- an ASCII text run,
- a UTF-16LE text run,
- binary suffix bytes.

Expected behavior:
- `parser_name = Some("probe_sheet_stream")`.
- `consumed_bytes > 0`.
- `consumed_bytes < total_bytes`.
- `/Sheet6` is absent from `unregistered_paths`.

Expected RED: `/Sheet6` is currently unregistered.

### Task 2: Implement Sheet Trace Dispatch

**Files:**
- Modify: `src/byte_audit/aggregate.rs`

Implementation:
- Add a private `is_top_level_sheet_stream(path)` helper.
- Add a dispatcher arm before the final `_ => None`.
- Run `probe_sheet_stream(sheet_name, path, data, &SheetProbeOptions::default())`.
- Consume non-overlapping `text_runs` as `TraceConfidence::Probed`.
- Leave all other bytes as leftover.

### Task 3: Update Docs

**Files:**
- Modify: `docs/byte-audit-guide.md`
- Modify: `CHANGELOG.md`

Document that `/Sheet*` streams are registered with partial text-run coverage only. Clarify that geometry / record chunks remain evidence and are not counted as consumed yet.

### Task 4: Verify

Run:

```bash
cargo fmt --check
cargo test byte_audit --lib
```

Expected: focused byte-audit tests pass.
