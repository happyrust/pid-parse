# Phase 33 PSM 0x0010 Discriminator Spec

> Date: 2026-06-22  
> Scope: development and test planning for the SmartPlant / Smart P&ID `.pid`
> Sheet PSM sub-record family `0x0010`.  
> Method: Spec Kit style requirements, plan, research, data model, tasks, and
> quickstart artifacts. This package is compatible with the repository's
> existing `docs/specs/` convention; the repository does not currently include a
> runnable `.specify/` script directory.

## Goal

Define an evidence-gated development and test plan for restarting `0x0010`
analysis without over-promoting audit-only parser output. The outcome should be
either a future parser implementation slice with bounded evidence, or a
documented negative closeout that keeps `0x0010` as `TypedAudit`.

## User Stories

### Parser Maintainer

As a parser maintainer, I need a clear gate before adding fields to
`SheetSubRecord0x0010Decoded` so that positional bytes are not renamed as
business semantics.

### IDA Analyst

As an IDA analyst, I need a prioritized target list and search plan so that
`ida-pro-mcp` work focuses on modules that can plausibly expose persisted
reader logic.

### QA Maintainer

As a QA maintainer, I need the parser, fixture, byte-audit, schema, and
panic-safety tests named before implementation so that any promotion is
repeatable and rollback-safe.

### Downstream Consumer

As a downstream JSON / CAD consumer, I need confidence classes to remain stable
so that `TypedAudit` records are not mistaken for decoded CAD geometry or
editable Sheet semantics.

## Functional Requirements

### FR-001: Preserve Current Confidence

The plan MUST keep `0x0010` at `TypedAudit` until all promotion gates are
satisfied. It MUST NOT rename `leading_word` to `sub_kind`, emit geometry from
`0x0010`, or add writer support.

### FR-002: Use IDA Evidence Only From Relevant Modules

The IDA plan MUST prioritize modules likely to contain persistence or RAD
dispatch logic: `radsrvitem.dll`, `J2DSrv.dll`, `style.dll`, `jengine.dll`,
`XceedRAD.dll`, `OLESITE.dll`, and `OLECRT.dll`.

The plan MUST treat the currently reachable `sppid.dll` and `core.dll` evidence
as insufficient for raw `.pid` parser promotion unless a future xref from a
relevant module leads there.

### FR-003: Require Bounded Byte Evidence

Any parser implementation proposal MUST name:

- source stream path;
- half-open byte ranges for every proposed field;
- record family and sub-family identity;
- fixture identities;
- cross-fixture distributions;
- byte-audit movement for decoded, audit/probe, and leftover ranges.

### FR-004: Require Tests Before Promotion

Any code slice that changes parser output MUST define:

- parser unit tests;
- real fixture ratchets or explicit soft-skip messages;
- schema/public DTO tests when public output changes;
- panic-safety coverage for new byte parser entry points;
- byte-audit assertions for consumed and leftover bytes;
- documentation updates in this Spec Kit package or linked analysis docs.

### FR-005: Support Negative Closeout

If IDA or controlled fixture evidence is insufficient, the work MUST produce a
negative closeout that records searched modules, unavailable modules, failed
anchors, and the exact reason confidence remains unchanged.

## Non-Goals

- This package does not implement a parser.
- This package does not claim the vendor `0x0010` format is decoded.
- This package does not add geometry output, writer support, or editable Sheet
  semantics.
- This package does not use MDF publish parity as raw `.pid` parser evidence.

## Acceptance Criteria

- `plan.md` names development phases and test gates.
- `research.md` records decisions, rationale, and alternatives.
- `data-model.md` defines the evidence records and test gate entities used by
  the plan.
- `tasks.md` contains independently checkable development and test tasks.
- `quickstart.md` gives runnable IDA and fixture-side validation commands.
- The plan explicitly explains why `.specify/` automation was not run in this
  repository state.
