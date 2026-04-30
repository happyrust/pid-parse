# PSMclustertable Record Evidence Matrix

> Date: 2026-05-06 plan follow-up
> Upstream plan: `docs/plans/2026-05-06-phase-11a-psmclustertable-records.md`

This note captures the first Phase 11a evidence pass for naming
`PSMclustertable` per-record fields. It is intentionally conservative:
values below are observed probe facts, not decoded field names yet.

---

## 1. Preconditions observed

Command context:

```bash
git status --short
```

Result: working tree is not clean. W1 artifacts are still uncommitted
(`docs/baselines/`, W1 plans, `CHANGELOG.md`, byte-audit docs / runner).
Therefore Phase 11a parser/model implementation should not start yet.

Evidence collection is still useful and safe because it only reads fixtures via
`pid_inspect --json`.

---

## 2. Fixtures sampled

Command shape used for each fixture:

```bash
cargo run --quiet --locked --bin pid_inspect -- <fixture.pid> --json
```

Sampled fixtures:

| Fixture | PSMclustertable size | Declared count | Parsed entries | Trailing bytes | PSMsegmenttable count |
|---|---:|---:|---:|---:|---:|
| `test-file/DWG-0201GP06-01.pid` | 265 | 5 | 5 | 0 | 4 |
| `test-file/DWG-0202GP06-01.pid` | 296 | 6 | 6 | 0 | 4 |
| `test-file/工艺管道及仪表流程-1.pid` | 265 | 5 | 5 | 0 | 4 |

Initial finding:

- DWG-0201 and the Chinese sample are byte-identical for all five observed
  `PSMclustertable` records.
- DWG-0202 shares those same first five records and adds one extra sheet
  record: `Sheet6615`.
- All sampled tables have `trailing_bytes = 0`, so the scanner attributes the
  entire stream to records.
- `PSMsegmenttable.count = 4` in all three fixtures, while cluster count is
  5/6. This invalidates a simple one-cluster-one-segment mapping.

---

## 3. Record matrix

### 3.1 Shared first five records

These five rows are identical across all three sampled fixtures.

| idx | name | record_offset | record_len | prefix_len | prefix_hex | first_u32_le | last_u32_le |
|---:|---|---:|---:|---:|---|---:|---:|
| 0 | `PSMcluster0` | 8 | 43 | 19 | `10 00 00 00 18 00 00 00 01 00 00 01 00 00 00 00 00 00 00` | 16 | 48 |
| 1 | `StyleCluster` | 51 | 41 | 15 | `1A 00 00 00 01 01 00 01 00 00 00 01 00 00 00` | 26 | 114 |
| 2 | `Dynamic Attributes Metadata` | 92 | 71 | 15 | `38 00 00 00 01 02 00 01 00 00 00 02 00 00 00` | 56 | 97 |
| 3 | `Sheet6` | 163 | 25 | 11 | `0E 00 00 00 01 03 00 00 00 00 00` | 14 | 54 |
| 4 | `Unclustered Dynamic Attributes` | 188 | 77 | 15 | `3E 00 00 00 01 04 00 01 00 00 00 03 00 00 00` | 62 | 115 |

### 3.2 DWG-0202 extra record

Only `test-file/DWG-0202GP06-01.pid` has this row:

| idx | name | record_offset | record_len | prefix_len | prefix_hex | first_u32_le | last_u32_le |
|---:|---|---:|---:|---:|---|---:|---:|
| 5 | `Sheet6615` | 265 | 31 | 11 | `14 00 00 00 01 05 00 00 00 00 00` | 20 | 53 |

---

## 4. Byte-level observations

### 4.1 Name byte-length candidate

For records 1-5, `prefix[0..4]` equals the UTF-16LE name byte length including
the NUL terminator:

| name | chars | `(chars + 1) * 2` | observed `first_u32_le` |
|---|---:|---:|---:|
| `StyleCluster` | 12 | 26 | 26 |
| `Dynamic Attributes Metadata` | 27 | 56 | 56 |
| `Sheet6` | 6 | 14 | 14 |
| `Unclustered Dynamic Attributes` | 30 | 62 | 62 |
| `Sheet6615` | 9 | 20 | 20 |

Exception: record 0 (`PSMcluster0`) has `first_u32_le = 16`, while
`prefix[4..8] = 24`, which matches `(11 + 1) * 2`. Record 0 likely has an
extra leading field before the name-length slot, so field naming must handle
record 0 separately.

### 4.2 Cluster ordinal candidate

For records 1-5, bytes after the first length slot include an apparent ordinal:

| idx | prefix segment | candidate ordinal |
|---:|---|---:|
| 1 | `... 01 01 00 ...` | 1 |
| 2 | `... 01 02 00 ...` | 2 |
| 3 | `... 01 03 00 ...` | 3 |
| 4 | `... 01 04 00 ...` | 4 |
| 5 | `... 01 05 00 ...` | 5 |

Record 0 again appears special: `... 01 00 00 01 ...` can still encode ordinal
0, but the surrounding layout is longer and should not be forced into the
same fixed prefix format.

### 4.3 Sheet vs non-sheet flag candidate

Non-sheet rows have a longer 15-byte prefix and contain `... 01 00 00 00
<n> 00 00 00` near the tail. Sheet rows (`Sheet6`, `Sheet6615`) have an
11-byte prefix ending in `00 00 00 00`.

Observed split:

| name | prefix_len | tail pattern | likely class |
|---|---:|---|---|
| `StyleCluster` | 15 | final u32 = 1 | non-sheet cluster |
| `Dynamic Attributes Metadata` | 15 | final u32 = 2 | non-sheet cluster |
| `Unclustered Dynamic Attributes` | 15 | final u32 = 3 | non-sheet cluster |
| `Sheet6` | 11 | no final u32 | sheet cluster |
| `Sheet6615` | 11 | no final u32 | sheet cluster |

This is promising but not yet a decoded field. The final u32 may be an ordinal,
owner id, non-sheet payload index, or some other SmartPlant-internal counter.

### 4.4 Segment count is not directly proven

All sampled fixtures have `PSMsegmenttable.count = 4`, but cluster counts are
5 and 6. No observed prefix field trivially sums to 4:

- Non-sheet final u32 values are `1, 2, 3`, which sum to 6.
- Sheet rows lack that final u32.
- The first record carries additional leading fields.

Do not name any prefix field `declared_segment_count` in the next code pass.
That name was a useful older hypothesis, but this evidence does not support it.

---

## 5. Candidate layout to test

The next red tests should treat this as a hypothesis only:

```text
record 0 special prefix:
  u32 unknown_a = 16
  u32 name_bytes_with_nul = 24
  bytes unknown_b = 01 00 00 01 00 00 00 00 00 00 00

records 1..N common prefix:
  u32 name_bytes_with_nul
  u8  constant_01
  u16 candidate_ordinal
  u8  candidate_non_sheet_marker
  [u8; 3] zero padding
  optional u32 candidate_non_sheet_payload_index
```

For sheet records, the optional final u32 is absent and
`candidate_non_sheet_marker = 0`.

For non-sheet records, the optional final u32 is present and
`candidate_non_sheet_marker = 1`.

---

## 6. Implementation guidance

Allowed names for the first parser pass:

- `name_bytes_with_nul`
- `candidate_ordinal`
- `candidate_non_sheet_marker`
- `candidate_non_sheet_payload_index`
- `unknown_prefix_bytes`

Names to avoid for now:

- `cluster_id`
- `type_tag`
- `declared_segment_count`
- `segment_count`
- `flags` unless the exact bit semantics are proven

Reason: the evidence supports layout stability and some structural candidates,
but not final SmartPlant semantics.

---

## 7. Blockers and next actions

Blockers before parser/model implementation:

- W1 worktree is still dirty; Phase 11a code changes should wait until W1 is
  committed or moved aside.
- Schema snapshot / byte-audit baseline gates may need updates after any new
  public decoded fields.

Next actions once W1 is clean:

1. Add red parser tests for `name_bytes_with_nul` and `candidate_ordinal`.
2. Add record-0 special-case test to avoid forcing all prefixes into one shape.
3. Add real fixture tests covering DWG-0201, DWG-0202, and the Chinese sample.
4. Add additive decoded view with conservative field names only.
5. Keep `PSMclustertable` coverage as `PartiallyDecoded` unless every prefix
   byte is explained and field names are promoted from `candidate_*`.
