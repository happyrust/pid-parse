# pid-parse Architecture

## Goal

`pid-parse` is a layered Rust parser for SmartPlant / Smart P&ID `.pid` files.

The project is intentionally organized as a **reverse-engineering friendly parser**, not a one-shot graphical decoder. The first objective is to make every internal stream visible, indexable, and attributable. Only after that do we deepen the binary decoding for objects, styles, geometry, and page layout.

---

## Design Principles

1. **Container first, semantics second**  
   We first parse the OLE/CFBF container and enumerate storages and streams. Semantic interpretation is added on top of that index.

2. **Retain raw provenance**  
   Parsed results should always be traceable back to their original stream path.

3. **Progressive decoding**  
   Unknown blocks are preserved as structured placeholders instead of being discarded.

4. **Stable public API, evolving internals**  
   The public entry point should remain small and predictable even while internal decoders are still being refined.

---

## High-Level Layering

The crate is split into six layers.

### 1. API Layer

Files:

- `src/api.rs`

Responsibilities:

- expose `PidParser`
- expose `ParseOptions`
- define the single public parse entrypoint

This is the only layer application code should call directly.

### 2. Container Layer

Files:

- `src/cfb/mod.rs`
- `src/cfb/tree.rs`
- `src/cfb/reader.rs`

Responsibilities:

- open `.pid` as OLE/CFBF compound file
- build a storage tree
- enumerate stream paths
- collect raw stream metadata such as size and leading magic bytes

This layer does **not** assign domain meaning. It only establishes the file system view of the document.

### 3. Model Layer

Files:

- `src/model.rs`

Responsibilities:

- define `PidDocument`
- define `StorageNode`, `StreamEntry`, `JSite`, `ClusterInfo`, `DynamicAttributesBlob`
- define the stable in-memory representation shared by all parsers

This layer is the contract between the parser internals and downstream tools.

### 4. Parser Utilities Layer

Files:

- `src/parsers/string_scan.rs`
- `src/parsers/drawing_xml.rs`
- `src/parsers/general_xml.rs`
- `src/parsers/jproperties.rs`

Responsibilities:

- scan ASCII and UTF-16LE strings from binary payloads
- parse tagged XML-like metadata blocks
- derive lightweight key-value views from `JProperties`

These utilities are intentionally conservative. They prefer robust extraction over premature precision.

### 5. Stream Semantics Layer

Files:

- `src/streams.rs`
- `src/streams/tagged_text.rs`
- `src/streams/summary.rs`
- `src/streams/jsite.rs`
- `src/streams/cluster.rs`
- `src/streams/dynamic_attrs.rs`

Responsibilities:

- map named streams to semantic handlers
- read drawing/general metadata from `TaggedTxtData`
- index `JSite*` storages and their symbol references
- index cluster streams
- extract dynamic-attribute strings and relationship-like names

This is where raw streams begin to become document concepts.

### 6. Reporting / Inspection Layer

Files:

- `src/inspect.rs`

Responsibilities:

- summarize the parsed document
- provide a quick report for reverse-engineering and debugging

This layer is for humans, not for core parsing.

---

## Core Data Flow

The parsing flow is:

1. `PidParser::parse_file()` receives a path.
2. `cfb::reader::parse_pid_file()` opens the compound file.
3. `cfb::tree::build_tree()` builds the storage tree.
4. `collect_streams()` enumerates streams and produces `StreamEntry` values.
5. Stream handlers enrich `PidDocument` in stages:
   - `summary`
   - `tagged_text`
   - `jsite`
   - `cluster`
   - `dynamic_attrs`
6. The fully assembled `PidDocument` is returned.

This staged enrichment model keeps the parser easy to extend. New semantic decoders can attach additional information to the same document without rewriting the whole pipeline.

---

## Why the Parser Is Not Centered on Geometry Yet

A `.pid` file contains multiple information classes:

- file-level metadata
- drawing/business metadata
- symbol/object instance metadata
- style and cluster information
- page-level graphical payloads
- dynamic attributes and relationships

The current implementation deliberately focuses on the first five steps needed for reliable reverse engineering:

1. open the container
2. enumerate streams
3. identify important stream families
4. extract visible text and keys
5. build stable object indexes

Only after those are dependable should we attempt full geometry reconstruction from streams such as `Sheet*` and `PSMcluster0`.

---

## Current Capability Boundary

The current first version is expected to do these things well:

- read the compound-file structure
- build a storage/stream tree
- collect stream previews
- parse `TaggedTxtData/Drawing`
- parse `TaggedTxtData/General`
- detect `JSite*` storages
- extract symbol file references such as `.sym`
- extract OLE-linked paths such as embedded template references
- detect cluster streams
- scan `Unclustered Dynamic Attributes`

The current first version is **not yet** expected to do these things fully:

- exact OLE property set decoding for summary streams
- binary record-boundary decoding for cluster streams
- exact object graph reconstruction
- exact page geometry reconstruction
- round-trip serialization

---

## Important Internal Types

### `PidDocument`

Top-level aggregate for everything currently known about a file.

### `StorageNode`

Recursive representation of the OLE/CFBF directory tree.

### `StreamEntry`

Flat stream index entry used for lightweight discovery and later semantic dispatch.

### `JSite`

Represents one `JSite*` storage and its immediate known semantics:

- property strings
- candidate symbol path
- OLE links
- additional embedded streams

### `ClusterInfo`

Represents a known cluster stream and basic facts about it:

- path
- size
- leading magic
- extracted strings
- inferred cluster kind

### `DynamicAttributesBlob`

Represents the current best-effort interpretation of the dynamic-attribute stream.

---

## Extensibility Strategy

The intended evolution path is:

### Phase 1: stable document indexing

- complete compile-closure
- add example binaries
- improve `inspect` output
- validate against multiple `.pid` samples

### Phase 2: stronger semantic extraction

- parse summary property sets more accurately
- improve XML metadata matching
- improve `JProperties` key/value extraction
- identify record boundaries in `Unclustered Dynamic Attributes`

### Phase 3: binary structure decoding

- decode `PSMclustertable`
- decode `PSMsegmenttable`
- decode `StyleCluster`
- classify `Sheet*` payload sections

### Phase 4: page/object reconstruction

- map page records to object instances
- recover text placement and object references
- build an analyzable page graph

---

## Public API Intention

The long-term public API should remain small.

```rust
let parser = pid_parse::PidParser::new();
let doc = parser.parse_file("drawing.pid")?;
```

Everything else should build on top of `PidDocument`.

This keeps downstream usage stable while we continue upgrading the internal decoders.

---

## Recommended Immediate Next Steps

1. add `src/inspect.rs`
2. tighten `cfb::reader` to avoid borrow conflicts while iterating streams
3. add `examples/inspect.rs`
4. run `cargo check`
5. validate against real `.pid` samples
6. improve `summary` and `dynamic_attrs` decoding after the first compile-stable run
