//! Publish reads a Backup Store (S4 of plan
//! `docs/plans/2026-10-09-a-plant-backup-becomes-one-backup-store.md`,
//! acceptance 10): the A01 drawing publishes to the same bytes from
//! every input -- the `Export.mdf` on its own, the `TEST02_p.zip`,
//! the directory it unpacks to and a Backup Store file written from
//! the zip -- and those bytes are the ones the MDF adapter produced
//! before S4. Skips when the TEST02 fixtures are absent.

mod common;

use std::path::Path;

use common::backup_store::scratch_dir;
use common::{A01_DRAWING_UID, A01_MDF_PATH, PLANT_NAME, SQLITE_PATH};
use pid_parse::backup::{build_backup_store, StoreOptions};
use pid_parse::publish::{
    classify_publish_input, load_drawing_graph, open_mdf_as_sqlite, open_publish_input,
    write_data_xml, write_meta_xml, PublishInput, PUBLISH_TABLES,
};
use rusqlite::types::Value as SqlValue;
use rusqlite::Connection;
use sha2::{Digest, Sha256};

const TEST02_ZIP: &str = "test-file/backup-test/TEST02_p.zip";
const TEST02_DIR: &str = "test-file/backup-test/TEST02_p";

/// A01's `_Data.xml` and `_Meta.xml` as the MDF adapter wrote them
/// before S4 (S1c: 8,482 and 1,481 bytes). A change here is a change
/// in what publish says about the drawing: analyse it before
/// accepting it (Q13), then update these two lines.
const A01_DATA_XML_SHA256: &str =
    "6ab41b663372e3c264b5f51994d2419c6054c7b7fc1c84d253a55175c002b0b2";
const A01_META_XML_SHA256: &str =
    "44291a6cbed4fced0193f7ac615317da0d7ab689cfea0997aec8f9314fbb1cac";

fn fixtures_present() -> bool {
    for path in [A01_MDF_PATH, TEST02_ZIP] {
        if !Path::new(path).exists() {
            eprintln!("skipping: fixture {path} not found");
            return false;
        }
    }
    true
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A01's two documents from an already opened publish connection.
fn publish_a01(conn: &Connection) -> (String, String) {
    let drawing = load_drawing_graph(conn, A01_DRAWING_UID).expect("load A01");
    (
        write_data_xml(&drawing, PLANT_NAME).expect("data xml"),
        write_meta_xml(&drawing, PLANT_NAME).expect("meta xml"),
    )
}

/// One staged table: its name, its column names, its rows as text.
type StagedTable = (String, Vec<String>, Vec<Vec<Option<String>>>);

/// Every publish table the connection holds, row by row, as text.
fn staged_tables(conn: &Connection) -> Vec<StagedTable> {
    let mut tables = Vec::new();
    for table in PUBLISH_TABLES {
        let quoted = format!("\"{table}\"");
        let mut stmt = match conn.prepare(&format!("SELECT * FROM {quoted} ORDER BY rowid")) {
            Ok(stmt) => stmt,
            Err(_) => continue, // not staged: the store lacks it
        };
        let columns: Vec<String> = stmt
            .column_names()
            .iter()
            .map(ToString::to_string)
            .collect();
        let rows = stmt
            .query_map([], |row| {
                (0..columns.len())
                    .map(|i| {
                        Ok(match row.get::<_, SqlValue>(i)? {
                            SqlValue::Null => None,
                            SqlValue::Text(text) => Some(text),
                            other => panic!("{table}: a staged column is not TEXT: {other:?}"),
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .expect("query")
            .collect::<Result<Vec<_>, _>>()
            .expect("rows");
        tables.push((table.to_string(), columns, rows));
    }
    tables
}

#[test]
fn a01_publishes_to_the_same_bytes_from_the_mdf_the_zip_the_directory_and_a_store_file() {
    if !fixtures_present() {
        return;
    }
    let from_mdf = publish_a01(&open_mdf_as_sqlite(Path::new(A01_MDF_PATH)).expect("MDF"));
    assert_eq!(8482, from_mdf.0.len(), "_Data.xml bytes");
    assert_eq!(1481, from_mdf.1.len(), "_Meta.xml bytes");
    assert_eq!(
        A01_DATA_XML_SHA256,
        sha256_hex(from_mdf.0.as_bytes()),
        "A01 _Data.xml is no longer what the MDF adapter wrote before S4 (Q13)"
    );
    assert_eq!(
        A01_META_XML_SHA256,
        sha256_hex(from_mdf.1.as_bytes()),
        "A01 _Meta.xml is no longer what the MDF adapter wrote before S4 (Q13)"
    );

    let zip = Path::new(TEST02_ZIP);
    assert_eq!(PublishInput::PlantBackup, classify_publish_input(zip));
    let from_zip = publish_a01(&open_publish_input(zip).expect("zip"));
    assert_eq!(
        from_mdf, from_zip,
        "the zip's store publishes other bytes than the MDF"
    );

    let dir = scratch_dir("publish_store_parity");
    let store_file = dir.join("TEST02.sqlite");
    let _ = std::fs::remove_file(&store_file);
    build_backup_store(zip, &store_file, &StoreOptions::default()).expect("write the store");
    assert_eq!(
        PublishInput::BackupStore,
        classify_publish_input(&store_file)
    );
    let from_store = publish_a01(&open_publish_input(&store_file).expect("store file"));
    assert_eq!(
        from_mdf, from_store,
        "the store file publishes other bytes than the MDF"
    );
    let _ = std::fs::remove_dir_all(&dir);

    let directory = Path::new(TEST02_DIR);
    if directory.is_dir() {
        assert_eq!(PublishInput::PlantBackup, classify_publish_input(directory));
        let from_dir = publish_a01(&open_publish_input(directory).expect("directory"));
        assert_eq!(
            from_mdf, from_dir,
            "the directory's store publishes other bytes than the MDF"
        );
    } else {
        eprintln!("skipping the directory input: {TEST02_DIR} is not checked out");
    }

    let mdf = Path::new(A01_MDF_PATH);
    assert_eq!(PublishInput::Mdf, classify_publish_input(mdf));
    if Path::new(SQLITE_PATH).exists() {
        assert_eq!(
            PublishInput::LegacySqlite,
            classify_publish_input(Path::new(SQLITE_PATH)),
            "the legacy mirror has no dump_table and stays a legacy input"
        );
    }
}

#[test]
fn the_staged_tables_from_the_zip_equal_those_from_the_mdf_row_for_row() {
    if !fixtures_present() {
        return;
    }
    let from_mdf = staged_tables(&open_mdf_as_sqlite(Path::new(A01_MDF_PATH)).expect("MDF"));
    let from_zip = staged_tables(&open_publish_input(Path::new(TEST02_ZIP)).expect("zip"));
    assert_eq!(
        from_mdf.len(),
        from_zip.len(),
        "a different number of tables staged"
    );
    for ((mdf_name, mdf_columns, mdf_rows), (zip_name, zip_columns, zip_rows)) in
        from_mdf.iter().zip(&from_zip)
    {
        assert_eq!(mdf_name, zip_name);
        assert_eq!(mdf_columns, zip_columns, "{mdf_name}: columns");
        assert_eq!(mdf_rows, zip_rows, "{mdf_name}: rows");
    }
    // TEST02 has no T_ProcessEquipment; the other 23 are staged.
    assert_eq!(23, from_mdf.len());

    // Q18: the dictionary tables come from the P&ID dictionary (pidd:
    // 127 codelists in 3,206 rows, 798 attributes), no longer from
    // the plant dictionary (13 in 130, 80); the business tables from
    // the P&ID schema, live rows only (publish_mdf_load pins them).
    let rows_of = |name: &str| {
        from_mdf
            .iter()
            .find(|(table, _, _)| table == name)
            .map(|(_, _, rows)| rows.len())
            .unwrap_or_else(|| panic!("{name} not staged"))
    };
    assert_eq!(3206, rows_of("codelists"));
    assert_eq!(798, rows_of("attributes"));
    assert_eq!(1, rows_of("T_Drawing"));
    assert_eq!(3, rows_of("T_PlantItem"));

    // The copy carries the source columns only: the store's
    // provenance columns stay behind.
    for (table, columns, _) in &from_mdf {
        assert!(
            !columns
                .iter()
                .any(|column| column == "_src_page" || column == "_src_slot"),
            "{table}: the provenance columns leaked into the publish tables"
        );
    }
    let codelist_columns: Vec<&str> = from_mdf
        .iter()
        .find(|(table, _, _)| table == "codelists")
        .map(|(_, columns, _)| columns.iter().map(String::as_str).collect())
        .unwrap();
    assert_eq!(
        vec![
            "codelist_number",
            "codelist_index",
            "codelist_text",
            "codelist_short_text",
            "codelist_constraint",
            "codelist_sort_value",
            "codelist_entry_disabled",
        ],
        codelist_columns
    );
}
