# PID real sheet layers and JSite geometry closeout

## Status ledger

| Work item | Status | Evidence |
|---|---|---|
| Foundation | complete | `814c8da` isolates aux-hi/boundary evidence |
| W1 JSheetLayer | complete | `4a6c302`; 290 layers, manager registration 290/290 |
| W2 entity layer identity | complete | `3a93681`; storage-local oid/name on normalized entities |
| W3 OCS presentation | complete | OCS `19e69888`; XDATA, properties, summary, `PID-HIDDEN`, DWG/DXF round-trip |
| W5 probe and symbol polyline | complete | pid-parse `af8e802`, OCS `6e1f40d3`; 618 symbols, 31 polylines, 11 closed |
| W4 JSite geometry | evidence-backed gap | `87c7225`; admitted OLE Sheet content stays enabled, untransformed LdcSite chains stay closed |
| W6 ledger | complete | this document; unrelated automation failures tracked as OpenCADStudio issue #941 |

## Verification

- `cargo test --lib --test parse_real_files`: 1198 passed.
- OCS `cargo test --test pid_import --test pid_panel_localization`: 33 passed.
- OCS focused closure test: 1 passed.
- `.sym` evidence: 618 files; `form=1/open` 20, `form=2/closed` 11, no counterexample.
- W4 transcripts: `w4-jsite-ownership.txt`, `w4-jsite-page-transform.txt`,
  `w4-tag181-parent-chain.txt` in the acceptance bundle.
- Existing non-PID automation baseline: 10 passed, 3 failed; issue
  <https://github.com/HakanSeven12/OpenCADStudio/issues/941> owns the three
  named failures independently.

## Rollback order

Use `git revert`, never workspace cleanup:

1. OCS `6e1f40d3`, then `19e69888`.
2. pid-parse `87c7225`, `af8e802`, `3a93681`, `4a6c302`, `814c8da`.

The original fixture hashes and pre-change patches are in the acceptance
bundle. Unrelated dirty files and the separate OCS merge-in-progress were not
included in any commit.
