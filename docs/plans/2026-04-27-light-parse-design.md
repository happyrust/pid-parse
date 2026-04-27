# Light Parse Design Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Design a measured light-parse mode for bulk scans without surprising existing `PidParser::parse_file` / `parse_package` behavior.

**Architecture:** Treat light parse as an explicit option profile, not a silent shortcut in existing entry points. Preserve CFB tree/stream inventory and writer-relevant raw bytes for package parsing, then skip expensive semantic passes only when the caller opts in. Measure real fixture runtime before claiming performance wins.

**Tech Stack:** Rust 2021, `PidParser`, `ParseOptions`, CFB reader pipeline, existing test fixtures under `test-file/`.

---

## Context

This is Task 8 from `docs/plans/2026-04-26-parser-api-consistency-fixes.md`.

Current parse pipeline in `src/cfb/reader.rs`:

1. Build CFB tree.
2. Collect stream inventory + package raw bytes.
3. Parse summary streams.
4. Optionally parse tagged text XML (`parse_xml`).
5. Optionally parse `JSite` properties (`parse_jsite_properties`).
6. Always parse clusters, dynamic attrs, PSM tables, doc registry, DocVersion2, sheet endpoints.
7. Always build object inventory, object graph, cross-reference, and layout.

Existing knobs:

- `scan_strings`
- `parse_xml`
- `parse_jsite_properties`
- `keep_unknown_streams`
- `max_preview_strings`

Gap:

- There is no single documented “light profile”.
- Expensive derived passes (`object_graph`, `cross_reference`, `layout`) are always run.
- No measured fixture baseline is attached to the design.

## Proposed Contract

Add an explicit `ParseProfile` or equivalent option in a follow-up implementation:

```rust
pub enum ParseProfile {
    Full,
    Light,
}
```

`Full` remains the default and preserves current behavior.

`Light` should keep:

- CFB tree and stream inventory.
- Summary streams.
- Raw stream retention for `PidPackage`.
- Basic clusters/sheet stream discovery needed for coverage-style tooling.

`Light` may skip:

- Tagged text XML body parsing.
- `JSite` property decoding.
- Dynamic attribute semantic enrichment.
- Object inventory / object graph.
- Cross-reference graph.
- Layout derivation.

The exact skip list must be validated against real fixture timings before implementation.

## Task 1: Design Doc

**Files:**

- Create: `docs/light-parse-design.md`

**Step 1: Document the contract**

Write a design doc with:

- Problem statement.
- Full vs Light output matrix.
- Public API proposal.
- Compatibility notes.
- Measurement plan and initial baseline commands.

**Step 2: Keep implementation out**

Do not touch `src/api.rs` / `src/cfb/reader.rs` until the design doc is reviewed or explicitly approved.

## Task 2: Baseline Measurements

**Files:**

- Modify: `docs/light-parse-design.md`

**Step 1: Identify fixture commands**

Use existing CLI/tests to identify at least one small fixture and one real A01 fixture. Prefer commands that already parse package/doc without adding new tooling.

**Step 2: Capture reproducible command lines**

Record commands, not just results. If precise timing requires a new benchmark harness, defer it to implementation.

## Task 3: API Implementation Plan

**Files:**

- Future modify: `src/api.rs`
- Future modify: `src/cfb/reader.rs`
- Future tests: `src/api.rs` or dedicated integration tests

**Step 1: Add explicit profile only after design**

Potential implementation:

```rust
pub enum ParseProfile {
    Full,
    Light,
}

pub struct ParseOptions {
    pub profile: ParseProfile,
    // existing fields...
}
```

**Step 2: Preserve default behavior**

`ParseOptions::default()` must remain full fidelity.

**Step 3: Test skipped derived fields**

Focused tests should assert light mode skips the documented derived fields while preserving stream inventory and package raw streams.

## Task 4: Verification

Future implementation must run:

```bash
cargo test --lib api::tests
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

Expected:

- Full profile remains byte/behavior-compatible with current default.
- Light profile behavior is explicit, documented, and measured.

## Out Of Scope

- Do not silently make `parse_file` light by default.
- Do not remove existing fine-grained `ParseOptions` knobs.
- Do not claim performance wins without fixture measurements.
