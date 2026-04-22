//! Object-graph DTO the SQLite loader feeds into the XML writer.
//!
//! Stage-1 scope: just enough shape to write a recognizable
//! `_Data.xml`. Richer fields (process-point numeric attributes,
//! symbology GUIDs, typicals) are additive and will land in
//! follow-up commits as the writer starts emitting them.

use std::collections::BTreeMap;
use std::fmt;

/// Error returned by any of the `publish::*` loaders. Kept string-
/// based so callers can bubble up context without defining an
/// enum for every SQL failure mode.
#[derive(Debug)]
pub enum PublishError {
    /// Generic SQLite / rusqlite failure.
    Sqlite(String),
    /// No drawing row matched the requested UID.
    DrawingNotFound { uid: String },
}

impl fmt::Display for PublishError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(msg) => write!(f, "SQLite: {msg}"),
            Self::DrawingNotFound { uid } => write!(f, "drawing UID `{uid}` not found"),
        }
    }
}

impl std::error::Error for PublishError {}

impl From<rusqlite::Error> for PublishError {
    fn from(err: rusqlite::Error) -> Self {
        Self::Sqlite(err.to_string())
    }
}

/// One SmartPlant model item — the shared identity for every
/// user-level object (Vessel / Nozzle / PipeRun / PipingPoint /
/// Instrument / Note / ...).
///
/// `item_type_name` mirrors `T_ModelItem.ItemTypeName` verbatim and
/// decides which SPPID subtable carries the rest of the attributes.
/// The writer consults this to pick the right `PIDxxx` XML tag.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PublishObject {
    /// `T_ModelItem.SP_ID`.
    pub uid: String,
    /// `T_ModelItem.ItemTypeName` — e.g. "Vessel", "Nozzle",
    /// "PipeRun", "PipingPoint", "Instrument", "Note".
    pub item_type_name: String,
    /// `T_ModelItem.Description` (optional in most rows).
    pub description: Option<String>,
    /// `T_ModelItem.SP_IsTypical` — "1" / null.
    pub is_typical: Option<String>,
    /// Business-subtable columns for this object, keyed by the
    /// SPPID column name (e.g. `"NominalDiameter"`,
    /// `"PipingMaterialsClass"`, `"EquipmentType"`).
    ///
    /// Populated by the loader when a matching row is found in
    /// the per-kind subtable (T_Equipment / T_Vessel / T_Nozzle /
    /// T_PipeRun / ...). Columns that store non-Text SQL types
    /// still surface here as their OrcaMDF-rendered string form.
    ///
    /// Values are kept as `String` rather than raw bytes so the
    /// XML writer can emit them directly; where numeric parsing
    /// is needed the writer (or a downstream consumer) calls
    /// `.parse::<...>()`.
    pub fields: BTreeMap<String, String>,
}

/// One representation (graphic instance) of a model item on a
/// drawing. Maps to `<PIDRepresentation>` + `DrawingItems` /
/// `DwgRepresentationComposition` rels in Publish Data XML.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PublishRepresentation {
    /// `T_Representation.SP_ID`.
    pub uid: String,
    /// `T_Representation.SP_ModelItemID` — references
    /// [`PublishObject::uid`]. Optional because annotation
    /// representations (labels, notes) can be drawing-scoped
    /// without a model item.
    pub model_item_uid: Option<String>,
    /// `T_Representation.SP_DrawingID` — the drawing this
    /// representation belongs to. Populated by the loader so
    /// callers can filter without recomputing the join.
    pub drawing_uid: String,
    /// `T_Representation.GraphicOID` — the runtime SmartPlant
    /// graphic object id. Surfaced as `GraphicOID="…"` in XML.
    pub graphic_oid: Option<i64>,
    /// `T_Representation.FileName` — symbol path (e.g.
    /// `\Equipment\Vessels\...\Horizontal Drum.sym`).
    pub symbol_path: Option<String>,
    /// `T_Representation.RepresentationType`.
    pub representation_type: Option<i64>,
}

/// One relationship row — the SPPID equivalent of an XML `<Rel>`
/// node. `source_uid` / `target_uid` correspond to
/// `SP_Item1ID` / `SP_Item2ID`; the loader does not normalize
/// directionality.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PublishRelationship {
    /// `T_Relationship.SP_ID`.
    pub uid: String,
    /// `T_Relationship.SP_DrawingID`.
    pub drawing_uid: String,
    /// `T_Relationship.SP_Item1ID`.
    pub source_uid: Option<String>,
    /// `T_Relationship.SP_Item2ID`.
    pub target_uid: Option<String>,
    /// `T_Relationship.GraphicOID` — rel-level graphic id.
    pub graphic_oid: Option<i64>,
    /// `T_Relationship.Item1Location` / `Item2Location` pair —
    /// directional hint.
    pub item1_location: Option<i64>,
    pub item2_location: Option<i64>,
    /// `T_Relationship.IsBinary`.
    pub is_binary: Option<i64>,
}

/// Top-level DTO — one drawing worth of SmartPlant data that the
/// XML writer will render into a single Publish Data document.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PublishDrawing {
    /// `T_Drawing.SP_ID` — the drawing's SmartPlant UID. Becomes
    /// `<Container DocUID>` in the XML output.
    pub drawing_uid: String,
    /// `T_Drawing.Name` — becomes `<Container DocName>` and
    /// `<PIDDrawing><IObject Name="...">`.
    pub drawing_name: String,
    /// `T_Drawing.DocumentCategory` — optional free-form string.
    pub document_category: Option<String>,
    /// `T_Drawing.DocumentType` — optional free-form string.
    pub document_type: Option<String>,
    /// `T_Drawing.Template` — the SmartPlant template name.
    pub template: Option<String>,
    /// `T_Drawing.Path` — the `.pid` drawing path in the SmartPlant
    /// archive tree.
    pub path: Option<String>,
    /// `T_Drawing.DateCreated` — free-form datetime string as
    /// emitted by OrcaMDF's value rendering.
    pub date_created: Option<String>,
    /// All model items that show up on this drawing. Populated by
    /// [`crate::publish::load_drawing_graph`] after matching
    /// T_Representation → T_ModelItem.
    pub objects: Vec<PublishObject>,
    /// Every representation tied to this drawing.
    pub representations: Vec<PublishRepresentation>,
    /// Every relationship tied to this drawing.
    pub relationships: Vec<PublishRelationship>,
}

impl PublishDrawing {
    /// Build a new DTO with every optional field unset. Convenience
    /// for tests and for the loader's "start-empty-then-fill"
    /// pattern.
    pub fn new(uid: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            drawing_uid: uid.into(),
            drawing_name: name.into(),
            ..Self::default()
        }
    }
}
