//! The TEST02 plant's `Export.mdf` read through the vendored `oxidized-mdf`:
//! what the reader hands a Backup Store (plan
//! `docs/plans/2026-10-09-a-plant-backup-becomes-one-backup-store.md`, S1).
//! Skips when the fixture is absent.

use std::collections::BTreeMap;

use oxidized_mdf::{GhostRow, MdfDatabase, ScannedRecord};

const TEST02_MDF: &str = "test-file/backup-test/TEST02_p/extracted/Export.mdf";
const PAGE_SIZE: usize = 8192;

/// The four schemas of the TEST02 plant: id, name and table count.
const SCHEMAS: [(i32, &str, usize); 4] = [
    (5, "TEST02d", 25),
    (6, "TEST02", 22),
    (7, "TEST02pidd", 25),
    (8, "TEST02pid", 82),
];

/// Column types over those 154 tables.
const COLUMN_TYPES: [(&str, usize); 5] = [
    ("datetime", 27),
    ("float", 263),
    ("image", 2),
    ("int", 650),
    ("nvarchar", 856),
];

/// The TEST02 tables holding a ghost row, and the live rows each keeps.
const GHOST_TABLES: [(&str, usize); 5] = [
    ("T_Equipment", 1),
    ("T_EquipmentOther", 0),
    ("T_PlantItem", 3),
    ("T_SmartFrameStorage", 1),
    ("T_Symbol", 3),
];

fn page(file: &[u8], page_id: u32) -> &[u8] {
    let start = page_id as usize * PAGE_SIZE;
    &file[start..start + PAGE_SIZE]
}

fn u16_at(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

#[test]
fn test02_ghost_rows_are_kept_apart_byte_for_byte() {
    let Ok(file) = std::fs::read(TEST02_MDF) else {
        eprintln!("skip: {TEST02_MDF} is absent");
        return;
    };
    let mut names = MdfDatabase::open(TEST02_MDF)
        .expect("open TEST02")
        .table_names();
    names.sort();
    names.dedup();

    let mut ghosts_by_table: BTreeMap<String, Vec<GhostRow>> = BTreeMap::new();
    for name in &names {
        let mut db = MdfDatabase::open(TEST02_MDF).expect("open TEST02");
        let ghosts = db
            .ghost_rows(name)
            .expect("table listed by table_names")
            .collect::<Result<Vec<_>, _>>()
            .expect("every page of the table reads");
        if !ghosts.is_empty() {
            ghosts_by_table.insert(name.clone(), ghosts);
        }
    }

    assert_eq!(
        GHOST_TABLES
            .iter()
            .map(|(table, _)| ((*table).to_string(), 1))
            .collect::<Vec<_>>(),
        ghosts_by_table
            .iter()
            .map(|(table, ghosts)| (table.clone(), ghosts.len()))
            .collect::<Vec<_>>()
    );
    for (table, ghosts) in &ghosts_by_table {
        let ghost = &ghosts[0];
        let page = page(&file, ghost.page_id);
        let slot_entry = PAGE_SIZE - 2 * (usize::from(ghost.slot) + 1);
        let offset = usize::from(u16_at(page, slot_entry));
        assert_eq!(6, ghost.record_type, "{table}: a ghost data record");
        assert_eq!(
            &page[offset..offset + ghost.bytes.len()],
            ghost.bytes.as_slice(),
            "{table}: the bytes its slot points at"
        );
        assert_eq!(
            1,
            u16_at(page, 58),
            "{table}: the page header's ghost count"
        );
    }

    for (table, live) in GHOST_TABLES {
        let mut db = MdfDatabase::open(TEST02_MDF).expect("open TEST02");
        assert_eq!(
            live,
            db.rows(table).expect("table exists").count(),
            "{table}: live rows"
        );
    }
}

#[test]
fn test02_every_table_scans_to_the_rows_its_catalog_counts() {
    let Ok(file) = std::fs::read(TEST02_MDF) else {
        eprintln!("skip: {TEST02_MDF} is absent");
        return;
    };
    let mut db = MdfDatabase::from_bytes(file).expect("open TEST02");
    let tables = db
        .user_tables()
        .expect("every table names its schema and column types")
        .into_iter()
        .filter(|table| table.schema_name != "sys")
        .collect::<Vec<_>>();

    let mut per_schema: BTreeMap<(i32, String), usize> = BTreeMap::new();
    let mut column_types: BTreeMap<String, usize> = BTreeMap::new();
    for table in &tables {
        *per_schema
            .entry((table.schema_id, table.schema_name.clone()))
            .or_default() += 1;
        for column in &table.columns {
            *column_types.entry(column.type_name.clone()).or_default() += 1;
        }
    }
    assert_eq!(
        SCHEMAS
            .iter()
            .map(|(id, name, count)| ((*id, (*name).to_string()), *count))
            .collect::<BTreeMap<_, _>>(),
        per_schema
    );
    assert_eq!(
        COLUMN_TYPES
            .iter()
            .map(|(name, count)| ((*name).to_string(), *count))
            .collect::<BTreeMap<_, _>>(),
        column_types
    );

    let mut live_total = 0;
    let mut ghost_tables = Vec::new();
    for table in &tables {
        let mut live = 0;
        for record in db.scan_table(table).expect("a heap or B-tree leaf chain") {
            match record.expect("every record reads") {
                ScannedRecord::Live { .. } => live += 1,
                ScannedRecord::Ghost(_) => ghost_tables.push(table.name.clone()),
            }
        }
        assert_eq!(
            table.rcrows, live,
            "{}.{}: live rows against rcrows",
            table.schema_name, table.name
        );
        live_total += live;
    }
    assert_eq!(37_470, live_total);
    ghost_tables.sort();
    assert_eq!(
        GHOST_TABLES
            .iter()
            .map(|(table, _)| (*table).to_string())
            .collect::<Vec<_>>(),
        ghost_tables
    );
}
