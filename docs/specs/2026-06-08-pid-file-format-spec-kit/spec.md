# PID File Format Spec Kit

> Date: 2026-06-08  
> Scope: SmartPlant / Smart P&ID `.pid` files as understood by `pid-parse`.  
> Method: Spec Kit style requirements first, then evidence, implementation plan,
> and verification tasks. This is an evidence-graded current-state
> specification, not an official vendor format specification.

## Goal

Build a maintainable specification package that enumerates every currently
known `.pid` file format layer, records the evidence level for each layer, and
defines the IDA-backed work needed to move remaining `Probe` / `AuditOnly`
families toward `Decoded`.

The package is intentionally linked to the existing Phase 26 and Phase 27
documents instead of replacing them:

- `docs/analysis/2026-06-03-pid-file-format-analysis-cn.md`
- `docs/analysis/2026-06-03-phase27-pid-data-type-matrix-cn.md`
- `docs/plans/2026-06-03-phase27-ida-driven-pid-data-type-extraction-plan-cn.md`

## User Stories

### Format Owner

As a parser maintainer, I need one evidence-graded inventory of all known `.pid`
format families so that I can decide whether a new decoder is safe to promote
from probe evidence to typed model output.

### Reverse Engineer

As an IDA analyst, I need each unknown or partial format entry to point to the
next binary evidence target so that `ida-pro-mcp` work is driven by gaps rather
than ad hoc string search.

### Downstream Consumer

As an H7CAD / JSON consumer, I need stable statements about which fields are
decoded, inferred, probe-only, or audit-only so that I do not treat
investigation evidence as final CAD geometry.

### QA / CI Maintainer

As a test maintainer, I need acceptance criteria and fixture gates for every
format promotion so that byte-audit, schema, and real-file tests catch format
drift.

## Evidence Levels

| Level | Meaning |
|---|---|
| `Decoded` | Byte layout and semantics are stable enough for typed model output. |
| `PartiallyDecoded` | Key structure is parsed, but some fields or semantics remain unnamed. |
| `IdentifiedOnly` | Stream / storage / type is identified, but structured parsing is limited. |
| `Probed` | Heuristic evidence useful for reverse engineering, not a stable contract. |
| `AuditOnly` | Structure is collected and regression-tested, but semantic naming is intentionally withheld. |
| `Inferred` | Derived geometry or relationship evidence from bounded source records; useful but not vendor-confirmed semantics. |
| `Leftover` | Bytes not declared consumed by parser / byte-audit. |

## Functional Requirements

### FR-001: Layered Format Inventory

The specification MUST cover the `.pid` format in these layers:

1. OLE / CFBF container.
2. Standard property sets and XML metadata.
3. Top-level PSM / cluster / registry streams.
4. JSite and symbol-related storages.
5. Dynamic Attributes and relationship evidence.
6. Sheet stream record families.
7. Normalized geometry and coordinate context.
8. Writer / package round-trip boundaries.
9. Backup / MDF publish XML boundary, as an adjunct and not part of the `.pid`
   binary format.

### FR-002: Evidence-First Field Claims

Every format entry MUST declare its evidence level. A field MUST NOT be called
`Decoded` unless at least one of these is true:

- The implementation has a typed parser, unit tests, schema exposure, and
  real-file regression coverage.
- IDA / controlled-diff evidence confirms both byte layout and semantics, and
  fixture bytes match the IDA layout.

### FR-003: IDA-Backed Gap Mapping

Every partial or unknown Sheet/PSM record family SHOULD list the next
`ida-pro-mcp` target:

- binary / IDB to open,
- type code or GUID,
- candidate function / vtable / string,
- expected evidence to confirm or reject.

### FR-004: Guardrails For Risky Promotions

The specification MUST preserve the following guardrails:

- `0x0010.leading_word` MUST NOT be renamed to `sub_kind` until IDA confirms the
  discriminator and enum.
- `0x0030` MUST remain `JStyleOverride`, not arc geometry.
- `0x00FA GraphicGroup` MUST remain audit-only until child/reference payload
  semantics are confirmed.
- `PidPageTransform::Available` MUST NOT appear without source coordinate
  space, units, transform direction, and bounded byte provenance.
- Text probes MUST NOT become inferred text geometry while binary-like text
  evidence remains unresolved.

### FR-005: Verification Hooks

Each future format promotion MUST define:

- parser unit tests,
- real fixture regression tests or a documented soft-skip reason,
- schema test when public DTOs change,
- byte-audit coverage effect,
- panic-safety coverage for byte-level parser entry points,
- documentation update in this spec kit or linked Phase analysis.

## Non-Goals

- This package does not claim to be the vendor's official `.pid`
  specification.
- This package does not implement new parser code by itself.
- This package does not reclassify audit-only or probe-only fields without new
  evidence.
- This package does not fold the MDF publish XML format into the `.pid` binary
  format. Publish XML remains a separate pipeline with an appendix-level link.

## Acceptance Criteria

- `data-model.md` contains all currently known top-level stream/storage
  families and Sheet/PSM type-code families.
- `research.md` summarizes the IDA-backed facts from Phase 27 and the latest
  blockers.
- `plan.md` defines the work sequence for IDA-backed completion.
- `tasks.md` has independently checkable tasks.
- `quickstart.md` explains how to reproduce parser / coverage / IDA review
  steps.
- Root planning files record this package as Phase 28 so the work can be
  resumed after context loss.
