# Phase 15 PSM 0x00FA GraphicGroup Records — Final Summary

**Status**: Complete as audit-only decoded group records.  
**Date**: 2026-05-14  
**Scope**: Parser, audit model collection, fixture ratchet, and verification.

---

## 1. Mission

Phase 15 turns PSM `0x00FA` `GraphicGroup` / `GraphicPersist` records
from probe-only evidence into conservative typed records. The milestone
captures stable header fields plus the raw variable tail, while avoiding
unstable child-reference semantics.

This phase does **not** emit normalized geometry entities from group
records and does **not** expose a stable `child_oids` field.

---

## 2. Result

Implemented:

- `SheetGraphicGroupDecoded`
- `decode_graphic_groups`
- `decode_graphic_group_at`
- `DecodedGraphicGroupRecord`
- `SheetGeometry::decoded_graphic_groups`
- Cluster pipeline mapping from raw `Sheet*` bytes to the audit
  collection
- Cross-fixture parser/model ratchet in `tests/parse_real_files.rs`
- Panic-safety matrix coverage in `tests/parser_panic_safety.rs`

The conservative decoder ratchet is **352** records across the current
four-fixture set:

| Fixture | Conservative decoded groups |
|---|---:|
| `DWG-0201GP06-01.pid` | 135 |
| `DWG-0202GP06-01.pid` | 84 |
| `工艺管道及仪表流程-1.pid` | 125 |
| `export-test/publish-data/A01/A01.pid` | 8 |
| **Total** | **352** |

The broad probe count is **353**. The one-record reduction is expected:
the decoder applies stricter header validation. The older Phase 14
count of **250** is now treated as a legacy discovery count because the
exact fixture set and filter rule were not preserved.

---

## 3. Stable Contract

The stable decoded fields are:

- byte range
- PSM type code and type flags
- `bytes_to_follow`
- group `oid`
- `parent_ref`
- `group_kind_word`
- `sub_type_word`
- raw reference payload from byte 18 onward

The variable tail remains raw. Probe output shows candidate geometry OID
offsets are bucket-specific:

- `btf=54`: commonly `+026`
- `btf=66`: commonly `+034`, sometimes `+022`
- `btf=104`: commonly `+034`
- `btf=122`: commonly `+046`, `+058`, `+078`, sometimes `+034`
- `btf=154`: around `+050`
- `btf=44`: often no known geometry OID

This is enough for audit evidence, not enough for stable child OID
semantics.

---

## 4. Verification

Passed:

```powershell
cargo run --release --example probe_psm_0x00fa_shape
cargo test --test parse_real_files graphic_group_decoder_ratchets_fixture_counts_and_header_fields -- --nocapture
cargo test --test parser_panic_safety -- --nocapture
cargo test --lib graphic_group -- --nocapture
cargo build --locked --workspace --all-targets
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo rustdoc --lib --locked -- -W missing-docs
```

The repository wrapper `bash .github/scripts/check-missing-docs.sh` could
not run on this Windows machine because `bash` resolves to the
WindowsApps shim and fails with `REGDB_E_CLASSNOTREG`. The underlying
rustdoc command from the script passed with `current=0`, `baseline=0`.

---

## 5. Remaining Boundaries

Still not claimed:

- child OID list semantics
- style / color / layer semantics
- `0x0010` sub-record decoding
- normalized group-to-geometry association in `geometry.rs`
- any change to `PidGeometryConfidence` or `PidGraphicKind`

Those should stay future work unless a later probe proves the tail
encoding per size/sub-type bucket.
