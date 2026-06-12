# Phase 29 Candidate Implementation Slices

> Derived from `snapshot-priority-backlog.md`. These are candidate slices, not
> approved implementation tasks. Each slice must still pass the evidence gates
> in `spec.md` before changing parser semantics.

## Selection Rule

Prioritize by value, not leftover bytes alone:

1. Prefer `Sheet*` when the goal is geometry extraction or H7CAD quality.
2. Prefer Dynamic Attributes when the goal is object / relationship semantics.
3. Prefer PSM tables when the goal is canonical graph authority.
4. Treat `JSite*` large leftover volume as demand-gated: implement only if a
   downstream consumer needs deeper symbol-instance semantics.
5. Treat `PSMspacemap`, `0x0010`, and `GraphicGroup` as IDA / controlled-evidence
   gated.

## Slice 29-A: Sheet Stream Delta And Unknown Record Prioritization

Status: candidate, recommended first non-IDA slice.

### Goal

Turn the `Sheet*` leftover hotspot into a ranked list of concrete unknown record
families, offsets, and fixture examples without promoting any new semantics.

### Evidence

`snapshot-priority-backlog.md` shows:

- `Sheet*` total bytes: 130,138
- consumed bytes: 8,962
- leftover bytes: 121,176
- coverage ratio: 0.06886536
- `/Sheet6` is the top individual Sheet path.

### Tasks

- Generate a per-fixture Sheet leftover inventory from existing byte-audit JSON.
- Group Sheet leftovers by nearby PSM type code / marker / size bucket when
  safely extractable.
- Cross-link each group to current parser status:
  - typed decoder,
  - audit-only decoder,
  - probe-only evidence,
  - unregistered / unknown.
- Produce a markdown table of top unknown Sheet record families.

### Acceptance Criteria

- No parser semantics change.
- Output identifies the top 10 Sheet unknown groups with fixture, path, byte
  range, and reason.
- The report explicitly says whether each group is `NeedsIDA`, `NeedsParser`, or
  `Blocked`.

### Suggested Output

`docs/analysis/2026-06-08-phase29-sheet-leftover-priority.md`

## Slice 29-B: PSMcluster0 Body Triage

Status: candidate, parser-research slice.

### Goal

Investigate why `/PSMcluster0` has high leftover volume despite known cluster
header / string table evidence.

### Evidence

`snapshot-priority-backlog.md` shows:

- `PSMcluster0` total bytes: 193,983
- consumed bytes: 810
- leftover bytes: 193,173
- coverage ratio: 0.00417562

### Tasks

- Compare `/PSMcluster0` byte-audit leftovers across the 6 fixtures.
- Identify repeated headers, string-table regions, and candidate record tables.
- Reconcile with existing `src/streams/cluster.rs` and `src/parsers/cluster.rs`
  behavior.
- Decide whether the next step is parser-only or IDA-backed.

### Acceptance Criteria

- No `Decoded` promotion without field semantics.
- Report includes at least one of:
  - parser backlog item with exact test target,
  - negative closeout,
  - IDA target request.

### Suggested Output

`docs/analysis/2026-06-08-phase29-psmcluster0-leftover-triage.md`

## Slice 29-C: Dynamic Attributes Deep Body Backlog

Status: candidate, medium value.

### Goal

Use the `Unclustered Dynamic Attributes` leftover hotspot to identify which DA
body fields would most improve object / relationship semantics.

### Evidence

`snapshot-priority-backlog.md` shows:

- total bytes: 143,061
- consumed bytes: 31,941
- leftover bytes: 111,120
- coverage ratio: 0.22326840

### Tasks

- Compare DA leftovers with known object graph / relationship fallback paths.
- Identify repeated body layouts not yet surfaced in model DTOs.
- Separate product-useful fields from low-value padding or opaque payload.

### Acceptance Criteria

- Any parser task must name the target object / relationship benefit.
- No field name is introduced from byte position alone.

### Suggested Output

`docs/analysis/2026-06-08-phase29-dynamic-attributes-body-backlog.md`

## Slice 29-D: PSMspacemap `tseg` Evidence Gate

Status: blocked until stronger evidence.

### Goal

Prepare, but do not implement, a parser for `PSMspacemap` segment pages.

### Evidence

`snapshot-priority-backlog.md` shows:

- total bytes: 62,802
- consumed bytes: 0
- leftover bytes: 62,802
- coverage ratio: 0
- common unregistered paths:
  - `/PSMspacemap/0x00000000`
  - `/PSMspacemap/0x00002000`
  - `/PSMspacemap/0x00004000`
  - `/PSMspacemap/0x00006000`

### Blocker

The format is identified, but record semantics are not proven. Implementing a
parser from page shape alone risks false confidence.

### Re-open Trigger

Proceed only when one of these appears:

- IDA confirms the `tseg` reader / writer layout.
- Controlled fixture diff proves page entry semantics.
- Multiple fixtures expose a stable layout tied to known PSM cluster / segment
  table records.

## Slice 29-E: JSite Symbol-Instance Demand Gate

Status: demand-gated.

### Goal

Avoid spending parser effort on the largest leftover family until a downstream
consumer names the symbol-instance data it needs.

### Evidence

`snapshot-priority-backlog.md` shows:

- `JSite*` total bytes: 341,562
- consumed bytes: 15,159
- leftover bytes: 326,403
- coverage ratio: 0.04438140
- 201 distinct paths.

### Decision

Large leftover volume alone is not enough. Start this slice only if H7CAD,
publish XML, or another consumer asks for a specific symbol-instance field,
reference, or transform.

## Slice 29-F: IDA Module Enablement

Status: blocked by environment.

### Goal

Unblock live IDA evidence refresh for ordinary geometry readers and
`0x0010` / JStyle deep semantics.

### Required Inputs

Open or provide IDB/module access for at least one of:

- `style.dll`
- `J2DSrv.dll`
- `sppid.dll`
- `XCeedRAD.dll`
- `smartplantpid.exe`

Also ensure `user-ida-pro-mcp` exposes readable tool descriptors, or document
the approved fallback workflow for calling its tools.

### Acceptance Criteria

- Reachable IDA instance listed.
- Target binary selected.
- Type / GUID / vtable searches recorded.
- Findings written back to `research.md` or a dated `docs/analysis/*` file.

## Recommended Next Slice

If IDA remains blocked, start with **Slice 29-A**. It improves the Sheet
unknown-record backlog without changing parser semantics and will make later IDA
work more targeted.

If IDA becomes available, start with **Slice 29-F**, then route into the specific
Sheet / JStyle / PSM target it unlocks.
