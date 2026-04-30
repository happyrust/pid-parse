# PID Geometry PR Split Inventory

> **Date:** 2026-05-01  
> **Purpose:** inventory current working-tree changes and assign them to reviewable PRs.  
> **Rule:** do not promote `Line + Inferred` or populate `SheetObjectGeometryHint` until object-coordinate mapping is source-proven.

## Current Working Trees

### `pid-parse`

Modified files currently observed:

- `CHANGELOG.md`
- `findings.md`
- `progress.md`
- `src/bin/pid_inspect.rs`
- `src/cfb/reader.rs`
- `src/crossref.rs`
- `src/import_view.rs`
- `src/inspect/report.rs`
- `src/lib.rs`
- `src/model.rs`
- `src/parsers/sheet_probe.rs`
- `src/schema.rs`
- `src/streams/cluster.rs`
- `task_plan.md`
- `tests/inspect_cli.rs`
- `tests/parse_real_files.rs`

Untracked files currently observed:

- `src/geometry.rs`
- `docs/diagrams/h7cad-pid-real-geometry-roadmap.svg`
- `docs/diagrams/h7cad-pid-real-geometry-roadmap.png`
- `docs/diagrams/h7cad-pid-text-placement-roadmap.svg`
- `docs/diagrams/h7cad-pid-text-placement-roadmap.png`
- `docs/plans/2026-05-01-h7cad-pid-pr-execution-checklist-cn.md`
- `docs/plans/2026-05-01-coordinate-quality-filter-plan.md`
- `docs/plans/2026-05-01-field-x-window-scoring-implementation-plan.md`
- `docs/plans/2026-05-01-graphic-identity-nearby-evidence-plan.md`
- `docs/plans/2026-05-01-graphic-identity-scanner-implementation-plan.md`
- `docs/plans/2026-05-01-next-pr-split-and-evidence-roadmap.md`
- `docs/plans/2026-05-01-sheet-object-geometry-mapping-probe-plan.md`
- `docs/plans/2026-05-01-sheet6-evidence-inventory.md`
- `docs/plans/2026-05-01-stable-evidence-scoring-integration-plan.md`
- `docs/plans/2026-05-01-stronger-sheet-record-shape-evidence-plan.md`
- `docs/plans/2026-05-01-text-placement-evidence-plan-cn.md`

### `H7CAD-pid-real-geometry-display`

Modified files currently observed:

- `src/app/update.rs`
- `src/io/pid_import.rs`
- `src/io/pid_screenshot.rs`
- `src/io/svg_export.rs`
- `src/modules/registry.rs`

Untracked files currently observed:

- `docs/plans/2026-04-30-pid-normalized-geometry-h7cad-integration-plan.md`
- `docs/plans/2026-05-01-pid-real-geometry-next-phase-plan.md`

## Proposed PR Boundaries

### PR 1 - Normalized Geometry Contract

Scope:

- Introduce the stable normalized geometry model.
- Export it through CLI/schema/reporting.
- Keep output evidence-focused: inferred points and probe unknowns only.

Candidate files:

- `src/geometry.rs`
- `src/lib.rs`
- `src/model.rs`
- `src/schema.rs`
- `src/bin/pid_inspect.rs`
- `src/inspect/report.rs`
- `tests/inspect_cli.rs`
- baseline portions of `tests/parse_real_files.rs`
- related docs: integration plan and changelog entries

Minimum validation:

```powershell
cargo test --test inspect_cli -- --nocapture
cargo test --test parse_real_files normalized_geometry_probe_baseline_on_real_fixture -- --nocapture
cargo test --lib schema -- --nocapture
```

Review notes:

- This PR should not include field-x scoring helpers.
- `SheetObjectGeometryHint` should not be included in PR 1. PR 1 only needs the normalized geometry contract and the Sheet evidence needed to derive inferred points / probe unknowns.

### PR 2 - H7CAD Inferred Point Rendering

Scope:

- Consume normalized geometry in H7CAD.
- Render inferred points as small point/cross entities.
- Keep topology preview separate from real geometry layers.

Candidate files:

- `src/io/pid_import.rs`
- `src/io/pid_screenshot.rs`
- `src/io/svg_export.rs`
- `src/app/update.rs` only if required by PID open flow or formatting drift
- `src/modules/registry.rs` only if required by module registration
- H7CAD plan docs

Minimum validation:

```powershell
cargo test -p H7CAD pid_bundle -- --nocapture
```

Review notes:

- No endpoint-line rendering.
- Unknown/probe-only geometry must remain non-rendered.
- If `src/app/update.rs` is mostly formatting churn, isolate or avoid it in this PR.

Inspection result:

- `src/app/update.rs` currently has a large diff (`1266` insertions / `527` deletions), but most sampled hunks are rustfmt line wrapping and import reordering.
- The PR 2-relevant functional hunks are:
  - PID open message uses `bundle.geometry_stats` and appends `"<n> inferred geometry points"`.
  - PID tabs use `fit_layers_matching(["PID_OBJECTS_", "PID_LAYOUT_TEXT", "PID_RELATIONSHIPS"])` before falling back to `fit_all`.
- Recommendation: stage only these functional hunks for PR 2 if possible. Keep broad rustfmt churn out of the point-rendering PR unless the repository owner accepts formatting-only noise.
- `src/modules/registry.rs` appears modified in `git status`, but `git diff --numstat` / `git diff --summary` showed no content diff. Treat it as line-ending or metadata noise unless a later diff proves otherwise.

### PR 3 - Sheet6 Evidence Guardrails

Scope:

- Add `SheetObjectGeometryHint` contract slot and guardrail tests.
- Document why `/Sheet6` cannot produce source-proven object-coordinate mappings yet.

Candidate files:

- `src/model.rs`
- `src/schema.rs`
- `src/cfb/reader.rs`
- `src/crossref.rs`
- `src/import_view.rs`
- `src/streams/cluster.rs`
- guardrail portions of `tests/parse_real_files.rs`
- `docs/plans/2026-05-01-sheet6-evidence-inventory.md`
- `docs/plans/2026-05-01-sheet-object-geometry-mapping-probe-plan.md`

Minimum validation:

```powershell
cargo test --test parse_real_files sheet6_object_geometry_hints_baseline_is_empty_until_mapping_is_proven -- --nocapture
cargo test --lib schema -- --nocapture
```

Review notes:

- `object_geometry_hints` must stay empty for current real fixture.
- This PR should explain endpoint records as semantic evidence, not coordinate evidence.
- `SheetObjectGeometryHint` belongs here, not PR 1. It is a forward-compatible guardrail slot for future object-coordinate mapping evidence.
- `src/model.rs` should be split so PR 1 introduces `SheetGeometry` / `SheetText` / `SheetEndpoint` / `SheetCoordinateHintDto`, while PR 3 adds `object_geometry_hints: Vec<SheetObjectGeometryHint>` and the `SheetObjectGeometryHint` DTO.
- `src/schema.rs` should follow the same split: normalized geometry schema tests in PR 1, `schema_exposes_sheet_geometry_dtos` coverage for `SheetObjectGeometryHint` in PR 3.

### PR 4 - Field-X Investigation And Scoring

Scope:

- Add experimental field-x window investigation helpers.
- Add scoring and quality filters that prove no current candidate is promotable.

Candidate files:

- `src/parsers/sheet_probe.rs`
- investigation/scoring portions of `tests/parse_real_files.rs`
- `docs/plans/2026-05-01-field-x-window-scoring-implementation-plan.md`
- `docs/plans/2026-05-01-stronger-sheet-record-shape-evidence-plan.md`
- `docs/plans/2026-05-01-stable-evidence-scoring-integration-plan.md`
- `docs/plans/2026-05-01-coordinate-quality-filter-plan.md`
- `docs/plans/2026-05-01-next-pr-split-and-evidence-roadmap.md`

Minimum validation:

```powershell
cargo test --lib parsers::sheet_probe -- --nocapture
cargo test --test parse_real_files sheet6_field_x_window_probe_finds_sample_endpoint_ids -- --nocapture
cargo test --test parse_real_files sheet6_field_x_window_scoring_reports_non_endpoint_candidates -- --nocapture
cargo test --test parse_real_files sheet6_field_x_window_features_report_chunk_shapes -- --nocapture
```

Review notes:

- Promotion threshold must not be lowered.
- `promotable=0` after quality filters is the expected outcome.
- Keep helpers framed as investigation tools, not production geometry extraction.

### PR 5 - GraphicIdentityNearby Investigation

Scope:

- Add identity-index and identity-window scanner helpers.
- Report same-object vs wrong-object identity evidence near Sheet field-x windows.
- Integrate same-object identity as scoring evidence without promoting geometry.

Candidate files:

- `src/parsers/sheet_probe.rs`
- identity report/scoring portions of `tests/parse_real_files.rs`
- `docs/plans/2026-05-01-graphic-identity-nearby-evidence-plan.md`
- `docs/plans/2026-05-01-graphic-identity-scanner-implementation-plan.md`

Minimum validation:

```powershell
cargo test --lib parsers::sheet_probe -- --nocapture
cargo test --test parse_real_files sheet6_field_x_window_identity_report -- --nocapture
cargo test --test parse_real_files sheet6_graphic_identity_scoring_keeps_object_hints_empty_until_proven -- --nocapture
```

Review notes:

- This should not be bundled into PR 4 unless reviewers explicitly want one larger investigation PR.
- Real `/Sheet6` identity report currently finds `same_object=11`, `wrong_object=414`.
- Real `/Sheet6` identity scoring currently remains `identity_supported=0`, `max_score=45`, `over_threshold=0`.
- `object_geometry_hints` must remain empty.

### PR 6 - Text Placement Investigation

Scope:

- Add investigation-only text placement candidates and scoring.
- Prove current `/Sheet6` text runs are not safe to promote to `Text + Inferred`.
- Keep normalized geometry and H7CAD behavior unchanged.

Candidate files:

- `src/parsers/sheet_probe.rs`
- text placement report/scoring portions of `tests/parse_real_files.rs`
- `docs/plans/2026-05-01-text-placement-evidence-plan-cn.md`
- `docs/diagrams/h7cad-pid-text-placement-roadmap.svg`
- `docs/diagrams/h7cad-pid-text-placement-roadmap.png`

Minimum validation:

```powershell
cargo test --lib parsers::sheet_probe -- --nocapture
cargo test --test parse_real_files sheet6_text_window_report_keeps_text_probe_only_until_position_is_proven -- --nocapture
```

Review notes:

- This should not be bundled into PR 5 unless reviewers explicitly want a larger investigation PR.
- Current `/Sheet6` text report finds `text_runs=9`, `candidates=121`, `same_chunk=25`, `coordinate_quality_passed=2`.
- Current `/Sheet6` text scoring remains `text_quality_passed=0`, `max_score=-50`, `over_threshold=0`.
- Normalized geometry must still have no `PidGraphicKind::Text`; `/Sheet6` text remains `ProbeOnly Unknown`.

## `tests/parse_real_files.rs` Hunk Split

The current diff adds imports plus one contiguous block of tests after
`sheet_probe_evidence_populates_on_real_sheet_fixture`.

Recommended split:

- PR 1:
  - `normalized_geometry_probe_baseline_on_real_fixture`
  - import only what this test needs; avoid pulling field-x helpers into PR 1.
- PR 3:
  - `sheet6_object_geometry_hints_baseline_is_empty_until_mapping_is_proven`
  - this test pairs with the `SheetObjectGeometryHint` DTO contract and evidence docs.
- PR 4:
  - `sheet6_field_x_window_probe_finds_sample_endpoint_ids`
  - `sheet6_field_x_window_scoring_reports_non_endpoint_candidates`
  - `sheet6_all_endpoint_field_x_window_scoring_report`
  - `sheet6_field_x_window_features_report_chunk_shapes`
  - field-x/scoring imports and `HashSet`.
- PR 5:
  - `sheet6_field_x_window_identity_report`
  - `sheet6_graphic_identity_scoring_keeps_object_hints_empty_until_proven`
  - `all_sheets_graphic_identity_scoring_report_keeps_object_hints_empty`
  - identity imports: `field_x_window_identities`, `sheet_identity_index_from_trailers`, `score_field_x_window_features_with_identities`
- PR 6:
  - `sheet6_text_window_report_keeps_text_probe_only_until_position_is_proven`
  - text placement imports: `sheet_text_window_candidates`, `score_sheet_text_window_candidates`

Manual staging note:

- Because the tests are currently adjacent, use hunk editing or a temporary split branch/file workflow when preparing the actual PRs.
- Keep `HashSet` out of PR 1/PR 3 unless their local imports require it.
- PR 4 owns all `field_x_windows`, `field_x_window_features`, `score_*`, `stable_*` imports.
- PR 5 owns identity scanner/scoring imports and should not alter H7CAD behavior.
- PR 6 owns text placement helper/scoring imports and should not alter normalized geometry or H7CAD behavior.

## Split Risks

- `tests/parse_real_files.rs` now contains baseline, guardrail, and scoring tests. It may need manual hunk staging per PR.
- `src/model.rs` mixes stable geometry DTOs and `SheetObjectGeometryHint`; decide whether the hint contract lands in PR 1 or PR 3.
- H7CAD may depend on local `pid-parse` DTO shape. Coordinate branch order so H7CAD PR points at a compatible `pid-parse` revision.
- `src/app/update.rs` has a large diff. Inspect before assigning; avoid burying unrelated formatting churn in the point-rendering PR.

## Immediate Next Actions

1. Stage only the two functional `src/app/update.rs` hunks for PR 2, or move them into a smaller follow-up patch.
2. Split `src/model.rs` / `src/schema.rs` so `SheetObjectGeometryHint` lands in PR 3.
3. Split `tests/parse_real_files.rs` into the PR 1 / PR 3 / PR 4 / PR 5 / PR 6 test groups documented above.
4. Split `src/parsers/sheet_probe.rs` carefully: PR 4 owns field-x/features/scoring; PR 5 owns identity scanner/scoring; PR 6 owns text placement candidates/scoring.
5. Only after the PR boundaries are stable, prepare commits or branches on explicit request.

## Focused Validation Snapshot

Last run: 2026-05-01.

`pid-parse`:

```powershell
cargo test --test inspect_cli -- --nocapture
cargo test --lib schema -- --nocapture
cargo test --test parse_real_files normalized_geometry_probe_baseline_on_real_fixture -- --nocapture
cargo test --test parse_real_files sheet6_object_geometry_hints_baseline_is_empty_until_mapping_is_proven -- --nocapture
cargo test --test parse_real_files sheet6_field_x_window_features_report_chunk_shapes -- --nocapture
cargo test --test parse_real_files sheet6_field_x_window_identity_report -- --nocapture
cargo test --test parse_real_files sheet6_graphic_identity_scoring_keeps_object_hints_empty_until_proven -- --nocapture
cargo test --test parse_real_files all_sheets_graphic_identity_scoring_report_keeps_object_hints_empty -- --nocapture
cargo test --test parse_real_files sheet6_text_window_report_keeps_text_probe_only_until_position_is_proven -- --nocapture
```

Result:

- `inspect_cli`: 10 passed.
- `schema`: 6 passed.
- normalized real fixture baseline: passed.
- `/Sheet6.object_geometry_hints == 0` guardrail: passed.
- field-x feature report: passed with `max_score=45`, `promotable=0`, `top_feature_scores=[]`.
- identity report: passed with `same_object=11`, `wrong_object=414`.
- identity scoring: passed with `identity_supported=0`, `max_score=45`, `over_threshold=0`.
- all-Sheet identity scoring: passed with `sheets=1`, `identity_supported=0`, `over_threshold=0`.
- text placement report: passed with `text_quality_passed=0`, `max_score=-50`, `over_threshold=0`.

`H7CAD-pid-real-geometry-display`:

```powershell
cargo test -p H7CAD pid_bundle -- --nocapture
```

Result:

- `pid_bundle`: 4 passed.

