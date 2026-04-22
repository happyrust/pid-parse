//! Load a Publish-Data DTO out of the SQLite mirror produced by
//! `tools/orca-mdf-probe`.
//!
//! Stage-1 A2: in addition to the single `T_Drawing` row, pull the
//! drawing's representations, their owning model items, and the
//! relationships that span them. Business-subtable fields
//! (Equipment / Vessel / Nozzle / PipeRun / ...) will layer on in
//! a later commit.
//!
//! # Join strategy
//!
//! SPPID anchors every drawing-side object at `T_Representation`.
//! Each row ties a `SP_ModelItemID` (pointing into `T_ModelItem`)
//! and a `SP_DrawingID` (pointing into `T_Drawing`). We use that
//! drawing ID to scope every downstream query so the loader only
//! returns rows relevant to the requested drawing.

use rusqlite::{params, Connection, OpenFlags};
use std::collections::BTreeSet;
use std::path::Path;

use super::model::{
    PublishDrawing, PublishError, PublishObject, PublishRelationship, PublishRepresentation,
};

/// Open a SQLite file produced by `OrcaMdfProbe --to-sqlite` in
/// read-only mode and return a ready-to-query connection. Exposed
/// publicly so integration tests and the eventual CLI can reuse
/// the same open logic.
pub fn open_readonly(path: &Path) -> Result<Connection, PublishError> {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(PublishError::from)
}

/// Load the drawing-level header row for `drawing_uid` from the
/// SQLite mirror. Returns [`PublishError::DrawingNotFound`] when no
/// `T_Drawing` row matches.
pub fn load_drawing(conn: &Connection, drawing_uid: &str) -> Result<PublishDrawing, PublishError> {
    // OrcaMDF ships every column as TEXT in the SQLite mirror, so
    // we deserialize everything as Option<String> and let higher
    // layers parse numerics / dates if they need to.
    let mut stmt = conn.prepare(
        "SELECT Name, DocumentCategory, DocumentType, Template, Path, DateCreated \
         FROM T_Drawing WHERE SP_ID = ?1",
    )?;

    let mut rows = stmt.query(params![drawing_uid])?;
    let Some(row) = rows.next()? else {
        return Err(PublishError::DrawingNotFound {
            uid: drawing_uid.to_string(),
        });
    };

    Ok(PublishDrawing {
        drawing_uid: drawing_uid.to_string(),
        drawing_name: row
            .get::<_, Option<String>>(0)?
            .unwrap_or_else(|| drawing_uid.to_string()),
        document_category: row.get(1)?,
        document_type: row.get(2)?,
        template: row.get(3)?,
        path: row.get(4)?,
        date_created: row.get(5)?,
        ..Default::default()
    })
}

/// Parse a SQLite TEXT column that SmartPlant / OrcaMDF may have
/// stored as either a numeric string ("42") or a full decimal
/// representation ("42.0"). Empty / NULL / non-numeric input
/// surfaces as `None` rather than an error — stage-1 treats these
/// values as decorative and does not want to fail the whole load
/// if one of them is unparsable.
fn parse_optional_i64(raw: Option<String>) -> Option<i64> {
    let raw = raw?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(n) = trimmed.parse::<i64>() {
        return Some(n);
    }
    // Tolerate decimal-looking input by truncating to the integer
    // portion. This is how GraphicOID values sometimes surface.
    if let Some(dot) = trimmed.find('.') {
        if let Ok(n) = trimmed[..dot].parse::<i64>() {
            return Some(n);
        }
    }
    None
}

/// Load every [`PublishRepresentation`] attached to `drawing_uid`,
/// sorted by the order SmartPlant stored them (SP_ID text order —
/// stable across runs).
pub fn load_representations(
    conn: &Connection,
    drawing_uid: &str,
) -> Result<Vec<PublishRepresentation>, PublishError> {
    let mut stmt = conn.prepare(
        "SELECT SP_ID, SP_ModelItemID, GraphicOID, FileName, RepresentationType \
         FROM T_Representation WHERE SP_DrawingID = ?1 ORDER BY SP_ID",
    )?;
    let mut rows = stmt.query(params![drawing_uid])?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        let uid: String = row.get(0)?;
        let model_item_uid: Option<String> = row.get(1)?;
        let graphic_oid_raw: Option<String> = row.get(2)?;
        let symbol_path: Option<String> = row.get(3)?;
        let rep_type_raw: Option<String> = row.get(4)?;
        out.push(PublishRepresentation {
            uid,
            model_item_uid,
            drawing_uid: drawing_uid.to_string(),
            graphic_oid: parse_optional_i64(graphic_oid_raw),
            symbol_path,
            representation_type: parse_optional_i64(rep_type_raw),
        });
    }
    Ok(out)
}

/// Load every [`PublishRelationship`] attached to `drawing_uid`.
pub fn load_relationships(
    conn: &Connection,
    drawing_uid: &str,
) -> Result<Vec<PublishRelationship>, PublishError> {
    let mut stmt = conn.prepare(
        "SELECT SP_ID, SP_Item1ID, SP_Item2ID, GraphicOID, \
                Item1Location, Item2Location, IsBinary \
         FROM T_Relationship WHERE SP_DrawingID = ?1 ORDER BY SP_ID",
    )?;
    let mut rows = stmt.query(params![drawing_uid])?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        let uid: String = row.get(0)?;
        let source_uid: Option<String> = row.get(1)?;
        let target_uid: Option<String> = row.get(2)?;
        let graphic_oid_raw: Option<String> = row.get(3)?;
        let loc1_raw: Option<String> = row.get(4)?;
        let loc2_raw: Option<String> = row.get(5)?;
        let is_binary_raw: Option<String> = row.get(6)?;
        out.push(PublishRelationship {
            uid,
            drawing_uid: drawing_uid.to_string(),
            source_uid,
            target_uid,
            graphic_oid: parse_optional_i64(graphic_oid_raw),
            item1_location: parse_optional_i64(loc1_raw),
            item2_location: parse_optional_i64(loc2_raw),
            is_binary: parse_optional_i64(is_binary_raw),
        });
    }
    Ok(out)
}

/// Load every `T_ModelItem` row whose `SP_ID` is in the given set.
/// Returns rows in the same order as the input set's iteration
/// (BTreeSet ⇒ lexicographic).
pub fn load_objects_by_uids(
    conn: &Connection,
    uids: &BTreeSet<String>,
) -> Result<Vec<PublishObject>, PublishError> {
    if uids.is_empty() {
        return Ok(Vec::new());
    }
    // Rusqlite does not support server-side IN (?) parameter
    // expansion on arbitrary-length collections, so we build the
    // placeholders ourselves. Safe because UIDs are opaque strings
    // we bind parametrically, not interpolated.
    let placeholders = vec!["?"; uids.len()].join(",");
    let sql = format!(
        "SELECT SP_ID, ItemTypeName, Description, SP_IsTypical \
         FROM T_ModelItem WHERE SP_ID IN ({placeholders}) ORDER BY SP_ID"
    );
    let mut stmt = conn.prepare(&sql)?;
    let params_vec: Vec<&dyn rusqlite::ToSql> = uids
        .iter()
        .map(|s| s as &dyn rusqlite::ToSql)
        .collect();
    let mut rows = stmt.query(params_vec.as_slice())?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        out.push(PublishObject {
            uid: row.get(0)?,
            item_type_name: row
                .get::<_, Option<String>>(1)?
                .unwrap_or_default(),
            description: row.get(2)?,
            is_typical: row.get(3)?,
            fields: std::collections::BTreeMap::new(),
        });
    }
    Ok(out)
}

/// Attach every non-null column of a single business-subtable row
/// to `obj.fields`. Column names from `SELECT *` are kept verbatim
/// (so downstream consumers can spot them in the DTO), with the
/// primary-key column `SP_ID` filtered out because it would just
/// duplicate `obj.uid`.
///
/// Empty strings are treated as "no value" and omitted to keep
/// the map tight; NULL comes through as absent.
fn attach_business_columns(
    conn: &Connection,
    table_name: &str,
    obj: &mut super::model::PublishObject,
) -> Result<(), PublishError> {
    // The table might not exist for every object type; treat a
    // "no such table" error as a soft skip so the loader tolerates
    // fixtures that ship a partial schema.
    let sql = format!("SELECT * FROM \"{}\" WHERE SP_ID = ?1", table_name.replace('"', "\"\""));
    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(rusqlite::Error::SqliteFailure(_, Some(msg))) if msg.contains("no such table") => {
            return Ok(());
        }
        Err(e) => return Err(PublishError::from(e)),
    };

    let col_names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
    let mut rows = stmt.query(params![obj.uid])?;
    if let Some(row) = rows.next()? {
        for (idx, name) in col_names.iter().enumerate() {
            if name == "SP_ID" {
                continue;
            }
            let value: Option<String> = row.get(idx)?;
            if let Some(v) = value {
                if v.is_empty() {
                    continue;
                }
                obj.fields.insert(name.clone(), v);
            }
        }
    }
    Ok(())
}

/// Return the list of SPPID subtables that carry business
/// attributes for a given `ItemTypeName`. Order matters: multiple
/// tables may contribute to the same object (e.g. Vessel rows
/// live in both T_Equipment — general equipment attributes —
/// and T_Vessel — vessel-specific dimensions), and later rows
/// overwrite earlier ones on column name collision.
fn subtables_for_item_type(item_type_name: &str) -> &'static [&'static str] {
    match item_type_name {
        // A vessel is an equipment subtype: general equipment
        // fields first, then vessel-specific fields.
        "Vessel" => &["T_PlantItem", "T_Equipment", "T_Vessel"],
        "Nozzle" => &["T_PlantItem", "T_EquipComponent", "T_Nozzle"],
        "PipeRun" => &["T_PlantItem", "T_Connector", "T_PipeRun"],
        "PipingPoint" => &["T_PipingPoint"],
        "PipingComp" => &["T_PlantItem", "T_InlineComp", "T_PipingComp"],
        "Instrument" | "InstrFunction" => &["T_PlantItem", "T_Instrument", "T_InstrFunction"],
        "Note" | "ItemNote" => &["T_ItemNote"],
        "Exchanger" => &["T_PlantItem", "T_Equipment", "T_Exchanger"],
        "Mechanical" => &["T_PlantItem", "T_Equipment", "T_Mechanical"],
        _ => &[],
    }
}

/// High-level entry point: load the complete Publish-Data DTO for
/// one drawing — header row + all representations + the model
/// items they point at + relationships + per-object business
/// fields from the appropriate SPPID subtables.
pub fn load_drawing_graph(
    conn: &Connection,
    drawing_uid: &str,
) -> Result<PublishDrawing, PublishError> {
    let mut drawing = load_drawing(conn, drawing_uid)?;

    drawing.representations = load_representations(conn, drawing_uid)?;
    drawing.relationships = load_relationships(conn, drawing_uid)?;

    // Every unique `SP_ModelItemID` referenced by a representation
    // — plus every `SP_Item1ID` / `SP_Item2ID` referenced by a
    // relationship — is an object that lives on this drawing.
    let mut model_item_uids: BTreeSet<String> = BTreeSet::new();
    for rep in &drawing.representations {
        if let Some(uid) = &rep.model_item_uid {
            if !uid.is_empty() {
                model_item_uids.insert(uid.clone());
            }
        }
    }
    for rel in &drawing.relationships {
        if let Some(uid) = &rel.source_uid {
            if !uid.is_empty() {
                model_item_uids.insert(uid.clone());
            }
        }
        if let Some(uid) = &rel.target_uid {
            if !uid.is_empty() {
                model_item_uids.insert(uid.clone());
            }
        }
    }

    drawing.objects = load_objects_by_uids(conn, &model_item_uids)?;

    // A4.1 — layer in business-subtable fields for every object.
    // Vessels / Nozzles / PipeRuns that reach this point gain
    // their `fields` populated; unknown item types get an empty
    // fields map, which the XML writer handles gracefully.
    for obj in &mut drawing.objects {
        let tables = subtables_for_item_type(&obj.item_type_name);
        for table in tables {
            attach_business_columns(conn, table, obj)?;
        }
    }

    Ok(drawing)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create an in-memory SQLite database, populate it with a
    /// minimal `T_Drawing` schema, and return the connection plus
    /// the synthetic drawing UID. Keeps the unit tests
    /// fixture-free.
    fn setup_synthetic_db() -> (Connection, String) {
        let conn = Connection::open_in_memory().expect("open in-memory sqlite");
        conn.execute(
            "CREATE TABLE T_Drawing (\
                SP_ID TEXT PRIMARY KEY, \
                Name TEXT, \
                DocumentCategory TEXT, \
                DocumentType TEXT, \
                Template TEXT, \
                Path TEXT, \
                DateCreated TEXT)",
            [],
        )
        .expect("create table");
        conn.execute(
            "INSERT INTO T_Drawing VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                "D9635C3C898840D1990B7E8BEE1D55DA",
                "A01",
                "6",
                "631",
                "A2-W-New.pid",
                "\\01\\01\\A01.pid",
                "2026/4/20 10:32:46",
            ],
        )
        .expect("insert row");
        (conn, "D9635C3C898840D1990B7E8BEE1D55DA".into())
    }

    #[test]
    fn load_drawing_populates_every_column() {
        let (conn, uid) = setup_synthetic_db();
        let drawing = load_drawing(&conn, &uid).expect("drawing row should load");

        assert_eq!(drawing.drawing_uid, uid);
        assert_eq!(drawing.drawing_name, "A01");
        assert_eq!(drawing.document_category.as_deref(), Some("6"));
        assert_eq!(drawing.document_type.as_deref(), Some("631"));
        assert_eq!(drawing.template.as_deref(), Some("A2-W-New.pid"));
        assert_eq!(drawing.path.as_deref(), Some("\\01\\01\\A01.pid"));
        assert_eq!(drawing.date_created.as_deref(), Some("2026/4/20 10:32:46"));
    }

    #[test]
    fn load_drawing_reports_missing_uid_cleanly() {
        let (conn, _uid) = setup_synthetic_db();
        let err = load_drawing(&conn, "NOT_IN_DB").unwrap_err();
        match err {
            PublishError::DrawingNotFound { uid } => assert_eq!(uid, "NOT_IN_DB"),
            other => panic!("expected DrawingNotFound, got {other:?}"),
        }
    }

    #[test]
    fn load_drawing_handles_null_columns() {
        // Real OrcaMDF output routinely has many NULL columns
        // (SmartPlant defaults that the user never filled in).
        // The loader must surface those as None rather than
        // panicking or returning an empty string.
        let conn = Connection::open_in_memory().expect("open sqlite");
        conn.execute(
            "CREATE TABLE T_Drawing (\
                SP_ID TEXT PRIMARY KEY, Name TEXT, DocumentCategory TEXT, \
                DocumentType TEXT, Template TEXT, Path TEXT, DateCreated TEXT)",
            [],
        )
        .expect("create table");
        conn.execute(
            "INSERT INTO T_Drawing VALUES ('UID-1', 'Drawing1', NULL, NULL, NULL, NULL, NULL)",
            [],
        )
        .expect("insert row");

        let d = load_drawing(&conn, "UID-1").expect("row should load");
        assert_eq!(d.drawing_name, "Drawing1");
        assert!(d.document_category.is_none());
        assert!(d.document_type.is_none());
        assert!(d.template.is_none());
        assert!(d.path.is_none());
        assert!(d.date_created.is_none());
    }
}
