//! Piping and signal port rows from an Oracle conventional `exp` dump.
//!
//! Plan `docs/plans/2026-10-12-n5-ports-from-oracle-exp.md` N5-1, and
//! `docs/plans/2026-10-13-n9-publish-the-whole-backup-package.md` N9-1
//! ([`ExpTables`] reads one dump for many drawings). The byte
//! layout is the one pinned by `examples/probe_oracle_exp_rows.rs` and
//! `docs/analysis/n5-exp-rows.md`: an ASCII `INSERT INTO` line, a binary
//! column header, length-prefixed fields, `0xFFFE` for NULL, two zero
//! bytes after each row, and `0xFFFF` at the end of the table. Character
//! payloads are UTF-16BE (charset id 2000). Numbers are Oracle `NUMBER`.
//!
//! `APPLICATIONS` is read first. Its first row must be `ID` 1,
//! `SmartPlant PID`, `SPPID`. If it is not, the port tables are not read.
//! This module does not run `imp`, does not connect to Oracle, and does
//! not write XML.

use std::collections::{HashMap, HashSet};
use std::fmt;

use super::pid_load::{
    ConnectivityEnd, ExpPublish, ExpRelation, PidOpcPoint, PidPort, PidPublishAttribute,
    PidPublishDrawing, PidPublishItem, PidPublishRepresentation, PidValueSource,
};

const CHARSET_AL16UTF16: u16 = 2000;
const CHARSET_FORM: u16 = 2;

/// Why an export dump was not read.
#[derive(Debug)]
pub enum ExpError {
    /// `APPLICATIONS` row 0 is not `ID` 1, `SmartPlant PID`, `SPPID`.
    /// The port tables are not read.
    FormatUnpinned,
    /// `name` is not the pinned row format.
    Table {
        /// `APPLICATIONS`, `T_PIPINGPOINT`, or `T_SIGNALPOINT`.
        name: String,
        /// What was wrong.
        detail: String,
    },
}

impl fmt::Display for ExpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FormatUnpinned => write!(
                f,
                "APPLICATIONS row 0 is not (1, SmartPlant PID, SPPID); refusing to read the port tables"
            ),
            Self::Table { name, detail } => write!(f, "{name}: {detail}"),
        }
    }
}

impl std::error::Error for ExpError {}

/// One `T_PIPINGPOINT` row with the columns N5 publishes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipingPoint {
    /// `SP_PLANTITEMID`.
    pub plant_item_id: String,
    /// `PIPINGPOINTNUMBER`.
    pub number: i64,
    /// `FLOWDIRECTION`, when the column is not NULL.
    pub flow_direction: Option<i64>,
    /// `NOMINALDIAMETER`, when the column is not NULL.
    pub nominal_diameter: Option<i64>,
}

impl PipingPoint {
    /// Parent UID, a dot, and the point number.
    #[must_use]
    pub fn uid(&self) -> String {
        format!("{}.{}", self.plant_item_id, self.number)
    }
}

/// One `T_SIGNALPOINT` row with the columns N5 publishes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalPoint {
    /// `SP_PLANTITEMID`.
    pub plant_item_id: String,
    /// `SIGNALPOINTNUMBER`.
    pub number: i64,
}

impl SignalPoint {
    /// Parent UID, a dot, and the point number.
    #[must_use]
    pub fn uid(&self) -> String {
        format!("{}.{}", self.plant_item_id, self.number)
    }
}

/// The two port tables, after `APPLICATIONS` has pinned the row format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortTables {
    /// Parsed `T_PIPINGPOINT` rows, including ones that are not port rows.
    pub piping_row_count: usize,
    /// Parsed `T_SIGNALPOINT` rows, including ones that are not port rows.
    pub signal_row_count: usize,
    /// Rows with a plant-item id and an integer point number.
    pub piping_points: Vec<PipingPoint>,
    /// Rows with a plant-item id and an integer point number.
    pub signal_points: Vec<SignalPoint>,
}

/// Port rows whose parent is in the caller's set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrawingPorts {
    /// Piping ports, in table order.
    pub piping: Vec<PipingPoint>,
    /// Signal ports, in table order.
    pub signal: Vec<SignalPoint>,
}

/// Tables from one Oracle `exp` dump, read once and shared by every drawing.
///
/// [`Self::read`] loads the port, connectivity, composition, OPC, model
/// item and plant item tables the publish stages follow, the first
/// `CODELISTS` insert (pipeline names spell codelist 19) and `T_DRAWING`
/// (the package index and the revision check). `APPLICATIONS` is pinned
/// first and is not kept: a dump that fails the pin returns
/// [`ExpError::FormatUnpinned`] and the other tables are not read.
///
/// Every other table is read cell by cell without judging the columns no
/// stage follows: `T_PIPERUN` alone has 293 columns, and a run that is
/// insulated or sloped states the thickness or the slope in SI units
/// beside its text (`SP_INSULTHICKSI` 0.05 for `50mm`), a `NUMBER` that is
/// not an integer. Such a `NUMBER`, a `DATE` that does not decode and a
/// character cell that is not AL16UTF16 are kept unread. A projection
/// refuses a column it follows when any cell of it was not read, whether it
/// requires the column or only uses it when the dump has one, so an unread
/// cell never passes for an empty one. A column the dump lacks is not an
/// unread one: where a projection can do without it, it still does. The
/// `&[u8]` functions still read only the tables that entry used before, so
/// one drawing keeps today's call.
pub struct ExpTables {
    piping_point: Table,
    signal_point: Table,
    connector: Table,
    representation: Table,
    piperun: Table,
    signalrun: Table,
    opc: Table,
    nozzle: Table,
    note: Table,
    itemnote: Table,
    modelitem: Table,
    plantitem: Table,
    inlinecomp: Table,
    codelists: Table,
    drawing: Table,
}

impl ExpTables {
    /// Read the dump once.
    ///
    /// # Errors
    ///
    /// Returns [`ExpError::FormatUnpinned`] when `APPLICATIONS` does not
    /// pin the row format, and [`ExpError::Table`] when a stored table's
    /// bytes do not frame. `CODELISTS` is the first insert of that name.
    pub fn read(dump: &[u8]) -> Result<Self, ExpError> {
        Self::pin(dump)?;
        Ok(Self {
            piping_point: read_table_tolerant(dump, "T_PIPINGPOINT")?,
            signal_point: read_table_tolerant(dump, "T_SIGNALPOINT")?,
            connector: read_table_tolerant(dump, "T_CONNECTOR")?,
            representation: read_table_tolerant(dump, "T_REPRESENTATION")?,
            piperun: read_table_tolerant(dump, "T_PIPERUN")?,
            signalrun: read_table_tolerant(dump, "T_SIGNALRUN")?,
            opc: read_table_tolerant(dump, "T_OPC")?,
            nozzle: read_table_tolerant(dump, "T_NOZZLE")?,
            note: read_table_tolerant(dump, "T_NOTE")?,
            itemnote: read_table_tolerant(dump, "T_ITEMNOTE")?,
            modelitem: read_table_tolerant(dump, "T_MODELITEM")?,
            plantitem: read_optional_table(dump, "T_PLANTITEM")?,
            inlinecomp: read_optional_table(dump, "T_INLINECOMP")?,
            codelists: read_table_tolerant(dump, "CODELISTS")?,
            drawing: read_table_tolerant(dump, "T_DRAWING")?,
        })
    }

    /// Projected piping and signal port rows.
    ///
    /// # Errors
    ///
    /// Returns [`ExpError::Table`] when a port table lacks a column the
    /// projection reads or kept it unread, a port row states no plant item
    /// or no integer point number, or a row's `FLOWDIRECTION` /
    /// `NOMINALDIAMETER` is neither an integer nor NULL.
    pub fn port_tables(&self) -> Result<PortTables, ExpError> {
        Ok(PortTables {
            piping_row_count: self.piping_point.rows.len(),
            signal_row_count: self.signal_point.rows.len(),
            piping_points: project_piping(&self.piping_point)?,
            signal_points: project_signal(&self.signal_point)?,
        })
    }

    /// Rows in the first `CODELISTS` insert.
    #[must_use]
    pub fn codelist_row_count(&self) -> usize {
        self.codelists.rows.len()
    }

    /// Rows in `T_DRAWING`.
    #[must_use]
    pub fn drawing_row_count(&self) -> usize {
        self.drawing.rows.len()
    }

    /// `T_DRAWING` rows, in table order.
    ///
    /// The join columns (`SP_ID`, `NAME`, `PATH`, `DRAWINGNUMBER`,
    /// `SP_PLANTGROUPID`) are required; every other projected column is
    /// optional — a dump that lacks it gets empty strings / `None`s,
    /// and a NULL cell reads the same way. A column the dump has is held
    /// to the same terms either way: one whose cells were not all read
    /// (database-charset text, a `NUMBER` that is not an integer, a `DATE`
    /// that does not decode) fails the projection rather than reading as
    /// empty.
    ///
    /// # Errors
    ///
    /// Returns [`ExpError::Table`] when `SP_ID`, `NAME`, `PATH`,
    /// `DRAWINGNUMBER` or `SP_PLANTGROUPID` is missing, or a projected
    /// column holds a cell that was not read.
    pub fn drawings(&self) -> Result<Vec<BackupDrawing>, ExpError> {
        project_drawings(&self.drawing)
    }

    /// `T_PIPERUN` rows that can become a pipeline name, each with the
    /// `T_PLANTITEM.ITEMTAG` of the plant item that shares its `SP_ID`.
    ///
    /// # Errors
    ///
    /// Returns [`ExpError::Table`] when a required `T_PIPERUN` column is
    /// missing, or a followed column of it or of `T_PLANTITEM` holds a cell
    /// that was not read. A dump with no `T_PLANTITEM`, or one without
    /// `SP_ID` or `ITEMTAG`, gives every run `item_tag: None`.
    pub fn pipe_runs(&self) -> Result<Vec<PipeRunRow>, ExpError> {
        let id = column_index(&self.piperun, "SP_ID")?;
        let class = column_index(&self.piperun, "PIPINGMATERIALSCLASS")?;
        let sequence = column_index(&self.piperun, "TAGSEQUENCENO")?;
        let insulation = column_index(&self.piperun, "INSULATIONSPEC")?;
        column_index(&self.piperun, "NOMINALDIAMETER")?;
        column_index(&self.piperun, "OPERFLUIDCODE")?;
        column_index(&self.piperun, "FLUIDSYSTEM")?;
        let item_tags = self.item_tags()?;
        let mut runs = Vec::with_capacity(self.piperun.rows.len());
        for (index, row) in self.piperun.rows.iter().enumerate() {
            let id_text = cell_text(row, id);
            if id_text.is_empty() {
                continue;
            }
            let id_key = norm_uid(&id_text);
            runs.push(PipeRunRow {
                item_tag: item_tags.get(&id_key).cloned(),
                id_key,
                id: id_text,
                nominal_diameter: optional_int(
                    "T_PIPERUN",
                    &self.piperun,
                    row,
                    "NOMINALDIAMETER",
                    index,
                )?,
                fluid_code: optional_int("T_PIPERUN", &self.piperun, row, "OPERFLUIDCODE", index)?,
                materials_class: cell_text(row, class),
                tag_sequence: cell_text(row, sequence),
                fluid_system: optional_int("T_PIPERUN", &self.piperun, row, "FLUIDSYSTEM", index)?,
                insulation_spec: cell_text(row, insulation),
            });
        }
        Ok(runs)
    }

    /// Piping components this drawing places that its `.pid` does not
    /// draw, each with the representation that places it.
    ///
    /// A row is a `T_REPRESENTATION` on `drawing_uid` with
    /// `REPRESENTATIONTYPE` 13 whose model item is a `PipingComp` and
    /// whose own UID is not in `drawn`. The reference publishes such a
    /// component with its ports and its representation; the item carries
    /// `T_INLINECOMP.NOMINALDIAMETER` when the dump states one. Empty
    /// when the dump has no `ITEMTYPENAME` column.
    ///
    /// # Errors
    ///
    /// Returns [`ExpError::Table`] when `T_REPRESENTATION` or
    /// `T_MODELITEM` lacks a join column, or a followed column of those
    /// two or of `T_INLINECOMP` holds a cell that was not read.
    pub fn dump_only_piping_components(
        &self,
        drawing_uid: &str,
        drawn: &HashSet<String>,
    ) -> Result<Vec<(PidPublishItem, PidPublishRepresentation)>, ExpError> {
        let drawing_uid = norm_uid(drawing_uid);
        let models = &self.modelitem;
        let model_id = column_index(models, "SP_ID")?;
        let Some(model_type) = optional_column_index(models, "ITEMTYPENAME")? else {
            return Ok(Vec::new());
        };
        let components: HashSet<String> = models
            .rows
            .iter()
            .filter(|row| text_of(row, model_type) == Some("PipingComp"))
            .filter_map(|row| text_of(row, model_id).map(norm_uid))
            .collect();
        let mut diameters = HashMap::<String, i64>::new();
        if let (Some(id), Some(diameter)) = (
            optional_column_index(&self.inlinecomp, "SP_ID")?,
            optional_column_index(&self.inlinecomp, "NOMINALDIAMETER")?,
        ) {
            for row in &self.inlinecomp.rows {
                if let (Some(id), Some(code)) =
                    (text_of(row, id), optional_cell_int(row, Some(diameter)))
                {
                    diameters.insert(norm_uid(id), code);
                }
            }
        }

        let reps = &self.representation;
        let rep_id = column_index(reps, "SP_ID")?;
        let rep_model = column_index(reps, "SP_MODELITEMID")?;
        let rep_drawing = column_index(reps, "SP_DRAWINGID")?;
        let rep_type = column_index(reps, "REPRESENTATIONTYPE")?;
        let rep_oid = column_index(reps, "GRAPHICOID")?;
        let mut found = Vec::new();
        for row in &reps.rows {
            if text_of(row, rep_drawing).map(norm_uid).as_deref() != Some(drawing_uid.as_str())
                || optional_cell_int(row, Some(rep_type)) != Some(13)
            {
                continue;
            }
            let (Some(representation), Some(model)) = (
                text_of(row, rep_id).map(norm_uid),
                text_of(row, rep_model).map(norm_uid),
            ) else {
                continue;
            };
            if drawn.contains(&representation) || !components.contains(&model) {
                continue;
            }
            let Some(graphic_oid) =
                optional_cell_int(row, Some(rep_oid)).and_then(|oid| u32::try_from(oid).ok())
            else {
                continue;
            };
            let attributes = diameters
                .get(&model)
                .and_then(|code| nominal_spelling(*code))
                .map(|value| PidPublishAttribute {
                    interface: "IPipeCrossSectionItem",
                    name: "NominalDiameter",
                    value,
                    source: backup("T_INLINECOMP", "NOMINALDIAMETER"),
                })
                .into_iter()
                .collect();
            found.push((
                PidPublishItem {
                    uid: model.clone(),
                    model_item_type: "PipingComp".to_string(),
                    class: Some("PIDPipingComponent"),
                    attributes,
                },
                PidPublishRepresentation {
                    uid: representation,
                    item_uid: Some(model),
                    graphic_oid,
                },
            ));
        }
        Ok(found)
    }

    /// `T_PLANTITEM.ITEMTAG` keyed by the folded `SP_ID`. Empty when the
    /// dump carries no `T_PLANTITEM` or the table lacks either column.
    fn item_tags(&self) -> Result<HashMap<String, String>, ExpError> {
        let (Some(id), Some(tag)) = (
            optional_column_index(&self.plantitem, "SP_ID")?,
            optional_column_index(&self.plantitem, "ITEMTAG")?,
        ) else {
            return Ok(HashMap::new());
        };
        Ok(self
            .plantitem
            .rows
            .iter()
            .map(|row| (norm_uid(&cell_text(row, id)), cell_text(row, tag)))
            .filter(|(id, _)| !id.is_empty())
            .collect())
    }

    /// `CODELIST_SHORT_TEXT` for one list number, keyed by `CODELIST_INDEX`.
    ///
    /// An empty short text is omitted. The caller treats a missing index as
    /// a code that cannot be spelled.
    ///
    /// # Errors
    ///
    /// Returns [`ExpError::Table`] when the codelist columns are missing or
    /// a number cell is not an integer.
    pub fn codelist_short_text(&self, list_number: i64) -> Result<HashMap<i64, String>, ExpError> {
        let short = column_index(&self.codelists, "CODELIST_SHORT_TEXT")?;
        column_index(&self.codelists, "CODELIST_NUMBER")?;
        column_index(&self.codelists, "CODELIST_INDEX")?;
        let mut texts = HashMap::new();
        for (row_index, row) in self.codelists.rows.iter().enumerate() {
            let list = optional_int(
                "CODELISTS",
                &self.codelists,
                row,
                "CODELIST_NUMBER",
                row_index,
            )?;
            let code = optional_int(
                "CODELISTS",
                &self.codelists,
                row,
                "CODELIST_INDEX",
                row_index,
            )?;
            let (Some(list), Some(code)) = (list, code) else {
                continue;
            };
            if list != list_number {
                continue;
            }
            let text = cell_text(row, short);
            if text.is_empty() {
                continue;
            }
            texts.insert(code, text);
        }
        Ok(texts)
    }

    fn pin(dump: &[u8]) -> Result<(), ExpError> {
        let applications = read_table(dump, "APPLICATIONS")?;
        if !applications_pinned(&applications) {
            return Err(ExpError::FormatUnpinned);
        }
        Ok(())
    }

    fn blank() -> Self {
        Self {
            piping_point: Table::blank("T_PIPINGPOINT"),
            signal_point: Table::blank("T_SIGNALPOINT"),
            connector: Table::blank("T_CONNECTOR"),
            representation: Table::blank("T_REPRESENTATION"),
            piperun: Table::blank("T_PIPERUN"),
            signalrun: Table::blank("T_SIGNALRUN"),
            opc: Table::blank("T_OPC"),
            nozzle: Table::blank("T_NOZZLE"),
            note: Table::blank("T_NOTE"),
            itemnote: Table::blank("T_ITEMNOTE"),
            modelitem: Table::blank("T_MODELITEM"),
            plantitem: Table::blank("T_PLANTITEM"),
            inlinecomp: Table::blank("T_INLINECOMP"),
            codelists: Table::blank("CODELISTS"),
            drawing: Table::blank("T_DRAWING"),
        }
    }

    #[cfg(test)]
    fn load_components(dump: &[u8]) -> Result<Self, ExpError> {
        Self::pin(dump)?;
        Ok(Self {
            representation: read_table_tolerant(dump, "T_REPRESENTATION")?,
            modelitem: read_table_tolerant(dump, "T_MODELITEM")?,
            inlinecomp: read_optional_table(dump, "T_INLINECOMP")?,
            ..Self::blank()
        })
    }

    fn load_ports(dump: &[u8]) -> Result<Self, ExpError> {
        Self::pin(dump)?;
        Ok(Self {
            piping_point: read_table_tolerant(dump, "T_PIPINGPOINT")?,
            signal_point: read_table_tolerant(dump, "T_SIGNALPOINT")?,
            ..Self::blank()
        })
    }

    fn load_connectivity(dump: &[u8]) -> Result<Self, ExpError> {
        Self::pin(dump)?;
        Ok(Self {
            connector: read_table_tolerant(dump, "T_CONNECTOR")?,
            representation: read_table_tolerant(dump, "T_REPRESENTATION")?,
            piping_point: read_table_tolerant(dump, "T_PIPINGPOINT")?,
            signal_point: read_table_tolerant(dump, "T_SIGNALPOINT")?,
            piperun: read_table_tolerant(dump, "T_PIPERUN")?,
            signalrun: read_table_tolerant(dump, "T_SIGNALRUN")?,
            opc: read_table_tolerant(dump, "T_OPC")?,
            ..Self::blank()
        })
    }

    fn load_compositions(dump: &[u8]) -> Result<Self, ExpError> {
        Self::pin(dump)?;
        Ok(Self {
            nozzle: read_table_tolerant(dump, "T_NOZZLE")?,
            note: read_table_tolerant(dump, "T_NOTE")?,
            itemnote: read_table_tolerant(dump, "T_ITEMNOTE")?,
            ..Self::blank()
        })
    }

    fn load_opc(dump: &[u8]) -> Result<Self, ExpError> {
        Self::pin(dump)?;
        Ok(Self {
            opc: read_table_tolerant(dump, "T_OPC")?,
            representation: read_table_tolerant(dump, "T_REPRESENTATION")?,
            modelitem: read_table_tolerant(dump, "T_MODELITEM")?,
            ..Self::blank()
        })
    }
}

/// One `T_PIPERUN` row the pipeline name is spelled from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipeRunRow {
    /// `SP_ID` as stored.
    pub id: String,
    /// `SP_ID` folded like [`BackupDrawing::id_key`].
    pub id_key: String,
    /// `NOMINALDIAMETER`. Empty when null.
    pub nominal_diameter: Option<i64>,
    /// `OPERFLUIDCODE`. Empty when null.
    pub fluid_code: Option<i64>,
    /// `PIPINGMATERIALSCLASS`.
    pub materials_class: String,
    /// `TAGSEQUENCENO`. Empty when the run is not a pipeline.
    pub tag_sequence: String,
    /// `FLUIDSYSTEM`. Empty when null.
    pub fluid_system: Option<i64>,
    /// `INSULATIONSPEC`.
    pub insulation_spec: String,
    /// `T_PLANTITEM.ITEMTAG` of the plant item with this `SP_ID`. `None`
    /// when no such row is in the dump; a NULL tag reads as `Some("")`.
    pub item_tag: Option<String>,
}

/// One `T_DRAWING` row a backup package hangs a `.pid` on.
///
/// `id` / `id_key` / `name` / `path` / `path_key` / `drawing_number` /
/// `plant_group_id` are the join surface and stay required — a dump
/// missing those columns fails [`ExpTables::drawings`]. Everything
/// else is display metadata: a column absent from the dump (or a NULL
/// cell) projects as an empty string / `None` instead of an error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupDrawing {
    /// `SP_ID` as stored.
    pub id: String,
    /// `SP_ID` with braces, dashes and case folded, for matching `DrawingNo`.
    pub id_key: String,
    /// `NAME`.
    pub name: String,
    /// `PATH` as stored.
    pub path: String,
    /// [`backup_path_key`] of [`Self::path`].
    pub path_key: String,
    /// `DRAWINGNUMBER`. Empty when the cell is null.
    pub drawing_number: String,
    /// `SP_PLANTGROUPID`, folded like [`Self::id_key`]. Empty when null.
    pub plant_group_id: String,
    /// `DESCRIPTION`. Empty when null or the column is absent.
    pub description: String,
    /// `TITLE`. Empty when null or the column is absent.
    pub title: String,
    /// `REVISION`. Empty when null or the column is absent.
    pub revision: String,
    /// `VERSION`. Empty when null or the column is absent.
    pub version: String,
    /// `DOCUMENTTYPE` as stored. `None` when null or the column is absent.
    pub document_type: Option<i64>,
    /// `DOCUMENTCATEGORY` as stored. `None` when null or the column is absent.
    pub document_category: Option<i64>,
    /// `ITEMSTATUS` as stored. `None` when null or the column is absent.
    pub item_status: Option<i64>,
    /// `TEMPLATE`. Empty when null or the column is absent.
    pub template: String,
    /// `DESIGNPHASE`. Empty when null or the column is absent.
    pub design_phase: String,
    /// `PROJECTNUMBER`. Empty when null or the column is absent.
    pub project_number: String,
    /// `DATECREATED` decoded to `YYYY/MM/DD HH:MM:SS`. Empty when null
    /// or the column is absent.
    pub date_created: String,
    /// `DRAWINGDATE` decoded to `YYYY/MM/DD HH:MM:SS`. Empty when null
    /// or the column is absent.
    pub drawing_date: String,
    /// `DATEOFDRAWING` — the free-text spelling `SmartPlant` keeps next
    /// to `DRAWINGDATE`. Empty when null or the column is absent.
    pub date_of_drawing: String,
    /// `LASTPUBLISHTIME` decoded to `YYYY/MM/DD HH:MM:SS`. Empty when
    /// null or the column is absent.
    pub last_publish_time: String,
    /// `LASTARCHIVETIME` decoded to `YYYY/MM/DD HH:MM:SS`. Empty when
    /// null or the column is absent.
    pub last_archive_time: String,
}

/// One `T_PLANTGROUP` row used to spell a drawing's place in the plant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlantGroup {
    /// `SP_ID`, folded like [`BackupDrawing::id_key`].
    pub id: String,
    /// `NAME`.
    pub name: String,
    /// Folded parent id. Empty for the plant root, whose `PARENTID` is `-1`.
    pub parent_id: String,
}

/// Slash-fold `path` and drop a leading or trailing slash.
///
/// `T_DRAWING.PATH` is `\00\00\name.pid`. A zip entry is `00/00/name.pid`.
/// Both become `00/00/name.pid`.
#[must_use]
pub fn backup_path_key(path: &str) -> String {
    path.replace('\\', "/").trim_matches('/').to_string()
}

/// Fold a drawing or group id the way [`BackupDrawing::id_key`] does.
#[must_use]
pub fn backup_uid_key(text: &str) -> String {
    norm_uid(text)
}

/// Whether the dump behind `tables` carries a `T_DRAWING` row for `uid`.
///
/// A `.pid` and an exp dump describe the same revision of a sheet only
/// when the drawing's own `DocUID` is one of the dump's `T_DRAWING.SP_ID`
/// cells, compared the way [`backup_uid_key`] folds both. A miss means the
/// dump belongs to another revision: none of its port, connectivity,
/// composition or pipeline content applies to this drawing, so an
/// `--oracle-exp` run that ignored the mismatch would publish the `.pid`
/// alone and still report success.
///
/// # Errors
///
/// Returns [`ExpError::Table`] when `T_DRAWING` lacks one of the columns
/// [`ExpTables::drawings`] requires, or a column it projects holds a cell
/// that was not read.
pub fn drawing_in_dump(tables: &ExpTables, uid: &str) -> Result<bool, ExpError> {
    let key = backup_uid_key(uid);
    Ok(tables
        .drawings()?
        .iter()
        .any(|drawing| drawing.id_key == key))
}

/// `T_PLANTGROUP` rows.
///
/// A dump with no such insert returns an empty list. Dates and byte
/// columns are skipped.
///
/// # Errors
///
/// Returns [`ExpError::Table`] when the insert is present but not the
/// pinned row shape, or when `SP_ID` / `NAME` / `PARENTID` is missing.
pub fn read_plant_groups(dump: &[u8]) -> Result<Vec<PlantGroup>, ExpError> {
    if find_slice(dump, b"INSERT INTO \"T_PLANTGROUP\"").is_none() {
        return Ok(Vec::new());
    }
    let table = read_table_tolerant(dump, "T_PLANTGROUP")?;
    let id = column_index(&table, "SP_ID")?;
    let name = column_index(&table, "NAME")?;
    let parent = column_index(&table, "PARENTID")?;
    let mut groups = Vec::with_capacity(table.rows.len());
    for row in &table.rows {
        let id = cell_text(row, id);
        if id.is_empty() {
            continue;
        }
        let parent_raw = cell_text(row, parent);
        let parent_id = if parent_raw.is_empty() || parent_raw == "-1" {
            String::new()
        } else {
            norm_uid(&parent_raw)
        };
        groups.push(PlantGroup {
            id: norm_uid(&id),
            name: cell_text(row, name),
            parent_id,
        });
    }
    Ok(groups)
}

/// `SQPlant \ 00 \ 00`: group names from the plant root down to `id`.
///
/// An unknown id is an empty string. A parent cycle stops the walk.
#[must_use]
pub fn plant_group_path(groups: &[PlantGroup], id: &str) -> String {
    if id.is_empty() {
        return String::new();
    }
    let by_id: HashMap<&str, &PlantGroup> = groups
        .iter()
        .map(|group| (group.id.as_str(), group))
        .collect();
    let mut names = Vec::new();
    let mut current = norm_uid(id);
    let mut seen = HashSet::new();
    while !current.is_empty() && seen.insert(current.clone()) {
        let Some(group) = by_id.get(current.as_str()) else {
            break;
        };
        names.push(group.name.clone());
        current = group.parent_id.clone();
    }
    names.reverse();
    names.join(" \\ ")
}

/// `T_DRAWING` rows for a backup-package drawing index.
///
/// Only `T_DRAWING` is read — callers listing a package's drawings do not
/// need the port/run tables [`ExpTables::read`] requires, and real dumps
/// have carried cells those strict reads reject. `APPLICATIONS` still
/// pins the row format, so a non-exp input is refused.
///
/// # Errors
///
/// Returns [`ExpError::FormatUnpinned`] when `APPLICATIONS` does not pin
/// the row format, and [`ExpError::Table`] when `T_DRAWING` is missing,
/// lacks `SP_ID` / `NAME` / `PATH` / `DRAWINGNUMBER` / `SP_PLANTGROUPID`,
/// or holds a cell that was not read in a column [`ExpTables::drawings`]
/// projects.
pub fn read_backup_drawings(dump: &[u8]) -> Result<Vec<BackupDrawing>, ExpError> {
    ExpTables::pin(dump)?;
    let drawing = read_table_tolerant(dump, "T_DRAWING")?;
    project_drawings(&drawing)
}

fn project_drawings(drawing: &Table) -> Result<Vec<BackupDrawing>, ExpError> {
    let id = column_index(drawing, "SP_ID")?;
    let name = column_index(drawing, "NAME")?;
    let path = column_index(drawing, "PATH")?;
    let number = column_index(drawing, "DRAWINGNUMBER")?;
    let group = column_index(drawing, "SP_PLANTGROUPID")?;
    let description = optional_column_index(drawing, "DESCRIPTION")?;
    let title = optional_column_index(drawing, "TITLE")?;
    let revision = optional_column_index(drawing, "REVISION")?;
    let version = optional_column_index(drawing, "VERSION")?;
    let document_type = optional_column_index(drawing, "DOCUMENTTYPE")?;
    let document_category = optional_column_index(drawing, "DOCUMENTCATEGORY")?;
    let item_status = optional_column_index(drawing, "ITEMSTATUS")?;
    let template = optional_column_index(drawing, "TEMPLATE")?;
    let design_phase = optional_column_index(drawing, "DESIGNPHASE")?;
    let project_number = optional_column_index(drawing, "PROJECTNUMBER")?;
    let date_created = optional_column_index(drawing, "DATECREATED")?;
    let drawing_date = optional_column_index(drawing, "DRAWINGDATE")?;
    let date_of_drawing = optional_column_index(drawing, "DATEOFDRAWING")?;
    let last_publish_time = optional_column_index(drawing, "LASTPUBLISHTIME")?;
    let last_archive_time = optional_column_index(drawing, "LASTARCHIVETIME")?;
    let mut drawings = Vec::with_capacity(drawing.rows.len());
    for row in &drawing.rows {
        let id = cell_text(row, id);
        if id.is_empty() {
            continue;
        }
        let path = cell_text(row, path);
        let group_id = cell_text(row, group);
        drawings.push(BackupDrawing {
            id_key: norm_uid(&id),
            id,
            name: cell_text(row, name),
            path_key: backup_path_key(&path),
            path,
            drawing_number: cell_text(row, number),
            plant_group_id: if group_id.is_empty() {
                String::new()
            } else {
                norm_uid(&group_id)
            },
            description: optional_cell_text(row, description),
            title: optional_cell_text(row, title),
            revision: optional_cell_text(row, revision),
            version: optional_cell_text(row, version),
            document_type: optional_cell_int(row, document_type),
            document_category: optional_cell_int(row, document_category),
            item_status: optional_cell_int(row, item_status),
            template: optional_cell_text(row, template),
            design_phase: optional_cell_text(row, design_phase),
            project_number: optional_cell_text(row, project_number),
            date_created: optional_cell_text(row, date_created),
            drawing_date: optional_cell_text(row, drawing_date),
            date_of_drawing: optional_cell_text(row, date_of_drawing),
            last_publish_time: optional_cell_text(row, last_publish_time),
            last_archive_time: optional_cell_text(row, last_archive_time),
        });
    }
    Ok(drawings)
}

fn cell_text(row: &[Field], index: usize) -> String {
    match row.get(index) {
        Some(Field::Text(text) | Field::Date(text)) => text.clone(),
        Some(Field::Int(value)) => value.to_string(),
        Some(Field::Null | Field::Unscaled) | None => String::new(),
    }
}

/// Read `T_PIPINGPOINT` and `T_SIGNALPOINT`.
///
/// `APPLICATIONS` is pinned first. A dump that fails the pin returns
/// [`ExpError::FormatUnpinned`] and the two port tables are not read.
///
/// # Errors
///
/// Returns [`ExpError::FormatUnpinned`] when the pin fails, and
/// [`ExpError::Table`] when a table's bytes are not the pinned format.
/// Loads `APPLICATIONS` and the two port tables, then
/// [`ExpTables::port_tables`].
pub fn read_port_tables(dump: &[u8]) -> Result<PortTables, ExpError> {
    ExpTables::load_ports(dump)?.port_tables()
}

/// UIDs [`super::pid_load::load_publish_from_pid`] emits as model items.
///
/// An item whose class the file does not decide is not emitted. Process
/// points and branch points are not plant-item parents, so they are not
/// in this set.
#[must_use]
pub fn published_parent_ids(drawing: &PidPublishDrawing) -> HashSet<String> {
    drawing
        .items
        .iter()
        .filter(|item| item.class.is_some())
        .map(|item| item.uid.clone())
        .collect()
}

/// Keep port rows whose `SP_PLANTITEMID` is in `parents`.
///
/// The set is the drawing's published item UIDs (Q2 a). This function
/// does not read a reference XML.
#[must_use]
pub fn ports_for_parents(tables: &PortTables, parents: &HashSet<String>) -> DrawingPorts {
    DrawingPorts {
        piping: tables
            .piping_points
            .iter()
            .filter(|point| parents.contains(&point.plant_item_id))
            .cloned()
            .collect(),
        signal: tables
            .signal_points
            .iter()
            .filter(|point| parents.contains(&point.plant_item_id))
            .cloned()
            .collect(),
    }
}

/// Spell a flow-direction code the way the reference writes it.
///
/// The five codes are the table in
/// `docs/analysis/n2a-port-relation-filter.md`. Anything else is not written.
#[must_use]
pub fn flow_spelling(code: i64) -> Option<&'static str> {
    match code {
        1 | 5 => Some("@{23F94152-B87D-11D6-80D4-00A0C92487AF}"),
        2 => Some("@{23F94146-B87D-11D6-80D4-00A0C92487AF}"),
        3 => Some("@{23F9414C-B87D-11D6-80D4-00A0C92487AF}"),
        4 => Some("@{BCB3F097-66CA-487B-9AF4-D8FCF18624A3}"),
        _ => None,
    }
}

/// Spell a nominal-diameter code the way the reference writes it.
///
/// Below 5000 the code is millimetres (`25` → `25 mm`). Above 5000,
/// `(code - 5000) / 32` is inches (`5032` → `1 in`, `5024` → `0.75 in`,
/// `5384` → `12 in`). A code that does not land on that grid, or is too
/// large to scale onto it, is `None`.
#[must_use]
pub fn nominal_spelling(code: i64) -> Option<String> {
    if (1..5000).contains(&code) {
        return Some(format!("{code} mm"));
    }
    if code <= 5000 {
        return None;
    }
    let scaled = (code - 5000).checked_mul(100)?;
    if scaled % 32 != 0 {
        return None;
    }
    let hundredths = scaled / 32;
    let whole = hundredths / 100;
    let fraction = hundredths % 100;
    let body = if fraction == 0 {
        format!("{whole}")
    } else if fraction % 10 == 0 {
        format!("{whole}.{}", fraction / 10)
    } else {
        format!("{whole}.{fraction:02}")
    };
    Some(format!("{body} in"))
}

/// Add the piping components this drawing places that its `.pid` does
/// not draw ([`ExpTables::dump_only_piping_components`]), keeping
/// [`PidPublishDrawing::items`] and [`PidPublishDrawing::representations`]
/// ordered by UID. Run it before [`attach_oracle_ports`] so their ports
/// hang on them. Returns how many were added.
///
/// # Errors
///
/// Returns [`ExpError::Table`] when a followed table lacks a join column
/// or holds a cell that was not read in a followed one.
pub fn attach_dump_only_components(
    drawing: &mut PidPublishDrawing,
    tables: &ExpTables,
) -> Result<usize, ExpError> {
    let drawn: HashSet<String> = drawing
        .representations
        .iter()
        .map(|representation| norm_uid(&representation.uid))
        .collect();
    let known: HashSet<String> = drawing
        .items
        .iter()
        .map(|item| norm_uid(&item.uid))
        .collect();
    let found = tables.dump_only_piping_components(&drawing.drawing_uid, &drawn)?;
    let mut added = 0;
    for (item, representation) in found {
        if !known.contains(&item.uid) {
            drawing.items.push(item);
            added += 1;
        }
        drawing.representations.push(representation);
    }
    drawing
        .items
        .sort_by(|left, right| left.uid.cmp(&right.uid));
    drawing
        .representations
        .sort_by(|left, right| left.uid.cmp(&right.uid));
    Ok(added)
}

/// Hang the dump's port rows on items [`published_parent_ids`] already emits.
///
/// [`super::pid_load::load_publish_from_pid`] itself is unchanged.
pub fn attach_oracle_ports(
    drawing: &mut PidPublishDrawing,
    tables: &PortTables,
    dump: &std::path::Path,
) {
    let parents = published_parent_ids(drawing);
    let selected = ports_for_parents(tables, &parents);
    let mut ports = Vec::with_capacity(selected.piping.len() + selected.signal.len());
    let mut unspelled = 0usize;
    for point in &selected.piping {
        if point
            .nominal_diameter
            .is_some_and(|code| nominal_spelling(code).is_none())
        {
            unspelled += 1;
        }
        ports.push(piping_port(point));
    }
    for point in &selected.signal {
        ports.push(signal_port(point));
    }
    drawing.ports = ports;
    drawing.exp = Some(ExpPublish {
        dump_path: dump.display().to_string(),
        nominal_diameter_unspelled: unspelled,
    });
}

fn backup(table: &'static str, column: &'static str) -> PidValueSource {
    PidValueSource::BackupRow { table, column }
}

fn piping_port(point: &PipingPoint) -> PidPort {
    let mut attributes = vec![PidPublishAttribute {
        interface: "IObject",
        name: "Name",
        value: point.number.to_string(),
        source: backup("T_PIPINGPOINT", "PIPINGPOINTNUMBER"),
    }];
    if let Some(code) = point.flow_direction.and_then(flow_spelling) {
        attributes.push(PidPublishAttribute {
            interface: "IConnection",
            name: "ConnectionFlowDirection",
            value: code.to_string(),
            source: backup("T_PIPINGPOINT", "FLOWDIRECTION"),
        });
    }
    if let Some(spelled) = point.nominal_diameter.and_then(nominal_spelling) {
        attributes.push(PidPublishAttribute {
            interface: "IPipeCrossSectionItem",
            name: "NominalDiameter",
            value: spelled,
            source: backup("T_PIPINGPOINT", "NOMINALDIAMETER"),
        });
    }
    PidPort {
        uid: point.uid(),
        parent_uid: point.plant_item_id.clone(),
        class: "PIDPipingPort",
        attributes,
    }
}

fn signal_port(point: &SignalPoint) -> PidPort {
    PidPort {
        uid: point.uid(),
        parent_uid: point.plant_item_id.clone(),
        class: "PIDSignalPort",
        attributes: vec![PidPublishAttribute {
            interface: "IObject",
            name: "Name",
            value: point.number.to_string(),
            source: backup("T_SIGNALPOINT", "SIGNALPOINTNUMBER"),
        }],
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Char,
    /// A character column that is not AL16UTF16 national text. The payload
    /// is skipped, and [`column_index`] refuses the column.
    Bytes,
    Num,
    /// Oracle `DATE` (type code 12). The payload is skipped.
    Date,
}

struct Column {
    name: String,
    kind: Kind,
    maxlen: u16,
    /// The first row whose cell in this column was kept unread
    /// ([`Field::Unscaled`]). [`optional_column_index`] refuses a column
    /// that has one.
    unread_row: Option<usize>,
}

enum Field {
    Null,
    Int(i64),
    Text(String),
    /// An Oracle `DATE` cell decoded to `YYYY/MM/DD HH:MM:SS`.
    Date(String),
    /// A cell kept unread: a `NUMBER` that is not an integer `i64` holds, a
    /// `DATE` that does not decode, or a skipped byte string.
    Unscaled,
}

struct Table {
    /// The `INSERT INTO` name, for error messages.
    name: String,
    columns: Vec<Column>,
    rows: Vec<Vec<Field>>,
}

impl Table {
    fn blank(name: &str) -> Self {
        Self {
            name: name.to_string(),
            columns: Vec::new(),
            rows: Vec::new(),
        }
    }
}

fn applications_pinned(table: &Table) -> bool {
    let Some(row) = table.rows.first() else {
        return false;
    };
    int_at(row, &table.columns, "ID") == Some(1)
        && text_at(row, &table.columns, "NAME").as_deref() == Some("SmartPlant PID")
        && text_at(row, &table.columns, "SHORTNAME").as_deref() == Some("SPPID")
}

fn project_piping(table: &Table) -> Result<Vec<PipingPoint>, ExpError> {
    for followed in [
        "SP_PLANTITEMID",
        "PIPINGPOINTNUMBER",
        "FLOWDIRECTION",
        "NOMINALDIAMETER",
    ] {
        column_index(table, followed)?;
    }
    let mut points = Vec::with_capacity(table.rows.len());
    for (index, row) in table.rows.iter().enumerate() {
        let plant_item_id = required_text("T_PIPINGPOINT", table, row, "SP_PLANTITEMID", index)?;
        let number = required_int("T_PIPINGPOINT", table, row, "PIPINGPOINTNUMBER", index)?;
        points.push(PipingPoint {
            plant_item_id,
            number,
            flow_direction: optional_int("T_PIPINGPOINT", table, row, "FLOWDIRECTION", index)?,
            nominal_diameter: optional_int("T_PIPINGPOINT", table, row, "NOMINALDIAMETER", index)?,
        });
    }
    Ok(points)
}

fn project_signal(table: &Table) -> Result<Vec<SignalPoint>, ExpError> {
    for followed in ["SP_PLANTITEMID", "SIGNALPOINTNUMBER"] {
        column_index(table, followed)?;
    }
    let mut points = Vec::with_capacity(table.rows.len());
    for (index, row) in table.rows.iter().enumerate() {
        points.push(SignalPoint {
            plant_item_id: required_text("T_SIGNALPOINT", table, row, "SP_PLANTITEMID", index)?,
            number: required_int("T_SIGNALPOINT", table, row, "SIGNALPOINTNUMBER", index)?,
        });
    }
    Ok(points)
}

fn required_text(
    table_name: &str,
    table: &Table,
    row: &[Field],
    name: &str,
    index: usize,
) -> Result<String, ExpError> {
    match text_at(row, &table.columns, name) {
        Some(text) if !text.is_empty() => Ok(text),
        _ => Err(table_error(
            table_name,
            format!("row {index} {name} is empty"),
        )),
    }
}

fn required_int(
    table_name: &str,
    table: &Table,
    row: &[Field],
    name: &str,
    index: usize,
) -> Result<i64, ExpError> {
    int_at(row, &table.columns, name)
        .ok_or_else(|| table_error(table_name, format!("row {index} {name} is not an integer")))
}

fn optional_int(
    table_name: &str,
    table: &Table,
    row: &[Field],
    name: &str,
    index: usize,
) -> Result<Option<i64>, ExpError> {
    match field_at(row, &table.columns, name) {
        Some(Field::Null) => Ok(None),
        Some(Field::Int(value)) => Ok(Some(*value)),
        _ => Err(table_error(
            table_name,
            format!("row {index} {name} is not an integer or NULL"),
        )),
    }
}

fn table_error(name: &str, detail: String) -> ExpError {
    ExpError::Table {
        name: name.to_string(),
        detail,
    }
}

fn int_at(row: &[Field], columns: &[Column], name: &str) -> Option<i64> {
    match field_at(row, columns, name)? {
        Field::Int(value) => Some(*value),
        Field::Null | Field::Text(_) | Field::Date(_) | Field::Unscaled => None,
    }
}

fn text_at(row: &[Field], columns: &[Column], name: &str) -> Option<String> {
    match field_at(row, columns, name)? {
        Field::Text(text) | Field::Date(text) => Some(text.clone()),
        Field::Null => Some(String::new()),
        Field::Int(value) => Some(value.to_string()),
        Field::Unscaled => None,
    }
}

fn field_at<'a>(row: &'a [Field], columns: &[Column], name: &str) -> Option<&'a Field> {
    let index = columns.iter().position(|column| column.name == name)?;
    row.get(index)
}

fn read_table(dump: &[u8], name: &str) -> Result<Table, ExpError> {
    let fail = |detail: String| ExpError::Table {
        name: name.to_string(),
        detail,
    };
    let marker = format!("INSERT INTO \"{name}\"");
    let at =
        find_slice(dump, marker.as_bytes()).ok_or_else(|| fail("INSERT not found".to_string()))?;
    let line_end = dump[at..]
        .iter()
        .position(|byte| *byte == b'\n')
        .map(|offset| at + offset)
        .ok_or_else(|| fail("INSERT line has no newline".to_string()))?;
    let line = std::str::from_utf8(&dump[at..line_end])
        .map_err(|_| fail("INSERT line is not ASCII".to_string()))?;
    let names = column_names(line).map_err(fail)?;
    let (columns, mut pos) = parse_header(dump, line_end + 1, &names).map_err(fail)?;
    let mut rows = Vec::new();
    loop {
        let peek = read_u16(dump, pos).map_err(fail)?;
        if peek == 0xFFFF {
            return Ok(Table {
                name: name.to_string(),
                columns,
                rows,
            });
        }
        let mut row = Vec::with_capacity(columns.len());
        for column in &columns {
            let (field, next) = read_field(dump, pos, column)
                .map_err(|detail| fail(format!("row {} {detail}", rows.len())))?;
            pos = next;
            row.push(field);
        }
        if dump.get(pos..pos + 2) != Some(&[0, 0]) {
            return Err(fail(format!(
                "row {} separator is {:02x?} at {pos:#x}",
                rows.len(),
                dump.get(pos..pos + 2)
            )));
        }
        pos += 2;
        rows.push(row);
    }
}

fn column_names(line: &str) -> Result<Vec<String>, String> {
    let open = line.find('(').ok_or("INSERT has no column list")?;
    let close = line.find(") VALUES").ok_or("INSERT has no VALUES")?;
    let list = line
        .get(open + 1..close)
        .ok_or("INSERT column list does not come before VALUES")?;
    let mut names = Vec::new();
    for piece in list.split(',') {
        let name = piece.trim().trim_matches('"');
        if name.is_empty() {
            return Err("empty column name".to_string());
        }
        names.push(name.to_string());
    }
    Ok(names)
}

fn parse_header(
    dump: &[u8],
    mut pos: usize,
    names: &[String],
) -> Result<(Vec<Column>, usize), String> {
    let count = read_u16(dump, pos)? as usize;
    pos += 2;
    if count != names.len() {
        return Err(format!(
            "header columns {count} != INSERT names {}",
            names.len()
        ));
    }
    let mut columns = Vec::with_capacity(count);
    for name in names {
        let kind_code = read_u16(dump, pos)?;
        pos += 2;
        let (kind, maxlen) = match kind_code {
            1 => {
                let maxlen = read_u16(dump, pos)?;
                pos += 2;
                let charset = read_u16(dump, pos)?;
                pos += 2;
                let form = read_u16(dump, pos)?;
                pos += 2;
                if charset != CHARSET_AL16UTF16 || form != CHARSET_FORM {
                    return Err(format!(
                        "column {name} charset {charset} form {form}, want 2000/2"
                    ));
                }
                (Kind::Char, maxlen)
            }
            2 => {
                let maxlen = read_u16(dump, pos)?;
                pos += 2;
                (Kind::Num, maxlen)
            }
            other => return Err(format!("column {name} type {other:#x}")),
        };
        columns.push(Column {
            name: name.clone(),
            kind,
            maxlen,
            unread_row: None,
        });
    }
    if dump.get(pos..pos + 4) != Some(&[0, 0, 0, 0]) {
        return Err(format!("column header terminator at {pos:#x}"));
    }
    Ok((columns, pos + 4))
}

fn read_field(dump: &[u8], pos: usize, column: &Column) -> Result<(Field, usize), String> {
    let len = read_u16(dump, pos)? as usize;
    let pos = pos + 2;
    if len == 0xFFFE {
        return Ok((Field::Null, pos));
    }
    if len > column.maxlen as usize + 8 {
        return Err(format!("length {len} at {pos:#x}"));
    }
    let raw = dump
        .get(pos..pos + len)
        .ok_or_else(|| format!("length {len} at {pos:#x}"))?;
    let next = pos + len;
    let field = match column.kind {
        Kind::Char => {
            if !len.is_multiple_of(2) {
                return Err(format!("odd UTF-16 length {len}"));
            }
            Field::Text(decode_utf16_be(raw))
        }
        Kind::Num => Field::Int(
            decode_i64(raw).ok_or_else(|| format!("NUMBER at {pos:#x} is not an integer"))?,
        ),
        Kind::Bytes | Kind::Date => {
            return Err(format!("column {} is not a port-table type", column.name));
        }
    };
    Ok((field, next))
}

fn decode_utf16_be(raw: &[u8]) -> String {
    let (chunks, _) = raw.as_chunks::<2>();
    let units = chunks
        .iter()
        .map(|pair| u16::from_be_bytes(*pair))
        .collect::<Vec<_>>();
    String::from_utf16_lossy(&units)
}

/// Oracle internal `NUMBER` to `i64`.
///
/// `80` is 0, `c1 02` is 1, `c2 1a 34` is 2551, `3e 64 66` is −1.
/// A fraction or a value that does not fit returns `None`.
fn decode_i64(raw: &[u8]) -> Option<i64> {
    if raw.is_empty() {
        return None;
    }
    if raw == [0x80] {
        return Some(0);
    }
    let negative = raw[0] < 0x80;
    let body = if negative && raw.last() == Some(&0x66) && raw.len() < 21 {
        &raw[..raw.len() - 1]
    } else {
        raw
    };
    if body.is_empty() {
        return None;
    }
    let exponent = if negative {
        62 - i32::from(body[0])
    } else {
        i32::from(body[0]) - 193
    };
    let mut total: i128 = 0;
    for (index, byte) in body[1..].iter().enumerate() {
        let digit = if negative {
            101 - i32::from(*byte)
        } else {
            i32::from(*byte) - 1
        };
        if !(0..100).contains(&digit) {
            return None;
        }
        let power = exponent - index as i32;
        if power < 0 {
            if digit != 0 {
                return None;
            }
            continue;
        }
        let mut place: i128 = i128::from(digit);
        for _ in 0..power {
            place = place.checked_mul(100)?;
        }
        total = total.checked_add(place)?;
    }
    let signed = if negative { -total } else { total };
    i64::try_from(signed).ok()
}

fn read_u16(dump: &[u8], pos: usize) -> Result<u16, String> {
    let bytes = dump
        .get(pos..pos + 2)
        .ok_or_else(|| format!("truncated at {pos:#x}"))?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn find_slice(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len())
        .position(|window| window == needle)
}

/// The connectivity relationships on `drawing_uid`, with their definition.
///
/// The rule is `docs/analysis/n7-connectivity-rows.md`. A pair is the
/// symbol end (`UID1`: a port, a `.BPT`, or the lesser OPC id) and the
/// run end (`UID2`: `T_PIPERUN` or `T_SIGNALRUN`). The definition follows
/// the symbol's item on that run: an item with points on both the start
/// and the end side of the run's connectors is one the run passes
/// through, and every pair of it is `TapOrFitting`; an item on the start
/// side only is `End1Conn`, on the end side only `End2Conn`. Piping or
/// signal follows the run's table. On I井场 that is 738 of 738 reference
/// rows, 0 wrong.
///
/// # Errors
///
/// Returns [`ExpError::FormatUnpinned`] when `APPLICATIONS` does not pin
/// the row format, and [`ExpError::Table`] when a followed table is not
/// that format. `T_REPRESENTATION` and `T_OPC` contain a date and a
/// byte-string column; those cells are skipped. The rows are
/// [`ExpTables::connectivity_ends`] after this function has loaded the
/// tables.
pub fn connectivity_ends_for_drawing(
    dump: &[u8],
    drawing_uid: &str,
) -> Result<Vec<ConnectivityEnd>, ExpError> {
    ExpTables::load_connectivity(dump)?.connectivity_ends(drawing_uid)
}

impl ExpTables {
    /// Connectivity ends of one drawing, from tables already read.
    ///
    /// # Errors
    ///
    /// Returns [`ExpError::Table`] when a followed table is missing a
    /// column or holds a cell that was not read in one, and when a point
    /// table does not project: the ends are looked up among the rows
    /// [`Self::port_tables`] reads, on its terms.
    pub fn connectivity_ends(&self, drawing_uid: &str) -> Result<Vec<ConnectivityEnd>, ExpError> {
        let drawing_uid = norm_uid(drawing_uid);
        let connector = &self.connector;
        let reps = &self.representation;
        let piping = &self.piping_point;
        let signal = &self.signal_point;
        let runs = &self.piperun;
        let signal_runs = &self.signalrun;
        let opc = &self.opc;

        let mut rep_of = HashMap::<String, (String, String)>::new();
        let rep_id = column_index(reps, "SP_ID")?;
        let rep_model = column_index(reps, "SP_MODELITEMID")?;
        let rep_drawing = column_index(reps, "SP_DRAWINGID")?;
        for row in &reps.rows {
            let Some(id) = text_of(row, rep_id) else {
                continue;
            };
            rep_of.insert(
                norm_uid(id),
                (
                    text_of(row, rep_model).map(norm_uid).unwrap_or_default(),
                    text_of(row, rep_drawing).map(norm_uid).unwrap_or_default(),
                ),
            );
        }
        // The port stage's own projection of the two point tables: a row it
        // refuses is refused here in the same words, and a connector end is
        // looked up among exactly the points that stage publishes.
        let pipe_pts: HashSet<(String, i64)> = project_piping(piping)?
            .into_iter()
            .map(|point| (norm_uid(&point.plant_item_id), point.number))
            .collect();
        let sig_pts: HashSet<(String, i64)> = project_signal(signal)?
            .into_iter()
            .map(|point| (norm_uid(&point.plant_item_id), point.number))
            .collect();
        let run_ids = id_set(runs, "SP_ID")?;
        let srun_ids = id_set(signal_runs, "SP_ID")?;
        let mut opc_pair = HashMap::<String, Option<String>>::new();
        let opc_id = column_index(opc, "SP_ID")?;
        let opc_paired = column_index(opc, "SP_PAIREDWITHID")?;
        for row in &opc.rows {
            let Some(id) = text_of(row, opc_id) else {
                continue;
            };
            let other = text_of(row, opc_paired)
                .map(norm_uid)
                .filter(|text| !text.is_empty());
            opc_pair.insert(norm_uid(id), other);
        }

        let sp_id = column_index(connector, "SP_ID")?;
        let item1 = column_index(connector, "SP_CONNECTITEM1ID")?;
        let item2 = column_index(connector, "SP_CONNECTITEM2ID")?;
        let start_object = column_index(connector, "STARTOBJECT")?;
        let end_object = column_index(connector, "ENDOBJECT")?;
        let mut ends: HashMap<(String, String), ConnectivityEnd> = HashMap::new();
        let index = EndIndex {
            rep_of: &rep_of,
            pipe_pts: &pipe_pts,
            sig_pts: &sig_pts,
            run_ids: &run_ids,
            srun_ids: &srun_ids,
            opc_pair: &opc_pair,
        };
        for row in &connector.rows {
            let Some(connector_id) = text_of(row, sp_id) else {
                continue;
            };
            let Some((model, drawing)) = rep_of.get(&norm_uid(connector_id)) else {
                continue;
            };
            if drawing != &drawing_uid {
                continue;
            }
            let run_uid = if run_ids.contains(model) || srun_ids.contains(model) {
                model.clone()
            } else {
                continue;
            };
            let sides = [
                (true, text_of(row, item1), int_of(row, start_object)),
                (false, text_of(row, item2), int_of(row, end_object)),
            ];
            for (is_start, item, object_index) in sides {
                let Some(item_id) = item else {
                    continue;
                };
                for symbol in symbol_uids(&index, &norm_uid(item_id), object_index) {
                    let entry = ends
                        .entry((symbol.clone(), run_uid.clone()))
                        .or_insert_with(|| ConnectivityEnd {
                            symbol_uid: symbol,
                            run_uid: run_uid.clone(),
                            on_start: false,
                            on_end: false,
                            def_uid: "",
                        });
                    if is_start {
                        entry.on_start = true;
                    } else {
                        entry.on_end = true;
                    }
                }
            }
        }
        // The sides an item was seen on, per run. A port's item is its
        // parent; a `.BPT` and an OPC are items of their own.
        let mut item_sides: HashMap<(String, String), (bool, bool)> = HashMap::new();
        for ((symbol, run), end) in &ends {
            let sides = item_sides
                .entry((item_of_symbol(symbol), run.clone()))
                .or_default();
            sides.0 |= end.on_start;
            sides.1 |= end.on_end;
        }
        for ((symbol, run), end) in &mut ends {
            let (item_start, item_end) = item_sides[&(item_of_symbol(symbol), run.clone())];
            end.def_uid =
                connectivity_def_uid(run_ids.contains(run), item_start && item_end, end.on_start);
        }
        let mut list: Vec<ConnectivityEnd> = ends.into_values().collect();
        list.sort_by(|left, right| {
            (&left.symbol_uid, &left.run_uid).cmp(&(&right.symbol_uid, &right.run_uid))
        });
        Ok(list)
    }
}

/// Nozzle compositions and item notes in the dump.
///
/// `T_NOZZLE` gives `EquipmentComponentComposition` (`SP_ID` the nozzle,
/// `SP_EQUIPMENTID` the equipment). `T_NOTE` gives `PBSItemNotes`
/// (`SP_ITEMNOTEID` the note, `SP_MODELITEMID` the model item). A model
/// item that is itself an item note (`T_ITEMNOTE.SP_ID`) is left out:
/// on I井场 those rows are note-to-note and the reference publishes none
/// of them as `PBSItemNotes`. The writer then keeps a row only when both
/// ends are objects this drawing already publishes. OPC branch points
/// are not read.
///
/// # Errors
///
/// Returns [`ExpError::FormatUnpinned`] when `APPLICATIONS` does not pin
/// the row format, and [`ExpError::Table`] when a followed table is not
/// that format. `T_NOTE` has a byte-string and date columns; those cells
/// are skipped. The rows are [`ExpTables::composition_relations`] after
/// this function has loaded the tables.
pub fn composition_relations(dump: &[u8]) -> Result<Vec<ExpRelation>, ExpError> {
    ExpTables::load_compositions(dump)?.composition_relations()
}

impl ExpTables {
    /// Nozzle and note relations from tables already read.
    ///
    /// # Errors
    ///
    /// Returns [`ExpError::Table`] when a followed table is missing a
    /// column or holds a cell that was not read in one.
    pub fn composition_relations(&self) -> Result<Vec<ExpRelation>, ExpError> {
        let nozzles = &self.nozzle;
        let notes = &self.note;
        let item_notes = &self.itemnote;
        let item_note_ids = id_set(item_notes, "SP_ID")?;

        let mut rels = Vec::new();
        let nozzle_id = column_index(nozzles, "SP_ID")?;
        let equipment_id = column_index(nozzles, "SP_EQUIPMENTID")?;
        for row in &nozzles.rows {
            let Some(nozzle) = text_of(row, nozzle_id) else {
                continue;
            };
            let Some(equipment) = text_of(row, equipment_id) else {
                continue;
            };
            rels.push(ExpRelation {
                def_uid: "EquipmentComponentComposition",
                uid1: norm_uid(nozzle),
                uid2: norm_uid(equipment),
            });
        }
        let note_id = column_index(notes, "SP_ITEMNOTEID")?;
        let model_id = column_index(notes, "SP_MODELITEMID")?;
        for row in &notes.rows {
            let Some(note) = text_of(row, note_id) else {
                continue;
            };
            let Some(model) = text_of(row, model_id) else {
                continue;
            };
            let model = norm_uid(model);
            if item_note_ids.contains(&model) {
                continue;
            }
            rels.push(ExpRelation {
                def_uid: "PBSItemNotes",
                uid1: norm_uid(note),
                uid2: model,
            });
        }
        rels.sort_by(|left, right| {
            (left.def_uid, &left.uid1, &left.uid2).cmp(&(right.def_uid, &right.uid1, &right.uid2))
        });
        rels.dedup();
        Ok(rels)
    }
}

/// Off-page connectors drawn on `drawing_uid`.
///
/// A row is a `T_REPRESENTATION` whose drawing is this one and whose
/// model item is in `T_OPC`. The published UID is the lesser of that
/// item and `SP_PAIREDWITHID` (the same id the connectivity end uses).
/// `Name` / `ToFromText` / `Description` come from the drawn side, and
/// from the other side only when the drawn side is empty. The caller
/// drops a point whose representation this `.pid` does not publish.
///
/// # Errors
///
/// Returns [`ExpError::FormatUnpinned`] when `APPLICATIONS` does not pin
/// the row format, and [`ExpError::Table`] when a followed table is not
/// that format. Byte-string and date cells are skipped. The rows are
/// [`ExpTables::opc_points`] after this function has loaded the tables.
pub fn opc_points_for_drawing(
    dump: &[u8],
    drawing_uid: &str,
) -> Result<Vec<PidOpcPoint>, ExpError> {
    ExpTables::load_opc(dump)?.opc_points(drawing_uid)
}

impl ExpTables {
    /// Off-page connectors of one drawing, from tables already read.
    ///
    /// # Errors
    ///
    /// Returns [`ExpError::Table`] when a followed table is missing a
    /// column or holds a cell that was not read in one.
    pub fn opc_points(&self, drawing_uid: &str) -> Result<Vec<PidOpcPoint>, ExpError> {
        let drawing_uid = norm_uid(drawing_uid);
        let opc = &self.opc;
        let reps = &self.representation;
        let models = &self.modelitem;

        let opc_id = column_index(opc, "SP_ID")?;
        let opc_paired = column_index(opc, "SP_PAIREDWITHID")?;
        let opc_tag = column_index(opc, "OPCTAG")?;
        let opc_text = column_index(opc, "TOFROMTEXT")?;
        let mut rows_of = HashMap::<String, (String, String, String)>::new();
        for row in &opc.rows {
            let Some(id) = text_of(row, opc_id) else {
                continue;
            };
            rows_of.insert(
                norm_uid(id),
                (
                    text_of(row, opc_paired).map(norm_uid).unwrap_or_default(),
                    text_of(row, opc_tag).unwrap_or("").to_string(),
                    text_of(row, opc_text).unwrap_or("").to_string(),
                ),
            );
        }
        let model_id = column_index(models, "SP_ID")?;
        let model_description = column_index(models, "DESCRIPTION")?;
        let mut descriptions = HashMap::<String, String>::new();
        for row in &models.rows {
            let Some(id) = text_of(row, model_id) else {
                continue;
            };
            if let Some(description) = text_of(row, model_description) {
                descriptions.insert(norm_uid(id), description.to_string());
            }
        }

        let rep_id = column_index(reps, "SP_ID")?;
        let rep_model = column_index(reps, "SP_MODELITEMID")?;
        let rep_drawing = column_index(reps, "SP_DRAWINGID")?;
        let mut points = Vec::new();
        for row in &reps.rows {
            if text_of(row, rep_drawing).map(norm_uid).as_deref() != Some(drawing_uid.as_str()) {
                continue;
            }
            let Some(model) = text_of(row, rep_model).map(norm_uid) else {
                continue;
            };
            let Some((paired, tag, to_from)) = rows_of.get(&model) else {
                continue;
            };
            let Some(representation) = text_of(row, rep_id) else {
                continue;
            };
            let other = rows_of.get(paired);
            let name = if !tag.is_empty() {
                tag.clone()
            } else {
                other.map(|(_, tag, _)| tag.clone()).unwrap_or_default()
            };
            let to_from_text = if !to_from.is_empty() {
                to_from.clone()
            } else {
                other.map(|(_, _, text)| text.clone()).unwrap_or_default()
            };
            let description = descriptions
                .get(&model)
                .cloned()
                .filter(|text| !text.is_empty())
                .or_else(|| descriptions.get(paired).cloned())
                .unwrap_or_default();
            points.push(PidOpcPoint {
                uid: lesser_id(&model, paired),
                representation_uid: norm_uid(representation),
                name,
                description,
                to_from_text,
            });
        }
        points.sort_by(|left, right| left.uid.cmp(&right.uid));
        points.dedup_by(|left, right| left.uid == right.uid);
        Ok(points)
    }
}

/// The item a symbol end belongs to: a port's parent (`PARENT.N` → `PARENT`),
/// a branch point (`REP.BPT`) or an OPC by itself.
fn item_of_symbol(symbol_uid: &str) -> String {
    if symbol_uid.ends_with(".BPT") {
        return symbol_uid.to_string();
    }
    match symbol_uid.rsplit_once('.') {
        Some((parent, number)) if !parent.is_empty() && number.parse::<u32>().is_ok() => {
            parent.to_string()
        }
        _ => symbol_uid.to_string(),
    }
}

/// The six definitions `sub_5F5E7A80` chooses between: `Tap` when the
/// item is not at an end of the run (its end index is `0xFFFF`), else
/// `End1` for the start side and `End2` for the end side.
fn connectivity_def_uid(piping: bool, tap: bool, on_start: bool) -> &'static str {
    match (piping, tap, on_start) {
        (true, true, _) => "PipingTapOrFitting",
        (false, true, _) => "SignalTapOrFitting",
        (true, false, true) => "PipingEnd1Conn",
        (true, false, false) => "PipingEnd2Conn",
        (false, false, true) => "SignalEnd1Conn",
        (false, false, false) => "SignalEnd2Conn",
    }
}

struct EndIndex<'a> {
    rep_of: &'a HashMap<String, (String, String)>,
    pipe_pts: &'a HashSet<(String, i64)>,
    sig_pts: &'a HashSet<(String, i64)>,
    run_ids: &'a HashSet<String>,
    srun_ids: &'a HashSet<String>,
    opc_pair: &'a HashMap<String, Option<String>>,
}

fn symbol_uids(index: &EndIndex<'_>, rep_id: &str, object_index: Option<i64>) -> Vec<String> {
    let Some((model, _)) = index.rep_of.get(rep_id) else {
        return Vec::new();
    };
    if model.is_empty() {
        return Vec::new();
    }
    if index.run_ids.contains(model) || index.srun_ids.contains(model) {
        return vec![format!("{rep_id}.BPT")];
    }
    if let Some(paired) = index.opc_pair.get(model) {
        return vec![lesser_id(model, paired.as_deref().unwrap_or(""))];
    }
    let Some(point) = object_index.filter(|value| *value > 0) else {
        return Vec::new();
    };
    let mut uids = Vec::new();
    if index.pipe_pts.contains(&(model.clone(), point)) {
        uids.push(format!("{model}.{point}"));
    }
    if index.sig_pts.contains(&(model.clone(), point)) {
        uids.push(format!("{model}.{point}"));
    }
    uids
}

fn id_set(table: &Table, name: &str) -> Result<HashSet<String>, ExpError> {
    let index = column_index(table, name)?;
    let mut ids = HashSet::new();
    for row in &table.rows {
        if let Some(text) = text_of(row, index) {
            ids.insert(norm_uid(text));
        }
    }
    Ok(ids)
}

/// The index of a column a projection requires: [`optional_column_index`],
/// with a table that lacks the column refused as well.
fn column_index(table: &Table, name: &str) -> Result<usize, ExpError> {
    optional_column_index(table, name)?
        .ok_or_else(|| table_error(&table.name, format!("column {name} is missing")))
}

/// The index of a column a projection follows when the dump has it: `None`
/// when `name` is not a column of `table`.
///
/// A column that is there is refused unless every cell of it was read. Its
/// header may declare text that is not AL16UTF16 ([`Kind::Bytes`]), or a
/// row may hold a `NUMBER` that is not an integer or a `DATE` that does not
/// decode ([`Field::Unscaled`]). The cell accessors read such a cell as an
/// empty one, so a projection that took the column anyway would publish a
/// drawing with part of the dump silently missing. Every projection takes
/// its columns through here or through [`column_index`].
fn optional_column_index(table: &Table, name: &str) -> Result<Option<usize>, ExpError> {
    let Some(index) = table.columns.iter().position(|column| column.name == name) else {
        return Ok(None);
    };
    let column = &table.columns[index];
    if column.kind == Kind::Bytes {
        return Err(table_error(
            &table.name,
            format!("column {name} is not AL16UTF16 text; its cells were not read"),
        ));
    }
    if let Some(row) = column.unread_row {
        let detail = match column.kind {
            Kind::Date => format!("row {row} {name} is a DATE that does not decode"),
            Kind::Char | Kind::Bytes | Kind::Num => {
                format!("row {row} {name} is not an integer or NULL")
            }
        };
        return Err(table_error(&table.name, detail));
    }
    Ok(Some(index))
}

/// [`cell_text`] for an [`optional_column_index`]. Absent columns read
/// as empty.
fn optional_cell_text(row: &[Field], index: Option<usize>) -> String {
    index.map_or_else(String::new, |index| cell_text(row, index))
}

/// Integer cell for an [`optional_column_index`]. `None` when the
/// column is absent, the cell is NULL, or the column is not a number
/// column.
fn optional_cell_int(row: &[Field], index: Option<usize>) -> Option<i64> {
    match index.and_then(|index| row.get(index)) {
        Some(Field::Int(value)) => Some(*value),
        _ => None,
    }
}

fn text_of(row: &[Field], index: usize) -> Option<&str> {
    match row.get(index) {
        Some(Field::Text(text)) if !text.is_empty() => Some(text),
        _ => None,
    }
}

fn int_of(row: &[Field], index: usize) -> Option<i64> {
    match row.get(index) {
        Some(Field::Int(value)) => Some(*value),
        _ => None,
    }
}

fn norm_uid(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        if ch == '{' || ch == '}' || ch == '-' || ch.is_whitespace() {
            continue;
        }
        out.extend(ch.to_uppercase());
    }
    out
}

/// The published OPC uid: the lesser of the two ids, or `model` when the
/// pair is empty. Same order `symbol_uids` uses for a connectivity end.
fn lesser_id(model: &str, paired: &str) -> String {
    if !paired.is_empty() && model > paired {
        paired.to_string()
    } else {
        model.to_string()
    }
}

/// [`read_table_tolerant`], or an empty table when the dump has no insert
/// for `name`. A present table that is not the pinned format still fails.
fn read_optional_table(dump: &[u8], name: &str) -> Result<Table, ExpError> {
    let marker = format!("INSERT INTO \"{name}\"");
    if find_slice(dump, marker.as_bytes()).is_none() {
        return Ok(Table::blank(name));
    }
    read_table_tolerant(dump, name)
}

fn read_table_tolerant(dump: &[u8], name: &str) -> Result<Table, ExpError> {
    let fail = |detail: String| ExpError::Table {
        name: name.to_string(),
        detail,
    };
    let marker = format!("INSERT INTO \"{name}\"");
    let at =
        find_slice(dump, marker.as_bytes()).ok_or_else(|| fail("INSERT not found".to_string()))?;
    let line_end = dump[at..]
        .iter()
        .position(|byte| *byte == b'\n')
        .map(|offset| at + offset)
        .ok_or_else(|| fail("INSERT line has no newline".to_string()))?;
    let line = std::str::from_utf8(&dump[at..line_end])
        .map_err(|_| fail("INSERT line is not ASCII".to_string()))?;
    let names = column_names(line).map_err(fail)?;
    let (mut columns, mut pos) = parse_header_tolerant(dump, line_end + 1, &names).map_err(fail)?;
    let mut rows = Vec::new();
    loop {
        let peek = read_u16(dump, pos).map_err(fail)?;
        if peek == 0xFFFF {
            return Ok(Table {
                name: name.to_string(),
                columns,
                rows,
            });
        }
        let mut row = Vec::with_capacity(columns.len());
        for column in &mut columns {
            let (field, next) = read_field_tolerant(dump, pos, column)
                .map_err(|detail| fail(format!("row {} {detail}", rows.len())))?;
            pos = next;
            if matches!(field, Field::Unscaled) && column.unread_row.is_none() {
                column.unread_row = Some(rows.len());
            }
            row.push(field);
        }
        if dump.get(pos..pos + 2) != Some(&[0, 0]) {
            return Err(fail(format!(
                "row {} separator is {:02x?} at {pos:#x}",
                rows.len(),
                dump.get(pos..pos + 2)
            )));
        }
        pos += 2;
        rows.push(row);
    }
}

fn parse_header_tolerant(
    dump: &[u8],
    mut pos: usize,
    names: &[String],
) -> Result<(Vec<Column>, usize), String> {
    let count = read_u16(dump, pos)? as usize;
    pos += 2;
    if count != names.len() {
        return Err(format!(
            "header columns {count} != INSERT names {}",
            names.len()
        ));
    }
    let mut columns = Vec::with_capacity(count);
    for name in names {
        let kind_code = read_u16(dump, pos)?;
        pos += 2;
        let (kind, maxlen) = match kind_code {
            1 => {
                let maxlen = read_u16(dump, pos)?;
                pos += 2;
                let charset = read_u16(dump, pos)?;
                pos += 2;
                let form = read_u16(dump, pos)?;
                pos += 2;
                if charset == CHARSET_AL16UTF16 && form == CHARSET_FORM {
                    (Kind::Char, maxlen)
                } else {
                    (Kind::Bytes, maxlen)
                }
            }
            2 => {
                let maxlen = read_u16(dump, pos)?;
                pos += 2;
                (Kind::Num, maxlen)
            }
            12 => {
                let maxlen = read_u16(dump, pos)?;
                pos += 2;
                (Kind::Date, maxlen)
            }
            other => return Err(format!("column {name} type {other:#x}")),
        };
        columns.push(Column {
            name: name.clone(),
            kind,
            maxlen,
            unread_row: None,
        });
    }
    if dump.get(pos..pos + 4) != Some(&[0, 0, 0, 0]) {
        return Err(format!("column header terminator at {pos:#x}"));
    }
    Ok((columns, pos + 4))
}

fn read_field_tolerant(dump: &[u8], pos: usize, column: &Column) -> Result<(Field, usize), String> {
    let len = read_u16(dump, pos)? as usize;
    let pos = pos + 2;
    if len == 0xFFFE {
        return Ok((Field::Null, pos));
    }
    if len > column.maxlen as usize + 8 {
        return Err(format!("length {len} at {pos:#x}"));
    }
    let raw = dump
        .get(pos..pos + len)
        .ok_or_else(|| format!("length {len} at {pos:#x}"))?;
    let next = pos + len;
    let field = match column.kind {
        Kind::Char => {
            if !len.is_multiple_of(2) {
                return Err(format!("odd UTF-16 length {len}"));
            }
            Field::Text(decode_utf16_be(raw))
        }
        Kind::Num => match decode_i64(raw) {
            Some(value) => Field::Int(value),
            None => Field::Unscaled,
        },
        Kind::Date => decode_oracle_date(raw).map_or(Field::Unscaled, Field::Date),
        Kind::Bytes => Field::Unscaled,
    };
    Ok((field, next))
}

/// Decode an Oracle 7-byte `DATE` cell into `YYYY/MM/DD HH:MM:SS`.
///
/// Bytes 0–1 hold the century and year offset by 100 each, bytes 2–3
/// are 1-based month and day, and bytes 4–6 are hour, minute and
/// second offset by 1. Any other shape is `None` — the caller stores
/// the cell as [`Field::Unscaled`] rather than a misleading date.
fn decode_oracle_date(raw: &[u8]) -> Option<String> {
    let &[century, year, month, day, hour, minute, second] = raw else {
        return None;
    };
    if !(100..=199).contains(&century)
        || !(100..=199).contains(&year)
        || !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
    {
        return None;
    }
    let year = (u32::from(century) - 100) * 100 + (u32::from(year) - 100);
    Some(format!(
        "{year:04}/{month:02}/{day:02} {:02}:{:02}:{:02}",
        hour.saturating_sub(1),
        minute.saturating_sub(1),
        second.saturating_sub(1),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nominal_and_flow_spellings_match_the_reference() {
        assert_eq!(nominal_spelling(25).as_deref(), Some("25 mm"));
        assert_eq!(nominal_spelling(5024).as_deref(), Some("0.75 in"));
        assert_eq!(nominal_spelling(5032).as_deref(), Some("1 in"));
        assert_eq!(nominal_spelling(5384).as_deref(), Some("12 in"));
        assert_eq!(nominal_spelling(0), None);
        assert_eq!(
            flow_spelling(3),
            Some("@{23F9414C-B87D-11D6-80D4-00A0C92487AF}")
        );
        assert_eq!(flow_spelling(9), None);
    }

    // A `NUMBER` cell can hold any integer `i64` does, and the inch grid
    // multiplies the code before it divides.
    #[test]
    fn a_nominal_diameter_code_at_the_far_end_of_i64_is_not_spelled() {
        assert_eq!(nominal_spelling(i64::MAX), None);
        assert_eq!(nominal_spelling(1_000_000_000_000_000_000), None);
        assert_eq!(nominal_spelling(i64::MIN), None);
        // A large code whose product still fits is spelled like any other.
        assert_eq!(
            nominal_spelling(5000 + 32 * 1_000_000).as_deref(),
            Some("1000000 in")
        );
    }

    #[test]
    fn oracle_number_samples() {
        assert_eq!(decode_i64(&[0x80]), Some(0));
        assert_eq!(decode_i64(&[0xc1, 0x02]), Some(1));
        assert_eq!(decode_i64(&[0xc2, 0x1a, 0x34]), Some(2551));
        assert_eq!(decode_i64(&[0x3e, 0x64, 0x66]), Some(-1));
        assert_eq!(decode_i64(&[0xc1, 0x03]), Some(2));
        assert_eq!(decode_i64(&[0xc1, 0x06]), Some(5));
        assert_eq!(decode_i64(&[0xc2, 0x02]), Some(100));
        assert_eq!(decode_i64(&[0xc2, 0x10, 0x1f]), Some(1530));
        assert_eq!(decode_i64(&[0x3e, 0x62, 0x66]), Some(-3));
    }

    #[test]
    fn fragment_covers_null_utf16_number_and_table_end() {
        let mut dump = table(
            "APPLICATIONS",
            &[num("ID"), ch("NAME"), ch("SHORTNAME")],
            &[row(&[
                Some(vec![0xc1, 0x02]),
                Some(utf16("SmartPlant PID")),
                Some(utf16("SPPID")),
            ])],
        );
        dump.extend(table(
            "T_PIPINGPOINT",
            &[
                ch("SP_PLANTITEMID"),
                num("PIPINGPOINTNUMBER"),
                num("FLOWDIRECTION"),
                num("NOMINALDIAMETER"),
            ],
            &[row(&[
                Some(utf16("AB")),
                Some(vec![0xc2, 0x1a, 0x34]),
                None,
                None,
            ])],
        ));
        dump.extend(b"\nCREATE TABLE ignored");
        dump.extend(table(
            "T_SIGNALPOINT",
            &[ch("SP_PLANTITEMID"), num("SIGNALPOINTNUMBER")],
            &[
                row(&[Some(utf16("AB")), Some(vec![0x80])]),
                row(&[Some(utf16("CD")), Some(vec![0xc1, 0x02])]),
            ],
        ));
        dump.extend(b"\nGRANT ignored");

        let tables = read_port_tables(&dump).unwrap();
        assert_eq!(tables.piping_row_count, 1);
        assert_eq!(tables.signal_row_count, 2);
        assert_eq!(tables.piping_points.len(), 1);
        let piping = &tables.piping_points[0];
        assert_eq!(piping.plant_item_id, "AB");
        assert_eq!(piping.number, 2551);
        assert_eq!(piping.flow_direction, None);
        assert_eq!(piping.nominal_diameter, None);
        assert_eq!(piping.uid(), "AB.2551");
        assert_eq!(tables.signal_points[0].number, 0);
        assert_eq!(tables.signal_points[1].uid(), "CD.1");

        let parents = HashSet::from(["AB".to_string()]);
        let selected = ports_for_parents(&tables, &parents);
        assert_eq!(selected.piping.len(), 1);
        assert_eq!(selected.signal.len(), 1);
        assert_eq!(selected.signal[0].plant_item_id, "AB");
    }

    #[test]
    fn unpinned_applications_does_not_read_port_tables() {
        let dump = table(
            "APPLICATIONS",
            &[num("ID"), ch("NAME"), ch("SHORTNAME")],
            &[row(&[
                Some(vec![0xc1, 0x03]),
                Some(utf16("SmartPlant PID")),
                Some(utf16("SPPID")),
            ])],
        );
        let error = read_port_tables(&dump).unwrap_err();
        assert!(matches!(error, ExpError::FormatUnpinned), "{error}");
        assert!(!error.to_string().contains("T_PIPINGPOINT"));
    }

    #[derive(Clone, Copy)]
    struct Col {
        name: &'static str,
        kind: u16,
    }

    fn num(name: &'static str) -> Col {
        Col { name, kind: 2 }
    }

    fn ch(name: &'static str) -> Col {
        Col { name, kind: 1 }
    }

    fn utf16(text: &str) -> Vec<u8> {
        text.encode_utf16().flat_map(u16::to_be_bytes).collect()
    }

    fn row(fields: &[Option<Vec<u8>>]) -> Vec<Option<Vec<u8>>> {
        fields.to_vec()
    }

    fn table(name: &str, columns: &[Col], rows: &[Vec<Option<Vec<u8>>>]) -> Vec<u8> {
        let listed = columns
            .iter()
            .map(|column| format!("\"{}\"", column.name))
            .collect::<Vec<_>>()
            .join(", ");
        let mut out = format!("INSERT INTO \"{name}\" ({listed}) VALUES\n").into_bytes();
        out.extend(u16::try_from(columns.len()).unwrap().to_le_bytes());
        for column in columns {
            let kind = if column.kind == DB_CHAR || column.kind == NATIONAL_UTF8 {
                1
            } else {
                column.kind
            };
            out.extend(kind.to_le_bytes());
            out.extend(4000u16.to_le_bytes());
            if column.kind == 1 {
                out.extend(CHARSET_AL16UTF16.to_le_bytes());
                out.extend(CHARSET_FORM.to_le_bytes());
            } else if column.kind == DB_CHAR {
                out.extend(852u16.to_le_bytes());
                out.extend(1u16.to_le_bytes());
            } else if column.kind == NATIONAL_UTF8 {
                out.extend(871u16.to_le_bytes());
                out.extend(CHARSET_FORM.to_le_bytes());
            }
        }
        out.extend([0, 0, 0, 0]);
        for fields in rows {
            for field in fields {
                match field {
                    None => out.extend(0xFFFEu16.to_le_bytes()),
                    Some(payload) => {
                        out.extend(u16::try_from(payload.len()).unwrap().to_le_bytes());
                        out.extend(payload);
                    }
                }
            }
            out.extend([0, 0]);
        }
        out.extend(0xFFFFu16.to_le_bytes());
        out
    }

    fn date(name: &'static str) -> Col {
        Col { name, kind: 12 }
    }

    /// Header kind the fixture writes as a character column in the database
    /// character set (charset 852, form 1) instead of AL16UTF16.
    const DB_CHAR: u16 = 0x0101;

    fn ch_db(name: &'static str) -> Col {
        Col {
            name,
            kind: DB_CHAR,
        }
    }

    /// Header kind the fixture writes as a national character column whose
    /// character set is UTF8 (charset 871, form 2): the form AL16UTF16 text
    /// carries, under another charset id.
    const NATIONAL_UTF8: u16 = 0x0201;

    fn ch_national_utf8(name: &'static str) -> Col {
        Col {
            name,
            kind: NATIONAL_UTF8,
        }
    }

    fn one() -> Vec<u8> {
        vec![0xc1, 0x02]
    }

    fn pinned() -> Vec<u8> {
        table(
            "APPLICATIONS",
            &[num("ID"), ch("NAME"), ch("SHORTNAME")],
            &[row(&[
                Some(one()),
                Some(utf16("SmartPlant PID")),
                Some(utf16("SPPID")),
            ])],
        )
    }

    #[test]
    fn a_column_list_after_values_is_refused_not_a_panic() {
        assert!(column_names("INSERT INTO \"T_X\" ) VALUES (").is_err());
        assert!(ExpTables::read(b"INSERT INTO \"APPLICATIONS\" ) VALUES (\n").is_err());
    }

    /// The tables [`ExpTables::read`] requires beyond the ones a test
    /// states itself, all empty.
    fn other_tables(stated: &[&str]) -> Vec<u8> {
        let mut dump = Vec::new();
        let empty: [(&str, &[Col]); 10] = [
            (
                "T_PIPINGPOINT",
                &[
                    ch("SP_PLANTITEMID"),
                    num("PIPINGPOINTNUMBER"),
                    num("FLOWDIRECTION"),
                    num("NOMINALDIAMETER"),
                ],
            ),
            (
                "T_SIGNALPOINT",
                &[ch("SP_PLANTITEMID"), num("SIGNALPOINTNUMBER")],
            ),
            ("T_CONNECTOR", &[ch("SP_ID")]),
            ("T_REPRESENTATION", &[ch("SP_ID")]),
            ("T_SIGNALRUN", &[ch("SP_ID")]),
            ("T_OPC", &[ch("SP_ID")]),
            ("T_NOZZLE", &[ch("SP_ID")]),
            ("T_NOTE", &[ch("SP_ID")]),
            ("T_ITEMNOTE", &[ch("SP_ID")]),
            ("T_MODELITEM", &[ch("SP_ID")]),
        ];
        for (name, columns) in empty {
            if !stated.contains(&name) {
                dump.extend(table(name, columns, &[]));
            }
        }
        dump.extend(table(
            "CODELISTS",
            &[
                num("CODELIST_NUMBER"),
                num("CODELIST_INDEX"),
                ch("CODELIST_SHORT_TEXT"),
            ],
            &[],
        ));
        dump.extend(table(
            "T_DRAWING",
            &[
                ch("SP_ID"),
                ch("NAME"),
                ch("PATH"),
                ch("DRAWINGNUMBER"),
                ch("SP_PLANTGROUPID"),
            ],
            &[],
        ));
        dump
    }

    /// The seven `T_PIPERUN` columns [`ExpTables::pipe_runs`] follows.
    fn pipe_run_columns() -> Vec<Col> {
        vec![
            ch("SP_ID"),
            num("NOMINALDIAMETER"),
            num("OPERFLUIDCODE"),
            ch("PIPINGMATERIALSCLASS"),
            ch("TAGSEQUENCENO"),
            num("FLUIDSYSTEM"),
            ch("INSULATIONSPEC"),
        ]
    }

    // DWG-0202GP06-01_p's first pipe run is insulated 50 mm, which its
    // `SP_INSULTHICKSI` states as 0.05; a plant also writes dates and
    // database-charset text into the table's other columns. None of those
    // is a column a stage follows, so the run reads.
    #[test]
    fn a_pipe_run_reads_past_the_columns_no_stage_follows() {
        let mut columns = pipe_run_columns();
        columns.extend([
            num("SP_INSULTHICKSI"),
            date("DATECREATED"),
            ch_db("REMARKS"),
        ]);
        let mut dump = pinned();
        dump.extend(table(
            "T_PIPERUN",
            &columns,
            &[row(&[
                Some(utf16("RUN1")),
                Some(vec![0xc2, 0x02]),
                None,
                Some(utf16("B5")),
                Some(utf16("0101")),
                None,
                None,
                // 0.05: one base-100 digit, 5, at exponent -1.
                Some(vec![0xc0, 0x06]),
                Some(vec![120, 126, 4, 25, 20, 12, 1]),
                Some(b"x".to_vec()),
            ])],
        ));
        dump.extend(other_tables(&[]));
        let tables = ExpTables::read(&dump).expect("the dump reads");
        let runs = tables.pipe_runs().expect("the followed columns read");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].id, "RUN1");
        assert_eq!(runs[0].nominal_diameter, Some(100));
        assert_eq!(runs[0].materials_class, "B5");
        assert_eq!(runs[0].tag_sequence, "0101");
    }

    #[test]
    fn a_fraction_in_a_followed_column_is_refused_with_its_table() {
        let mut dump = pinned();
        dump.extend(table(
            "T_PIPERUN",
            &pipe_run_columns(),
            &[row(&[
                Some(utf16("RUN1")),
                Some(vec![0xc0, 0x1f]),
                None,
                None,
                Some(utf16("0101")),
                None,
                None,
            ])],
        ));
        dump.extend(other_tables(&[]));
        let tables = ExpTables::read(&dump).expect("the dump reads");
        let error = tables.pipe_runs().unwrap_err().to_string();
        assert!(error.contains("T_PIPERUN"), "{error}");
        assert!(error.contains("NOMINALDIAMETER"), "{error}");
    }

    #[test]
    fn a_followed_column_left_unread_is_refused_with_its_table() {
        let mut columns = pipe_run_columns();
        columns[4] = ch_db("TAGSEQUENCENO");
        let mut dump = pinned();
        dump.extend(table(
            "T_PIPERUN",
            &columns,
            &[row(&[
                Some(utf16("RUN1")),
                None,
                None,
                None,
                Some(b"0101".to_vec()),
                None,
                None,
            ])],
        ));
        dump.extend(other_tables(&[]));
        let tables = ExpTables::read(&dump).expect("the dump reads");
        let error = tables.pipe_runs().unwrap_err().to_string();
        assert!(error.contains("T_PIPERUN"), "{error}");
        assert!(error.contains("TAGSEQUENCENO"), "{error}");
        assert!(error.contains("not AL16UTF16"), "{error}");

        let mut dump = pinned();
        dump.extend(table(
            "T_PIPINGPOINT",
            &[
                ch_db("SP_PLANTITEMID"),
                num("PIPINGPOINTNUMBER"),
                num("FLOWDIRECTION"),
                num("NOMINALDIAMETER"),
            ],
            &[row(&[Some(b"AB".to_vec()), Some(one()), None, None])],
        ));
        dump.extend(table(
            "T_SIGNALPOINT",
            &[ch("SP_PLANTITEMID"), num("SIGNALPOINTNUMBER")],
            &[],
        ));
        let error = read_port_tables(&dump).unwrap_err().to_string();
        assert!(error.contains("T_PIPINGPOINT"), "{error}");
        assert!(error.contains("SP_PLANTITEMID"), "{error}");
        assert!(error.contains("not AL16UTF16"), "{error}");
    }

    // The header's form says national text; only charset id 2000 says
    // UTF-16. A database whose national character set is UTF8 writes form 2
    // under id 871, and its bytes read as UTF-16 would publish a tag
    // sequence nobody wrote.
    #[test]
    fn national_text_in_another_charset_is_kept_unread() {
        let mut columns = pipe_run_columns();
        columns[4] = ch_national_utf8("TAGSEQUENCENO");
        let mut dump = pinned();
        dump.extend(table(
            "T_PIPERUN",
            &columns,
            &[row(&[
                Some(utf16("RUN1")),
                None,
                None,
                None,
                Some(b"0101".to_vec()),
                None,
                None,
            ])],
        ));
        dump.extend(other_tables(&[]));
        let tables = ExpTables::read(&dump).expect("the dump reads");
        let error = tables.pipe_runs().unwrap_err().to_string();
        assert_eq!(
            error,
            "T_PIPERUN: column TAGSEQUENCENO is not AL16UTF16 text; its cells were not read"
        );
    }

    // A port table answers for its columns before any row is read: a table
    // with no rows still has to carry them, and a column that was not read
    // is reported as that, not as a row whose cell is empty.
    #[test]
    fn a_port_table_answers_for_its_columns_before_its_rows() {
        let ports = |piping: Vec<u8>, signal: Vec<u8>| {
            let mut dump = pinned();
            dump.extend(piping);
            dump.extend(signal);
            read_port_tables(&dump)
                .map(|tables| (tables.piping_points.len(), tables.signal_points.len()))
        };
        let piping_columns = [
            ch("SP_PLANTITEMID"),
            num("PIPINGPOINTNUMBER"),
            num("FLOWDIRECTION"),
            num("NOMINALDIAMETER"),
        ];
        let signal_columns = [ch("SP_PLANTITEMID"), num("SIGNALPOINTNUMBER")];
        let no_piping = || table("T_PIPINGPOINT", &piping_columns, &[]);
        let no_signal = || table("T_SIGNALPOINT", &signal_columns, &[]);
        assert_eq!(
            ports(no_piping(), no_signal()).expect("two empty tables"),
            (0, 0)
        );

        let refused = |piping: Vec<u8>, signal: Vec<u8>| {
            ports(piping, signal)
                .expect_err("the port tables were accepted")
                .to_string()
        };
        for (missing, column) in piping_columns.iter().enumerate() {
            let mut short = piping_columns.to_vec();
            short.remove(missing);
            assert_eq!(
                refused(table("T_PIPINGPOINT", &short, &[]), no_signal()),
                format!("T_PIPINGPOINT: column {} is missing", column.name)
            );
        }
        for (missing, column) in signal_columns.iter().enumerate() {
            let mut short = signal_columns.to_vec();
            short.remove(missing);
            assert_eq!(
                refused(no_piping(), table("T_SIGNALPOINT", &short, &[])),
                format!("T_SIGNALPOINT: column {} is missing", column.name)
            );
        }

        let mut unread = piping_columns;
        unread[0] = ch_db("SP_PLANTITEMID");
        assert_eq!(
            refused(
                table(
                    "T_PIPINGPOINT",
                    &unread,
                    &[row(&[Some(b"AB".to_vec()), Some(one()), None, None])]
                ),
                no_signal()
            ),
            "T_PIPINGPOINT: column SP_PLANTITEMID is not AL16UTF16 text; its cells were not read"
        );
        let mut unread = signal_columns;
        unread[0] = ch_db("SP_PLANTITEMID");
        assert_eq!(
            refused(
                no_piping(),
                table(
                    "T_SIGNALPOINT",
                    &unread,
                    &[row(&[Some(b"AB".to_vec()), Some(one())])]
                )
            ),
            "T_SIGNALPOINT: column SP_PLANTITEMID is not AL16UTF16 text; its cells were not read"
        );
    }

    /// One table of [`followed_fixture`]: its name, its columns and its rows.
    type FixtureTable = (&'static str, Vec<Col>, Vec<Vec<Option<Vec<u8>>>>);

    /// Every table a projection follows, each with exactly the columns
    /// followed and rows every projection accepts: on drawing `D`,
    /// equipment `E` whose piping point 1 starts connector `C` of run `R`,
    /// which ends on off-page connector `O1`; a nozzle, a note and a piping
    /// component `P` only the dump places.
    fn followed_fixture() -> Vec<FixtureTable> {
        let text = |value: &str| Some(utf16(value));
        let two = || Some(vec![0xc1, 0x03]);
        let a_date = || Some(vec![120, 126, 4, 25, 20, 12, 1]);
        vec![
            (
                "T_PIPINGPOINT",
                vec![
                    ch("SP_PLANTITEMID"),
                    num("PIPINGPOINTNUMBER"),
                    num("FLOWDIRECTION"),
                    num("NOMINALDIAMETER"),
                ],
                vec![row(&[
                    text("E"),
                    Some(one()),
                    two(),
                    Some(vec![0xc2, 0x02]),
                ])],
            ),
            (
                "T_SIGNALPOINT",
                vec![ch("SP_PLANTITEMID"), num("SIGNALPOINTNUMBER")],
                vec![row(&[text("E"), two()])],
            ),
            (
                "T_CONNECTOR",
                vec![
                    ch("SP_ID"),
                    ch("SP_CONNECTITEM1ID"),
                    ch("SP_CONNECTITEM2ID"),
                    num("STARTOBJECT"),
                    num("ENDOBJECT"),
                ],
                vec![row(&[
                    text("C"),
                    text("RE"),
                    text("RO"),
                    Some(one()),
                    Some(one()),
                ])],
            ),
            (
                "T_REPRESENTATION",
                vec![
                    ch("SP_ID"),
                    ch("SP_MODELITEMID"),
                    ch("SP_DRAWINGID"),
                    num("REPRESENTATIONTYPE"),
                    num("GRAPHICOID"),
                ],
                vec![
                    row(&[text("C"), text("R"), text("D"), two(), Some(one())]),
                    row(&[text("RE"), text("E"), text("D"), two(), two()]),
                    row(&[
                        text("RO"),
                        text("O1"),
                        text("D"),
                        two(),
                        Some(vec![0xc1, 0x04]),
                    ]),
                    // Representation type 13 is a piping component.
                    row(&[
                        text("RP"),
                        text("P"),
                        text("D"),
                        Some(vec![0xc1, 0x0e]),
                        Some(vec![0xc1, 0x06]),
                    ]),
                ],
            ),
            (
                "T_PIPERUN",
                pipe_run_columns(),
                vec![row(&[
                    text("R"),
                    Some(vec![0xc2, 0x02]),
                    Some(one()),
                    text("B5"),
                    text("0101"),
                    Some(one()),
                    text("P40"),
                ])],
            ),
            ("T_SIGNALRUN", vec![ch("SP_ID")], vec![row(&[text("S")])]),
            (
                "T_OPC",
                vec![
                    ch("SP_ID"),
                    ch("SP_PAIREDWITHID"),
                    ch("OPCTAG"),
                    ch("TOFROMTEXT"),
                ],
                vec![row(&[text("O1"), text("O0"), text("565"), text("to G")])],
            ),
            (
                "T_NOZZLE",
                vec![ch("SP_ID"), ch("SP_EQUIPMENTID")],
                vec![row(&[text("N"), text("E")])],
            ),
            (
                "T_NOTE",
                vec![ch("SP_ITEMNOTEID"), ch("SP_MODELITEMID")],
                vec![row(&[text("T"), text("E")])],
            ),
            ("T_ITEMNOTE", vec![ch("SP_ID")], vec![row(&[text("T")])]),
            (
                "T_MODELITEM",
                vec![ch("SP_ID"), ch("DESCRIPTION"), ch("ITEMTYPENAME")],
                vec![
                    row(&[text("P"), text("a valve"), text("PipingComp")]),
                    row(&[text("O1"), text("to G"), text("OPC")]),
                ],
            ),
            (
                "T_PLANTITEM",
                vec![ch("SP_ID"), ch("ITEMTAG")],
                vec![row(&[text("R"), text("OD-50")])],
            ),
            (
                "T_INLINECOMP",
                vec![ch("SP_ID"), num("NOMINALDIAMETER")],
                vec![row(&[text("P"), Some(vec![0xc1, 0x1a])])],
            ),
            (
                "CODELISTS",
                vec![
                    num("CODELIST_NUMBER"),
                    num("CODELIST_INDEX"),
                    ch("CODELIST_SHORT_TEXT"),
                ],
                vec![row(&[Some(vec![0xc1, 0x14]), Some(one()), text("OD")])],
            ),
            (
                "T_DRAWING",
                vec![
                    ch("SP_ID"),
                    ch("NAME"),
                    ch("PATH"),
                    ch("DRAWINGNUMBER"),
                    ch("SP_PLANTGROUPID"),
                    ch("DESCRIPTION"),
                    ch("TITLE"),
                    ch("REVISION"),
                    ch("VERSION"),
                    num("DOCUMENTTYPE"),
                    num("DOCUMENTCATEGORY"),
                    num("ITEMSTATUS"),
                    ch("TEMPLATE"),
                    ch("DESIGNPHASE"),
                    ch("PROJECTNUMBER"),
                    date("DATECREATED"),
                    date("DRAWINGDATE"),
                    ch("DATEOFDRAWING"),
                    date("LASTPUBLISHTIME"),
                    date("LASTARCHIVETIME"),
                ],
                vec![row(&[
                    text("D"),
                    text("One"),
                    text("\\00\\One.pid"),
                    text("D-1"),
                    text("G"),
                    text("a drawing"),
                    text("a title"),
                    text("A"),
                    text("1"),
                    Some(one()),
                    Some(one()),
                    Some(one()),
                    text("A1.pid"),
                    text("FEED"),
                    text("P-1"),
                    a_date(),
                    a_date(),
                    text("2026"),
                    a_date(),
                    a_date(),
                ])],
            ),
            (
                "T_PLANTGROUP",
                vec![ch("SP_ID"), ch("NAME"), ch("PARENTID")],
                vec![row(&[text("G"), text("Plant"), text("-1")])],
            ),
        ]
    }

    /// The fifteen `T_DRAWING` columns [`followed_fixture`] carries beyond
    /// the five the package index joins on.
    const DRAWING_DISPLAY_COLUMNS: [&str; 15] = [
        "DESCRIPTION",
        "TITLE",
        "REVISION",
        "VERSION",
        "DOCUMENTTYPE",
        "DOCUMENTCATEGORY",
        "ITEMSTATUS",
        "TEMPLATE",
        "DESIGNPHASE",
        "PROJECTNUMBER",
        "DATECREATED",
        "DRAWINGDATE",
        "DATEOFDRAWING",
        "LASTPUBLISHTIME",
        "LASTARCHIVETIME",
    ];

    /// The pinned `APPLICATIONS` table followed by `tables`.
    fn dump_of(tables: &[FixtureTable]) -> Vec<u8> {
        let mut dump = pinned();
        for (name, columns, rows) in tables {
            dump.extend(table(name, columns, rows));
        }
        dump
    }

    /// [`followed_fixture`] without one of its tables (`columns` is `None`)
    /// or without some columns of it.
    fn fixture_without(table_name: &str, columns: Option<&[&str]>) -> Vec<u8> {
        let mut tables = followed_fixture();
        let at = tables
            .iter()
            .position(|(name, _, _)| *name == table_name)
            .expect("a fixture table");
        match columns {
            None => {
                tables.remove(at);
            }
            Some(names) => {
                let (_, stated, rows) = &mut tables[at];
                for name in names {
                    let index = stated
                        .iter()
                        .position(|column| column.name == *name)
                        .expect("a fixture column");
                    stated.remove(index);
                    for row in rows.iter_mut() {
                        row.remove(index);
                    }
                }
            }
        }
        dump_of(&tables)
    }

    /// Every projection of a dump, stopping at the first that refuses it.
    fn every_projection(dump: &[u8]) -> Result<(), ExpError> {
        let tables = ExpTables::read(dump)?;
        tables.port_tables()?;
        tables.pipe_runs()?;
        tables.codelist_short_text(19)?;
        tables.connectivity_ends("D")?;
        tables.composition_relations()?;
        tables.opc_points("D")?;
        tables.drawings()?;
        tables.dump_only_piping_components("D", &HashSet::new())?;
        read_plant_groups(dump)?;
        Ok(())
    }

    // Every projection reads something out of the fixture, so each of its
    // columns is one some projection follows.
    #[test]
    fn the_followed_fixture_projects_through_every_stage() {
        let dump = dump_of(&followed_fixture());
        let tables = ExpTables::read(&dump).expect("the dump reads");
        let ports = tables.port_tables().expect("ports");
        assert_eq!(
            (ports.piping_points.len(), ports.signal_points.len()),
            (1, 1)
        );
        let runs = tables.pipe_runs().expect("runs");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].item_tag.as_deref(), Some("OD-50"));
        assert_eq!(runs[0].insulation_spec, "P40");
        assert_eq!(tables.codelist_short_text(19).expect("codes")[&1], "OD");
        let ends = tables.connectivity_ends("D").expect("ends");
        assert_eq!(
            ends.iter()
                .map(|end| end.symbol_uid.as_str())
                .collect::<Vec<_>>(),
            ["E.1", "O0"]
        );
        assert_eq!(tables.composition_relations().expect("rels").len(), 2);
        let opc = tables.opc_points("D").expect("off-page");
        assert_eq!(opc.len(), 1);
        assert_eq!(opc[0].to_from_text, "to G");
        let drawings = tables.drawings().expect("drawings");
        assert_eq!(drawings[0].template, "A1.pid");
        assert_eq!(drawings[0].document_type, Some(1));
        assert_eq!(drawings[0].last_archive_time, "2026/04/25 19:11:00");
        let components = tables
            .dump_only_piping_components("D", &HashSet::new())
            .expect("components");
        assert_eq!(components.len(), 1);
        assert_eq!(components[0].0.attributes[0].value, "25 mm");
        assert_eq!(read_plant_groups(&dump).expect("groups").len(), 1);
    }

    // Each followed column in turn is made one the reader keeps unread: a
    // character column becomes database-charset text, a number column holds
    // a fraction, a date column a cell that is not a date. Whichever
    // projection follows the column says which table and column it could
    // not read, and why. A projection that read the column as empty cells
    // instead would leave every projection accepting the dump.
    #[test]
    fn every_followed_column_is_refused_when_its_cells_were_not_read() {
        let fixture = followed_fixture();
        every_projection(&dump_of(&fixture)).expect("the fixture as stated projects");
        let mut columns_tried = 0;
        let mut not_refused = Vec::new();
        for (table_index, (table_name, columns, _)) in fixture.iter().enumerate() {
            for (column_index, column) in columns.iter().enumerate() {
                let mut damaged = followed_fixture();
                let (_, damaged_columns, damaged_rows) = &mut damaged[table_index];
                let reason = match column.kind {
                    1 => {
                        damaged_columns[column_index] = ch_db(column.name);
                        damaged_rows[0][column_index] = Some(b"x".to_vec());
                        "is not AL16UTF16 text"
                    }
                    2 => {
                        // 0.3: one base-100 digit, 30, at exponent -1.
                        damaged_rows[0][column_index] = Some(vec![0xc0, 0x1f]);
                        "is not an integer"
                    }
                    _ => {
                        damaged_rows[0][column_index] = Some(vec![1, 2, 3]);
                        "is a DATE that does not decode"
                    }
                };
                columns_tried += 1;
                let what = format!("{table_name}.{}", column.name);
                match every_projection(&dump_of(&damaged)) {
                    Ok(()) => not_refused.push(format!("{what}: every projection accepted it")),
                    Err(error) => {
                        let error = error.to_string();
                        if !(error.starts_with(&format!("{table_name}: "))
                            && error.contains(column.name)
                            && error.contains(reason))
                        {
                            not_refused.push(format!("{what}: refused as `{error}`"));
                        }
                    }
                }
            }
        }
        assert_eq!(columns_tried, 66);
        assert!(
            not_refused.is_empty(),
            "{} of {columns_tried} followed columns were not refused by name and reason:\n{}",
            not_refused.len(),
            not_refused.join("\n")
        );
    }

    // A dump may lack `T_PLANTITEM`, `T_INLINECOMP`, the item-type column
    // and the drawing's display columns, and what it lacks reads as nothing
    // stated. That is the other case from a column it has and the reader
    // kept unread.
    #[test]
    fn a_column_the_dump_lacks_reads_as_nothing_stated() {
        let no_components = HashSet::new();

        for dump in [
            fixture_without("T_PLANTITEM", None),
            fixture_without("T_PLANTITEM", Some(&["ITEMTAG"])),
            fixture_without("T_PLANTITEM", Some(&["SP_ID"])),
        ] {
            every_projection(&dump).expect("a dump without item tags projects");
            let tables = ExpTables::read(&dump).expect("reads");
            assert_eq!(tables.pipe_runs().expect("runs")[0].item_tag, None);
        }

        let dump = fixture_without("T_MODELITEM", Some(&["ITEMTYPENAME"]));
        every_projection(&dump).expect("a dump without item types projects");
        let tables = ExpTables::read(&dump).expect("reads");
        assert!(tables
            .dump_only_piping_components("D", &no_components)
            .expect("components")
            .is_empty());

        for dump in [
            fixture_without("T_INLINECOMP", None),
            fixture_without("T_INLINECOMP", Some(&["NOMINALDIAMETER"])),
            fixture_without("T_INLINECOMP", Some(&["SP_ID"])),
        ] {
            every_projection(&dump).expect("a dump without inline diameters projects");
            let tables = ExpTables::read(&dump).expect("reads");
            let components = tables
                .dump_only_piping_components("D", &no_components)
                .expect("components");
            assert_eq!(components.len(), 1);
            assert!(components[0].0.attributes.is_empty());
        }

        for column in DRAWING_DISPLAY_COLUMNS {
            every_projection(&fixture_without("T_DRAWING", Some(&[column])))
                .unwrap_or_else(|error| panic!("without {column}: {error}"));
        }
        let dump = fixture_without("T_DRAWING", Some(&DRAWING_DISPLAY_COLUMNS));
        let drawings = ExpTables::read(&dump)
            .expect("reads")
            .drawings()
            .expect("the join columns are enough");
        assert_eq!(drawings[0].name, "One");
        assert_eq!(
            (
                drawings[0].template.as_str(),
                drawings[0].document_type,
                drawings[0].date_created.as_str()
            ),
            ("", None, "")
        );
    }

    // The connectivity stage looks a connector's end up among the port
    // rows. It takes them through the port stage's own projection, so what
    // that stage refuses in a point table this one refuses in the same
    // words, and neither drops a point the other reads.
    #[test]
    fn the_connectivity_stage_refuses_the_port_rows_the_port_stage_refuses() {
        let with_point_number = |number: Option<Vec<u8>>| {
            let mut tables = followed_fixture();
            let (name, columns, rows) = &mut tables[0];
            assert_eq!(
                (*name, columns[1].name),
                ("T_PIPINGPOINT", "PIPINGPOINTNUMBER")
            );
            rows[0][1] = number;
            dump_of(&tables)
        };

        let read = with_point_number(Some(one()));
        let ports = read_port_tables(&read).expect("ports");
        assert_eq!(ports.piping_points.len(), 1);
        let ends = connectivity_ends_for_drawing(&read, "D").expect("ends");
        assert!(ends.iter().any(|end| end.symbol_uid == "E.1"), "{ends:?}");

        // 0.3 is a point number the reader kept unread; NULL is none at all.
        for (number, reason) in [
            (
                Some(vec![0xc0, 0x1f]),
                "row 0 PIPINGPOINTNUMBER is not an integer or NULL",
            ),
            (None, "row 0 PIPINGPOINTNUMBER is not an integer"),
        ] {
            let dump = with_point_number(number);
            let ports = read_port_tables(&dump).unwrap_err().to_string();
            let ends = connectivity_ends_for_drawing(&dump, "D")
                .unwrap_err()
                .to_string();
            assert_eq!(ports, format!("T_PIPINGPOINT: {reason}"));
            assert_eq!(ends, ports);
        }
    }

    // One ExpTables serves two drawings. The fragment also stores CODELISTS
    // and T_DRAWING; the drawing table has a date column.
    #[test]
    fn one_exp_tables_serves_two_drawings() {
        let mut dump = table(
            "APPLICATIONS",
            &[num("ID"), ch("NAME"), ch("SHORTNAME")],
            &[row(&[
                Some(one()),
                Some(utf16("SmartPlant PID")),
                Some(utf16("SPPID")),
            ])],
        );
        dump.extend(table(
            "T_PIPINGPOINT",
            &[
                ch("SP_PLANTITEMID"),
                num("PIPINGPOINTNUMBER"),
                num("FLOWDIRECTION"),
                num("NOMINALDIAMETER"),
            ],
            &[
                row(&[Some(utf16("PARENT1")), Some(one()), None, None]),
                row(&[Some(utf16("PARENT2")), Some(one()), None, None]),
            ],
        ));
        dump.extend(table(
            "T_SIGNALPOINT",
            &[ch("SP_PLANTITEMID"), num("SIGNALPOINTNUMBER")],
            &[],
        ));
        dump.extend(table(
            "T_CONNECTOR",
            &[
                ch("SP_ID"),
                ch("SP_CONNECTITEM1ID"),
                ch("SP_CONNECTITEM2ID"),
                num("STARTOBJECT"),
                num("ENDOBJECT"),
            ],
            &[
                row(&[
                    Some(utf16("CONN1")),
                    Some(utf16("ITEMA")),
                    None,
                    Some(one()),
                    None,
                ]),
                row(&[
                    Some(utf16("CONN2")),
                    Some(utf16("ITEMB")),
                    None,
                    Some(one()),
                    None,
                ]),
            ],
        ));
        dump.extend(table(
            "T_REPRESENTATION",
            &[ch("SP_ID"), ch("SP_MODELITEMID"), ch("SP_DRAWINGID")],
            &[
                row(&[
                    Some(utf16("CONN1")),
                    Some(utf16("RUN1")),
                    Some(utf16("DRAW1")),
                ]),
                row(&[
                    Some(utf16("ITEMA")),
                    Some(utf16("PARENT1")),
                    Some(utf16("DRAW1")),
                ]),
                row(&[
                    Some(utf16("CONN2")),
                    Some(utf16("RUN2")),
                    Some(utf16("DRAW2")),
                ]),
                row(&[
                    Some(utf16("ITEMB")),
                    Some(utf16("PARENT2")),
                    Some(utf16("DRAW2")),
                ]),
            ],
        ));
        dump.extend(table(
            "T_PIPERUN",
            &[ch("SP_ID")],
            &[row(&[Some(utf16("RUN1"))]), row(&[Some(utf16("RUN2"))])],
        ));
        dump.extend(table("T_SIGNALRUN", &[ch("SP_ID")], &[]));
        dump.extend(table(
            "T_OPC",
            &[
                ch("SP_ID"),
                ch("SP_PAIREDWITHID"),
                ch("OPCTAG"),
                ch("TOFROMTEXT"),
            ],
            &[],
        ));
        dump.extend(table("T_NOZZLE", &[ch("SP_ID"), ch("SP_EQUIPMENTID")], &[]));
        dump.extend(table(
            "T_NOTE",
            &[ch("SP_ITEMNOTEID"), ch("SP_MODELITEMID")],
            &[],
        ));
        dump.extend(table("T_ITEMNOTE", &[ch("SP_ID")], &[]));
        dump.extend(table("T_MODELITEM", &[ch("SP_ID"), ch("DESCRIPTION")], &[]));
        dump.extend(table(
            "CODELISTS",
            &[
                num("CODELIST_NUMBER"),
                num("CODELIST_INDEX"),
                ch("CODELIST_TEXT"),
                ch("CODELIST_SHORT_TEXT"),
                num("CODELIST_CONSTRAINT"),
                num("CODELIST_SORT_VALUE"),
                num("CODELIST_ENTRY_DISABLED"),
            ],
            &[row(&[
                Some(one()),
                Some(one()),
                Some(utf16("code")),
                Some(utf16("CI")),
                None,
                None,
                None,
            ])],
        ));
        dump.extend(table(
            "T_DRAWING",
            &[
                ch("SP_ID"),
                ch("NAME"),
                ch("PATH"),
                ch("DRAWINGNUMBER"),
                ch("SP_PLANTGROUPID"),
                date("DATECREATED"),
            ],
            &[
                row(&[
                    Some(utf16("DRAW1")),
                    Some(utf16("one")),
                    Some(utf16("\\a.pid")),
                    Some(utf16("D-1")),
                    Some(utf16("G1")),
                    None,
                ]),
                row(&[
                    Some(utf16("DRAW2")),
                    Some(utf16("two")),
                    Some(utf16("\\b.pid")),
                    Some(utf16("D-2")),
                    Some(utf16("G1")),
                    None,
                ]),
            ],
        ));

        let tables = ExpTables::read(&dump).unwrap();
        assert_eq!(tables.codelist_row_count(), 1);
        assert_eq!(tables.drawing_row_count(), 2);
        let drawings = tables.drawings().unwrap();
        assert_eq!(drawings.len(), 2);
        assert_eq!(drawings[0].id, "DRAW1");
        assert_eq!(drawings[0].name, "one");
        assert_eq!(drawings[0].path_key, "a.pid");
        assert_eq!(drawings[0].drawing_number, "D-1");
        assert_eq!(drawings[0].plant_group_id, norm_uid("G1"));
        let ports = tables.port_tables().unwrap();
        assert_eq!(ports.piping_points.len(), 2);
        assert_eq!(ports.signal_points.len(), 0);

        let first = tables.connectivity_ends("DRAW1").unwrap();
        let second = tables.connectivity_ends("DRAW2").unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].symbol_uid, "PARENT1.1");
        assert_eq!(first[0].run_uid, "RUN1");
        assert_eq!(first[0].def_uid, "PipingEnd1Conn");
        assert!(first[0].on_start);
        assert!(!first[0].on_end);
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].symbol_uid, "PARENT2.1");
        assert_eq!(second[0].run_uid, "RUN2");
        assert!(tables.composition_relations().unwrap().is_empty());
        assert!(tables.opc_points("DRAW1").unwrap().is_empty());
        assert!(tables.opc_points("DRAW2").unwrap().is_empty());
    }

    // The drawing-metadata projection: optional columns surface when
    // present, read empty when absent, and a 7-byte DATE cell decodes.
    #[test]
    fn drawings_surface_optional_metadata_and_dates() {
        let dump = table(
            "T_DRAWING",
            &[
                ch("SP_ID"),
                ch("NAME"),
                ch("PATH"),
                ch("DRAWINGNUMBER"),
                ch("SP_PLANTGROUPID"),
                ch("DESCRIPTION"),
                ch("TITLE"),
                num("DOCUMENTTYPE"),
                date("DATECREATED"),
                ch("TEMPLATE"),
            ],
            &[row(&[
                Some(utf16("DRAW1")),
                Some(utf16("one")),
                Some(utf16("\\a.pid")),
                Some(utf16("D-1")),
                Some(utf16("G1")),
                Some(utf16("desc-one")),
                Some(utf16("title-one")),
                Some(vec![0xc1, 0x05]),
                Some(vec![120, 124, 6, 15, 11, 31, 46]),
                Some(utf16("A1.pidtpl")),
            ])],
        );
        let mut tables = ExpTables::blank();
        tables.drawing = read_table_tolerant(&dump, "T_DRAWING").unwrap();
        let drawings = tables.drawings().unwrap();
        assert_eq!(drawings.len(), 1);
        let drawing = &drawings[0];
        assert_eq!(drawing.name, "one");
        assert_eq!(drawing.description, "desc-one");
        assert_eq!(drawing.title, "title-one");
        assert_eq!(drawing.document_type, Some(4));
        assert_eq!(drawing.date_created, "2024/06/15 10:30:45");
        assert_eq!(drawing.template, "A1.pidtpl");
        assert_eq!(drawing.version, "");
        assert_eq!(drawing.revision, "");
        assert_eq!(drawing.item_status, None);
        assert_eq!(drawing.document_category, None);
        assert_eq!(drawing.design_phase, "");
        assert_eq!(drawing.drawing_date, "");
        assert_eq!(drawing.last_publish_time, "");
    }

    /// The revision guard's hit side: the `.pid`'s `DocUID` is one of the
    /// dump's `T_DRAWING.SP_ID` cells once both are folded.
    #[test]
    fn drawing_in_dump_finds_the_row_under_the_folded_uid() {
        let dump = table(
            "T_DRAWING",
            &[
                ch("SP_ID"),
                ch("NAME"),
                ch("PATH"),
                ch("DRAWINGNUMBER"),
                ch("SP_PLANTGROUPID"),
            ],
            &[
                row(&[
                    Some(utf16("D710241072D54ECCB8F5961357166E5D")),
                    Some(utf16("一级闪蒸分离器A")),
                    Some(utf16("\\a.pid")),
                    Some(utf16("DWG-0000集05-01")),
                    Some(utf16("G1")),
                ]),
                row(&[
                    Some(utf16("AAFCDC31FA3C4177BECB02EBBB6F776F")),
                    Some(utf16("一级闪蒸分离器B")),
                    Some(utf16("\\b.pid")),
                    Some(utf16("DWG-0000集05-02")),
                    Some(utf16("G1")),
                ]),
            ],
        );
        let mut tables = ExpTables::blank();
        tables.drawing = read_table_tolerant(&dump, "T_DRAWING").unwrap();

        assert!(drawing_in_dump(&tables, "D710241072D54ECCB8F5961357166E5D").unwrap());
        assert!(drawing_in_dump(&tables, "{D7102410-72D5-4ECC-B8F5-961357166E5D}").unwrap());
        assert!(drawing_in_dump(&tables, "aafcdc31fa3c4177becb02ebbb6f776f").unwrap());
    }

    /// The revision guard's miss side: a drawing the dump does not list is
    /// a different revision, not an empty one.
    #[test]
    fn drawing_in_dump_misses_a_uid_the_dump_does_not_carry() {
        let dump = table(
            "T_DRAWING",
            &[
                ch("SP_ID"),
                ch("NAME"),
                ch("PATH"),
                ch("DRAWINGNUMBER"),
                ch("SP_PLANTGROUPID"),
            ],
            &[row(&[
                Some(utf16("D710241072D54ECCB8F5961357166E5D")),
                Some(utf16("一级闪蒸分离器A")),
                Some(utf16("\\a.pid")),
                Some(utf16("DWG-0000集05-01")),
                Some(utf16("G1")),
            ])],
        );
        let mut tables = ExpTables::blank();
        tables.drawing = read_table_tolerant(&dump, "T_DRAWING").unwrap();

        assert!(!drawing_in_dump(&tables, "AAFCDC31FA3C4177BECB02EBBB6F776F").unwrap());
        assert!(!drawing_in_dump(&tables, "").unwrap());
    }

    #[test]
    fn oracle_date_decoding_rejects_noncanonical_cells() {
        assert_eq!(
            decode_oracle_date(&[120, 124, 6, 15, 11, 31, 46]).as_deref(),
            Some("2024/06/15 10:30:45")
        );
        assert_eq!(
            decode_oracle_date(&[120, 124, 1, 1, 1, 1, 1]).as_deref(),
            Some("2024/01/01 00:00:00")
        );
        assert_eq!(decode_oracle_date(&[1, 2, 3]), None);
        assert_eq!(decode_oracle_date(&[120, 124, 13, 1, 1, 1, 1]), None);
        assert_eq!(decode_oracle_date(&[99, 124, 6, 15, 1, 1, 1]), None);
    }

    #[test]
    fn plant_group_path_joins_names_from_the_root() {
        let dump = table(
            "T_PLANTGROUP",
            &[ch("SP_ID"), ch("NAME"), ch("PARENTID"), date("DATECREATED")],
            &[
                row(&[
                    Some(utf16("ROOT")),
                    Some(utf16("SQPlant")),
                    Some(utf16("-1")),
                    None,
                ]),
                row(&[
                    Some(utf16("MID")),
                    Some(utf16("00")),
                    Some(utf16("ROOT")),
                    None,
                ]),
                row(&[
                    Some(utf16("LEAF")),
                    Some(utf16("00")),
                    Some(utf16("MID")),
                    None,
                ]),
            ],
        );
        let groups = read_plant_groups(&dump).unwrap();
        assert_eq!(plant_group_path(&groups, "LEAF"), "SQPlant \\ 00 \\ 00");
        assert_eq!(plant_group_path(&groups, ""), "");
        assert!(read_plant_groups(b"not a dump").unwrap().is_empty());
    }

    // Only a stockpiled piping component the .pid does not draw becomes a
    // dump-only item: a run's representation, a representation the .pid
    // already has, and another drawing's row are all left alone.
    #[test]
    fn dump_only_piping_components_come_from_this_drawings_representations() {
        let mut dump = table(
            "APPLICATIONS",
            &[num("ID"), ch("NAME"), ch("SHORTNAME")],
            &[row(&[
                Some(one()),
                Some(utf16("SmartPlant PID")),
                Some(utf16("SPPID")),
            ])],
        );
        let thirteen = vec![0xc1, 0x0e];
        dump.extend(table(
            "T_REPRESENTATION",
            &[
                ch("SP_ID"),
                num("REPRESENTATIONTYPE"),
                num("GRAPHICOID"),
                ch("SP_DRAWINGID"),
                ch("SP_MODELITEMID"),
            ],
            &[
                // 44116
                row(&[
                    Some(utf16("REP1")),
                    Some(thirteen.clone()),
                    Some(vec![0xc3, 5, 42, 17]),
                    Some(utf16("DRAW1")),
                    Some(utf16("COMP1")),
                ]),
                row(&[
                    Some(utf16("REP2")),
                    Some(thirteen.clone()),
                    Some(one()),
                    Some(utf16("DRAW1")),
                    Some(utf16("RUN1")),
                ]),
                row(&[
                    Some(utf16("REP3")),
                    Some(thirteen.clone()),
                    Some(one()),
                    Some(utf16("DRAW1")),
                    Some(utf16("COMP2")),
                ]),
                row(&[
                    Some(utf16("REP4")),
                    Some(thirteen.clone()),
                    Some(one()),
                    Some(utf16("DRAW2")),
                    Some(utf16("COMP3")),
                ]),
                row(&[
                    Some(utf16("REP5")),
                    Some(vec![0xc1, 0x0d]),
                    Some(one()),
                    Some(utf16("DRAW1")),
                    Some(utf16("COMP4")),
                ]),
            ],
        ));
        dump.extend(table(
            "T_MODELITEM",
            &[ch("SP_ID"), ch("ITEMTYPENAME"), ch("DESCRIPTION")],
            &[
                row(&[Some(utf16("COMP1")), Some(utf16("PipingComp")), None]),
                row(&[Some(utf16("RUN1")), Some(utf16("PipeRun")), None]),
                row(&[Some(utf16("COMP2")), Some(utf16("PipingComp")), None]),
                row(&[Some(utf16("COMP3")), Some(utf16("PipingComp")), None]),
                row(&[Some(utf16("COMP4")), Some(utf16("PipingComp")), None]),
            ],
        ));
        dump.extend(table(
            "T_INLINECOMP",
            &[ch("SP_ID"), num("NOMINALDIAMETER")],
            // 5192 is 6 in.
            &[row(&[Some(utf16("COMP1")), Some(vec![0xc2, 52, 93])])],
        ));

        let tables = ExpTables::load_components(&dump).unwrap();
        let drawn = HashSet::from(["REP3".to_string()]);
        let found = tables.dump_only_piping_components("DRAW1", &drawn).unwrap();
        assert_eq!(found.len(), 1, "{found:?}");
        let (item, representation) = &found[0];
        assert_eq!(item.uid, "COMP1");
        assert_eq!(item.model_item_type, "PipingComp");
        assert_eq!(item.class, Some("PIDPipingComponent"));
        assert_eq!(item.attributes.len(), 1);
        assert_eq!(item.attributes[0].interface, "IPipeCrossSectionItem");
        assert_eq!(item.attributes[0].name, "NominalDiameter");
        assert_eq!(item.attributes[0].value, "6 in");
        assert_eq!(representation.uid, "REP1");
        assert_eq!(representation.item_uid.as_deref(), Some("COMP1"));
        assert_eq!(representation.graphic_oid, 44116);
    }

    #[test]
    fn exp_tables_refuses_an_unpinned_dump_before_other_tables() {
        let dump = table(
            "APPLICATIONS",
            &[num("ID"), ch("NAME"), ch("SHORTNAME")],
            &[row(&[
                Some(vec![0xc1, 0x03]),
                Some(utf16("SmartPlant PID")),
                Some(utf16("SPPID")),
            ])],
        );
        let Err(error) = ExpTables::read(&dump) else {
            panic!("unpinned dump was read");
        };
        assert!(matches!(error, ExpError::FormatUnpinned), "{error}");
        assert!(!error.to_string().contains("T_PIPINGPOINT"));
    }

    #[test]
    fn exp_tables_reads_the_backup_dump_once() {
        let dump_path = std::path::PathBuf::from(r"D:\work\cad\pid-test-data\Export.dmp");
        if !dump_path.is_file() {
            eprintln!("skip: oracle exp corpus is missing");
            return;
        }
        let dump = std::fs::read(&dump_path).unwrap();
        let tables = ExpTables::read(&dump).unwrap();
        assert_eq!(tables.drawing_row_count(), 53);
        assert!(tables.codelist_row_count() > 0);
        let ports = tables.port_tables().unwrap();
        assert_eq!(ports.piping_row_count, 9_628);
        assert_eq!(ports.signal_row_count, 22_057);
        let from_dump = read_port_tables(&dump).unwrap();
        assert_eq!(ports, from_dump);
    }

    #[test]
    fn ijingchang_ports_match_the_reference() {
        let dump_path = std::path::PathBuf::from(r"D:\work\cad\pid-test-data\Export.dmp");
        let corpus = std::path::PathBuf::from(r"D:\work\cad\PID自研软件验证");
        let stem = "I井场 注采阀组工艺及自控流程图";
        let pid = corpus.join(format!("{stem}.pid"));
        let reference_path = corpus.join(format!("{stem}_Data.xml"));
        if !dump_path.is_file() || !pid.is_file() || !reference_path.is_file() {
            eprintln!("skip: oracle exp corpus or I井场 .pid is missing");
            return;
        }
        let dump = std::fs::read(&dump_path).unwrap();
        let tables = read_port_tables(&dump).unwrap();
        assert_eq!(tables.piping_row_count, 9_628);
        assert_eq!(tables.signal_row_count, 22_057);
        assert_eq!(tables.piping_points.len(), 9_628);
        assert_eq!(tables.signal_points.len(), 22_057);

        let drawing = super::super::pid_load::load_publish_from_pid(&pid).unwrap();
        let parents = published_parent_ids(&drawing);
        let selected = ports_for_parents(&tables, &parents);
        let xml = std::fs::read_to_string(&reference_path).unwrap();
        let (reference_piping, reference_signal) = reference_port_uids(&xml);

        let piping_extra = extras(&selected.piping, &reference_piping);
        let signal_extra = extras_signal(&selected.signal, &reference_signal);
        assert!(
            piping_extra.is_empty() && signal_extra.is_empty(),
            "extra piping {} signal {}\n{piping_extra:?}\n{signal_extra:?}",
            piping_extra.len(),
            signal_extra.len()
        );
        assert_eq!(selected.piping.len(), 362);
        assert_eq!(selected.signal.len(), 915);
        assert_eq!(reference_piping.len(), 362);
        assert_eq!(reference_signal.len(), 915);
        let piping_uids = selected
            .piping
            .iter()
            .map(PipingPoint::uid)
            .collect::<HashSet<_>>();
        let signal_uids = selected
            .signal
            .iter()
            .map(SignalPoint::uid)
            .collect::<HashSet<_>>();
        assert!(reference_piping.is_subset(&piping_uids));
        assert!(reference_signal.is_subset(&signal_uids));
    }

    #[test]
    fn ijingchang_connectivity_ends_match_the_reference_pairs() {
        let dump_path = std::path::PathBuf::from(r"D:\work\cad\pid-test-data\Export.dmp");
        let corpus = std::path::PathBuf::from(r"D:\work\cad\PID自研软件验证");
        let stem = "I井场 注采阀组工艺及自控流程图";
        let pid = corpus.join(format!("{stem}.pid"));
        let reference_path = corpus.join(format!("{stem}_Data.xml"));
        if !dump_path.is_file() || !pid.is_file() || !reference_path.is_file() {
            eprintln!("skip: oracle exp corpus or I井场 .pid is missing");
            return;
        }
        let dump = std::fs::read(&dump_path).unwrap();
        let drawing = super::super::pid_load::load_publish_from_pid(&pid).unwrap();
        let ends = connectivity_ends_for_drawing(&dump, &drawing.drawing_uid).unwrap();
        let xml = std::fs::read_to_string(&reference_path).unwrap();
        let reference = reference_connectivity_pairs(&xml);
        let got = ends
            .iter()
            .map(|end| (end.symbol_uid.clone(), end.run_uid.clone()))
            .collect::<HashSet<_>>();
        let reference_pairs = reference.keys().cloned().collect::<HashSet<_>>();
        let missing = reference_pairs.difference(&got).count();
        let extra = got.difference(&reference_pairs).count();
        assert_eq!(
            (ends.len(), missing, extra),
            (738, 0, 0),
            "drawing {}",
            drawing.drawing_uid
        );
        assert!(ends.iter().all(|end| end.on_start || end.on_end));

        // The definition: every pair's DefUID is the reference's, and the
        // six counts are the reference's (PTF 369, PE1 129, PE2 143,
        // SE1 38, SE2 46, STF 13).
        let wrong_def = ends
            .iter()
            .filter(|end| {
                reference.get(&(end.symbol_uid.clone(), end.run_uid.clone()))
                    != Some(&end.def_uid.to_string())
            })
            .map(|end| {
                format!(
                    "{} {} -> {} {}",
                    end.def_uid, end.symbol_uid, end.run_uid, end.on_start
                )
            })
            .collect::<Vec<_>>();
        assert!(
            wrong_def.is_empty(),
            "{} wrong definitions, first: {:?}",
            wrong_def.len(),
            wrong_def.iter().take(5).collect::<Vec<_>>()
        );
        let count = |def: &str| ends.iter().filter(|end| end.def_uid == def).count();
        assert_eq!(
            [
                count("PipingTapOrFitting"),
                count("PipingEnd1Conn"),
                count("PipingEnd2Conn"),
                count("SignalEnd1Conn"),
                count("SignalEnd2Conn"),
                count("SignalTapOrFitting"),
            ],
            [369, 129, 143, 38, 46, 13]
        );
    }

    #[test]
    fn ijingchang_nozzle_and_note_rows_match_the_reference() {
        let dump_path = std::path::PathBuf::from(r"D:\work\cad\pid-test-data\Export.dmp");
        let corpus = std::path::PathBuf::from(r"D:\work\cad\PID自研软件验证");
        let stem = "I井场 注采阀组工艺及自控流程图";
        let pid = corpus.join(format!("{stem}.pid"));
        let reference_path = corpus.join(format!("{stem}_Data.xml"));
        if !dump_path.is_file() || !pid.is_file() || !reference_path.is_file() {
            eprintln!("skip: oracle exp corpus or I井场 .pid is missing");
            return;
        }
        let dump = std::fs::read(&dump_path).unwrap();
        let drawing = super::super::pid_load::load_publish_from_pid(&pid).unwrap();
        let rels = composition_relations(&dump).unwrap();
        let nozzles = rels
            .iter()
            .filter(|rel| rel.def_uid == "EquipmentComponentComposition")
            .count();
        let notes = rels
            .iter()
            .filter(|rel| rel.def_uid == "PBSItemNotes")
            .count();
        assert_eq!((nozzles, notes), (377, 114));

        let published = drawing
            .items
            .iter()
            .filter(|item| item.class.is_some())
            .map(|item| item.uid.as_str())
            .collect::<HashSet<_>>();
        let kept = |def_uid: &str| {
            rels.iter()
                .filter(|rel| {
                    rel.def_uid == def_uid
                        && published.contains(rel.uid1.as_str())
                        && published.contains(rel.uid2.as_str())
                })
                .map(|rel| (rel.uid1.clone(), rel.uid2.clone()))
                .collect::<HashSet<_>>()
        };
        let xml = std::fs::read_to_string(&reference_path).unwrap();
        let reference_nozzles = reference_uid_pairs(&xml, &["EquipmentComponentComposition"]);
        let reference_notes = reference_uid_pairs(&xml, &["PBSItemNotes"]);
        let got_nozzles = kept("EquipmentComponentComposition");
        let got_notes = kept("PBSItemNotes");
        assert_eq!(
            (
                got_nozzles.len(),
                reference_nozzles.difference(&got_nozzles).count(),
                got_nozzles.difference(&reference_nozzles).count(),
            ),
            (7, 0, 0)
        );
        assert_eq!(
            (
                got_notes.len(),
                reference_notes.difference(&got_notes).count(),
                got_notes.difference(&reference_notes).count(),
            ),
            (6, 0, 0)
        );
    }

    #[test]
    fn ijingchang_opc_points_match_the_reference() {
        let dump_path = std::path::PathBuf::from(r"D:\work\cad\pid-test-data\Export.dmp");
        let corpus = std::path::PathBuf::from(r"D:\work\cad\PID自研软件验证");
        let stem = "I井场 注采阀组工艺及自控流程图";
        let pid = corpus.join(format!("{stem}.pid"));
        let reference_path = corpus.join(format!("{stem}_Data.xml"));
        if !dump_path.is_file() || !pid.is_file() || !reference_path.is_file() {
            eprintln!("skip: oracle exp corpus or I井场 .pid is missing");
            return;
        }
        let dump = std::fs::read(&dump_path).unwrap();
        let drawing = super::super::pid_load::load_publish_from_pid(&pid).unwrap();
        let points = opc_points_for_drawing(&dump, &drawing.drawing_uid).unwrap();
        let representations = drawing
            .representations
            .iter()
            .map(|representation| representation.uid.as_str())
            .collect::<HashSet<_>>();
        let kept = points
            .into_iter()
            .filter(|point| representations.contains(point.representation_uid.as_str()))
            .collect::<Vec<_>>();
        let xml = std::fs::read_to_string(&reference_path).unwrap();
        let reference = reference_opc_points(&xml);
        let got = kept
            .iter()
            .map(|point| point.uid.clone())
            .collect::<HashSet<_>>();
        let wanted = reference.keys().cloned().collect::<HashSet<_>>();
        assert_eq!(
            (
                kept.len(),
                wanted.difference(&got).count(),
                got.difference(&wanted).count()
            ),
            (2, 0, 0)
        );
        for point in &kept {
            let (name, description, to_from, representation) = &reference[&point.uid];
            assert_eq!(&point.name, name, "{}", point.uid);
            assert_eq!(&point.description, description, "{}", point.uid);
            assert_eq!(&point.to_from_text, to_from, "{}", point.uid);
            assert_eq!(&point.representation_uid, representation, "{}", point.uid);
        }
    }

    #[test]
    fn the_item_of_a_symbol_end_is_the_port_parent_or_the_point_itself() {
        assert_eq!(item_of_symbol("AAD117E6.1"), "AAD117E6");
        assert_eq!(item_of_symbol("AAD117E6.12"), "AAD117E6");
        assert_eq!(item_of_symbol("5F3C2A10.BPT"), "5F3C2A10.BPT");
        assert_eq!(item_of_symbol("0C9D4E7B"), "0C9D4E7B");
        assert_eq!(item_of_symbol("A.B"), "A.B");
    }

    #[test]
    fn the_definition_is_tap_through_an_item_and_end_at_one_side() {
        assert_eq!(connectivity_def_uid(true, true, true), "PipingTapOrFitting");
        assert_eq!(
            connectivity_def_uid(true, true, false),
            "PipingTapOrFitting"
        );
        assert_eq!(connectivity_def_uid(true, false, true), "PipingEnd1Conn");
        assert_eq!(connectivity_def_uid(true, false, false), "PipingEnd2Conn");
        assert_eq!(
            connectivity_def_uid(false, true, false),
            "SignalTapOrFitting"
        );
        assert_eq!(connectivity_def_uid(false, false, true), "SignalEnd1Conn");
        assert_eq!(connectivity_def_uid(false, false, false), "SignalEnd2Conn");
    }

    /// Reference connectivity rows: `(UID1, UID2)` → `DefUID`.
    fn reference_connectivity_pairs(xml: &str) -> HashMap<(String, String), String> {
        const DEFS: [&str; 6] = [
            "PipingTapOrFitting",
            "PipingEnd1Conn",
            "PipingEnd2Conn",
            "SignalEnd1Conn",
            "SignalEnd2Conn",
            "SignalTapOrFitting",
        ];
        reference_rel_map(xml, &DEFS)
    }

    /// Reference rows of `defs`: `(UID1, UID2)`.
    fn reference_uid_pairs(xml: &str, defs: &[&str]) -> HashSet<(String, String)> {
        reference_rel_map(xml, defs).into_keys().collect()
    }

    /// `PIDBranchPoint` UID → `(Name, Description, ToFromText, representation)`.
    fn reference_opc_points(xml: &str) -> HashMap<String, (String, String, String, String)> {
        let mut reader = quick_xml::Reader::from_str(xml);
        reader.config_mut().trim_text(true);
        let mut points = HashMap::new();
        let mut drawing = HashMap::<String, String>::new();
        let mut depth = 0usize;
        let mut tag = String::new();
        let mut uid = String::new();
        let mut name = String::new();
        let mut description = String::new();
        let mut to_from = String::new();
        let mut uid1 = String::new();
        let mut uid2 = String::new();
        let mut def = String::new();
        loop {
            match reader.read_event() {
                Ok(quick_xml::events::Event::Start(start)) => {
                    if depth == 1 {
                        tag = start.name().as_ref().to_string();
                        uid.clear();
                        name.clear();
                        description.clear();
                        to_from.clear();
                        uid1.clear();
                        uid2.clear();
                        def.clear();
                    } else if depth == 2 {
                        take_opc(&start, &mut uid, &mut name, &mut description, &mut to_from);
                        take_rel(&start, &mut uid1, &mut uid2, &mut def);
                    }
                    depth += 1;
                }
                Ok(quick_xml::events::Event::Empty(start)) => {
                    if depth == 2 {
                        take_opc(&start, &mut uid, &mut name, &mut description, &mut to_from);
                        take_rel(&start, &mut uid1, &mut uid2, &mut def);
                    }
                }
                Ok(quick_xml::events::Event::End(_)) => {
                    depth = depth.saturating_sub(1);
                    if depth == 1 && tag == "PIDBranchPoint" && !uid.is_empty() {
                        points.insert(
                            norm_uid(&uid),
                            (
                                name.clone(),
                                description.clone(),
                                to_from.clone(),
                                String::new(),
                            ),
                        );
                    }
                    if depth == 1 && tag == "Rel" && def == "DrawingItems" {
                        drawing.insert(norm_uid(&uid2), norm_uid(&uid1));
                    }
                }
                Ok(quick_xml::events::Event::Eof) => break,
                Err(err) => panic!("reference xml: {err}"),
                _ => {}
            }
        }
        for (uid, representation) in drawing {
            if let Some(point) = points.get_mut(&uid) {
                point.3 = representation;
            }
        }
        points
    }

    fn take_opc(
        start: &quick_xml::events::BytesStart<'_>,
        uid: &mut String,
        name: &mut String,
        description: &mut String,
        to_from: &mut String,
    ) {
        let interface = start.name().as_ref().to_string();
        for attribute in start.attributes().flatten() {
            let key = attribute.key.as_ref().to_string();
            let value = attribute
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .map(std::borrow::Cow::into_owned)
                .unwrap_or_default();
            match (interface.as_str(), key.as_str()) {
                ("IObject", "UID") => *uid = value,
                ("IObject", "Name") => *name = value,
                ("IObject", "Description") => *description = value,
                ("IOffpageConnector", "ToFromText") => *to_from = value,
                _ => {}
            }
        }
    }

    fn reference_rel_map(xml: &str, defs: &[&str]) -> HashMap<(String, String), String> {
        let mut reader = quick_xml::Reader::from_str(xml);
        reader.config_mut().trim_text(true);
        let mut pairs = HashMap::new();
        let mut depth = 0usize;
        let mut tag = String::new();
        let mut uid1 = String::new();
        let mut uid2 = String::new();
        let mut def = String::new();
        loop {
            match reader.read_event() {
                Ok(quick_xml::events::Event::Start(start)) => {
                    if depth == 1 {
                        tag = start.name().as_ref().to_string();
                        uid1.clear();
                        uid2.clear();
                        def.clear();
                    } else if depth == 2 && tag == "Rel" {
                        take_rel(&start, &mut uid1, &mut uid2, &mut def);
                    }
                    depth += 1;
                }
                Ok(quick_xml::events::Event::Empty(start)) => {
                    if depth == 2 && tag == "Rel" {
                        take_rel(&start, &mut uid1, &mut uid2, &mut def);
                    }
                }
                Ok(quick_xml::events::Event::End(_)) => {
                    depth = depth.saturating_sub(1);
                    if depth == 1 && tag == "Rel" && defs.contains(&def.as_str()) {
                        pairs.insert((norm_uid(&uid1), norm_uid(&uid2)), def.clone());
                    }
                }
                Ok(quick_xml::events::Event::Eof) => break,
                Err(err) => panic!("reference xml: {err}"),
                _ => {}
            }
        }
        pairs
    }

    fn take_rel(
        start: &quick_xml::events::BytesStart<'_>,
        uid1: &mut String,
        uid2: &mut String,
        def: &mut String,
    ) {
        if start.name().as_ref() != "IRel" {
            return;
        }
        for attribute in start.attributes().flatten() {
            let name = attribute.key.as_ref().to_string();
            let value = attribute
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .map(std::borrow::Cow::into_owned)
                .unwrap_or_default();
            match name.as_str() {
                "UID1" => *uid1 = value,
                "UID2" => *uid2 = value,
                "DefUID" => *def = value,
                _ => {}
            }
        }
    }

    fn extras(points: &[PipingPoint], reference: &HashSet<String>) -> Vec<String> {
        points
            .iter()
            .map(PipingPoint::uid)
            .filter(|uid| !reference.contains(uid))
            .collect()
    }

    fn extras_signal(points: &[SignalPoint], reference: &HashSet<String>) -> Vec<String> {
        points
            .iter()
            .map(SignalPoint::uid)
            .filter(|uid| !reference.contains(uid))
            .collect()
    }

    fn reference_port_uids(xml: &str) -> (HashSet<String>, HashSet<String>) {
        use quick_xml::events::Event;
        use quick_xml::Reader;

        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);
        let mut depth = 0usize;
        let mut tag = String::new();
        let mut uid = String::new();
        let mut piping = HashSet::new();
        let mut signal = HashSet::new();
        loop {
            match reader.read_event() {
                Ok(Event::Start(start)) => {
                    if depth == 1 {
                        tag = start.name().as_ref().to_string();
                        uid.clear();
                    } else if depth == 2 {
                        take_uid(&start, &mut uid);
                    }
                    depth += 1;
                }
                Ok(Event::Empty(start)) => {
                    if depth == 2 {
                        take_uid(&start, &mut uid);
                    }
                }
                Ok(Event::End(_)) => {
                    depth = depth.saturating_sub(1);
                    if depth == 1 && !uid.is_empty() {
                        if tag == "PIDPipingPort" {
                            piping.insert(uid.clone());
                        } else if tag == "PIDSignalPort" {
                            signal.insert(uid.clone());
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Err(err) => panic!("reference xml: {err}"),
                _ => {}
            }
        }
        (piping, signal)
    }

    #[test]
    fn equipment_and_instrument_rows_are_read_from_the_dump_only() {
        let path = std::path::Path::new(r"D:\work\cad\pid-test-data\Export.dmp");
        let publish = std::path::Path::new(
            r"D:\work\cad\PID自研软件验证\publish\I井场 注采阀组工艺及自控流程图_Data.xml",
        );
        if !path.is_file() || !publish.is_file() {
            eprintln!("skip: oracle exp dump or I井场 publish xml is missing");
            return;
        }
        let dump = std::fs::read(path).expect("dump");
        let published = std::fs::read_to_string(publish).expect("ijingchang xml");
        let model = read_table_tolerant(&dump, "T_MODELITEM").expect("model items");
        let model_ids: std::collections::HashSet<String> = model
            .rows
            .iter()
            .filter_map(|row| text_at(row, &model.columns, "SP_ID"))
            .filter(|id| !id.is_empty())
            .collect();
        let equipment = read_table_tolerant(&dump, "T_EQUIPMENT").expect("equipment");
        let instrument = read_table_tolerant(&dump, "T_INSTRUMENT").expect("instrument");
        assert_eq!((equipment.rows.len(), equipment.columns.len()), (402, 63));
        assert_eq!(
            (instrument.rows.len(), instrument.columns.len()),
            (3718, 126)
        );
        assert_eq!(
            nonempty_columns(&equipment),
            [
                "CLASS",
                "DESIGNTEMP",
                "EQUIPMENTLOAD",
                "EQUIPMENTSUBCLASS",
                "EQUIPMENTTYPE",
                "INSULPURPOSE",
                "OPERTEMP",
                "SP_ID",
                "TAGPREFIX",
                "TAGSEQUENCENO",
                "TRIMSPEC",
            ]
        );
        assert_eq!(
            nonempty_columns(&instrument),
            [
                "ALARMHIGH",
                "ALARMHIGHHIGH",
                "ALARMLOW",
                "ALARMLOWLOW",
                "INSTRFLUIDCODE",
                "INSTRFLUIDSYSTEM",
                "INSTRUMENTCLASS",
                "INSTRUMENTTYPE",
                "INSULATIONSPEC",
                "ISINLINE",
                "LOCATION",
                "MEASUREDVARIABLECODE",
                "SP_ID",
                "SP_PIPERUNID",
                "SP_SIGNALRUNID",
                "TAGSEQUENCENO",
            ]
        );
        assert_eq!(ids_in(&equipment, &model_ids, &published), (402, 402, 18));
        assert_eq!(
            ids_in(&instrument, &model_ids, &published),
            (3718, 3718, 150)
        );
    }

    fn nonempty_columns(table: &Table) -> Vec<&str> {
        let mut names: Vec<&str> = table
            .columns
            .iter()
            .enumerate()
            .filter(|(index, _)| {
                table
                    .rows
                    .iter()
                    .any(|row| !matches!(row.get(*index), Some(Field::Null) | None))
            })
            .map(|(_, column)| column.name.as_str())
            .collect();
        names.sort_unstable();
        names
    }

    fn ids_in(
        table: &Table,
        model_ids: &std::collections::HashSet<String>,
        published: &str,
    ) -> (usize, usize, usize) {
        let ids: Vec<String> = table
            .rows
            .iter()
            .filter_map(|row| text_at(row, &table.columns, "SP_ID"))
            .filter(|id| !id.is_empty())
            .collect();
        let in_model = ids.iter().filter(|id| model_ids.contains(*id)).count();
        let in_drawing = ids
            .iter()
            .filter(|id| published.contains(&format!("UID=\"{id}\"")))
            .count();
        (ids.len(), in_model, in_drawing)
    }

    fn take_uid(start: &quick_xml::events::BytesStart<'_>, uid: &mut String) {
        if start.name().as_ref() != "IObject" {
            return;
        }
        for attribute in start.attributes().flatten() {
            if attribute.key.as_ref() == "UID" {
                *uid = attribute
                    .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                    .map(std::borrow::Cow::into_owned)
                    .unwrap_or_default();
            }
        }
    }
}
