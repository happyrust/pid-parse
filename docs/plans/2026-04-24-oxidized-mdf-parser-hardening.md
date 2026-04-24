# Oxidized MDF Parser Hardening Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make the vendored `oxidized-mdf` parser safer and more complete for the current publish pipeline, starting with the smallest parser gaps that still contain hard failures.

**Architecture:** Keep the existing `publish::mdf_load -> in-memory SQLite -> sqlite_load` pipeline intact. Improve the vendored parser in narrow, test-first slices so we remove known `todo!()` / panic edges without widening the loader contract or attempting a full general-purpose MDF reconstruction project in one step.

**Tech Stack:** Rust, vendored `oxidized-mdf`, `rust_decimal`, existing `cargo test` suites in both the vendored crate and the parent crate.

---

### Task 1: Support `decimal(20..=28)` row decoding

**Files:**
- Modify: `vendor/oxidized-mdf/src/pages.rs`
- Test: `vendor/oxidized-mdf/src/pages.rs`

**Step 1: Write the failing test**

Add a unit test beside the existing `parse_decimal` cases for a `precision <= 28` value that uses the 12-byte SQL Server decimal payload path.

Expected fixture:
- sign byte = `0x01`
- payload bytes = `[121, 223, 226, 61, 68, 166, 54, 15, 110, 5, 1, 0]`
- precision = `25`
- scale = `4`
- expected decimal = `123456789012345678901.2345`

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test parse_decimal --lib
```

Expected: FAIL because `parse_decimal_opt()` currently hits `todo!()` for `precision <= 28`.

**Step 3: Write minimal implementation**

Implement the `precision <= 28` branch by decoding the 12-byte little-endian integer payload into a positive `i128`, then applying the existing sign-byte + scale handling.

Constraints:
- keep the existing storage-width rules unchanged
- do not redesign the decimal API
- keep behavior identical for existing `<= 19` and `> 28` branches

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test parse_decimal --lib
```

Expected: PASS for both existing decimal cases and the new `precision <= 28` case.

**Step 5: Run broader safety net**

Run:

```bash
cargo test
```

inside `vendor/oxidized-mdf`.

Expected: all vendored parser tests remain green.

### Task 2: Replace parser panic edges with recoverable failures

**Files:**
- Modify: `vendor/oxidized-mdf/src/pages.rs`
- Modify: `vendor/oxidized-mdf/src/lib.rs`
- Possibly modify: `vendor/oxidized-mdf/src/error.rs`
- Test: `vendor/oxidized-mdf/src/pages.rs`

**Step 1: Write failing tests**

Add focused tests for:
- unknown record-type bits in `Record::try_from`
- any path where row parsing should return an error instead of panicking the whole stream

**Step 2: Verify RED**

Run only the new tests and confirm the current behavior is a panic / wrong failure mode.

**Step 3: Implement minimal recovery**

Convert the panic edge into an error that callers can surface or skip explicitly, instead of crashing the parser.

**Step 4: Verify GREEN**

Run the targeted tests, then full vendored parser tests.

### Task 3: Tighten parent-project verification for the MDF reader contract

**Files:**
- Modify: `tests/publish_mdf_load.rs`
- Possibly modify: `tests/publish_a01_raw_residual.rs`

**Step 1: Add one focused regression gate**

Pick one publish-relevant field that depends on the newly hardened parser path and assert it still stages correctly through `open_mdf_as_sqlite`.

**Step 2: Verify RED / GREEN**

Run the targeted parent test before and after the implementation if behavior changes.

**Step 3: Final verification**

Run:

```bash
cargo test --test publish_mdf_load -- --nocapture
cargo test --test publish_xml_cli -- --nocapture
```

Expected: publish path remains green after vendored parser changes.
