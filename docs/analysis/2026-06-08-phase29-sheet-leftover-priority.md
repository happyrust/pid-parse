# Phase 29-A Sheet Leftover Priority Report

> Generated from the 2026-06-08 Spec Kit byte-audit snapshots. This report
> ranks Sheet-related leftover regions only; it does not promote any parser
> semantics from `Probed` / `AuditOnly` to `Decoded`.

## Executive Summary

- `Sheet*` remains the highest-value non-IDA parser target because it carries geometry evidence and direct H7CAD impact.
- The dominant registered path is `/Sheet6` across all 6 fixtures.
- Two fixtures also contain `/Sheet6615`, a small registered Sheet-like stream.
- `publish-a01` contains nested `/JSite204/Sheet*` streams that are currently unregistered in byte-audit.
- Next parser work should first produce a Sheet unknown-record inventory from `/Sheet6`, not immediately name fields.

## Aggregate Sheet Paths

| Path | Fixtures | Total bytes | Consumed | Leftover | Ratio | Registered hits | Classification |
|---|---:|---:|---:|---:|---:|---:|---|
| `/Sheet6` | 6 | 129,506 | 8,818 | 120,688 | 0.06808951 | 6 | NeedsParser; IDA helpful for record semantics |
| `/JSite204/Sheet6` | 1 | 6,870 | 0 | 6,870 | 0.00000000 | 0 | NeedsRegistration + NeedsParser |
| `/Sheet6615` | 2 | 632 | 144 | 488 | 0.22784810 | 2 | NeedsParser + NeedsIDA |
| `/JSite204/Sheet12` | 1 | 48 | 0 | 48 | 0.00000000 | 0 | NeedsRegistration + NeedsParser |
| `/JSite204/Sheet22` | 1 | 8 | 0 | 8 | 0.00000000 | 0 | NeedsRegistration + NeedsParser |
| `/JSite204/Sheet32` | 1 | 8 | 0 | 8 | 0.00000000 | 0 | NeedsRegistration + NeedsParser |
| `/JSite204/Sheet41` | 1 | 8 | 0 | 8 | 0.00000000 | 0 | NeedsRegistration + NeedsParser |
| `/JSite204/Sheet51` | 1 | 8 | 0 | 8 | 0.00000000 | 0 | NeedsRegistration + NeedsParser |

## Top Sheet Leftover Items

| Rank | Fixture | Path | Total | Consumed | Leftover | Ratio | Parser | Sample leftover ranges | Reason |
|---:|---|---|---:|---:|---:|---:|---|---|---|
| 1 | `nonascii-process-1` | `/Sheet6` | 46,540 | 2,487 | 44,053 | 0.05343790 | `probe_sheet_stream` | `0..290, 302..311, 319..616, 624..656, 664..712, 720..770, 778..896, 904..1079, +97 more` | registered Sheet stream with high leftover; needs unknown-record inventory |
| 2 | `dwg0201` | `/Sheet6` | 29,594 | 2,195 | 27,399 | 0.07417044 | `probe_sheet_stream` | `0..81, 89..184, 196..221, 247..290, 300..306, 318..409, 417..568, 584..624, +83 more` | registered Sheet stream with high leftover; needs unknown-record inventory |
| 3 | `publish-dwg0202` | `/Sheet6` | 23,739 | 1,704 | 22,035 | 0.07178062 | `probe_sheet_stream` | `0..289, 299..322, 348..403, 413..419, 431..478, 488..531, 539..560, 572..661, +67 more` | registered Sheet stream with high leftover; needs unknown-record inventory |
| 4 | `dwg0202` | `/Sheet6` | 23,731 | 1,704 | 22,027 | 0.07180481 | `probe_sheet_stream` | `0..281, 291..314, 340..395, 405..411, 423..470, 480..523, 531..552, 564..653, +67 more` | registered Sheet stream with high leftover; needs unknown-record inventory |
| 5 | `publish-a01` | `/JSite204/Sheet6` | 6,870 | 0 | 6,870 | 0.00000000 | `(unregistered)` | `-` | nested Sheet-like stream not registered by byte-audit; needs registration decision first |
| 6 | `d06` | `/Sheet6` | 4,122 | 448 | 3,674 | 0.10868511 | `probe_sheet_stream` | `0..453, 465..578, 592..633, 659..708, 716..722, 730..775, 789..864, 872..951, +15 more` | registered Sheet stream with high leftover; needs unknown-record inventory |
| 7 | `publish-a01` | `/Sheet6` | 1,780 | 280 | 1,500 | 0.15730338 | `probe_sheet_stream` | `0..74, 100..186, 212..741, 749..883, 891..908, 934..1095, 1103..1300, 1320..1323, +6 more` | registered Sheet stream with high leftover; needs unknown-record inventory |
| 8 | `dwg0202` | `/Sheet6615` | 316 | 72 | 244 | 0.22784810 | `probe_sheet_stream` | `0..133, 143..173, 183..189, 203..229, 245..249, 259..289, 301..316` | registered Sheet stream with high leftover; needs unknown-record inventory |
| 9 | `publish-dwg0202` | `/Sheet6615` | 316 | 72 | 244 | 0.22784810 | `probe_sheet_stream` | `0..133, 143..173, 183..189, 203..229, 245..249, 259..289, 301..316` | registered Sheet stream with high leftover; needs unknown-record inventory |
| 10 | `publish-a01` | `/JSite204/Sheet12` | 48 | 0 | 48 | 0.00000000 | `(unregistered)` | `-` | nested Sheet-like stream not registered by byte-audit; needs registration decision first |
| 11 | `publish-a01` | `/JSite204/Sheet22` | 8 | 0 | 8 | 0.00000000 | `(unregistered)` | `-` | nested Sheet-like stream not registered by byte-audit; needs registration decision first |
| 12 | `publish-a01` | `/JSite204/Sheet32` | 8 | 0 | 8 | 0.00000000 | `(unregistered)` | `-` | nested Sheet-like stream not registered by byte-audit; needs registration decision first |
| 13 | `publish-a01` | `/JSite204/Sheet41` | 8 | 0 | 8 | 0.00000000 | `(unregistered)` | `-` | nested Sheet-like stream not registered by byte-audit; needs registration decision first |
| 14 | `publish-a01` | `/JSite204/Sheet51` | 8 | 0 | 8 | 0.00000000 | `(unregistered)` | `-` | nested Sheet-like stream not registered by byte-audit; needs registration decision first |

## Proposed 29-A Work Items

1. Build a Sheet leftover extractor that reads byte-audit ranges and emits bounded windows from the source stream bytes.
2. Group `/Sheet6` leftover windows by local record shape: candidate PSM type code, bytes-to-follow, size bucket, and marker bytes when available.
3. Map each group to the current decoder surface: typed, audit-only, probe-only, or unknown.
4. Keep `/JSite204/Sheet*` nested streams separate until byte-audit registration semantics are decided.
5. Write a follow-up report with top unknown record groups and re-open triggers for IDA.

## Guardrails

- Do not infer field names from offset or leftover size alone.
- Do not promote `0x0010`, `GraphicGroup`, page transform, or text placement as part of this slice.
- Do not treat `/JSite*/Sheet*` nested streams as equivalent to top-level `/Sheet*` until stream ownership is understood.
- If a group cannot be tied to a typed or audit-only decoder, classify it as `NeedsIDA` or `NeedsParser`, not `Decoded`.

## Acceptance For Next Slice

A follow-up 29-A implementation slice is complete when it produces a top-10 unknown Sheet record group table with fixture, stream path, bounded byte range, candidate shape, current parser status, and next action.
