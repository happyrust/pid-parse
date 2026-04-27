# From Bytes Pure Memory Parse Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Reimplement `PidPackage::from_bytes` so it parses directly from an in-memory `Read + Seek` source instead of writing a scratch `.pid` file.

**Architecture:** Keep public APIs unchanged and introduce an internal reader-generic parser path in `src/cfb/reader.rs`. Disk parsing remains a thin wrapper around `cfb::open(path)`, while byte parsing uses `std::io::Cursor<Vec<u8>>` and returns a package with `source_path: None`.

**Tech Stack:** Rust 2021, `cfb::CompoundFile<R>`, `std::io::{Read, Seek, Cursor}`, existing `PidParser` / `PidPackage` tests.

---

## Context

This is Task 7 from `docs/plans/2026-04-26-parser-api-consistency-fixes.md`.

Current behavior:

- `PidPackage::from_bytes` writes bytes to a unique temp file.
- It then calls `PidParser::new().parse_package(&scratch)`.
- The parsed package inherits the scratch path as `source_path`.
- Cleanup is best effort and can leave temp files behind on OS or antivirus locks.

Target behavior:

- `PidPackage::from_bytes` parses via `Cursor<Vec<u8>>`.
- No temp path helper is needed for production code.
- Packages created from memory have `source_path == None`.
- `PidParser::parse_file` / `parse_package` keep their current path-based public shape.

## Task 1: Failing Test For In-Memory Source Identity

**Files:**

- Modify: `src/api.rs`

**Step 1: Write the failing test**

Add a focused test beside the existing `from_bytes` tests:

```rust
#[test]
fn from_bytes_marks_package_as_memory_sourced() {
    let bytes = build_minimal_cfb_bytes();
    let pkg = PidPackage::from_bytes(&bytes).expect("parse");

    assert!(
        pkg.source_path.is_none(),
        "from_bytes should parse directly from memory, not expose a scratch file path"
    );
}
```

**Step 2: Verify RED**

Run:

```bash
cargo test --lib api::tests::from_bytes_marks_package_as_memory_sourced
```

Expected before implementation:

- FAIL
- `pkg.source_path` is `Some(<temp>/pid-parse-from-bytes-...)`

## Task 2: Reader-Generic Parser Core

**Files:**

- Modify: `src/cfb/reader.rs`

**Step 1: Extract the core parser**

Create an internal helper:

```rust
fn parse_pid_package_from_cfb<R: Read + std::io::Seek>(
    cfb: &mut ::cfb::CompoundFile<R>,
    source_path: Option<PathBuf>,
    options: &ParseOptions,
) -> Result<PidPackage, PidError>
```

Move the current body of `parse_pid_package` into that helper.

**Step 2: Keep disk wrapper stable**

`parse_pid_package(path, options)` should become:

```rust
let mut cfb = ::cfb::open(path)?;
parse_pid_package_from_cfb(&mut cfb, Some(path.to_path_buf()), options)
```

`parse_pid_file` remains:

```rust
Ok(parse_pid_package(path, options)?.parsed)
```

**Step 3: Add memory wrapper**

Add:

```rust
pub fn parse_pid_package_from_reader<R: Read + std::io::Seek>(
    reader: R,
    options: &ParseOptions,
) -> Result<PidPackage, PidError>
```

It should open `::cfb::CompoundFile::open(reader)?` and pass `source_path: None` to the core helper.

## Task 3: Replace Temp-File `from_bytes`

**Files:**

- Modify: `src/api.rs`

**Step 1: Implement minimal green**

Replace the temp-file implementation with:

```rust
let cursor = std::io::Cursor::new(bytes.to_vec());
crate::cfb::reader::parse_pid_package_from_reader(cursor, &PidParser::new().options)
```

Because `from_bytes` is implemented inside `api.rs`, it can access `PidParser`'s private `options` field.

**Step 2: Remove production temp helper if unused**

Delete `unique_temp_path` only if no tests still need it. If tests need a path helper, keep it under `#[cfg(test)]` or replace test usage with `std::env::temp_dir()` local setup.

## Task 4: Verification

**Files:**

- Modify if needed: `src/api.rs`
- Modify if needed: `src/cfb/reader.rs`

**Step 1: Verify focused tests**

Run:

```bash
cargo test --lib api::tests::from_bytes
cargo test --lib api::tests::from_path_matches_parse_package_behavior
```

Expected:

- Existing `from_bytes` tests still pass.
- New `from_bytes_marks_package_as_memory_sourced` passes.
- Invalid input still returns a non-empty parse error.

**Step 2: Verify formatting / lints for touched code**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --locked --lib -- -D warnings
```

Expected:

- No formatting drift.
- No new warnings.

## Out Of Scope

- No public API shape changes.
- No behavior change for path-based parsing.
- No light-parse or `keep_unknown_streams` semantic changes.
- No attempt to refresh `PidPackage.parsed` after raw stream mutation.
