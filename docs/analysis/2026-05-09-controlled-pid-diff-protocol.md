# Controlled PID Diff Protocol

> Date: 2026-05-09
> Scope: Phase 14 evidence collection for decoded Sheet geometry records.

## Goal

Produce before/after `.pid` pairs that isolate one visible drawing operation.

The output of this protocol should let `pid-parse` compare CFB streams and
identify source-backed byte ranges for one geometry class without guessing from
mixed production drawings.

## Fixture Set

Create a separate pair for each operation:

- place one straight line;
- place one open polyline with three vertices;
- place one closed polyline with four vertices;
- place one circle;
- place one arc;
- place one text label with known value, height, and rotation;
- place one symbol with known catalog path, insertion, rotation, and scale.

Prefer a blank or nearly blank drawing template. If a blank drawing is not
available, use the smallest drawing where the operation can be visually isolated.

## Capture Steps

For each operation:

1. Copy the original `.pid` to `before/<case>.pid`.
2. Open the drawing in SmartPlant P&ID.
3. Perform exactly one visible operation.
4. Save and close the drawing.
5. Copy the resulting `.pid` to `after/<case>.pid`.
6. Record the expected semantic payload in `metadata/<case>.json`.

The metadata should include:

```json
{
  "case": "one-line",
  "operation": "place_line",
  "expected": {
    "start": [100.0, 100.0],
    "end": [200.0, 100.0],
    "units": "mm"
  },
  "notes": "Coordinates are operator-entered or measured from the SPPID UI."
}
```

Do not edit `.pid` bytes manually. Public SmartPlant notes indicate arbitrary
byte edits can make the drawing fail to open, so all deltas should come from the
application itself.

## Repository Intake

Do not commit proprietary `.pid` files unless the project owner explicitly
approves it. For local investigation, put them under `test-file/` using a clear
directory such as:

```text
test-file/controlled-diff/
  before/one-line.pid
  after/one-line.pid
  metadata/one-line.json
```

Once files exist locally, the next parser work should:

- compare stream inventories between `before` and `after`;
- list added, removed, and changed CFB streams;
- isolate byte ranges within changed Sheet streams;
- correlate changed ranges with the operation metadata;
- add an investigation regression before any typed decoder promotion.

The regression harness now looks for local cases under `test-file/controlled-diff`
and soft-skips when none are present:

```powershell
cargo test --locked -j 1 --test parse_real_files controlled_pid_diff_pairs_report_stream_level_evidence_when_available -- --nocapture
```

For every discovered `before/<case>.pid` + `after/<case>.pid` pair, the harness
requires `metadata/<case>.json`, parses the metadata as structured JSON, checks
that `case` matches the filename stem, requires a non-empty `operation`, requires
an `expected` payload, parses both packages, computes stream-level diffs, and
prints the first modified stream with mismatch context. It does not promote any
geometry by itself.

The same intake path is available from the inspection CLI:

```powershell
pid_inspect --controlled-diff-dir test-file/controlled-diff
pid_inspect --controlled-diff-dir test-file/controlled-diff --json
```

The CLI prints one summary per case plus the first modified stream and mismatch
context. With `--json`, it emits a structured report for scripting or CI. It
exits with an error when metadata is invalid, when a pair cannot be parsed, or
when a pair has no stream-level changes.

## Promotion Criteria

A controlled diff can support a decoder only when it proves:

- the changed byte range is bounded;
- the changed range is tied to the intended operation, not unrelated metadata;
- the same shape repeats across at least two independent drawings or operations;
- coordinates and style fields match the captured expected payload;
- generated `NormalizedPidGeometry` provenance points back to the changed Sheet
  stream and byte range.

Until those criteria pass, results remain investigation evidence only.
