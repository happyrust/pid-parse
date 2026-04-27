# Sheet Endpoint Diagnostics Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make `populate_sheet_endpoints` failures visible without turning partial or damaged Sheet streams into hard parse failures.

**Architecture:** Keep the parser's default success path tolerant. Add a small diagnostic field to `SheetStream` that records why endpoint-pair extraction could not run for that sheet. `populate_sheet_endpoints` should still fill `endpoint_records` when possible, but failed re-open/read paths should leave a structured message for reports, JSON, and downstream debugging.

**Tech Stack:** Rust 2021, `cfb`, existing `PidDocument` / `SheetStream`, current inspect report and cross-reference diagnostics.

---

## Context

This is Task 5 from `docs/plans/2026-04-26-parser-api-consistency-fixes.md`.

Current behavior in `src/cfb/reader.rs`:

```rust
for sheet in &mut doc.sheet_streams {
    if let Ok(mut s) = cfb.open_stream(&sheet.path) {
        let mut data = Vec::new();
        s.read_to_end(&mut data)?;
        sheet.endpoint_records = parse_endpoint_records(...);
    }
}
```

Problem:

- `open_stream` failures are silently skipped.
- `read_to_end` failures still hard-fail parse.
- Reports can show missing relationship endpoints, but not whether endpoint extraction was skipped because a Sheet stream could not be re-opened/read.

Target behavior:

- Default parse remains tolerant.
- Each affected `SheetStream` carries an endpoint extraction diagnostic.
- Reports can surface the diagnostic without guessing from empty `endpoint_records`.

## Task 1: Add Model Diagnostic Field

**Files:**

- Modify: `src/model.rs`

**Step 1: Add optional field to `SheetStream`**

Add:

```rust
#[serde(skip_serializing_if = "Option::is_none")]
pub endpoint_decode_error: Option<String>,
```

Place it immediately after `endpoint_records` so all endpoint-related data stays together.

**Step 2: Update test constructors**

Search for `SheetStream {` literals and add:

```rust
endpoint_decode_error: None,
```

Expected affected test modules include `crossref.rs`, `inspect/coverage.rs`, and `inspect/report.rs`.

## Task 2: Capture Re-open / Read Diagnostics

**Files:**

- Modify: `src/cfb/reader.rs`

**Step 1: Replace silent skip**

Change `if let Ok(mut s) = cfb.open_stream(&sheet.path)` to `match`:

```rust
match cfb.open_stream(&sheet.path) {
    Ok(mut s) => { ... }
    Err(e) => {
        sheet.endpoint_decode_error =
            Some(format!("failed to reopen sheet stream for endpoint records: {e}"));
    }
}
```

**Step 2: Tolerate read failures**

If `read_to_end` fails, store:

```rust
sheet.endpoint_decode_error =
    Some(format!("failed to read sheet stream for endpoint records: {e}"));
continue;
```

Do not return `Err` from `populate_sheet_endpoints` for these per-sheet failures.

## Task 3: Report Surface

**Files:**

- Modify: `src/inspect/report.rs`

**Step 1: Include diagnostics in Sheet provenance output**

When report output lists sheet endpoint coverage, append the error text for any sheet with `endpoint_decode_error`.

Expected style:

```text
/Sheet6 endpoint_records=0 ... endpoint_error="failed to reopen ..."
```

Keep existing output stable when the field is `None`.

## Task 4: Tests

**Files:**

- Modify/add tests around `src/cfb/reader.rs` or `src/inspect/report.rs`

**Step 1: Unit-test report rendering**

Add a report-level test that constructs a `SheetStream` with
`endpoint_decode_error: Some(...)` and asserts the rendered report contains the diagnostic.

**Step 2: Parser behavior test if feasible**

If a synthetic CFB can reliably trigger a per-sheet re-open/read failure without failing earlier collection, add a parser test. If not, keep parser code covered by report + model constructor tests and document why the failure is hard to synthesize with `cfb`.

## Task 5: Verification

Run:

```bash
cargo test --lib inspect::report::tests::report_shows_sheet_endpoint_decode_error
cargo test --lib crossref::tests
cargo clippy --locked --lib -- -D warnings
cargo fmt --all -- --check
```

Expected:

- Existing successful endpoint extraction behavior is unchanged.
- Sheets with extraction failures carry visible diagnostics.
- No new hard parse failures for per-sheet endpoint extraction problems.

## Out Of Scope

- Do not redesign cross-reference endpoint coverage.
- Do not convert all parser soft failures into a global diagnostics system.
- Do not change `SheetEndpointRecord` binary parsing heuristics.
