# Phase 35 mainline: P0 worktree closeout, then curve-family decoders

Status: Accepted (grill-with-docs Q1, 2026-07-16)

## Context

Phase 34-A..F is scoped closed. Phase 35-A already admitted a stable
`0x0059 igCircle2d` layout (`layout-admitted; semantic proof required`).
The working tree still carries a large Phase 33/34 backlog (tracked edits
plus untracked docs/probes/`src/config.rs`/`src/stream_paths.rs`). ROADMAP
also still lists `0x0010` Mode B as an IDA-gated side path whose expected
outcome is container/audit rather than new drawable geometry.

## Decision

1. **P0 first**: package and commit the Phase 33/34 backlog into reviewable
   units until the worktree is clean enough for a single-slice PR boundary.
2. **P0 packaging (grill Q2)**: split into **3–4 review-unit commits**, not
   one mega-commit and not “code-only / docs later”:
   - commit 1 — parser core (`config` / `stream_paths` + sheet geometry
     pipeline + related tests);
   - commit 2 — Phase 34 analysis / goal package docs;
   - commit 3 — Phase 33 `0x0010` IDA evidence docs;
   - commit 4 — planning / misc (roadmap, CHANGELOG, findings, ADRs).
3. **Mainline after P0**: curve-family decoders, starting with
   `igCircle2d` (`0x0059`) through the Phase 35-B semantic gate.
4. **35-B proof path (grill Q3)**: prefer **IDA native `igCircle2d`
   reader** evidence for naming `+18/+26/+34` and the terminal byte;
   fall back to controlled fixture diff only if the reader cannot be
   located. Until proof lands: no `SheetIgCircle2dDecoded`, no atlas
   confidence bump, no nested-JSite→top-level geometry projection.
5. **Projection boundary (grill Q4)**: after semantic proof, emit
   document-level `PidGraphicEntity` circles only from **top-level /
   registered symbol `Sheet*`** paths. Nested `/JSite*` and StyleCluster
   hits remain typed audit / corpus evidence until a separate ownership
   gate passes.
6. **Curve-family order (grill Q5)**: strict serial, one evidence gate and
   one PR per family:
   `igCircle2d (0x0059)` → `igArc2d (0x0061)` → `igEllipticalArc2d (0x007E)`
   → `igEllipse2d (0x0063)` → `igBSplineCurve2d (0x005D)`. Do not start the
   next family until the previous family's semantic gate passes.
7. **Side track**: `0x0010` Mode B remains secondary; do not let it own the
   Phase 35 title unless the curve path stalls on evidence.

## Consequences

- No new Phase 35 decoder/model/schema work lands on top of the dirty
  backlog until P0 commits land (or the user explicitly re-authorizes
  working on a dirty tree).
- `src/config.rs` and `src/stream_paths.rs` must travel with any commit that
  includes the current `lib.rs` `mod config` / `pub mod stream_paths`
  declarations; HEAD alone does not contain those modules.
- Nested JSite ownership projection and atlas confidence remain unchanged
  until a later grill decision / evidence gate.
