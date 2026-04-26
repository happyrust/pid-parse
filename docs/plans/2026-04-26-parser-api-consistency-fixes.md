# Parser API Consistency Fixes Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix the parser/writer API consistency issues found in the current `main` analysis while keeping each PR small and reviewable.

**Architecture:** Start with behavior/reporting correctness before performance work. Contract/documentation fixes should come before semantic behavior changes. Any parser behavior change must preserve writer passthrough expectations unless the API explicitly opts out.

**Tech Stack:** Rust 2021, `cfb`, existing `PidParser` / `PidPackage` / `PidWriter`, existing CLI integration tests, Markdown docs.

---

## Execution Status

- Selected first executable fix: make `pid_writer_validate` count `summary_updates_encoded` as edited summary streams.
- Reason: small surface, clear regression test, low risk, no parser behavior contract change.

### Task 1: Validator edited paths for encoded summary updates

**Files:**
- Modify: `src/bin/pid_writer_validate.rs`
- Modify: `tests/writer_validate_cli.rs`

**Steps:**

1. Add a CLI integration test using a WritePlan that only sets `metadata_updates.summary_updates_encoded`.
2. Assert validator JSON or human output reports summary streams as edited.
3. Update `collect_edited_paths_from_plan` so non-empty `summary_updates_encoded` marks both summary property-set paths as edited, matching `summary_updates` and `summary_deletions`.
4. Run focused validator tests.

**Expected Result:** Encoded summary edits are visible in validator reports.

### Task 2: Contract docs for raw stream edits vs parsed model

**Files:**
- Modify: `src/package.rs`
- Modify: `docs/writer-quickstart.md` or `docs/writer-clsid-and-timestamps.md`

**Steps:**

1. Document that `replace_stream` / `set_xml_tag` mutate raw stream bytes and do not automatically refresh `PidPackage.parsed`.
2. Recommend reparsing written bytes or adding a future full-package `reparse()` helper.
3. Avoid local partial reparse until crossref/layout invalidation is designed.

**Expected Result:** API contract is explicit; callers do not assume `parsed` is live after raw mutation.

### Task 3: `keep_unknown_streams` contract decision

**Files:**
- Modify: `src/api.rs`
- Modify: `src/cfb/reader.rs`
- Add/modify tests after contract is approved.

**Steps:**

1. Decide whether `keep_unknown_streams` controls `PidDocument.unknown_streams`, `PidPackage.streams`, or both.
2. Preserve writer passthrough by default.
3. If raw stream filtering is desired, introduce a more explicit option rather than overloading `keep_unknown_streams`.

**Expected Result:** Option behavior is no longer misleading.

### Task 4: Writer pipeline reuse in validator

**Files:**
- Modify: `src/writer/mod.rs`
- Modify: `src/bin/pid_writer_validate.rs`
- Modify: writer/validator tests.

**Steps:**

1. Extract a public `apply_plan_to_package`-style helper if one does not already exist.
2. Make validator call the same helper as writer paths.
3. Keep report output stable.

**Expected Result:** Validator and writer cannot drift on apply-order semantics.

### Task 5: `populate_sheet_endpoints` failure visibility

**Files:**
- Modify: `src/cfb/reader.rs`
- Modify/add tests around damaged or missing Sheet stream behavior.

**Steps:**

1. Replace silent `if let Ok` skip with warning or structured diagnostic.
2. Preserve default parse success where possible.
3. Expose missing endpoint evidence in report/debug output.

**Expected Result:** Endpoint loss is visible without turning every partial fixture into a hard parse failure.

### Task 6: Documentation drift pass

**Files:**
- Modify: `src/writer/mod.rs`
- Modify: `src/writer/summary_write.rs`
- Modify: any CFB writer docs if present.

**Steps:**

1. Align module comments with current support for CLSID, summary deletions, encoded updates, timestamp/state bits, and experimental sheet patches.
2. Keep this PR documentation-only.

**Expected Result:** Module comments match shipped behavior.

### Task 7: `from_bytes` pure in-memory parse

**Files:**
- Modify: `src/api.rs`
- Modify: `src/cfb/reader.rs`
- Modify tests in `src/api.rs`.

**Steps:**

1. Introduce reader-generic parse entry for `Read + Seek`.
2. Reimplement `PidPackage::from_bytes` with `Cursor<Vec<u8>>`.
3. Keep public API unchanged.

**Expected Result:** `from_bytes` avoids temp files while preserving behavior.

### Task 8: Light parse design

**Files:**
- Future design doc first, then `src/api.rs` / `src/cfb/reader.rs`.

**Steps:**

1. Define which passes light mode skips.
2. Add options rather than surprising existing `parse_file`.
3. Measure memory/time on real fixture before claiming performance wins.

**Expected Result:** Performance work follows a measured design, not an API shortcut.
