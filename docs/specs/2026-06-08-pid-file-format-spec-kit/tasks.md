# Tasks: PID File Format Spec Kit

## Phase 28-A: Spec Package Bootstrap

- [x] Create Spec Kit package directory.
- [x] Write `spec.md` with scope, evidence levels, requirements, guardrails, and
  acceptance criteria.
- [x] Write `plan.md` with Phase 28 execution slices.
- [x] Write `research.md` with parser and IDA findings.
- [x] Write `data-model.md` with all current known format families.
- [x] Write `tasks.md`.
- [x] Write `quickstart.md`.

## Phase 28-B: Planning File Sync

- [x] Add Phase 28 entry to `task_plan.md`.
- [x] Add Phase 28 findings to `findings.md`.
- [x] Add Phase 28 session log to `progress.md`.

## Phase 28-C: IDA Availability Check

- [x] Inspect `ida-pro-mcp` tool descriptors when available.
- [x] List reachable IDA instances.
- [x] Confirm whether `style.dll`, `J2DSrv.dll`, `sppid.dll`,
  `XCeedRAD.dll`, or `smartplantpid.exe` is open.
- [x] If only `radsrvitem.dll` and unrelated `core.dll` are available, mark
  deep `0x0010` / ordinary geometry reader work as blocked.

Checked 2026-06-10: reachable instances are `core.dll`
(AVEVA Everything3D, unrelated) and `radsrvitem.dll` only. None of
`style.dll` / `J2DSrv.dll` / `sppid.dll` / `XCeedRAD.dll` /
`smartplantpid.exe` is open. Deep `0x0010` discriminator work and
ordinary geometry reader confirmation remain **blocked**; Phase 28-D
stays pending until one of those IDBs is opened.

## Phase 28-D: IDA Evidence Refresh

- [ ] Re-export or verify `sub_56448F70` type-code mapper.
- [ ] Re-check `sub_56445F40` dispatch targets:
  - [ ] `0x004D -> sub_564468B0`
  - [ ] `0x00FA -> sub_56446020`
  - [ ] `0x003D -> sub_564464D0`
  - [ ] default -> `sub_564462F0`
- [ ] For each P0 decoded type, record whether IDA provides only type identity
  or also reader field layout:
  - [ ] `0x0018 igLine2d`
  - [ ] `0x004D igTextBox`
  - [ ] `0x005E igPoint2d`
  - [ ] `0x0084 igLineString2d`
  - [ ] `0x00CE igSymbol2d`
- [ ] Search newly opened modules for:
  - [ ] `1D1928C0`
  - [ ] `JStyleBase`
  - [ ] `IJPersist`
  - [ ] `DoIO`
  - [ ] `GraphicGroup`
  - [ ] ordinary IGDS type names.

## Phase 28-E: Fixture And Byte-Audit Snapshot

- [x] Locate current `.pid` fixtures.
- [x] If fixtures exist, run coverage JSON for representative samples.
- [x] If fixtures exist, run byte-audit JSON for representative samples.
- [ ] If fixtures are absent, document the blocked state and do not invent
  current coverage numbers.

Local fixture snapshots generated:

- `d06-coverage.json`
- `d06-byte-audit.json`
- `nonascii-process-1-coverage.json`
- `nonascii-process-1-byte-audit.json`
- `dwg0201-coverage.json`
- `dwg0201-byte-audit.json`
- `dwg0202-coverage.json`
- `dwg0202-byte-audit.json`
- `publish-a01-coverage.json`
- `publish-a01-byte-audit.json`
- `publish-dwg0202-coverage.json`
- `publish-dwg0202-byte-audit.json`

## Phase 28-F: Format Completion Backlog

- [x] For each entry in `data-model.md`, assign one of:
  - [x] `Complete`
  - [x] `NeedsFixture`
  - [x] `NeedsIDA`
  - [x] `NeedsParser`
  - [x] `Blocked`
- [x] Create Phase 29+ parser backlog from entries that have both IDA and
  fixture evidence (`phase29-candidate-slices.md`, Phase 28-H).
- [x] Keep entries without sufficient evidence as documentation-only negative
  closeouts (`data-model.md` Completion Classification table).

## Phase 28-G: Snapshot Priority Backlog

- [x] Aggregate 6-fixture byte-audit snapshots by stream family.
- [x] Rank highest leftover families and individual paths.
- [x] Record common unregistered paths.
- [x] Generate `snapshot-priority-backlog.md`.
- [x] Convert the backlog into Phase 29 implementation slices after IDA / user
  priority is clear.

## Phase 28-H: Phase 29 Candidate Slices

- [x] Convert the snapshot priority backlog into Phase 29 candidate slices.
- [x] Define Slice 29-A Sheet stream delta and unknown record prioritization.
- [x] Define Slice 29-B PSMcluster0 body triage.
- [x] Define Slice 29-C Dynamic Attributes deep body backlog.
- [x] Define Slice 29-D PSMspacemap `tseg` evidence gate.
- [x] Define Slice 29-E JSite symbol-instance demand gate.
- [x] Define Slice 29-F IDA module enablement.
- [x] Record recommended next slice: 29-A if IDA remains blocked, 29-F if IDA
  becomes available.

## Phase 29-A: Sheet Leftover Priority Report

- [x] Generate `docs/analysis/2026-06-08-phase29-sheet-leftover-priority.md`.
- [x] Aggregate Sheet paths across 6 fixtures.
- [x] Rank top Sheet leftover items.
- [x] Separate registered top-level `/Sheet*` paths from nested
  `/JSite204/Sheet*` unregistered paths.
- [x] Define follow-up extractor acceptance criteria.
- [x] Build bounded Sheet leftover window extractor:
  `examples/probe_phase29_sheet_leftover_windows.rs`.
- [x] Generate `docs/analysis/2026-06-08-phase29-sheet-leftover-windows.md`.
- [x] Group Sheet leftovers by conservative local byte shape.
- [x] Map groups to investigation-only next actions.
- [x] Review top groups manually against existing typed / audit-only decoders:
  `docs/analysis/2026-06-08-phase29-sheet-leftover-review.md`.

## Phase 29-B: Sheet Byte-Audit Trace Integration

- [x] Add trace consumption for existing Sheet typed / audit-only decoders.
- [x] Keep decoded vs audit-only / probed confidence distinct.
- [x] Add focused synthetic byte-audit test for one known typed Sheet record.
- [x] Fix union consumed-byte accounting for overlapping confidence ranges.
- [x] Re-run 6-fixture snapshot matrix.
- [x] Re-run bounded Sheet leftover windows after trace integration.
- [x] Document results in
  `docs/analysis/2026-06-08-phase29-sheet-byte-audit-trace-integration.md`.

## Phase 29-C: After-Trace Remaining Groups Review

- [x] Review `phase29-sheet-leftover-windows-after-trace.md`.
- [x] Classify remaining top groups.
- [x] Document review in
  `docs/analysis/2026-06-08-phase29-sheet-after-trace-review.md`.
- [x] Select recommended next slice: Phase 29-C1 Symbol Reject Probe.
- [x] Generate `docs/analysis/2026-06-08-phase29-igsymbol-reject-probe.md`.
- [x] Classify rejected `0x00CE` candidates by validation failure reason.
- [x] Keep `decode_igsymbols` validation unchanged.
- [x] Review nested `/JSite204/Sheet*` ownership / registration.
- [x] Document nested Sheet review in
  `docs/analysis/2026-06-08-phase29-nested-sheet-ownership-review.md`.
- [x] Keep nested `JSite*/Sheet*` out of top-level Sheet registration.

## Phase 29-D: JSite Nested Package Inventory

- [x] Generate nested `JSite*` package inventory across 6 fixtures:
  `docs/analysis/2026-06-08-phase29-nested-jsite-package-inventory.md`.
- [x] Include parent JSite id, child stream names, sizes, and mirrored top-level families.
- [x] Include JProperties symbol path / GUID evidence when available.
- [x] Classify each nested package as `NeedsOwnership`, `CanTraceHeaderOnly`, or
  `IgnoreUntilConsumerNeeds`.
- [x] Add header-only byte-audit trace for nested JSite cluster-family child
  streams.
- [x] Preserve nested `JProperties` parser routing.
- [x] Re-run 6-fixture snapshots and nested inventory after header-only trace.

## Phase 29-E: Verification And Closeout

- [x] Run focused byte-audit tests.
- [x] Run formatting check.
- [x] Read lints for edited files.
- [x] Synchronize final Phase 29 findings / progress.

## Phase 29-F: PSMcluster0 Body Triage (candidate Slice 29-B)

- [x] Build bounded body probe:
  `examples/probe_phase29_psmcluster0_body_triage.rs`.
- [x] Compare `/PSMcluster0` byte-audit leftovers across 6 fixtures.
- [x] Test the Sheet PSM record envelope against the post-string-table
  body (strict walk + min-chain-3 resync scan).
- [x] Document the single-chain result, `record_count - 2` invariant,
  type histograms, and payload texture in
  `docs/analysis/2026-06-08-phase29-psmcluster0-leftover-triage.md`.
- [x] Decide next step: parser-only audit-only record-chain walker
  (backlog filed in the triage doc) + secondary IDA target request.
- [x] Implement the PSMcluster0 audit-only walker with the test targets
  named in the triage doc:
  - [x] `decode_cluster_body_record_at` / `decode_cluster_body_records`
    (shared core) + `decode_psm_cluster0_body_records` (full-coverage
    gate) + `parse_psm_cluster0_with_trace` extension.
  - [x] 8 unit tests + 3 panic-safety entries.
  - [x] Cross-fixture ratchet test
    `psmcluster0_body_chain_matches_record_count_invariant`
    (6/6 fixtures, `record_count - 2`, consumed ratio = 1.0).
  - [x] 6-fixture snapshots regenerated; `/PSMcluster0` leftover → 0.

## Phase 29-G: StyleCluster Body Triage (Slice 29-B follow-up)

- [x] Run the generalized body probe against `/StyleCluster`
  (`probe_phase29_psmcluster0_body_triage -- StyleCluster`).
- [x] Document the end-anchored tail chain, GUID-table-like prefix,
  `record_count` mismatch caveat, and namespace warning in
  `docs/analysis/2026-06-08-phase29-stylecluster-leftover-triage.md`.
- [x] Implement `decode_style_cluster_body_records` +
  `parse_style_cluster_with_trace` (earliest end-anchored chain locator,
  records fully `Probed`, prefix stays leftover).
- [x] 6 unit tests + panic-safety entry + cross-fixture ratchet
  `stylecluster_body_chain_is_end_anchored_across_fixtures`
  (6/6 fixtures, consumed ratios 0.631–0.898).
- [x] 6-fixture snapshots regenerated; `/StyleCluster` leftover
  83,468 → 12,300 (prefix only).
- [x] Characterize the GUID-table-like prefix:
  `docs/analysis/2026-06-08-phase29-stylecluster-prefix-characterization.md`
  (12-byte opener + 532-byte cross-fixture constant region + 42-byte
  style-slot texture; no uniform GUID stride → documentation-only
  closeout, no parser).
- [ ] IDA target request: `stream_type 0x005A` reader, prefix layout
  (count 13, GUID entries, 42-byte slots), `record_count` semantics,
  names for `0x002C`–`0x002E`.

## Phase 29-I: Unclustered Dynamic Attributes Body Triage (candidate Slice 29-C)

- [x] Confirm IDA module availability first (2026-06-11): only `core.dll`
  (unrelated) and `radsrvitem.dll` reachable; Slice 29-F stays blocked,
  so the recommended non-IDA route continues.
- [x] Build the DA-specific body probe:
  `examples/probe_phase29_da_body_triage.rs` (whole-stream envelope chain
  test + landmark alignment + attribute census).
- [x] Prove the body is a single end-anchored `0x0089` envelope chain on
  6/6 fixtures (coverage 0.9978–0.9998; only an 8-byte prologue outside
  the chain; zero tail gap).
- [x] Prove the prologue is the cluster-family magic `0x6C90_F544` + u32
  record counter (counter == literal-marker records 6/6; == strict chain
  records 5/6 — one flagged head in `nonascii-process-1`).
- [x] Prove the 31-byte "trailer" is byte-identical to the next record's
  envelope head (417/417 trailer offsets coincide with chain heads).
- [x] File the backlog + census + IDA target request in
  `docs/analysis/2026-06-08-phase29-dynamic-attributes-body-backlog.md`.
- [x] Implement the audit-only DA body-chain walker
  (`decode_unclustered_da_body_records` +
  `parse_unclustered_da_with_trace`, all-`Probed` claims, end-anchored
  full-coverage gate) with the test targets named in the backlog doc:
  - [x] 7 unit tests + panic-safety entry.
  - [x] aggregate `/Unclustered Dynamic Attributes` branch runs walker +
    landmark scanner under parser name `parse_unclustered_da`
    (+ 1 new aggregate test; landmark-only synthetic test updated).
  - [x] Cross-fixture ratchet `da_body_chain_is_end_anchored_across_fixtures`
    (6/6 fixtures, records 47/69/231/169/22/169, leftover = 0).
- [x] Regenerate the 6-fixture snapshots;
  `/Unclustered Dynamic Attributes` leftover 111,120 → 0 confirmed
  (whole-file ratios now 0.273–0.670).

## Phase 29-J: Nested JSite Cluster Body Dispatch (Slice B/C follow-up)

- [x] Probe nested twins with the existing walkers:
  `examples/probe_phase29_nested_cluster_bodies.rs` — 23/23 one-level
  nested `JSite*` `PSMcluster0` (11) / `StyleCluster` (11) /
  `Unclustered Dynamic Attributes` (1) streams decode end-anchored
  chains with the unmodified top-level walkers; nested `PSMcluster0`
  keeps the `record_count - 2` invariant (chain start 145 on 11/11).
- [x] Dispatch the nested byte-audit branch to the full walkers
  (`parse_psm_cluster0` / `parse_style_cluster` /
  `parse_unclustered_da`); `Sheet*` and `Dynamic Attributes Metadata`
  stay header-only pending ownership / semantic review; nested
  `Unclustered Dynamic Attributes` added to the nested registry.
- [x] Tests: aggregate dispatch test
  (`nested_jsite_cluster_bodies_dispatch_to_full_walkers`) +
  cross-fixture ratchet
  (`nested_jsite_cluster_bodies_are_end_anchored_across_fixtures`,
  23 streams, nested PSMcluster0/DA leftover = 0, nested StyleCluster
  leftover = prefix only).
- [x] Regenerate the 6-fixture snapshots: `JSite*` family leftover
  325,843 → 74,559 (ratio 0.7817); whole-file ratios 0.630–0.880.

## Phase 29-K: Per-Record DA Attribute Scoping (Slice C named benefit #1)

- [x] Factor the section-body parser out of `try_parse_record`
  (`parse_section_body`, extraction unchanged byte-for-byte).
- [x] Add `parse_attribute_records_chain_scoped`: chain-gated
  per-record section parsing with legacy fallback; recovers
  flagged-head records and ignores `0x89 0x00` byte pairs inside
  payloads.
- [x] Switch the `/Unclustered Dynamic Attributes` document pipeline
  (`streams/dynamic_attrs.rs`) to the chain-scoped extraction;
  `streams/cluster.rs` keeps the legacy generic scan.
- [x] Tests: 4 unit tests + panic-safety entry + cross-fixture ratchet
  (`da_chain_scoped_attribute_extraction_matches_or_beats_legacy_scan`,
  nonascii 68 → 69, others unchanged).
- [x] Snapshots unaffected (extraction does not change byte-audit
  claims; verified by hash-stable coverage JSON).
- [ ] Head-field surfacing for non-signature records (benefit #2):
  IDA-gated, deferred.

## Phase 29-L: Nested JSite Registry Dispatch (top-level parser reuse)

- [x] Probe nested registry twins with the existing top-level parsers:
  `examples/probe_phase29_nested_registry_streams.rs` — 68 streams,
  98.4% consumed; `DocVersion2/3` / `PSMclustertable` /
  `PSMsegmenttable` parse fully, `PSMroots` keeps the same 4-byte tail
  as its top-level twin, 4-byte `AppObject` stubs gate out cleanly,
  the `JSite204` summary pair parses partially like the top-level pair.
- [x] Dispatch nested `JSite*` registry children to the top-level
  parsers in the byte-audit (`nested_jsite_registry_parser` helper);
  `JSitesList` stays unregistered (no parser exists, demand-gated).
- [x] Tests: aggregate dispatch test
  (`nested_jsite_registry_children_dispatch_to_top_level_parsers`) +
  cross-fixture ratchet
  (`nested_jsite_registry_streams_reuse_top_level_parsers`).
- [x] Regenerate the 6-fixture snapshots: `JSite*` family leftover
  74,559 → 66,778 (ratio 0.8045); unregistered paths drop to 12–19 per
  fixture; whole-file ratios 0.664–0.888.

## Phase 29-M: JSitesList / Revision Unregistered Tail Closeout

- [x] Probe the last multi-fixture common-unregistered top-level paths:
  `examples/probe_phase29_unregistered_tails.rs` — `/JSitesList` is an
  `"OLEM"` magic + u32 count + u32 slot table; `/TaggedTxtData/Revision`
  is a 0-byte placeholder (5/5).
- [x] Ship `src/parsers/jsites_list.rs` (`parse_jsites_list` +
  trace variant; header `Decoded`, logical table `Probed`, stale
  trailing slots stay leftover); register top-level + nested
  `JSitesList` and the `Revision` placeholder in the byte-audit.
- [x] Ratchet: counts {9,10,20,13,5,13}, trailing {0,0,0,3,0,3},
  **storage matches == count on 6/6 fixtures** (the stream is the
  `JSite<id>` storage directory).
- [x] Regenerate the 6-fixture snapshots: unregistered paths drop to
  9–14 per fixture (distinct 51 → 38); remaining multi-fixture
  unregistered paths are all IDA / demand gated.
- [ ] IDA follow-up: `"OLEM"` writer, count-vs-slot stale tail, and
  slot-id semantics (would unlock model-layer exposure).

## Validation Tasks

- [x] Run `cargo fmt --all -- --check` (passed; Phase 29 closeout).
- [x] Run focused doc-relevant parser tests if parser code changes
  (`cargo test --lib byte_audit`, 44 passed).
- [x] Run full pre-commit gate before any parser promotion (all five
  gates passed, exit code 0; see `progress.md` Phase 29 full
  validation follow-up).
- [x] Read lints for edited markdown / source files (no errors).

These boxes record the Phase 28/29 closeout run. Re-run the same
gates for any future parser promotion.
