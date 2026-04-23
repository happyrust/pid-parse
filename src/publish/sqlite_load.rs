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
    CodelistIndex, PublishDrawing, PublishError, PublishObject, PublishRelationship,
    PublishRepresentation,
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

/// Load every `T_PipingPoint` row whose parent `SP_PlantItemID`
/// is in the given set, and materialize it as a synthetic
/// [`PublishObject`] the XML writer can render via
/// `write_piping_port`. PipingPoints are drawing-scoped only
/// through their parent PlantItem (Nozzle / PipingComp / ...), so
/// this function must be called AFTER the drawing's main object
/// list is populated; the caller passes the union of those
/// objects' UIDs as the lookup key.
///
/// The resulting `PublishObject`s carry:
/// - `uid` = `T_PipingPoint.SP_ID`
/// - `item_type_name` = `"PipingPoint"` (so the writer dispatch
///   picks `write_piping_port`)
/// - `fields` populated from the point's business columns
///   (`NominalDiameter`, `FlowDirection`, `EndPrep`,
///   `PipingPointUsage`, `PipingPointNumber`) plus
///   `SP_PlantItemID` for tools that want to link back to the
///   owning PlantItem.
///
/// Empty-string values are filtered to keep the `fields` map
/// tight; NULL columns are omitted automatically. Returns an
/// empty `Vec` when either the `plant_item_uids` set is empty or
/// the `T_PipingPoint` table is missing from the fixture.
pub fn load_piping_points_for_objects(
    conn: &Connection,
    plant_item_uids: &BTreeSet<String>,
) -> Result<Vec<PublishObject>, PublishError> {
    if plant_item_uids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = vec!["?"; plant_item_uids.len()].join(",");
    let sql = format!(
        "SELECT SP_ID, SP_PlantItemID, NominalDiameter, FlowDirection, \
                EndPrep, PipingPointUsage, PipingPointNumber \
         FROM T_PipingPoint WHERE SP_PlantItemID IN ({placeholders}) ORDER BY SP_ID"
    );
    let Some(mut stmt) = prepare_optional(conn, &sql)? else {
        return Ok(Vec::new());
    };
    let params_vec: Vec<&dyn rusqlite::ToSql> = plant_item_uids
        .iter()
        .map(|s| s as &dyn rusqlite::ToSql)
        .collect();
    let mut rows = stmt.query(params_vec.as_slice())?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        let uid: String = row.get(0)?;
        let parent_plant_item: Option<String> = row.get(1)?;
        let mut fields = std::collections::BTreeMap::new();
        // SP_PlantItemID ties the point back to its Nozzle /
        // PipingComp / ... owner. Writers / downstream consumers
        // can walk this to surface cross-object information.
        if let Some(parent) = parent_plant_item {
            if !parent.is_empty() {
                fields.insert("SP_PlantItemID".into(), parent);
            }
        }
        for (col_idx, col_name) in [
            (2, "NominalDiameter"),
            (3, "FlowDirection"),
            (4, "EndPrep"),
            (5, "PipingPointUsage"),
            (6, "PipingPointNumber"),
        ] {
            let value: Option<String> = row.get(col_idx)?;
            if let Some(v) = value {
                if !v.is_empty() {
                    fields.insert(col_name.to_string(), v);
                }
            }
        }
        out.push(PublishObject {
            uid,
            item_type_name: "PipingPoint".into(),
            description: None,
            is_typical: None,
            fields,
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

/// Translate a rusqlite "no such table" error into an `Ok(None)`
/// so callers can soft-skip missing tables (common in partial
/// fixtures). Any other preparation error is bubbled up as a
/// [`PublishError::Sqlite`].
fn prepare_optional<'conn>(
    conn: &'conn Connection,
    sql: &str,
) -> Result<Option<rusqlite::Statement<'conn>>, PublishError> {
    match conn.prepare(sql) {
        Ok(s) => Ok(Some(s)),
        Err(rusqlite::Error::SqliteFailure(_, Some(msg))) if msg.contains("no such table") => {
            Ok(None)
        }
        Err(e) => Err(PublishError::from(e)),
    }
}

/// Load the SmartPlant codelist + attribute-codelist metadata out
/// of `conn`, returning a [`CodelistIndex`] that the XML writer
/// consults when resolving coded attribute values (e.g.
/// `EquipmentType = "0"` → `"Horizontal Drum"`).
///
/// Both underlying tables — `codelists` and `attributes` — are
/// treated as optional. When a fixture's SQLite mirror has not
/// populated them (because OrcaMDF skipped the catalog, or the
/// export scope is drawing-only) the loader returns a default
/// empty index; callers fall through to whatever lookup they
/// already had.
///
/// Rows with NULL / empty `codelist_number` / `codelist_index` /
/// `codelist_text` are filtered out to keep the index tight.
/// Similarly `attribute_codelisted` values of `""` or `"0"` (the
/// SPPID "no codelist" sentinels) do not register an
/// attribute-name mapping.
pub fn load_codelist_index(conn: &Connection) -> Result<CodelistIndex, PublishError> {
    let mut idx = CodelistIndex::default();

    // Codelist entry rows: (codelist_number, codelist_index) →
    // codelist_text. The table in the OrcaMDF SQLite mirror is
    // lowercase-`codelists` because the C# probe preserves
    // SmartPlant's catalog-layer naming (user-data tables are
    // uppercase `T_*`; catalog tables keep their original case).
    let codelists_sql = "SELECT codelist_number, codelist_index, codelist_text \
                         FROM codelists";
    if let Some(mut stmt) = prepare_optional(conn, codelists_sql)? {
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let number: Option<String> = row.get(0)?;
            let index: Option<String> = row.get(1)?;
            let text: Option<String> = row.get(2)?;
            let (Some(number), Some(index), Some(text)) = (number, index, text) else {
                continue;
            };
            if number.is_empty() || index.is_empty() || text.is_empty() {
                continue;
            }
            idx.insert_entry(number, index, text);
        }
    }

    // attribute_name → codelist_number mapping. The SPPID metadata
    // uses `attribute_codelisted = "0"` (or empty) to mean "this
    // attribute is not backed by a codelist"; both sentinels are
    // filtered out here so `lookup_by_attribute` can trust any
    // registered mapping.
    let attributes_sql = "SELECT attribute_name, attribute_codelisted \
                          FROM attributes";
    if let Some(mut stmt) = prepare_optional(conn, attributes_sql)? {
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let name: Option<String> = row.get(0)?;
            let codelist_number: Option<String> = row.get(1)?;
            let (Some(name), Some(codelist_number)) = (name, codelist_number) else {
                continue;
            };
            if name.is_empty()
                || codelist_number.is_empty()
                || codelist_number == "0"
            {
                continue;
            }
            idx.insert_attribute_mapping(name, codelist_number);
        }
    }

    Ok(idx)
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
    // The table might not exist for every object type; `prepare_optional`
    // returns `Ok(None)` in that case so fixtures with partial schemas
    // do not fail the whole load.
    let sql = format!("SELECT * FROM \"{}\" WHERE SP_ID = ?1", table_name.replace('"', "\"\""));
    let Some(mut stmt) = prepare_optional(conn, &sql)? else {
        return Ok(());
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
/// fields from the appropriate SPPID subtables, plus the
/// plant-wide codelist metadata so the writer can resolve coded
/// attribute values.
pub fn load_drawing_graph(
    conn: &Connection,
    drawing_uid: &str,
) -> Result<PublishDrawing, PublishError> {
    let mut drawing = load_drawing(conn, drawing_uid)?;

    drawing.representations = load_representations(conn, drawing_uid)?;
    drawing.relationships = load_relationships(conn, drawing_uid)?;
    // A7: codelist is plant-scoped, not drawing-scoped, but
    // attaching it to the `PublishDrawing` keeps the writer's
    // inputs self-contained. Loaders that want to share the
    // catalog across many drawings can clone the index from any
    // previously loaded drawing.
    drawing.codelist = load_codelist_index(conn)?;

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

    // A9 — pull drawing-scoped T_PipingPoint rows. Each physical
    // port is attached to an existing PlantItem (typically a
    // Nozzle or PipingComp) via `SP_PlantItemID`, so we enumerate
    // ports whose parent UID is in the drawing's main object list.
    // Synthesized as `PublishObject { item_type_name: "PipingPoint", ... }`
    // so the writer's dispatcher picks `write_piping_port`
    // automatically.
    let parent_uids: BTreeSet<String> =
        drawing.objects.iter().map(|o| o.uid.clone()).collect();
    let piping_points = load_piping_points_for_objects(conn, &parent_uids)?;
    if !piping_points.is_empty() {
        let existing: BTreeSet<String> =
            drawing.objects.iter().map(|o| o.uid.clone()).collect();
        for point in piping_points {
            if !existing.contains(&point.uid) {
                drawing.objects.push(point);
            }
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

    /// Create an in-memory SQLite with the catalog-layer tables
    /// SmartPlant ships alongside every export: `codelists` and
    /// `attributes`. The function does NOT populate any rows —
    /// individual tests seed whatever they need.
    fn setup_codelist_db() -> Connection {
        let conn = Connection::open_in_memory().expect("open sqlite");
        conn.execute(
            "CREATE TABLE codelists (\
                codelist_number TEXT, codelist_index TEXT, codelist_text TEXT, \
                codelist_short_text TEXT)",
            [],
        )
        .expect("create codelists");
        conn.execute(
            "CREATE TABLE attributes (\
                attribute_number TEXT, attribute_name TEXT, \
                attribute_codelisted TEXT)",
            [],
        )
        .expect("create attributes");
        conn
    }

    #[test]
    fn load_codelist_index_on_missing_tables_returns_empty() {
        // An export that shipped `T_Drawing` but skipped the
        // catalog layer should not make the loader error —
        // `prepare_optional` soft-skips both `codelists` and
        // `attributes`.
        let (conn, _uid) = setup_synthetic_db();
        let idx = load_codelist_index(&conn).expect("should not error on missing tables");
        assert!(idx.is_empty());
    }

    #[test]
    fn load_codelist_index_ignores_null_and_empty_rows() {
        let conn = setup_codelist_db();
        // Three codelist rows: one valid, one with NULL text,
        // one with empty codelist_number.
        conn.execute(
            "INSERT INTO codelists VALUES ('28', '0', 'Horizontal Drum', 'HD')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO codelists VALUES ('28', '1', NULL, NULL)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO codelists VALUES ('', '2', 'Ghost', 'G')",
            [],
        )
        .unwrap();
        // Three attribute rows: one codelisted, one empty sentinel,
        // one zero sentinel.
        conn.execute(
            "INSERT INTO attributes VALUES ('1', 'EquipmentType', '28')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO attributes VALUES ('2', 'TagPrefix', '')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO attributes VALUES ('3', 'NominalDiameter', '0')",
            [],
        )
        .unwrap();

        let idx = load_codelist_index(&conn).expect("load");
        assert_eq!(idx.entry_count(), 1);
        assert_eq!(idx.attribute_mapping_count(), 1);
        assert_eq!(idx.lookup("28", "0"), Some("Horizontal Drum"));
        assert_eq!(
            idx.lookup_by_attribute("EquipmentType", "0"),
            Some("Horizontal Drum"),
        );
        // Filtered-out attributes did not register a mapping.
        assert!(idx.lookup_by_attribute("TagPrefix", "V").is_none());
        assert!(idx.lookup_by_attribute("NominalDiameter", "250").is_none());
    }

    /// Seed an in-memory SQLite with the `T_PipingPoint` shape so
    /// A9 loader tests can insert their own rows. Returns the
    /// connection to the caller.
    fn setup_piping_point_db() -> Connection {
        let conn = Connection::open_in_memory().expect("open sqlite");
        conn.execute(
            "CREATE TABLE T_PipingPoint (\
                SP_ID TEXT, SP_PlantItemID TEXT, \
                NominalDiameter TEXT, FlowDirection TEXT, \
                EndPrep TEXT, PipingPointUsage TEXT, \
                PipingPointNumber TEXT)",
            [],
        )
        .expect("create T_PipingPoint");
        conn
    }

    #[test]
    fn load_piping_points_returns_empty_when_parent_set_is_empty() {
        let conn = setup_piping_point_db();
        let uids = BTreeSet::new();
        let out = load_piping_points_for_objects(&conn, &uids).expect("ok");
        assert!(out.is_empty());
    }

    #[test]
    fn load_piping_points_soft_skips_missing_table() {
        // A fixture without T_PipingPoint must not error —
        // `prepare_optional` turns the missing table into an empty
        // result.
        let (conn, _uid) = setup_synthetic_db();
        let mut uids = BTreeSet::new();
        uids.insert("SOMETHING".to_string());
        let out = load_piping_points_for_objects(&conn, &uids).expect("ok");
        assert!(out.is_empty());
    }

    #[test]
    fn load_piping_points_filters_to_requested_parents() {
        let conn = setup_piping_point_db();
        // Insert three points: two parented to NOZZLE1, one to an
        // unrelated PlantItem. The function should only surface
        // the two with the requested parent.
        for (id, parent, dn) in [
            ("PP1", "NOZZLE1", "250"),
            ("PP2", "NOZZLE1", "150"),
            ("PP3", "ELSEWHERE", "50"),
        ] {
            conn.execute(
                "INSERT INTO T_PipingPoint \
                 VALUES (?1, ?2, ?3, '', '', '', '0')",
                params![id, parent, dn],
            )
            .unwrap();
        }
        let mut uids = BTreeSet::new();
        uids.insert("NOZZLE1".to_string());
        let out = load_piping_points_for_objects(&conn, &uids).expect("ok");
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].uid, "PP1");
        assert_eq!(out[0].item_type_name, "PipingPoint");
        assert_eq!(
            out[0].fields.get("SP_PlantItemID").map(String::as_str),
            Some("NOZZLE1"),
        );
        assert_eq!(
            out[0].fields.get("NominalDiameter").map(String::as_str),
            Some("250"),
        );
        // Empty-string FlowDirection / EndPrep should not land
        // in the fields map.
        assert!(!out[0].fields.contains_key("FlowDirection"));
        assert!(!out[0].fields.contains_key("EndPrep"));
        assert_eq!(out[1].uid, "PP2");
    }

    #[test]
    fn load_piping_points_tolerates_null_and_empty_columns() {
        let conn = setup_piping_point_db();
        // Insert a row with NULL in most columns and empty string
        // for FlowDirection. Only SP_PlantItemID +
        // PipingPointNumber should survive the tight-fields
        // filter.
        conn.execute(
            "INSERT INTO T_PipingPoint \
             VALUES ('PP1', 'NOZZLE1', NULL, '', NULL, NULL, '3')",
            [],
        )
        .unwrap();
        let mut uids = BTreeSet::new();
        uids.insert("NOZZLE1".to_string());
        let out = load_piping_points_for_objects(&conn, &uids).expect("ok");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].fields.len(), 2); // SP_PlantItemID + PipingPointNumber
        assert_eq!(
            out[0].fields.get("PipingPointNumber").map(String::as_str),
            Some("3"),
        );
    }

    #[test]
    fn load_codelist_index_populates_multiple_entries_in_order_agnostic_way() {
        let conn = setup_codelist_db();
        for (number, index, text) in [
            ("28", "0", "Horizontal Drum"),
            ("28", "1", "Vertical Drum"),
            ("28", "2", "Reactor"),
            ("14", "0", "Gate Valve"),
        ] {
            conn.execute(
                "INSERT INTO codelists VALUES (?1, ?2, ?3, NULL)",
                params![number, index, text],
            )
            .unwrap();
        }
        conn.execute(
            "INSERT INTO attributes VALUES ('1', 'EquipmentType', '28')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO attributes VALUES ('2', 'ValveType', '14')",
            [],
        )
        .unwrap();

        let idx = load_codelist_index(&conn).expect("load");
        assert_eq!(idx.entry_count(), 4);
        assert_eq!(idx.attribute_mapping_count(), 2);
        assert_eq!(
            idx.lookup_by_attribute("EquipmentType", "2"),
            Some("Reactor"),
        );
        assert_eq!(
            idx.lookup_by_attribute("ValveType", "0"),
            Some("Gate Valve"),
        );
    }
}
