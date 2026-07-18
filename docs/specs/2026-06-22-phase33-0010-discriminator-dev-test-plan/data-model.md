# Data Model: Phase 33 Evidence And Test Planning

## InvestigationTarget

Represents one IDA module or controlled fixture source.

Fields:

- `name`: module or fixture name.
- `kind`: `ida_module` | `controlled_fixture` | `real_pid_fixture`.
- `path`: source path when known.
- `availability`: `available` | `missing` | `tooling_gated` | `not_checked`.
- `role`: why this target can prove or reject `0x0010` semantics.
- `scope_limit`: what this target cannot prove.

Validation rules:

- `sppid.dll` and `core.dll` may be recorded, but cannot authorize promotion
  without a relevant xref chain from a persistence module.
- Missing targets must include a re-open trigger.

## IdaEvidenceRecord

Represents one concrete IDA observation.

Fields:

- `module`: IDA module name.
- `addr`: function, string, vtable, or data address.
- `symbol`: current name or description.
- `anchor`: searched GUID, type code, string, or xref source.
- `evidence_kind`: `reader_candidate` | `dispatcher` | `type_identity` |
  `negative_search` | `runtime_only`.
- `summary`: short observation.
- `confidence_impact`: `promote_candidate` | `no_change` |
  `negative_closeout`.

Validation rules:

- A `reader_candidate` must show persisted byte or stream context access.
- `type_identity` alone cannot promote parser output.

## FixtureBucket

Represents fixture-side evidence for existing `0x0010` records.

Fields:

- `fixture_id`: stable fixture label.
- `stream_path`: example source stream, such as `/Sheet6`.
- `record_count`: count of `0x0010` records.
- `size_bucket`: payload or record size bucket.
- `leading_word_distribution`: positional word counts.
- `example_ranges`: bounded byte ranges for review.

Validation rules:

- Single-fixture buckets are investigation-only.
- Cross-fixture distributions are required before semantic naming.

## CandidateLayout

Represents a proposed field layout for a sub-family.

Fields:

- `family`: always `0x0010` for this package.
- `sub_family_name`: proposed name, only allowed after evidence.
- `discriminator_offset`: byte offset and interpretation.
- `fields`: ordered list of `{name, range, type, evidence}`.
- `ida_evidence`: linked `IdaEvidenceRecord` ids.
- `fixture_evidence`: linked `FixtureBucket` ids.
- `status`: `proposed` | `accepted_for_parser` | `rejected`.

Validation rules:

- `sub_family_name` must remain absent while status is `proposed`.
- Every accepted field must have a half-open byte range.
- Rejected layouts must record why they were rejected.

## ParserSlice

Represents a future code change, if evidence is sufficient.

Fields:

- `name`: slice name.
- `precondition`: evidence gate required before starting.
- `parser_entry_points`: functions to add or modify.
- `model_surface`: public DTO/schema changes.
- `byte_audit_delta`: expected decoded/audit/probe/leftover movement.
- `rollback_criteria`: conditions that revert the slice to audit-only.

Validation rules:

- No `ParserSlice` may start from IDA type identity alone.
- Writer scope must be `none` for this phase.

## TestGate

Represents one required validation gate.

Fields:

- `name`: gate name.
- `command`: runnable command or manual IDA step.
- `scope`: `doc_only` | `focused_parser` | `fixture` | `panic_safety` |
  `schema` | `full_workspace`.
- `required_for`: `documentation` | `parser_change` | `public_surface_change`.
- `expected_result`: pass condition.

Validation rules:

- Parser changes require at least one gate in every scope except `doc_only`.
- Private fixture gates must soft-skip explicitly when fixtures are missing.

## NegativeCloseout

Represents a valid no-promotion result.

Fields:

- `reason`: concise blocker.
- `searched_targets`: IDA modules or fixture sources checked.
- `failed_anchors`: search terms or xrefs that did not produce evidence.
- `unchanged_confidence`: expected value, usually `TypedAudit`.
- `reopen_trigger`: what new binary, fixture, or xref would restart work.

Validation rules:

- A negative closeout must not change parser code.
- It must update the relevant analysis doc or roadmap note.
