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

## JSON Output

`--byte-audit --json` serializes `ByteAuditReport`.

Important fields:

- `traces`: full `ParserTrace` records for streams with registered parsers
- `per_stream`: deterministic map keyed by stream path
- `unregistered_paths`: sorted stream paths with no registered parser
- `overall_coverage_ratio`: package-level consumed byte ratio

Use JSON mode for CI and regression baselines.

## Baseline Rules

Once real `.pid` fixtures are available under `test-file/`, baseline checks
should compare the current JSON report to a committed reference.

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
