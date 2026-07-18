# Phase 33: PSM 0x0010 Discriminator IDA Plan

> Date: 2026-06-22  
> Scope: restart the gated investigation for SmartPlant / Smart P&ID Sheet PSM
> sub-record family `0x0010` in `pid-parse`.  
> Status: planning artifact only. No parser confidence, DTO, schema, writer, or
> bundle contract changes are introduced by this note.

## Context

The current repository already parses SmartPlant `.pid` files and exports a
confidence-ledgered bundle. The unresolved `0x0010` family is not a missing
container parser. It is a gated semantic-promotion problem inside already
identified `Sheet*` record streams.

Current repository baseline:

- `SheetSubRecord0x0010Decoded` is `TypedAudit`, not `Decoded`.
- The parser validates the stable PSM envelope and preserves raw payload bytes.
- `leading_word` means `payload[0..2]` as little-endian `u16`; it is positional
  evidence only and must not be renamed to `sub_kind`.
- Historical Phase 20 IDA work reached partial class identity for GUID
  `1D1928C0-0000-0000-C000-000000000046`, but did not recover a concrete
  `Read`, `Load`, or `IOContext::DoIO` sequence.
- The 2026-06-19 atlas and roadmap keep `ROADMAP-0010` at `TypedAudit`.
- The currently reachable `sppid.dll` and `core.dll` IDA instances are negative
  evidence for raw `.pid` persistence and should not be used for promotion.

## Goal

Produce bounded evidence that either:

1. proves a discriminator and field layout for at least one `0x0010` sub-family,
   allowing a future parser slice to promote that sub-family under the
   repository gates; or
2. closes the investigation negatively and preserves `0x0010` as `TypedAudit`
   with explicit reasons.

This phase should not directly implement a typed parser unless the evidence
gate below is already satisfied.

## IDA Target Order

Use `ida-pro-mcp` only against modules that can plausibly contain the persisted
reader or type-dispatch path:

1. `radsrvitem.dll`
   - Primary target for RAD persisted object dispatch, PSM type tables, and
     `PSMSerializeIn` / `PSMSerializeOut` paths.
2. `J2DSrv.dll`
   - Candidate for 2D record and Sheet/RAD projection helpers.
3. `style.dll`
   - Known positive source for `JStyleOverride 0x0030`; useful for references
     from `JStyleOverride` into `0x0010`, but not sufficient by itself.
4. `jengine.dll` / `XceedRAD.dll`
   - Secondary candidates for generic RAD persistence or geometry helpers.
5. `OLESITE.dll` / `OLECRT.dll`
   - Use only if the trace crosses into OLE storage or site-list ownership.

Do not spend additional broad-search time in `sppid.dll` or `core.dll` unless a
new xref from a relevant module points there.

## IDA Work Items

### A. Instance And Scope Check

1. Run `list_instances`.
2. Select an existing relevant target if available.
3. If a relevant target is not available, open it with the existing
   `ida-reverse` scripts rather than ad-hoc commands.
4. Record module name, IDB path, image base, function count, string count, and
   whether Hex-Rays is ready.

Stop condition: if no target beyond `sppid.dll` / `core.dll` is reachable and
no binary path is available, write a tooling-gated closeout and do not touch
parser code.

### B. Rebuild The `0x0010` Native Path

Search and trace these anchors:

- GUID: `1D1928C0-0000-0000-C000-000000000046`
- PSM type: `0x0010`
- parent alias: `0x0115`
- dispatch names: `PSMSerializeIn`, `PSMSerializeOut`, `Persist`, `DoIO`,
  `IOContext`, `Read`, `Load`, `Write`, `Save`
- nearby families: `JStyleOverride`, `GraphicGroup`, `RAD_OBJECT_TYPE`

For every hit, capture:

- function address and current name;
- callers and callees;
- any string or constant xrefs;
- whether the function reads from a byte/stream context or only manipulates
  runtime objects.

### C. Compare IDA Sequence With Fixture Buckets

Use existing parser output as the fixture side of the evidence:

- record count per fixture;
- payload size buckets;
- `leading_word` distribution;
- example stream path and byte range;
- references from `JStyleOverride` offsets `+38..41` and `+56..59`, where
  already documented.

The IDA side must explain one or more of those distributions with bounded field
reads. A function that merely dispatches a runtime class is not enough.

### D. Negative Evidence Is A Valid Result

Close negatively if any of these happen:

- only type-table identity is recovered;
- only runtime object construction is recovered;
- the candidate function never reads persisted bytes;
- the discriminator offset differs across major buckets and no secondary
  discriminator is recovered;
- evidence is limited to one fixture or one record shape;
- the only support is `leading_word` frequency.

The closeout should explicitly keep `ROADMAP-0010` as `TypedAudit`.

## Promotion Gate For A Future Parser Slice

A future parser implementation may start only after the investigation provides:

- stream path, for example `/Sheet6`;
- half-open byte ranges for every proposed field;
- record family and sub-family identity;
- fixture identity and cross-fixture ratchets;
- representative bytes and expected decoded values;
- byte-audit movement for decoded, audit/probe, and leftover ranges;
- malformed and truncated parser cases;
- panic-safety coverage in `tests/parser_panic_safety.rs`;
- schema/public DTO tests for any public output change;
- rollback criteria if IDA or fixture evidence contradicts the proposed layout.

Writer support remains out of scope. Reader confidence never authorizes Sheet
semantic write-back.

## Recommended Immediate Commands

When a relevant IDA instance is reachable:

```text
list_instances
select_instance(<radsrvitem-or-J2DSrv-port>)
survey_binary(detail_level = "minimal")
find_regex("PSMSerializeIn|PSMSerializeOut|IOContext|DoIO|JStyleOverride|GraphicGroup|RAD_OBJECT_TYPE|0010|0115")
entity_query(functions/imports/strings around the hits)
analyze_function(candidate_addr)
xrefs_to(candidate strings, GUID references, and dispatch table entries)
```

When fixture-side evidence needs refreshing:

```powershell
cargo run --release --example probe_psm_0x0010_sub_kind
cargo test --locked --test parse_real_files sub_records_0x0010 -- --nocapture
cargo test --locked --test parse_real_files jstyle_override_decoder_emits_audit_records_with_provenance -- --nocapture
cargo run --bin pid_inspect -- test-file/D06.pid --byte-audit --json
```

Use the exact local fixture paths that are available in the working tree. Tests
that rely on private fixtures should soft-skip with explicit messages.

## Deliverables

The next implementation-independent deliverable should be one focused analysis
note:

```text
docs/analysis/<date>-phase33-0010-discriminator-ida-evidence.md
```

It should contain:

- target IDA module list and availability;
- searched anchors and misses;
- candidate functions with addresses;
- decompiled or summarized reader sequences;
- fixture bucket reconciliation;
- promotion decision or negative closeout;
- explicit atlas/roadmap confidence impact.

If the decision is positive, open a separate parser implementation slice. If the
decision is negative or tooling-gated, update only docs and keep parser code
unchanged.
