# Tasks: Phase 33 PSM 0x0010 Discriminator

## Phase 33-A: Spec Kit Package

- [x] Create the Spec Kit style package directory.
- [x] Write `spec.md` with goals, requirements, non-goals, and acceptance
  criteria.
- [x] Write `plan.md` with phases and test strategy.
- [x] Write `research.md` with decisions and alternatives.
- [x] Write `data-model.md` with evidence and gate entities.
- [x] Write `quickstart.md` with runnable commands.
- [x] Validate markdown/lints for the new package.

## Phase 33-B: IDA Availability Check

- [x] List reachable IDA instances.
- [x] Record whether `radsrvitem.dll` is reachable.
- [x] Record whether `J2DSrv.dll` is reachable.
- [x] Record whether `style.dll` is reachable.
- [x] Record whether `jengine.dll` or `XceedRAD.dll` is reachable.
- [x] Record whether `OLESITE.dll` or `OLECRT.dll` is reachable.
- [x] If only `sppid.dll` / `core.dll` are reachable, write a tooling-gated
  closeout and stop before parser work.

Result: 2026-06-23 `list_instances` returned only reachable `sppid.dll`
(`127.0.0.1:13337`) and `core.dll` (`127.0.0.1:13338`). Closeout:
`docs/analysis/2026-06-23-phase33-0010-discriminator-ida-evidence.md`.

## Phase 33-C: IDA Evidence Collection

- [ ] Search the selected module for GUID
  `1D1928C0-0000-0000-C000-000000000046`.
- [ ] Search for PSM type `0x0010` and parent alias `0x0115`.
- [ ] Search for `PSMSerializeIn`, `PSMSerializeOut`, `IOContext`, `DoIO`,
  `Read`, `Load`, `Write`, and `Save`.
- [ ] Search for nearby family anchors: `JStyleOverride`, `GraphicGroup`,
  `RAD_OBJECT_TYPE`.
- [ ] Analyze candidate functions and classify them as persisted readers,
  dispatchers, runtime-only helpers, type identity, or negative hits.
- [ ] Record candidate addresses, callers, callees, and confidence impact in a
  focused analysis note.

## Phase 33-D: Fixture Bucket Reconciliation

- [ ] Refresh or reuse `0x0010` record count and size bucket distributions.
- [ ] Refresh or reuse `leading_word` distribution.
- [ ] Collect representative stream paths and byte ranges.
- [ ] Compare any IDA reader sequence against fixture buckets.
- [ ] Decide whether the evidence supports a named sub-family.

## Phase 33-E: Parser Implementation Gate

Only start this phase if Phase 33-D accepts a candidate layout.

- [ ] Add red unit tests for the proven sub-family.
- [ ] Implement the narrow decoder change.
- [ ] Keep unrelated `0x0010` payloads audit-only.
- [ ] Add or update model DTOs only for proven fields.
- [ ] Update schema tests if public output changes.
- [ ] Update byte-audit traces and assert decoded/audit/probe/leftover movement.
- [ ] Add panic-safety cases.
- [ ] Update atlas, roadmap, and analysis docs with confidence impact.

## Phase 33-F: Negative Closeout Gate

Use this phase if evidence is insufficient.

- [x] Record searched modules and unavailable modules.
- [x] Record failed anchors and candidate functions that were runtime-only.
- [x] State why `0x0010` remains `TypedAudit`.
- [x] Name the re-open trigger.
- [x] Confirm no parser, schema, or writer code was changed.

Result: negative closeout only. Preferred IDA targets were unavailable, anchors
were not re-searched in out-of-scope `sppid.dll` / `core.dll`, and no parser,
schema, writer, or bundle confidence change was made.

## Phase 33-G: Verification

Doc-only package:

- [x] `cargo fmt --all -- --check`
- [x] Read lints for new Spec Kit files.

If parser code changes:

- [ ] `cargo test --locked --lib parsers::sheet_records::tests::sub_record_0x0010 -- --nocapture`
- [ ] `cargo test --locked --test parse_real_files sub_records_0x0010 -- --nocapture`
- [ ] `cargo test --locked --test parser_panic_safety -- --nocapture`
- [ ] `cargo test --locked --lib schema -- --nocapture`
- [ ] `cargo build --locked --workspace --all-targets`
- [ ] `cargo test --locked --workspace --all-targets`
- [ ] `cargo clippy --locked --workspace --all-targets -- -D warnings`
- [ ] `cargo rustdoc --lib --locked -- -W missing-docs`
