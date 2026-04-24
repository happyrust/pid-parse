//! Integration test: apply [`scan_sysschobjs_rows`] to the MDF data
//! page we reverse-engineered as the primary sysschobjs data page
//! (#2173 in the TEST02 fixture) and assert that the SmartPlant
//! `T_*` tables are recoverable by name.
//!
//! # Scope (post-pivot)
//!
//! Stage-1 originally aimed at a fully-self-hosted row decoder — we
//! wrote this scanner to pull the `object_id ↔ name` mapping out of
//! `sysschobjs` without any SQL Server involvement. After the pivot
//! to the [OrcaMDF wrapper path](../tools/orca-mdf-probe), the
//! canonical way to recover that mapping is to run
//! `OrcaMdfProbe --export sysschobjs` (or any other DMV call) on
//! the extracted `.mdf`. This scanner remains in the tree as a
//! **lightweight Rust-side sanity check**: if even the byte-level
//! name recovery stops working, the upstream MTF / MDF layers are
//! silently broken.
//!
//! Accordingly this test now asserts **name** recovery only. The
//! object_id bytes require a deeper row-format decoder that stage-1
//! no longer intends to ship; see the module docs in
//! `src/backup/syscatalog.rs` for details.

use pid_parse::backup::mdf_page::PAGE_SIZE;
use pid_parse::backup::{scan_sysschobjs_rows, SysschobjsRow};
use std::path::Path;

const MSDA_PATH: &str = "test-file/backup-test/TEST02_p/extracted/Export.msda.bin";
const MDF_BASE: usize = 0x3F0;
const SYSSCHOBJS_PAGE_INDEX: usize = 2173;

fn load_msda() -> Option<Vec<u8>> {
    let p = Path::new(MSDA_PATH);
    if !p.exists() {
        eprintln!("skipping: fixture {MSDA_PATH} not found");
        return None;
    }
    Some(std::fs::read(p).unwrap_or_else(|e| panic!("read {MSDA_PATH}: {e}")))
}

#[test]
fn real_page_2173_exposes_sppid_user_tables() {
    let Some(data) = load_msda() else {
        return;
    };
    let page_offset = MDF_BASE + SYSSCHOBJS_PAGE_INDEX * PAGE_SIZE;
    let page = &data[page_offset..page_offset + PAGE_SIZE];

    let rows = scan_sysschobjs_rows(page);
    assert!(
        rows.len() >= 20,
        "expected many sysschobjs rows on page #2173, got {}",
        rows.len()
    );

    // Collect names into a set so assertions are order-independent.
    let names: std::collections::BTreeSet<String> = rows.iter().map(|r| r.name.clone()).collect();

    // The SmartPlant TEST02 schema spans several sysschobjs
    // pages. Page #2173 is empirically known to hold ~39 user
    // tables including `T_ModelItem` and `T_Equipment` — we
    // assert on those as a minimal contract. Cross-page
    // aggregation lands in a later integration test.
    let expected_on_page_2173 = ["T_ModelItem", "T_Equipment"];
    for want in expected_on_page_2173 {
        assert!(
            names.contains(want),
            "scanner missed `{want}`; found {} names: {:?}",
            names.len(),
            names.iter().collect::<Vec<_>>()
        );
    }

    // 20+ `T_*` rows is the observed floor for this fixture; a
    // regression that drops below that is a real bug.
    let t_count = names.iter().filter(|n| n.starts_with("T_")).count();
    assert!(
        t_count >= 20,
        "expected at least 20 `T_*` rows on the catalog page; got {t_count}"
    );

    // Diagnostic echo when -- --nocapture is passed.
    eprintln!("sysschobjs rows recovered from page #2173:");
    for row in rows.iter().take(30) {
        eprintln!(
            "  obj_id=0x{:08X} ({:>6})  name=`{}`  marker@0x{:04X}",
            row.object_id, row.object_id, row.name, row.marker_offset
        );
    }
    if rows.len() > 30 {
        eprintln!("  ... ({} more)", rows.len() - 30);
    }

    // Find the T_ModelItem row by name and do a round-trip Eq /
    // clone check so the struct's derived traits don't silently
    // regress. The `object_id` field is NOT asserted because the
    // 4-byte leading window in sysschobjs rows is not uniformly
    // `object_id` on every SQL Server version — see module docs
    // for why stage-1 no longer decodes it.
    let modelitem = rows
        .iter()
        .find(|r| r.name == "T_ModelItem")
        .expect("T_ModelItem row should be present by virtue of the loop above");
    assert_eq!(*modelitem, modelitem.clone());
    let _ = SysschobjsRow {
        marker_offset: modelitem.marker_offset,
        object_id: modelitem.object_id,
        name: modelitem.name.clone(),
    };
}
