# Quickstart: Reproducing PID Format Evidence

## Read The Spec Package

Start here:

1. `spec.md`
2. `data-model.md`
3. `research.md`
4. `plan.md`
5. `tasks.md`

Then follow detailed source documents:

- `docs/analysis/2026-06-03-pid-file-format-analysis-cn.md`
- `docs/analysis/2026-06-03-phase27-pid-data-type-matrix-cn.md`
- `docs/plans/2026-06-03-phase27-ida-driven-pid-data-type-extraction-plan-cn.md`

## Parser Inspection Commands

Use a real `.pid` sample when available:

```bash
cargo run --bin pid_inspect -- drawing.pid
cargo run --bin pid_inspect -- drawing.pid --json
cargo run --bin pid_inspect -- drawing.pid --schema
cargo run --bin pid_inspect -- drawing.pid --coverage
cargo run --bin pid_inspect -- drawing.pid --byte-audit
cargo run --bin pid_inspect -- drawing.pid --geometry-summary
```

For machine-readable snapshots:

```bash
cargo run --bin pid_inspect -- drawing.pid --coverage --json > coverage.json
cargo run --bin pid_inspect -- drawing.pid --byte-audit --json > byte-audit.json
```

Do not publish new coverage numbers if no real `.pid` fixture is available.

Local fixture snapshots included in this package:

```text
docs/specs/2026-06-08-pid-file-format-spec-kit/*-coverage.json
docs/specs/2026-06-08-pid-file-format-spec-kit/*-byte-audit.json
```

They were generated with the same `pid_inspect` commands for these fixture ids:

| Fixture id | Source path |
|---|---|
| `d06` | `test-file/D06.pid` |
| `nonascii-process-1` | `test-file/工艺管道及仪表流程-1.pid` |
| `dwg0201` | `test-file/DWG-0201GP06-01.pid` |
| `dwg0202` | `test-file/DWG-0202GP06-01.pid` |
| `publish-a01` | `test-file/export-test/publish-data/A01/A01.pid` |
| `publish-dwg0202` | `test-file/export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid` |

Example generation command:

```bash
cargo run --bin pid_inspect -- test-file/D06.pid --coverage --json > docs/specs/2026-06-08-pid-file-format-spec-kit/d06-coverage.json
cargo run --bin pid_inspect -- test-file/D06.pid --byte-audit --json > docs/specs/2026-06-08-pid-file-format-spec-kit/d06-byte-audit.json
```

## Focused Test Commands

Sheet and real-file regression surface:

```bash
cargo test --test parse_real_files -- --nocapture
cargo test --test parse_real_files d06_pid_parses_with_expected_structure_and_geometry_summary -- --nocapture
cargo test --test parse_real_files d06_text_placement_regression_keeps_text_probes_unpromoted -- --nocapture
```

Parser-level checks:

```bash
cargo test --lib parsers::sheet_records -- --nocapture
cargo test --lib schema -- --nocapture
```

Full gates for parser changes:

```bash
cargo build --locked --workspace --all-targets
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo rustdoc --lib --locked -- -W missing-docs
```

## IDA Workflow

Use `ida-pro-mcp` to refresh evidence when tool descriptors and IDA instances
are available.

Minimum review loop:

1. List reachable IDA instances.
2. Select the target binary.
3. Survey the binary.
4. Search type names, GUIDs, or key strings.
5. Analyze candidate functions.
6. Record findings in `research.md` or a linked `docs/analysis/*` file.

Priority binaries:

1. `style.dll`
2. `J2DSrv.dll`
3. `sppid.dll`
4. `XCeedRAD.dll`
5. `smartplantpid.exe`
6. `radsrvitem.dll`

Priority strings / symbols:

```text
1D1928C0
JStyleBase
IJPersist
DoIO
Read
GraphicGroup
igLine2d
igPoint2d
igLineString2d
igSymbol2d
igCircle2d
igArc2d
igEllipse2d
igEllipticalArc2d
GLine2d
```

## Evidence Promotion Checklist

Before changing a parser status to `Decoded`, verify:

- [ ] IDA, controlled-diff, or multi-fixture evidence confirms field semantics.
- [ ] Existing parser layout matches fixture bytes.
- [ ] Unit tests cover canonical and rejection cases.
- [ ] Real fixture tests cover the type or soft-skip reason is documented.
- [ ] Schema changes are tested.
- [ ] Byte-audit effect is documented.
- [ ] Panic-safety entry points are covered for byte parsers.
- [ ] `data-model.md` and relevant Phase analysis docs are updated.

## Negative Closeout Checklist

Use this when evidence is insufficient:

- [ ] Record what was searched.
- [ ] Record which binary / fixture was unavailable.
- [ ] Record the exact reason no parser promotion is allowed.
- [ ] Keep current evidence level unchanged.
- [ ] Add a re-open trigger, such as a new fixture or newly opened IDB.
