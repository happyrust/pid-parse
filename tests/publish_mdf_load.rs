//! Rust MDF loader gates for the publish XML pipeline.

use std::path::Path;
use std::sync::{Mutex, PoisonError};

use oxidized_mdf::MdfDatabase;
use pid_parse::publish::mdf_load::PUBLISH_TABLES;
use pid_parse::publish::{mdf_bytes_from_mtf_dump, open_mdf_as_sqlite, open_mtf_dump_as_sqlite};

const A01_MDF_PATH: &str = "test-file/backup-test/TEST02_p/extracted/Export.mdf";

/// The package's database as the package keeps it: an MTF dump with the
/// image of [`A01_MDF_PATH`] inside.
const TEST02_DUMP_PATH: &str = "test-file/backup-test/TEST02_p/Export.dmp";

/// Held by the tests that stage a dump. Staging one keeps the image in a
/// temp file for as long as it is read, and the test that looks for an image
/// left behind must not find the one another test is reading.
static STAGING_A_DUMP: Mutex<()> = Mutex::new(());

fn table_count(conn: &rusqlite::Connection, table: &str) -> usize {
    let sql = format!("SELECT count(*) FROM \"{}\"", table.replace('"', "\"\""));
    conn.query_row(&sql, [], |row| row.get::<_, i64>(0))
        .expect("count table") as usize
}

#[test]
fn mdf_adapter_loads_a01_publish_core_tables() {
    if !Path::new(A01_MDF_PATH).exists() {
        eprintln!("skipping: MDF fixture {A01_MDF_PATH} not found");
        return;
    }
    let conn = open_mdf_as_sqlite(Path::new(A01_MDF_PATH)).expect("open MDF through Rust loader");
    assert_eq!(table_count(&conn, "T_Drawing"), 1);
    assert_eq!(table_count(&conn, "T_Representation"), 6);
    assert_eq!(table_count(&conn, "T_Relationship"), 3);
    assert_eq!(table_count(&conn, "T_ModelItem"), 4);
    // 3, not 4: the fourth row on T_PlantItem's pages is a ghost record —
    // a deleted plant item whose T_ModelItem row is already gone.
    assert_eq!(table_count(&conn, "T_PlantItem"), 3);
    assert_eq!(table_count(&conn, "T_Connector"), 1);
    assert_eq!(table_count(&conn, "T_PipeRun"), 1);
    let date_created: String = conn
        .query_row(
            "SELECT DateCreated FROM T_Drawing \
             WHERE SP_ID = 'D9635C3C898840D1990B7E8BEE1D55DA'",
            [],
            |row| row.get(0),
        )
        .expect("read drawing DateCreated");
    assert_eq!(date_created, "2026/4/20 10:32:46");
    let pipe_model_uid: String = conn
        .query_row(
            "SELECT SP_ModelItemID FROM T_Representation \
             WHERE SP_ID = 'AC9DFB6629974E428402C938E60F4B9C'",
            [],
            |row| row.get(0),
        )
        .expect("read pipe representation model uid");
    assert_eq!(pipe_model_uid, "185EF98B03E844158E3BD8E82806E6CF");
    let pipe_run: (String, String, String, String, String) = conn
        .query_row(
            "SELECT NominalDiameter, PipingMaterialsClass, TagSequenceNo, \
                    PipeRunType, PipeRunClass \
             FROM T_PipeRun \
             WHERE SP_ID = '185EF98B03E844158E3BD8E82806E6CF'",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .expect("read pipe run business columns");
    assert_eq!(
        pipe_run,
        (
            "250".to_string(),
            "B5".to_string(),
            "0102102".to_string(),
            "20".to_string(),
            "1".to_string(),
        )
    );
}

#[test]
fn strict_rows_load_a01_equip_component_table() {
    if !Path::new(A01_MDF_PATH).exists() {
        eprintln!("skipping: MDF fixture {A01_MDF_PATH} not found");
        return;
    }

    let mut db = MdfDatabase::open(A01_MDF_PATH).expect("open TEST02 MDF");
    let rows = db
        .try_rows("T_EquipComponent")
        .expect("T_EquipComponent exists")
        .collect::<Result<Vec<_>, _>>()
        .expect("strict rows should load T_EquipComponent");

    assert!(!rows.is_empty(), "T_EquipComponent should contain rows");
}

#[test]
fn strict_rows_load_a01_codelists_table() {
    if !Path::new(A01_MDF_PATH).exists() {
        eprintln!("skipping: MDF fixture {A01_MDF_PATH} not found");
        return;
    }

    let mut db = MdfDatabase::open(A01_MDF_PATH).expect("open TEST02 MDF");
    let rows = db
        .try_rows("codelists")
        .expect("codelists exists")
        .collect::<Result<Vec<_>, _>>()
        .expect("strict rows should load codelists");

    assert!(!rows.is_empty(), "codelists should contain rows");
}

/// #27: an attribute reaches its codelist through `attribute_datatype`
/// (`C90`), not through the `attribute_codelisted` flag (`T`). This MDF's
/// catalog is the plant-structure one: 15 codelisted attributes over 13
/// lists, and no equipment-type list.
#[test]
fn a01_codelist_resolves_an_attribute_through_its_datatype() {
    if !Path::new(A01_MDF_PATH).exists() {
        eprintln!("skipping: MDF fixture {A01_MDF_PATH} not found");
        return;
    }
    let conn = open_mdf_as_sqlite(Path::new(A01_MDF_PATH)).expect("open MDF through Rust loader");
    let index = pid_parse::publish::load_codelist_index(&conn).expect("codelist index");
    assert_eq!(index.attribute_mapping_count(), 15);
    assert_eq!(index.entry_count(), 129);
    assert_eq!(
        index.lookup_by_attribute("PlantGroupType", "65"),
        Some("Unit")
    );
    assert_eq!(
        index.lookup_by_attribute("DataAccessType", "2"),
        Some("UNC")
    );
    assert_eq!(index.lookup_by_attribute("EquipmentType", "241"), None);
}

/// The one real drawing the database line can be fed is written in full:
/// every item has a writer of its own, and no `T_Relationship` row ends on
/// an item. The account says so, and the document agrees with it.
#[test]
fn a01_is_written_in_full_and_the_account_says_so() {
    if !Path::new(A01_MDF_PATH).exists() {
        eprintln!("skipping: MDF fixture {A01_MDF_PATH} not found");
        return;
    }
    let drawing = pid_parse::publish::load_drawing_graph_from_mdf(
        Path::new(A01_MDF_PATH),
        "D9635C3C898840D1990B7E8BEE1D55DA",
    )
    .expect("load A01 through the Rust MDF loader");
    assert!(!drawing.objects.is_empty(), "A01 should carry items");
    assert!(
        !drawing.relationships.is_empty(),
        "A01 should carry relationship rows"
    );

    let shortfalls = pid_parse::publish::database_shortfalls(&drawing);
    assert!(shortfalls.is_empty(), "A01 has shortfalls: {shortfalls:?}");
    assert!(shortfalls.describe().is_empty());

    let xml = pid_parse::publish::write_data_xml(&drawing, "TEST02").expect("write A01");
    assert_eq!(xml.matches("<PIDItem>").count(), 0);
    assert_eq!(xml.matches(r#"DefUID="Relationship""#).count(), 0);
    eprintln!(
        "A01: {} items, {} relationship rows, no shortfalls",
        drawing.objects.len(),
        drawing.relationships.len()
    );
}

/// A staged table: the names of its columns, and its rows in the order they
/// went in.
type StagedTable = (Vec<String>, Vec<Vec<Option<String>>>);

/// What `conn` holds of `table`, or `None` when the table was not staged.
fn staged_table(conn: &rusqlite::Connection, table: &str) -> Option<StagedTable> {
    let held: i64 = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [table],
            |row| row.get(0),
        )
        .expect("look the table up");
    if held == 0 {
        return None;
    }
    let sql = format!(
        "SELECT * FROM \"{}\" ORDER BY rowid",
        table.replace('"', "\"\"")
    );
    let mut statement = conn.prepare(&sql).expect("prepare the read");
    let columns: Vec<String> = statement.column_names().map(str::to_string).collect();
    let mut rows = Vec::new();
    let mut cursor = statement.query([]).expect("read the table");
    while let Some(row) = cursor.next().expect("read a row") {
        rows.push(
            (0..columns.len())
                .map(|column| row.get(column).expect("a staged value is text or null"))
                .collect(),
        );
    }
    Some((columns, rows))
}

/// A package keeps its database as an MTF `Export.dmp` with the `.mdf`
/// inside. Staging the dump gives what staging that `.mdf` gives: the same
/// tables, with the same columns and the same rows in the same order.
#[test]
fn an_mtf_dump_stages_what_its_mdf_stages() {
    if !Path::new(A01_MDF_PATH).exists() || !Path::new(TEST02_DUMP_PATH).exists() {
        eprintln!("skipping: fixture {A01_MDF_PATH} or {TEST02_DUMP_PATH} not found");
        return;
    }
    let _staging = STAGING_A_DUMP
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let dump = std::fs::read(TEST02_DUMP_PATH).expect("read the dump");
    let from_dump = open_mtf_dump_as_sqlite(&dump).expect("stage the dump");
    let from_mdf = open_mdf_as_sqlite(Path::new(A01_MDF_PATH)).expect("stage the .mdf");

    let mut rows = 0usize;
    for table in PUBLISH_TABLES {
        let in_mdf = staged_table(&from_mdf, table);
        assert_eq!(staged_table(&from_dump, table), in_mdf, "{table}");
        rows += in_mdf.map_or(0, |(_, rows)| rows.len());
    }
    assert!(rows > 0, "nothing was staged, so nothing was compared");
}

/// The image `mdf_bytes_from_mtf_dump` takes out of the dump is the `.mdf`
/// the package was extracted to, byte for byte.
#[test]
fn the_image_inside_the_dump_is_the_extracted_mdf() {
    if !Path::new(A01_MDF_PATH).exists() || !Path::new(TEST02_DUMP_PATH).exists() {
        eprintln!("skipping: fixture {A01_MDF_PATH} or {TEST02_DUMP_PATH} not found");
        return;
    }
    let dump = std::fs::read(TEST02_DUMP_PATH).expect("read the dump");
    let image = mdf_bytes_from_mtf_dump(&dump).expect("take the image out of the dump");
    let mdf = std::fs::read(A01_MDF_PATH).expect("read the .mdf");
    assert_eq!(image.len(), mdf.len());
    // Not `assert_eq!`: a failure would print both images.
    assert!(
        image == mdf,
        "the image and the .mdf are as long and differ"
    );
}

/// Staging a dump writes its image to a temp file for the reader. The file
/// is gone when staging returns: an image the size of a plant's database is
/// not left in the temp directory.
#[test]
fn staging_a_dump_leaves_no_image_in_the_temp_directory() {
    if !Path::new(TEST02_DUMP_PATH).exists() {
        eprintln!("skipping: fixture {TEST02_DUMP_PATH} not found");
        return;
    }
    let _staging = STAGING_A_DUMP
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let dump = std::fs::read(TEST02_DUMP_PATH).expect("read the dump");
    let conn = open_mtf_dump_as_sqlite(&dump).expect("stage the dump");

    // The loader names its temp images `pid-parse-mtf-<process id>-…`.
    let ours = format!("pid-parse-mtf-{}-", std::process::id());
    let left: Vec<String> = std::fs::read_dir(std::env::temp_dir())
        .expect("list the temp directory")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with(&ours))
        .collect();
    assert!(left.is_empty(), "left in the temp directory: {left:?}");
    // The staged database does not need the image any more.
    assert_eq!(table_count(&conn, "T_Drawing"), 1);
}
