# Goal Prompt: Phase 34 Full Sheet Geometry Decode

> Plannotator gate approved on 2026-06-23. Slice 34-A inventory is recorded in
> `docs/analysis/2026-06-26-phase34-geometry-completeness-inventory.md`.

Use this prompt when starting implementation after approval:

```text
/goal
Goal: Improve SmartPlant / Smart P&ID Sheet geometry completeness without
violating Probe / Decode confidence boundaries.

Repository: pid-parse

Must read:
- docs/plans/2026-06-23-phase34-full-sheet-geometry-decode-plan-cn.md
- docs/analysis/2026-06-26-phase34-geometry-completeness-inventory.md
- docs/analysis/2026-06-26-phase34-0020-igrectangle2d-readiness.md
- goals/phase34-full-sheet-geometry-decode/brief.md
- goals/phase34-full-sheet-geometry-decode/plan.md
- goals/phase34-full-sheet-geometry-decode/verification.md
- goals/phase34-full-sheet-geometry-decode/blockers.md
- AGENTS.md

Slice 34-A geometry completeness inventory is complete. Do not edit parser code
until the `0x0020` byte-layout note accepts a candidate slice.

If Slice 34-B starts, target only `0x0020 igRectangle2d`. Prove byte ranges,
add parser tests first, keep `0x0013`, `0x003D`, `0x0010`, and `0x00FA` out of
drawable geometry unless separate evidence is produced.

Forbidden:
- no `0x0030` arc revival;
- no `0x0010 sub_kind` naming;
- no geometry emission from audit-only records;
- no writer behavior change;
- no commit without explicit user authorization.

Report:
{"type":"phase34_progress","slice":"...","parser_change":true|false,"evidence":"...","next":"..."}
```

## Startup Checklist

- [x] Plannotator gate approved.
- [ ] Current git status reviewed.
- [ ] Existing probe outputs refreshed.
- [x] First action was inventory, not parser promotion.
- [ ] Commit/index changes are explicitly authorized before any git mutation.
