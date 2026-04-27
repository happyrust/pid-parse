# Keep Unknown Streams Contract Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make `ParseOptions::keep_unknown_streams` mean one explicit thing: retain unknown-stream diagnostics in the decoded model, without ever dropping package raw streams needed for writer passthrough.

**Architecture:** Preserve `PidPackage.streams` / raw byte retention independently of parser decode options. Populate `PidDocument.unknown_streams` from the already-built stream inventory when the option is enabled; leave it empty when disabled. Keep the existing `JSite.raw_streams` behavior under the same option because it is also a decoded diagnostic view.

**Tech Stack:** Rust 2021, `cfb`, existing `PidParser` / `PidPackage`, inline synthetic CFB unit tests.

---

## Task 1: Characterize Raw Passthrough vs Decoded Diagnostics

**Files:**

- Modify: `src/api.rs`

**Step 1: Write failing tests**

Add tests beside the existing parser API tests:

```rust
#[test]
fn keep_unknown_streams_false_keeps_package_raw_streams() {
    let bytes = build_cfb_bytes_with_unknown_top_level_stream();
    let path = write_temp_pid(&bytes);
    let parser = PidParser::with_options(ParseOptions {
        keep_unknown_streams: false,
        ..ParseOptions::default()
    });

    let pkg = parser.parse_package(&path).expect("parse");

    assert!(pkg.get_stream("/MysteryTopLevel").is_some());
    assert!(pkg.parsed.unknown_streams.is_empty());
}

#[test]
fn keep_unknown_streams_true_populates_unknown_stream_inventory() {
    let bytes = build_cfb_bytes_with_unknown_top_level_stream();
    let path = write_temp_pid(&bytes);

    let pkg = PidParser::new().parse_package(&path).expect("parse");

    assert!(pkg
        .parsed
        .unknown_streams
        .iter()
        .any(|s| s.path == "/MysteryTopLevel"));
}
```

**Step 2: Verify RED**

Run:

```bash
cargo test --lib api::tests::keep_unknown_streams
```

Expected before implementation:

- `keep_unknown_streams_false_keeps_package_raw_streams` passes the raw stream assertion.
- `keep_unknown_streams_true_populates_unknown_stream_inventory` fails because `PidDocument.unknown_streams` is not populated yet.

## Task 2: Populate Unknown Stream Inventory

**Files:**

- Modify: `src/cfb/reader.rs`
- Modify if needed: `src/api.rs`

**Step 1: Add inventory helper**

After `PidDocument` is initialized with `streams`, populate:

```rust
if options.keep_unknown_streams {
    doc.unknown_streams = crate::inspect::unidentified_top_level_streams(&doc)
        .into_iter()
        .map(|stream| crate::model::UnknownStream {
            path: stream.path.clone(),
            size: stream.size,
            magic_u32_le: stream.magic_u32_le,
            magic_tag: stream.magic_u32_le.and_then(crate::parsers::magic::magic_tag),
        })
        .collect();
}
```

**Step 2: Keep raw streams independent**

Do not filter `raw_streams` in `collect_streams_and_bytes`; writer passthrough must remain byte-preserving even when `keep_unknown_streams == false`.

**Step 3: Verify GREEN**

Run:

```bash
cargo test --lib api::tests::keep_unknown_streams
```

Expected:

- Both characterization tests pass.

## Task 3: Documentation And Focused Gates

**Files:**

- Modify: `src/api.rs`
- Modify: `src/model.rs`
- Modify: `CHANGELOG.md` if this ships as a separate commit.

**Step 1: Update docs**

Clarify that `keep_unknown_streams` controls decoded diagnostic retention (`PidDocument.unknown_streams` and embedded `JSite.raw_streams`), not package-side raw stream preservation.

**Step 2: Run focused checks**

Run:

```bash
cargo test --lib api::tests::keep_unknown_streams
cargo clippy --locked --lib -- -D warnings
cargo fmt --all -- --check
```

Expected:

- Focused tests pass.
- No new clippy or rustfmt failures.
