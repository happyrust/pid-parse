# Phase 19 RAD sibling-sweep hypothesis — falsified (2026-05-17)

## TL;DR

The deferred Phase 19 working hypothesis from Phase 18 — **"the contiguous
RAD CLSID range `47FCC330..47FCC33E` maps 1:1 to PSM type codes
`0x29..0x35` and may contain other annotation-like records"** — is
**falsified** by an empirical probe. Across all 4 Sheet-bearing fixtures
on `/Sheet6`, only `0x0030` carries records (= the existing
`JStyleOverride` family, Phase 16). Every other type code in the swept
range has zero hits.

The Phase 19 goal package should therefore **not** be drafted around the
"sibling sweep" angle. Pivot needed — see §Recommended next angles.

## Probe details

| Artifact | Value |
|---|---|
| Probe binary | `examples/probe_rad_siblings_0x0029_0x0035.rs` (currently untracked) |
| Probed stream | `/Sheet6` only |
| Probed type-code range | `0x0029..=0x0035` (13 codes) |
| Validation predicate | `type_code & 0x3FFF == X` AND `bytes_to_follow ∈ [8, 100_000]` AND `bytes_to_follow ≤ remaining_stream` |
| Scan mode | Non-advancing (every byte offset that satisfies the predicate counts) |
| Fixtures | `DWG-0201GP06-01.pid`, `DWG-0202GP06-01.pid`, `工艺管道及仪表流程-1.pid`, `export-test/publish-data/A01/A01.pid` |

## Cross-fixture totals

| Type code | DWG-0201 | DWG-0202 | 工艺管道-1 | A01 | Total | Note |
|---|---:|---:|---:|---:|---:|---|
| `0x0029` | 0 | 0 | 0 | 0 | **0** | empty |
| `0x002A` | 0 | 0 | 0 | 0 | **0** | empty |
| `0x002B` | 0 | 0 | 0 | 0 | **0** | empty |
| `0x002C` | 0 | 0 | 0 | 0 | **0** | empty |
| `0x002D` | 0 | 0 | 0 | 0 | **0** | empty |
| `0x002E` | 0 | 0 | 0 | 0 | **0** | empty |
| `0x002F` | 0 | 0 | 0 | 0 | **0** | empty |
| `0x0030` | 20 | 34 | 59 | 2 | **115** | JStyleOverride (Phase 16/17, 98 after strict validation) |
| `0x0031` | 0 | 0 | 0 | 0 | **0** | empty |
| `0x0032` | 0 | 0 | 0 | 0 | **0** | empty |
| `0x0033` | 0 | 0 | 0 | 0 | **0** | empty |
| `0x0034` | 0 | 0 | 0 | 0 | **0** | empty |
| `0x0035` | 0 | 0 | 0 | 0 | **0** | empty |

## What this rules out

- A 1:1 CLSID-range → PSM-type-code mapping for the RAD `style.dll`
  `47FCC330..47FCC33E` siblings is **not** how SmartPlant assigns PSM
  type codes on `/Sheet6`.
- Drafting a Phase 19 package whose AC is "decode the 12 sibling
  classes" would chase a zero-record target.

## What this does NOT rule out

- Other `Sheet*` streams (e.g. `Sheet0`, `Sheet1`, …) were **not**
  probed. A broader cross-stream probe might still surface hits.
- A non-contiguous PSM↔CLSID mapping (RAD siblings could live at
  arbitrary type codes elsewhere in the `0x0000..0x3FFF` 14-bit space).
- A different RAD DLL family (`J2DSrv.dll`, `JUTIL.dll`, etc.) may
  expose additional annotation-like records under other type codes.

## Recommended next angles for Phase 19

Pick one before drafting the goal package; the previous "sibling sweep"
plan is dead.

| Angle | Why it's promising | Why it might fail |
|---|---|---|
| **A. Sub-kind discriminator reverse engineering for `0x0010`** | Phase 18 already shipped 582 audit records with diverse `raw_payload` sizes (13 / 16 / 21 / 28 / 31 / 43 / 45 / 46 / 50 / 70 / 74 / 76 / 86 / 94 / 99). Same-fixture size buckets strongly suggest a sub-kind discriminator byte. Direct payoff: typed sub-records that downstream consumers can read. | Without IDA confirmation of the discriminator field position we risk repeating the Phase 14 GArc2d naming mistake. |
| **B. Reference-chain resolver `JStyleOverride.referenced_oid_{a,c}` → `0x0010`** | Both endpoints already exist in stable DTOs (Phase 16/18). Resolver is pure cross-record bookkeeping, no new disk-format reverse engineering. | Requires defining the resolver's surface (graph edges? side-table?) and a contract test. Modest value until the 0x0010 payload is typed. |
| **C. Broader unhandled-type-code inventory across all `Sheet*` streams** | Honest "what's left" map — replaces the falsified sibling hypothesis with a data-driven priority list. Enables informed Phase 19/20 scoping. | Pure inventory; no decoder lands this phase. May feel like procrastination. |
| **D. IDA-assisted `0x0010` class identification** | Most authoritative source for sub-kind discriminator and field semantics. Aligns with Phase 16's `JStyleOverride` reverse-engineering chain. | Phase 18 `blockers.md` flags IDA instance loads as Stop-And-Ask; needs explicit user authorization. |

## Stop-And-Ask

Per Phase 18 `blockers.md` §1 ("跨 fixture 总数 < 638 或 > 638 / 出现要新增
`PidGraphicKind` variant 的诱因 / 出现要解析 reference chain 的诱因 / 出现
要把 sub-kind discriminator 命名为字段的诱因") and the explicit
goal-prompt instruction "**不主动扩到 sub-kind 字段反向 / reference
resolver / Phase 19**", drafting the Phase 19 goal package without user
confirmation of the new angle would exceed the standing authorization.

Awaiting user direction (A / B / C / D, or a different angle) before
creating `goals/phase19-…/` and running `/goal`.
