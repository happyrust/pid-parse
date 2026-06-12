# Snapshot Priority Backlog

> Regenerated after the Phase 29 Slice L nested registry dispatch: the
> Slice B/C audit-only body-chain walkers run on nested `JSite*` cluster
> twins, and nested `JSite*` registry children (`PSMclustertable`,
> `PSMroots`, `PSMsegmenttable`, `DocVersion2/3`, `AppObject`, summary
> pair) now reuse their top-level parsers. This is a prioritization aid,
> not a semantic proof by itself.

## Fixture Summary

| Fixture id | Coverage entries | Fully | Partial | Identified | Total bytes | Consumed | Leftover | Ratio | Unregistered |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `d06` | 26 | 7 | 6 | 13 | 69,579 | 54,676 | 14,903 | 0.78581180 | 11 |
| `nonascii-process-1` | 25 | 6 | 6 | 13 | 211,094 | 174,374 | 36,720 | 0.82604903 | 11 |
| `dwg0201` | 37 | 7 | 6 | 24 | 223,326 | 198,351 | 24,975 | 0.88816800 | 11 |
| `dwg0202` | 31 | 7 | 6 | 18 | 206,431 | 175,856 | 30,575 | 0.85188760 | 9 |
| `publish-a01` | 22 | 7 | 6 | 9 | 63,211 | 42,051 | 21,160 | 0.66524816 | 14 |
| `publish-dwg0202` | 31 | 7 | 6 | 18 | 206,579 | 175,982 | 30,597 | 0.85188717 | 9 |

## Highest Leftover Families

| Family | Fixtures | Total bytes | Consumed | Leftover | Ratio | Distinct paths | Priority note |
|---|---:|---:|---:|---:|---:|---:|---|
| `JSite*` | 6 | 341,562 | 274,784 | 66,778 | 0.80449230 | 201 | Nested cluster bodies chain-accounted (Slice B/C walkers) and nested registry children reuse top-level parsers (Slice L); remaining leftover is nested `PSMspacemap` pages (IDA-gated), nested `Sheet6` (ownership-gated), StyleCluster prefixes, and small `JSitesList` / OLE payloads |
| `PSMspacemap` | 6 | 62,802 | 0 | 62,802 | 0.00000000 | 4 | IDA / controlled-evidence gated |
| `StyleCluster` | 6 | 83,564 | 71,264 | 12,300 | 0.85281221 | 1 | Tail chain walker shipped (Phase 29 Slice B follow-up); remaining leftover is the GUID-table-like prefix (`2026-06-08-phase29-stylecluster-leftover-triage.md`) |
| `Sheet*` | 6 | 130,138 | 113,393 | 16,745 | 0.87132890 | 2 | After trace integration: review residual groups only |
| `DocumentSummaryInformation` | 6 | 2,576 | 2,041 | 535 | 0.79231366 | 1 | Review if semantically important |
| `SummaryInformation` | 6 | 1,800 | 1,706 | 94 | 0.94777778 | 1 | Review if semantically important |
| `Dynamic Attributes Metadata` | 6 | 168 | 96 | 72 | 0.57142857 | 1 | Review if semantically important |
| `PSMroots` | 6 | 1,580 | 1,556 | 24 | 0.98481013 | 1 | Review if semantically important |
| `Unclustered Dynamic Attributes` | 6 | 143,061 | 143,061 | 0 | 1.00000000 | 1 | Fully accounted by the Phase 29 Slice C audit-only body-chain walker (8-byte prologue + end-anchored `0x0089` chain) plus the `Decoded` landmark scanner; record semantics still IDA-gated (`2026-06-08-phase29-dynamic-attributes-body-backlog.md`) |
| `PSMcluster0` | 6 | 193,983 | 193,983 | 0 | 1.00000000 | 1 | Fully accounted by the Phase 29 Slice B audit-only body-chain walker; record semantics still IDA-gated (`2026-06-08-phase29-psmcluster0-leftover-triage.md`) |
| `AppObject` | 6 | 4,040 | 4,040 | 0 | 1.00000000 | 1 | Review if semantically important |
| `DocVersion2` | 6 | 270 | 270 | 0 | 1.00000000 | 1 | Review if semantically important |
| `DocVersion3` | 6 | 1,056 | 1,056 | 0 | 1.00000000 | 1 | Review if semantically important |
| `JTaggedTxtStgList` | 5 | 350 | 350 | 0 | 1.00000000 | 1 | Review if semantically important |
| `PSMclustertable` | 6 | 1,652 | 1,652 | 0 | 1.00000000 | 1 | Review if semantically important |
| `PSMsegmenttable` | 6 | 72 | 72 | 0 | 1.00000000 | 1 | Review if semantically important |
| `TaggedTxtData/*` | 5 | 11,546 | 11,546 | 0 | 1.00000000 | 3 | Review if semantically important |

## Highest Leftover Individual Paths

| Path | Fixtures | Total bytes | Consumed | Leftover | Ratio | Parser |
|---|---:|---:|---:|---:|---:|---|
| `/PSMspacemap/0x00000000` | 6 | 61,360 | 0 | 61,360 | 0.00000000 | `(unregistered)` |
| `/Sheet6` | 6 | 129,506 | 113,217 | 16,289 | 0.87422204 | `probe_sheet_stream` |
| `/StyleCluster` | 6 | 83,564 | 71,264 | 12,300 | 0.85280743 | `parse_style_cluster` |
| `/JSite793/StyleCluster` | 2 | 38,100 | 26,732 | 11,368 | 0.70162730 | `parse_style_cluster` |
| `/JSite793/PSMspacemap/0x00000000` | 2 | 11,364 | 0 | 11,364 | 0.00000000 | `(unregistered)` |
| `/JSite204/Sheet6` | 1 | 6,870 | 16 | 6,854 | 0.00232897 | `parse_nested_jsite_cluster_header` |
| `/JSite329/PSMspacemap/0x00000000` | 1 | 5,146 | 0 | 5,146 | 0.00000000 | `(unregistered)` |
| `/JSite329/StyleCluster` | 1 | 16,944 | 13,610 | 3,334 | 0.80323418 | `parse_style_cluster` |
| `/JSite793/PSMspacemap/0x00006000` | 2 | 3,284 | 0 | 3,284 | 0.00000000 | `(unregistered)` |
| `/JSite145/PSMspacemap/0x00000000` | 1 | 2,450 | 0 | 2,450 | 0.00000000 | `(unregistered)` |
| `/JSite7559/PSMspacemap/0x00000000` | 1 | 2,426 | 0 | 2,426 | 0.00000000 | `(unregistered)` |
| `/JSite204/PSMspacemap/0x00000000` | 1 | 2,114 | 0 | 2,114 | 0.00000000 | `(unregistered)` |
| `/JSite204/StyleCluster` | 1 | 5,301 | 3,433 | 1,868 | 0.64761366 | `parse_style_cluster` |
| `/JSite7559/StyleCluster` | 1 | 8,375 | 6,511 | 1,864 | 0.77743284 | `parse_style_cluster` |
| `/JSite145/StyleCluster` | 1 | 4,485 | 2,811 | 1,674 | 0.62675585 | `parse_style_cluster` |
| `/JSite39/StyleCluster` | 1 | 3,803 | 2,309 | 1,494 | 0.60715225 | `parse_style_cluster` |
| `/PSMspacemap/0x00006000` | 6 | 1,298 | 0 | 1,298 | 0.00000000 | `(unregistered)` |
| `/JSite39/PSMspacemap/0x00000000` | 1 | 1,254 | 0 | 1,254 | 0.00000000 | `(unregistered)` |
| `/JSite23/\x01Ole` | 3 | 1,017 | 0 | 1,017 | 0.00000000 | `(unregistered)` |
| `/JSite396/StyleCluster` | 1 | 2,269 | 1,297 | 972 | 0.57161745 | `parse_style_cluster` |
| `/JSite151/StyleCluster` | 1 | 1,871 | 997 | 874 | 0.53287012 | `parse_style_cluster` |
| `/JSite396/PSMspacemap/0x00000000` | 1 | 860 | 0 | 860 | 0.00000000 | `(unregistered)` |
| `/JSite121/StyleCluster` | 1 | 1,877 | 1,033 | 844 | 0.55034630 | `parse_style_cluster` |
| `/JSite6963/StyleCluster` | 1 | 1,877 | 1,033 | 844 | 0.55034630 | `parse_style_cluster` |
| `/JSite121/PSMspacemap/0x00000000` | 1 | 832 | 0 | 832 | 0.00000000 | `(unregistered)` |
| `/JSite151/PSMspacemap/0x00000000` | 1 | 590 | 0 | 590 | 0.00000000 | `(unregistered)` |
| `/\x05DocumentSummaryInformation` | 6 | 2,576 | 2,041 | 535 | 0.79231366 | `parse_summary_property_set` |
| `/Sheet6615` | 2 | 632 | 176 | 456 | 0.27848101 | `probe_sheet_stream` |
| `/JSite3644/JProperties` | 1 | 466 | 14 | 452 | 0.03004292 | `parse_jproperties` |

All nested `JSite*/PSMcluster0` paths (11 streams), the nested
`JSite204/Unclustered Dynamic Attributes` stream, and the nested
registry children (`DocVersion2/3`, `PSMclustertable`,
`PSMsegmenttable`; `PSMroots` keeps only its top-level 4-byte tail per
stream) now report zero or tail-only leftover and have left this table.
`/JSitesList` is registered by the Slice M parser (24 stale-tail bytes
leftover across `dwg0202` / `publish-dwg0202`), and the 0-byte
`/TaggedTxtData/Revision` placeholder is registered with zero claims.

## Common Unregistered Paths

After the Phase 29 Slice L nested registry dispatch and the Slice M
`JSitesList` / `Revision` closeout, 38 distinct unregistered paths
remain; everything with fixture count ≥ 2:

| Path | Fixture count |
|---|---:|
| `/PSMspacemap/0x00000000` | 6 |
| `/PSMspacemap/0x00002000` | 6 |
| `/PSMspacemap/0x00004000` | 6 |
| `/PSMspacemap/0x00006000` | 6 |
| `/JSite23/\x01Ole` | 3 |
| `/JSite793/PSMspacemap/0x00000000` | 2 |
| `/JSite793/PSMspacemap/0x00002000` | 2 |
| `/JSite793/PSMspacemap/0x00004000` | 2 |
| `/JSite793/PSMspacemap/0x00006000` | 2 |

The count-1 tail is nested per-JSite `PSMspacemap` pages and
`\x01Ole` payloads of the same two families — all IDA / demand gated.

## Recommended Backlog

1. `JSite*`: nested cluster bodies chain-accounted and nested registry children parser-registered (leftover 325,843 → 66,778, ratio 0.8045); the remainder is nested `PSMspacemap` pages (IDA-gated), nested `Sheet6` (ownership-gated), StyleCluster prefixes (IDA-gated), and small `JSitesList` / OLE payloads (demand-gated, no parser exists yet).
2. `PSMcluster0`: **done** — body-chain walker ships in Phase 29 Slice B; remaining work (record-type naming) is IDA-gated.
3. `StyleCluster`: **tail chain done; prefix closed out** — the remaining 12,300-byte prefix is precisely mapped (12-byte opener + 532-byte constant boilerplate + 42-byte style slots, no uniform GUID stride) and stays leftover pending IDA (`2026-06-08-phase29-stylecluster-prefix-characterization.md`).
4. `Unclustered Dynamic Attributes`: **done** — Phase 29 Slice C proved the same cluster-family record chain as `/PSMcluster0` (6/6 end-anchored, `0x0089`-typed, 31-byte "trailer" = next record's envelope head) and the audit-only walker now ships (`parse_unclustered_da`); leftover 111,120 → 0. Remaining work (record-type naming, prologue counter semantics) is IDA-gated.
5. `PSMspacemap`: still IDA / controlled-evidence gated.
6. `Sheet*`: after trace integration and header tracing, review only residual groups in the after-trace report.
