# Phase 29-C2 Nested Sheet Ownership / Registration Review

> Date: 2026-06-08  
> Scope: classify nested `JSite*/Sheet*` streams that appear in byte-audit
> leftovers after Sheet trace integration.

## Key Finding

Nested `JSite*/Sheet*` streams should **not** be registered as ordinary
top-level Sheet geometry streams yet.

`publish-a01` shows the clearest example: `/JSite204` contains a whole nested
PSM-style stream set, not just one orphan Sheet stream.

Observed `/JSite204` streams include:

| Path | Total bytes | Parser |
|---|---:|---|
| `/JSite204/\x05DocumentSummaryInformation` | 128 | unregistered |
| `/JSite204/\x05SummaryInformation` | 280 | unregistered |
| `/JSite204/AppObject` | 4 | unregistered |
| `/JSite204/DocVersion2` | 75 | unregistered |
| `/JSite204/DocVersion3` | 288 | unregistered |
| `/JSite204/Dynamic Attributes Metadata` | 28 | unregistered |
| `/JSite204/JSitesList` | 24 | unregistered |
| `/JSite204/PSMcluster0` | 3,523 | unregistered |
| `/JSite204/PSMclustertable` | 400 | unregistered |
| `/JSite204/PSMroots` | 172 | unregistered |
| `/JSite204/PSMsegmenttable` | 12 | unregistered |
| `/JSite204/PSMspacemap/0x00000000` | 2,114 | unregistered |
| `/JSite204/Sheet6` | 6,870 | unregistered |
| `/JSite204/StyleCluster` | 5,301 | unregistered |
| `/JSite204/Unclustered Dynamic Attributes` | 395 | unregistered |

This is structurally closer to an embedded symbol-local PSM package than to a
top-level drawing Sheet.

## Cross-Fixture Pattern

Nested unregistered JSite stream groups are not limited to `JSite204`.

Top groups by unregistered child-stream count:

| JSite | Unregistered child paths |
|---|---:|
| `JSite793` | 28 |
| `JSite204` | 23 |
| `JSite145` | 13 |
| `JSite151` | 13 |
| `JSite6963` | 13 |
| `JSite7559` | 13 |
| `JSite329` | 13 |
| `JSite396` | 13 |
| `JSite121` | 13 |
| `JSite39` | 13 |

This suggests a recurring nested storage pattern.

## Decision

Do **not** register nested `JSite*/Sheet*` streams through the same
`top_level_sheet_name()` path used for `/Sheet6`.

Instead, model this as a separate ownership question:

1. What does a nested JSite PSM package represent?
2. Does it encode symbol definitions, reusable drawing fragments, or local
   symbol geometry?
3. Which downstream consumer needs it?
4. Should byte-audit register nested child streams as:
   - nested package inventory only,
   - cluster-header-only traces,
   - full recursive PSM parser traces,
   - or remain unregistered until ownership is understood?

## Recommended Next Slice

Phase 29-D: JSite nested package inventory.

### Goal

Produce an inventory of nested `JSite*` packages across fixtures:

- parent `JSite` id,
- child stream names,
- child stream sizes,
- whether the child path mirrors a top-level PID stream family,
- symbol path / GUID evidence from `JProperties`,
- candidate ownership role.

### Non-Goals

- Do not parse nested `Sheet*` as top-level drawing geometry.
- Do not emit normalized geometry from nested Sheet streams.
- Do not merge nested PSM roots / clusters into the main object graph.

### Acceptance Criteria

- One markdown inventory table exists for all nested JSite packages in the 6
  local fixtures.
- Each nested package is classified as `NeedsOwnership`, `CanTraceHeaderOnly`,
  or `IgnoreUntilConsumerNeeds`.
- No parser semantics are changed.

## Guardrail

Nested `JSite*/Sheet*` streams are likely important, but they belong to symbol
or embedded-fragment ownership. Treating them as normal page geometry would mix
definition geometry with drawing-instance geometry.
