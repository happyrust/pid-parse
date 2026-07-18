# Verification: Phase 34 Full Sheet Geometry Decode

> Plannotator gate approved on 2026-06-23. Status synchronized through
> Phase 34-F on 2026-07-10. “Complete” here means the current six-PID fixture
> scope has a quantified candidate classification and every accepted parser
> promotion passed its evidence gates; it does not mean every vendor geometry
> class, page transform, ownership projection, or semantic writer is complete.

## Planning Verification

| Check | Command / Method | Expected |
|---|---|---|
| Goal package exists | inspect `goals/phase34-full-sheet-geometry-decode/` | brief / plan / verification / blockers / goal-prompt + progress log |
| Main plan exists | inspect `docs/plans/2026-06-23-phase34-full-sheet-geometry-decode-plan-cn.md` | full route and gates |
| Plannotator gate | `plannotator annotate goals/phase34-full-sheet-geometry-decode --gate --json` | approved (`2026-06-23`) |
| Contract sync | inspect atlas, roadmap, task plan and this file | Phase 34-D/E facts and scoped completion language agree |
| Formatting | `cargo fmt --all -- --check` | pass |
| Whitespace | `git diff --check` | pass |

## Evidence Refresh Commands

```powershell
cargo run --quiet --example probe_psm_type_code_histogram
cargo run --quiet --example probe_psm_undecoded_shapes
```

For current fixtures, expected undecoded candidates are:

- `0x0013`: 20 hits / 3 fixture paths — now accepted by
  `decode_igboundaries` as a typed association;
- `0x003D`: 12 hits / 6 fixtures — structural `igSmartFrame2d`, no drawable
  decoder;
- `0x0020`: 4 hits / 3 fixture paths — ownership-gated negative, no decoder.

Phase 34-E broadens evidence discovery beyond Sheet-named streams:

```powershell
cargo run --quiet --example probe_curve_family_corpus_scan -- test-file
```

Registered corpus expectation:

- `igCircle2d`: 79
- `igArc2d`: 29
- `igEllipse2d`: 0
- `igEllipticalArc2d`: 4
- `igBSplineCurve2d`: 2

The backup `.sym` corpus adds all five families (`616/279/44/50/55`), but these
are evidence sources, not production decoder counts.

## Implementation Verification

Phase 34-D `0x0013 igBoundary2d` focused gates:

```powershell
cargo test --locked --lib igboundary -- --nocapture
cargo test --locked --test parse_real_files igboundaries_decoder_emits_typed_audit_records_with_provenance -- --nocapture
cargo test --locked --test parser_panic_safety -- --nocapture
cargo test --locked --lib schema -- --nocapture
cargo test --locked --lib byte_audit -- --nocapture
```

Required ratchets:

- exact per-fixture boundary counts `0/5/10/0/0/5`;
- 20/20 records close within `1e-9`;
- 60/60 member OIDs resolve to same-stream canonical `igLine2d`;
- 60/60 member geometries equal the same-index boundary segment;
- accepted records are byte-audit `Decoded`;
- `decoded_igboundaries` is present in the public schema;
- no normalized boundary entity is emitted.

Full gate:

```powershell
cargo build --locked --workspace --all-targets
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo rustdoc --lib --locked -- -W missing-docs
```

Recorded results:

- 2026-07-07: all five pre-commit gates passed after Phase 34-D
  (`944` lib tests and `104` `parse_real_files` tests recorded).
- 2026-07-10: `cargo test --locked --workspace --all-targets` rerun passed with
  exit code `0`; IDE diagnostics for the changed Rust/test files are empty.
- Phase 34-F is documentation/status synchronization only. Re-run formatting
  and whitespace checks after the sync; it does not change parser behavior.

## Acceptance Criteria

- [x] Six-fixture geometry scope is consistent across inventory, probes and
  real-file tests.
- [x] `0x0020` was rejected rather than promoted when layout and ownership
  evidence disagreed.
- [x] `0x0013` was promoted only after every field range, count formula,
  member reference and cross-fixture ratchet was proven.
- [x] `igBoundary2d` is typed and byte-audit `Decoded`, but emits no duplicate
  normalized geometry.
- [x] `0x0010` and `0x00FA` emit no geometry.
- [x] `0x0030` remains `JStyleOverride`; no arc regression.
- [x] `0x003D` page-frame scalars do not make
  `PidPageTransform::Available`.
- [x] Phase 34-E local curve evidence is classified
  `IdentifiedOnly / NeedsParser`; no speculative decoder or nested ownership
  promotion was added.
- [x] Byte-audit, schema, panic-safety and geometry-emission boundaries are
  documented.
