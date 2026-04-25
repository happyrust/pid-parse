# Sheet Chunk Probe Enhancement Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Enrich the experimental Sheet probe with report-level evidence for record-type candidates, text runs, and coordinate-like pairs before attempting semantic Sheet geometry decoding.

**Architecture:** Keep `Sheet*` output evidence-only. Extend `SheetProbeReport` with additive fields that summarize byte patterns across the whole stream while preserving the existing chunk-splitting API and `SheetChunk` fields. The CLI already serializes the full report for `--probe-sheet-chunks --json`; text mode will print compact summaries above chunk details.

**Tech Stack:** Rust 2021, existing `src/parsers/sheet_probe.rs`, existing `pid_inspect --probe-sheet-chunks` CLI, unit tests in `sheet_probe.rs`.

---

### Task 1: Add Probe-Level Evidence Types

**Files:**
- Modify: `src/parsers/sheet_probe.rs`

**Step 1: Write failing unit tests**

Add tests for:
- `records_marker_following_u16_type_counts`
- `captures_text_runs_with_offsets`
- `captures_coordinate_pair_hints`

Expected RED: fields such as `record_type_counts`, `text_runs`, and `coordinate_hints` do not exist yet.

**Step 2: Implement minimal additive model**

Add report fields:
- `record_type_counts: BTreeMap<String, usize>`
- `text_runs: Vec<SheetTextRun>`
- `coordinate_hints: Vec<SheetCoordinateHint>`

Add small supporting types with rustdoc:
- `SheetTextRun`
- `SheetTextEncoding`
- `SheetCoordinateHint`

### Task 2: Populate Evidence in `probe_sheet_stream`

**Files:**
- Modify: `src/parsers/sheet_probe.rs`

**Step 1: Record type candidates**

Scan for `0x89` marker bytes. When at least two following bytes exist, read the next little-endian `u16` and count it as `0xNNNN`.

This is intentionally a candidate frequency, not a semantic claim.

**Step 2: Text runs**

Collect ASCII and UTF-16LE printable runs with offsets and byte lengths. Use existing thresholds from `SheetProbeOptions` so behavior stays aligned with boundary detection.

**Step 3: Coordinate hints**

Scan aligned adjacent `i32` pairs and keep plausible `(x, y)` candidates where both values are in a conservative drawing-coordinate range and not both zero.

### Task 3: Surface Summaries in CLI Text Mode

**Files:**
- Modify: `src/bin/pid_inspect.rs`

Print compact report-level lines before chunk rows:
- `record types: 0x00CE=12, ...`
- `text runs: N`
- `coordinate hints: N`

JSON mode needs no extra work because the report derives `Serialize`.

### Task 4: Verify

Run:

```bash
cargo test --lib sheet_probe -- --nocapture
cargo test --test inspect_cli byte_audit -- --nocapture
cargo fmt --all
```

Expected: all targeted tests pass; no linter errors.
