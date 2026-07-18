# Quickstart: Phase 33 PSM 0x0010 Evidence Plan

## Read The Package

Start with:

1. `spec.md`
2. `plan.md`
3. `research.md`
4. `data-model.md`
5. `tasks.md`

Linked context:

- `docs/analysis/2026-06-22-phase33-0010-discriminator-ida-plan.md`
- `docs/analysis/2026-06-19-authoritative-pid-format-atlas.md`
- `docs/plans/2026-06-19-pid-parser-roadmap-gates.md`
- `docs/analysis/2026-06-19-ida-evidence-baseline.md`

## IDA Evidence Loop

Use `ida-pro-mcp` after checking each tool schema. Minimum loop:

```text
list_instances
select_instance(<target-port>)
survey_binary(detail_level = "minimal")
find_regex("1D1928C0|0010|0115|PSMSerializeIn|PSMSerializeOut|IOContext|DoIO|JStyleOverride|GraphicGroup|RAD_OBJECT_TYPE")
entity_query(functions/imports/strings around hits)
analyze_function(candidate_addr)
xrefs_to(candidate strings, GUID references, or dispatch entries)
```

Preferred target order:

1. `radsrvitem.dll`
2. `J2DSrv.dll`
3. `style.dll`
4. `jengine.dll` / `XceedRAD.dll`
5. `OLESITE.dll` / `OLECRT.dll`

Do not continue broad search in `sppid.dll` or `core.dll` unless a relevant
xref chain leads there.

## Fixture-Side Commands

Refresh `0x0010` distribution evidence:

```powershell
cargo run --release --example probe_psm_0x0010_sub_kind
cargo test --locked --test parse_real_files sub_records_0x0010 -- --nocapture
```

Keep nearby family evidence available:

```powershell
cargo test --locked --test parse_real_files jstyle_override_decoder_emits_audit_records_with_provenance -- --nocapture
cargo test --locked --lib parsers::sheet_records::tests::sub_record_0x0010 -- --nocapture
```

Inspect a compact fixture when present:

```powershell
cargo run --bin pid_inspect -- test-file/D06.pid --byte-audit --json
cargo run --bin pid_inspect -- test-file/D06.pid --geometry-summary
```

If a private fixture is missing, record the soft-skip message and do not invent
coverage numbers.

## Parser Change Gate

Before implementation, confirm all are true:

- [ ] Reader or controlled fixture evidence proves persisted byte fields.
- [ ] Every proposed field has a half-open byte range.
- [ ] At least two fixture families agree, or the single-fixture limitation is
  explicitly rejected as insufficient.
- [ ] Byte-audit decoded/audit/probe/leftover movement is known.
- [ ] A rollback condition is written.

Then run focused tests:

```powershell
cargo test --locked --lib parsers::sheet_records::tests::sub_record_0x0010 -- --nocapture
cargo test --locked --test parse_real_files sub_records_0x0010 -- --nocapture
cargo test --locked --test parser_panic_safety -- --nocapture
cargo test --locked --lib schema -- --nocapture
```

Run the full workspace gate before merging parser code:

```powershell
cargo build --locked --workspace --all-targets
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo rustdoc --lib --locked -- -W missing-docs
```

## Negative Closeout Template

Use this if evidence remains insufficient:

```text
Target modules checked:
- ...

Anchors searched:
- ...

Candidate functions:
- ...

Reason for no promotion:
- ...

Confidence impact:
- ROADMAP-0010 remains TypedAudit.

Re-open trigger:
- ...
```

Negative closeout should update an analysis note only. It should not change
parser code, schema, writer behavior, or bundle confidence.
