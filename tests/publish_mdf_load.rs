//! Rust MDF loader gates for the publish XML pipeline.

use std::path::Path;

use oxidized_mdf::MdfDatabase;
use pid_parse::publish::open_mdf_as_sqlite;

const A01_MDF_PATH: &str = "test-file/backup-test/TEST02_p/extracted/Export.mdf";

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
    assert_eq!(table_count(&conn, "T_PlantItem"), 4);
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
