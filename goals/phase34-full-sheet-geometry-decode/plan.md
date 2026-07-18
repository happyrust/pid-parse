# Plan: Phase 34 Full Sheet Geometry Decode

> Plannotator gate approved on 2026-06-23. Slice 34-A inventory is recorded in
> `docs/analysis/2026-06-26-phase34-geometry-completeness-inventory.md`.
> Phase 34-F synchronized this plan with the implemented 34-D boundary decoder
> and the 34-E local curve-corpus discovery on 2026-07-10.

## 1. Approach

```text
local .pid fixtures
  -> geometry-summary + byte-audit + PSM type histogram
  -> remaining type classification
  -> one evidence-gated decoder slice
  -> model/schema/geometry/byte-audit integration
  -> full gates
```

The goal is complete geometry coverage for the current fixture set, not a
claim that every vendor geometry class is known.

## 2. Slices

| Slice | Purpose | Output | Status |
|---|---|---|---|
| 34-A | Geometry completeness inventory | Six-fixture decoded/inferred/probe/leftover inventory and candidate classification | Complete |
| 34-B | `0x0020 igRectangle2d` candidate | Ownership + relaxed neighbor evidence | Closed negative; no decoder |
| 34-C | `0x0013` / `0x003D` closeout | Boundary candidate and SmartFrame structural classification | Complete |
| 34-D | `0x0013 igBoundary2d` grammar and parser | Fully typed association DTO, byte-audit claim, schema/panic/fixture ratchets | Complete; no duplicate geometry emission |
| 34-E | Curve-family evidence expansion | All-stream corpus scan and representative `.sym` extraction plan | Complete read-only; local evidence found |
| 34-F | Contract/status update | Atlas, roadmap, task plan, goal verification/status agreement | Complete |

## 3. Implementation Outcome And Next Candidate

`0x0020 igRectangle2d` was the original first candidate, but Phase 34-B
correctly rejected implementation:

- records occur only in mini-sheet or nested JSite ownership domains;
- the two domains give contradictory meanings to the candidate f64 offsets;
- no stable two-extent rectangle layout is proven.

Phase 34-D instead accepted `0x0013 igBoundary2d` after proving its complete
grammar and association semantics. It remains non-emitting because its segments
duplicate referenced `igLine2d` geometry.

The next implementation candidate is `igCircle2d 0x0059`, after Phase 34-E1
extracts minimal `.sym` fixtures and Phase 34-E2 proves a bounded field layout.
Each curve family remains an independent decoder slice.

## 4. Evidence Rules

Any parser slice must provide:

- stream path and half-open byte ranges;
- record type and payload size;
- fixture ids and count distribution;
- expected decoded values;
- malformed/truncated rejection tests;
- byte-audit movement;
- schema/public DTO tests when public output changes;
- panic-safety entry.

## 5. Verification

Planning-only:

```powershell
plannotator annotate goals/phase34-full-sheet-geometry-decode --gate --json
cargo fmt --all -- --check
git diff --check
```

Implementation:

```powershell
cargo test --locked --lib parsers::sheet_records -- --nocapture
cargo test --locked --test parse_real_files rectangle -- --nocapture
cargo test --locked --test parser_panic_safety -- --nocapture
cargo test --locked --lib schema -- --nocapture
cargo test --locked --lib byte_audit -- --nocapture
cargo build --locked --workspace --all-targets
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo rustdoc --lib --locked -- -W missing-docs
```

## 6. Decision Points

| Point | Default |
|---|---|
| Plannotator gate unavailable | was a stop condition before 2026-06-23 approval |
| `0x0020` layout uncertain | do not implement decoder |
| `0x0013` / `0x003D` look geometric by eye | still require reader or controlled fixture evidence |
| Curve records found outside Sheet-only scan | use `.sym` as byte-layout fixtures; retain nested JSite ownership gate |
| Commit strategy | ask user explicitly |
