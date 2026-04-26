# Byte Audit Guide

`pid_inspect --byte-audit` answers a lower-level question than the regular
coverage report:

- coverage report: "Which top-level streams are known / partial / unknown?"
- byte audit: "For each raw stream, which bytes did a registered parser claim?"

The feature is evidence-oriented. A high byte coverage ratio means parser
surface is expanding; it does not automatically mean the decoded fields are
semantically complete.

## CLI Usage

Human-readable report:

```bash
cargo run --bin pid_inspect -- drawing.pid --byte-audit
```

Machine-readable JSON:

```bash
cargo run --bin pid_inspect -- drawing.pid --byte-audit --json > audit.json
```

Compare the current report with a committed baseline:

```bash
cargo run --bin pid_inspect -- drawing.pid --byte-audit --byte-audit-baseline docs/baselines/drawing.byte-audit.json
```

`--byte-audit-baseline` prints a `ByteAuditComparison`-style text summary and
exits with code `3` when regressions are present. With `--json`, stdout is the
serialized `ByteAuditComparison`; the exit-code policy is the same.

## Text Output

The text report prints:

- `Total stream bytes`: sum of every raw stream size in the package
- `Overall consumed`: bytes covered by registered `_with_trace` parsers
- `Overall leftover`: bytes not claimed by a registered parser
- `Overall coverage`: `consumed / total`
- `Fully consumed traced streams`: traced streams with zero leftover bytes
- `Unregistered streams`: stream paths with no byte-audit parser yet
- per-stream rows: coverage percentage, stream path, consumed/total/leftover
  bytes, and parser name or `unregistered`

Example shape:

```text
--- Byte Audit ---
Total stream bytes: 196608
Overall consumed:   187234
Overall leftover:   9374
Overall coverage:   95.2%
Fully consumed traced streams: 7
Unregistered streams: 12
  [100.0%] /DocVersion3 (192 B consumed / 192 B total, 0 B leftover) parse_doc_version3
  [  0.0%] /MysteryStream (0 B consumed / 48 B total, 48 B leftover) unregistered
```

Baseline comparison shape:

```text
--- Byte Audit Baseline Comparison ---
Regressions: 1
  [overall_coverage_decreased] (overall) baseline=0.950000 current=0.920000
Improvements: 1
  [stream_became_traced] /MysteryStream baseline=unregistered current=parse_mystery
```

## JSON Output

`--byte-audit --json` serializes `ByteAuditReport`.

Important fields:

- `traces`: full `ParserTrace` records for streams with registered parsers
- `per_stream`: deterministic map keyed by stream path
- `unregistered_paths`: sorted stream paths with no registered parser
- `overall_coverage_ratio`: package-level consumed byte ratio

Use JSON mode for CI and regression baselines.

Currently registered stream families include the PSM tables, DocVersion
streams, `AppObject`, `JTaggedTxtStgList`, UTF-8 parseable
`TaggedTxtData/Drawing` / `TaggedTxtData/General` XML metadata streams, and
`*/JProperties` streams.

`*/JProperties` coverage is intentionally partial: only recovered ASCII /
UTF-16LE text runs are marked as `Probed`; opaque binary prefix, suffix, and
gaps remain leftover inventory.

Top-level `/Sheet*` streams are also registered through the experimental sheet
probe. Only non-overlapping printable text runs are marked as `Probed` today;
candidate chunks, record-type counts, and coordinate hints remain
reverse-engineering evidence and do not count as consumed geometry bytes yet.

## Programmatic API

The byte-audit framework is also exposed as a library-level surface, so
downstream consumers can produce / compare reports without going through the
`pid_inspect` binary.

Main entry points:

- `pid_parse::byte_audit::aggregate::byte_audit_report(&PidPackage) -> ByteAuditReport`
  — generate a full report for an in-memory `PidPackage`.
- `pid_parse::byte_audit::compare::compare_byte_audit_reports(baseline, current) -> ByteAuditComparison`
  — pure-data baseline diff; classifies coverage deltas as
  `regressions` (`OverallCoverageDecreased`, `StreamMissing`,
  `StreamConsumedBytesDecreased`, `StreamBecameUnregistered`) or
  `improvements` (`StreamBecameTraced`, `NewTracedStream`).

Re-exported types (`pid_parse::byte_audit::*`):

- `ByteAuditReport` — package-level summary with `traces`, `per_stream`,
  `unregistered_paths`, and overall byte counters.
- `StreamAuditSummary` — per-stream rollup (`path`, `total_bytes`,
  `consumed_bytes`, `leftover_bytes`, `coverage_ratio`, `parser_name`).
- `ByteAuditComparison` — `regressions` + `improvements` + `is_clean()`.
- `ParserTrace` / `ParserTraceBuilder` / `ByteRange` / `TraceConfidence`
  (`Decoded` / `Probed`) — low-level building blocks for new
  `_with_trace` parsers.

Both `ByteAuditReport` and `ByteAuditComparison` derive `Serialize` /
`Deserialize` / `JsonSchema`, so they round-trip through the same JSON shape
as `--byte-audit --json` and can be diffed in CI without re-running the
binary.

A complete zero-fixture demo lives in `examples/byte_audit_demo.rs`:

```bash
cargo run --example byte_audit_demo
```

It builds a 4-stream synthetic `PidPackage` (`/PSMsegmenttable`,
`/DocVersion2`, `/TaggedTxtData/Drawing`, `/MysteryStream`), prints a
per-stream + overall breakdown, serializes the report, and runs a baseline
diff that surfaces both `OverallCoverageDecreased` and
`StreamConsumedBytesDecreased` regressions. Use it as the entry-point sample
when integrating the framework into a downstream service or CI tool.

## Baseline Rules

Once real `.pid` fixtures are available under `test-file/`, baseline checks
should compare the current JSON report to a committed reference.

Optional runner:

```bash
bash .github/scripts/check-byte-audit-baselines.sh
```

The runner scans `docs/baselines/*.byte-audit.json` and derives matching
private fixture paths as `test-file/<name>.pid`. For example,
`docs/baselines/DWG-0201GP06-01.byte-audit.json` compares against
`test-file/DWG-0201GP06-01.pid`.

Public CI runs the same script. It exits successfully when no baselines exist,
or when a baseline's matching private fixture is absent, so the repository can
carry baseline tooling without committing plant data.

The library-level comparator is available as
`pid_parse::byte_audit::compare_byte_audit_reports(baseline, current)`. It
returns `ByteAuditComparison` with separate `regressions` and `improvements`
lists, so CI can fail only on regressions while still printing newly traced
streams for review.

Recommended rules:

1. `overall_coverage_ratio` must not decrease.
2. For every path that existed in the baseline, `consumed_bytes` must not
   decrease unless the baseline is intentionally regenerated.
3. Newly traced streams may move from `unregistered_paths` into `traces`; this
   is an improvement.
4. A stream moving from traced to unregistered is a regression.
5. `leftover_bytes` may stay non-zero for probe-level parsers. Treat it as
   work inventory, not automatically as failure.

## Current Limitations

This checkout may not contain private real `.pid` fixtures under `test-file/`.
When fixtures are absent, real-file tests soft-skip and no meaningful byte
baseline can be produced.

The safe next step after restoring fixtures is:

```bash
cargo run --bin pid_inspect -- test-file/DWG-0201GP06-01.pid --byte-audit --json > docs/baselines/DWG-0201GP06-01.byte-audit.json
cargo run --bin pid_inspect -- test-file/DWG-0201GP06-01.pid --probe-sheet-chunks Sheet6 --json > docs/baselines/DWG-0201GP06-01.Sheet6.probe.json
```

Only commit baselines after reviewing that they do not contain sensitive plant
data that should stay out of the repository.
