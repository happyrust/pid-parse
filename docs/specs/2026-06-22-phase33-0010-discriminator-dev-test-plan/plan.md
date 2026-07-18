# Implementation Plan: Phase 33 PSM 0x0010 Discriminator

## Technical Context

The repository already has a layered SmartPlant / Smart P&ID `.pid` parser,
byte-audit tracing, real fixture ratchets, and IDA evidence documents. The
`0x0010` family is currently represented as audit-only data:

- parser family: `decode_sub_records_0x0010`;
- current confidence: `TypedAudit`;
- public caution: `leading_word` is positional evidence only;
- main blocker: no recovered persisted reader sequence or stable discriminator.

This package follows the existing repository convention under `docs/specs/`.
The official Spec Kit `speckit-plan` setup script was not run because this
repository currently has no `.specify/` directory, no
`.specify/scripts/powershell/setup-plan.ps1`, and no `.specify/memory`
constitution file.

Primary context:

- `docs/analysis/2026-06-19-authoritative-pid-format-atlas.md`
- `docs/plans/2026-06-19-pid-parser-roadmap-gates.md`
- `docs/analysis/2026-06-19-ida-evidence-baseline.md`
- `docs/analysis/2026-06-22-phase33-0010-discriminator-ida-plan.md`
- `AGENTS.md`
- `findings.md`
- `task_plan.md`

## Constitution Check

Repository-local guardrails:

- Do not promote probe or audit bytes to decoded semantics without bounded
  evidence.
- Do not infer writer support from reader support.
- Do not hide leftover bytes by marking a recognized stream as fully consumed.
- Do not rename positional bytes as business semantics.
- Keep parser changes panic-safe and bounds-checked.
- Keep private fixture tests soft-skippable with explicit messages.

No violations are planned. If a future implementation slice cannot satisfy
these gates, it must remain documentation-only.

## Phase 0: Research And Scope Lock

Status: planned.

Actions:

1. Confirm reachable IDA instances.
2. Determine whether a relevant module is available beyond `sppid.dll` /
   `core.dll`.
3. Reconcile historical Phase 18-20 findings with the current atlas.
4. Select either:
   - positive investigation path if a reader candidate exists; or
   - negative closeout path if only type-table or runtime glue is visible.

Output: `research.md` updated with the selected path and evidence boundary.

## Phase 1: Evidence Model And Contracts

Status: planned.

Actions:

1. Use `data-model.md` as the local evidence contract.
2. Treat `contracts/` as not applicable for this doc-only package because no
   external API or CLI surface is added.
3. Use `quickstart.md` as the runnable validation guide.

Output: evidence entities, test gate definitions, and quickstart commands.

## Phase 2: IDA Evidence Collection

Status: gated by IDA module availability.

Target order:

1. `radsrvitem.dll`
2. `J2DSrv.dll`
3. `style.dll`
4. `jengine.dll` / `XceedRAD.dll`
5. `OLESITE.dll` / `OLECRT.dll`

Required observations:

- module availability and health;
- search anchors and misses;
- candidate functions and addresses;
- decompiled reader or dispatcher sequence;
- whether any candidate reads persisted bytes;
- mapping between native reads and existing fixture payload buckets.

Output: a focused analysis note under `docs/analysis/`.

## Phase 3: Parser Implementation Slice

Status: conditional.

Start this phase only if Phase 2 produces bounded field evidence.

Minimum implementation steps:

1. Add red parser tests for one proven sub-family.
2. Add a narrow decoder helper without changing unrelated record families.
3. Mirror output into model/schema only for proven fields.
4. Update byte-audit traces so decoded, audit/probe, and leftover bytes remain
   distinct.
5. Add panic-safety entries.
6. Update atlas and roadmap confidence only for the proven sub-family.

If evidence proves only a partial envelope, keep the existing audit-only output
and document the closeout.

## Phase 4: Test Plan

Status: required for any implementation slice.

Doc-only validation:

```powershell
cargo fmt --all -- --check
```

Focused parser validation:

```powershell
cargo test --locked --lib parsers::sheet_records::tests::sub_record_0x0010 -- --nocapture
cargo test --locked --test parse_real_files sub_records_0x0010 -- --nocapture
cargo test --locked --test parse_real_files jstyle_override_decoder_emits_audit_records_with_provenance -- --nocapture
```

Safety and public-surface validation:

```powershell
cargo test --locked --test parser_panic_safety -- --nocapture
cargo test --locked --lib schema -- --nocapture
cargo test --locked --lib byte_audit -- --nocapture
```

Full gate for parser changes:

```powershell
cargo build --locked --workspace --all-targets
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo rustdoc --lib --locked -- -W missing-docs
```

## Risks

| Risk | Impact | Mitigation |
|---|---|---|
| Relevant IDA modules unavailable | Cannot recover persisted reader evidence | Produce tooling-gated negative closeout. |
| Reader candidate only proves runtime type identity | No parser confidence increase | Keep `TypedAudit`; record exact boundary. |
| Single-fixture pattern looks convincing | Overfit parser fields | Require cross-fixture ratchets before implementation. |
| Byte-audit ranges overlap existing audit traces | Misleading coverage | Keep decoded/audit/probe/leftover movements explicit. |
| Public DTO churn | Downstream JSON breakage | Add schema tests and keep fields optional/narrow. |

## Completion Report Template

When this plan is executed, report:

- active IDA module and evidence status;
- whether Phase 3 is authorized or blocked;
- generated analysis docs;
- test commands run and outcomes;
- confidence impact on `ROADMAP-0010`.
