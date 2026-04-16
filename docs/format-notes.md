# pid-parse format notes

This repository treats a SmartPlant / Smart P&ID `.pid` file as a layered container.

## Layer 1: OLE / CFBF container

The outer file is a Compound File Binary container with storages and streams.
The parser walks the storage tree and records every stream path, size, and a small preview.

## Layer 2: tagged metadata

`/TaggedTxtData/Drawing` and `/TaggedTxtData/General` are treated as document metadata streams.
The initial parser extracts simple XML-like tags from those streams and maps them into `DrawingMeta` and `GeneralMeta`.

## Layer 3: JSite objects

Storages with names starting with `JSite` are treated as object-instance containers.
The parser reads `JProperties`, scans strings, finds probable `.sym` references, and also captures OLE link payloads when present.

## Layer 4: clusters and sheets

The parser indexes:

- `PSMcluster0`
- `StyleCluster`
- `Dynamic Attributes Metadata`
- `Unclustered Dynamic Attributes`
- `Sheet*`

This stage is still mostly structural. It records sizes, string previews, and magic values.

## Layer 5: dynamic attributes

`/Unclustered Dynamic Attributes` is scanned for ASCII and UTF-16LE strings.
The initial parser also extracts relationship-like names and common class names.

## Current status

The repository currently supports:

- container traversal
- stream indexing
- tagged text extraction
- JSite indexing
- cluster indexing
- dynamic-attribute string scanning

It does not yet fully decode binary geometry, object graphs, or precise property-set layouts.
