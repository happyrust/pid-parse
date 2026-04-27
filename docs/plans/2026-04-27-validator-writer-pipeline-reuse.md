# Validator Writer Pipeline Reuse Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make `pid_writer_validate --apply-plan` build its expected package through the same writer plan application helper as `PidWriter::write_to` / `write_to_bytes`.

**Architecture:** `src/writer/mod.rs` already has a private `apply_plan_to_package` helper that defines the canonical writer order: metadata updates, stream replacements, then sheet patches. Expose that helper as documented public API, keep `PidWriter::{write_to, write_to_bytes}` using it, and replace the validator's duplicated in-memory plan application with a call to the same helper.

**Tech Stack:** Rust 2021, `pid_parse::writer::{PidWriter, WritePlan}`, existing `tests/writer_validate_cli.rs` CLI fixtures.

---

## Context

This is Task 4 from `docs/plans/2026-04-26-parser-api-consistency-fixes.md`.

Current state:

- `src/writer/mod.rs` has private `apply_plan_to_package(package, plan)`.
- `PidWriter::write_to` and `PidWriter::write_to_bytes` call that helper.
- `src/bin/pid_writer_validate.rs::run_validate_with_plan` duplicates the same order manually when it builds the expected package:
  - `metadata_write::apply_metadata_updates`
  - `package.replace_stream`
  - `sheet_patch::apply_sheet_patch_to_package`
- Any future writer ordering change would need to update both places.

Target state:

- One public helper owns the plan-application order.
- Validator uses the same helper for expected package construction.
- CLI output and edited-path classification remain unchanged.

## Task 1: Public Writer Helper

**Files:**

- Modify: `src/writer/mod.rs`

**Step 1: Make the helper public and documented**

Change:

```rust
fn apply_plan_to_package(package: &mut PidPackage, plan: &WritePlan) -> Result<(), PidError>
```

to:

```rust
pub fn apply_plan_to_package(package: &mut PidPackage, plan: &WritePlan) -> Result<(), PidError>
```

Keep the existing implementation body unchanged. Expand the doc comment to state that this is the canonical in-place application order shared by writer entry points and validation tooling.

**Step 2: Verify no API docs regression**

Run:

```bash
cargo clippy --locked --lib -- -D warnings
```

Expected: no missing-docs or doc markdown regression.

## Task 2: Validator Reuse

**Files:**

- Modify: `src/bin/pid_writer_validate.rs`

**Step 1: Update imports**

Change:

```rust
use pid_parse::writer::{EncodedString, PidWriter, WritePlan};
```

to:

```rust
use pid_parse::writer::{apply_plan_to_package, EncodedString, PidWriter, WritePlan};
```

**Step 2: Replace duplicated apply logic**

Inside `run_validate_with_plan`, replace the manual metadata / stream replacement / sheet patch block with:

```rust
let mut expected = original.clone();
apply_plan_to_package(&mut expected, plan)
    .map_err(|e| ValidateError::Edit(format!("apply_plan: {e}")))?;
```

Keep `PidWriter::write_to(&original, plan, output)` unchanged so the test still compares independently written bytes against the expected package.

## Task 3: Regression Tests

**Files:**

- Modify if needed: `tests/writer_validate_cli.rs`

**Step 1: Reuse existing coverage first**

Run:

```bash
cargo test --test writer_validate_cli validate_apply_plan -- --nocapture
```

Expected: existing apply-plan tests continue to pass, covering passthrough, drawing metadata, stream replacement, invalid JSON, conflicts, and encoded summary edits.

**Step 2: Add a test only if a gap appears**

If existing tests do not cover combined metadata + stream replacement + sheet patch ordering, add one focused test that builds a WritePlan containing multiple edit channels and asserts the JSON report remains `mismatched == 0`.

## Task 4: Verification

Run:

```bash
cargo test --test writer_validate_cli validate_apply_plan -- --nocapture
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

Expected:

- Validator behavior remains stable.
- Writer helper is the single source of truth for plan application order.
- No formatting or clippy regressions.

## Out Of Scope

- Do not change `WritePlan` JSON shape.
- Do not change edited-path classification.
- Do not alter non-`--apply-plan` CLI edit paths in this task.
- Do not commit this plan together with the current dirty Task 3 work unless the user explicitly asks to batch them.
