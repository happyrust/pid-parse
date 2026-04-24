//! Object-graph DTO the SQLite loader feeds into the XML writer.
//!
//! Current scope: enough shape to drive the shipped `_Data.xml` and
//! `_Meta.xml` writers, including plant-wide codelist metadata and
//! the explicit A01/DWG style selector.
//!
//! The DTO still lacks DWG-MDF-derived branch-point nodes and any
//! loader canonical fields that cannot be validated without the DWG
//! `Export.mdf` fixture; those remain additive.

use std::collections::BTreeMap;
use std::fmt;

/// SmartPlant codelist lookup — maps `(codelist_number, codelist_index)`
/// pairs to their human-readable display text, plus an auxiliary
/// `attribute_name → codelist_number` map so callers can resolve
/// business-field values (e.g. `EquipmentType = "0"`) to
/// descriptions (e.g. `"Horizontal Drum"`) by attribute name alone.
///
/// The SmartPlant metadata catalog stores this as two tables:
///
/// * `codelists(codelist_number, codelist_index, codelist_text, ...)`
///   — one row per enum entry.
/// * `attributes(attribute_name, attribute_codelisted, ...)`
///   — declares which codelist an attribute pulls from.
///
/// Stage-1 A7 surfaces this to the XML writer so `EqTypeDescription`
/// no longer has to rely solely on symbol-path parsing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CodelistIndex {
    /// `(codelist_number, codelist_index) → codelist_text`.
    /// Both keys are kept as `String` because the publish loader
    /// stages them as TEXT regardless of the underlying SQL Server
    /// type.
    entries: BTreeMap<(String, String), String>,
    /// `attribute_name → codelist_number`, sourced from the
    /// `attributes` metadata table's `attribute_codelisted` column.
    /// Empty / zero / null values are filtered out at load time —
    /// they mean "this attribute is not codelisted".
    attribute_codelist: BTreeMap<String, String>,
}

impl CodelistIndex {
    /// Insert a single codelist entry. Duplicate keys overwrite —
    /// real SPPID catalogs do not produce duplicates, but the
    /// loader is conservative rather than panicking on import.
    pub fn insert_entry(
        &mut self,
        codelist_number: impl Into<String>,
        codelist_index: impl Into<String>,
        codelist_text: impl Into<String>,
    ) {
        self.entries
            .insert((codelist_number.into(), codelist_index.into()), codelist_text.into());
    }

    /// Record that `attribute_name` is codelisted under
    /// `codelist_number`. Used by [`CodelistIndex::lookup_by_attribute`].
    pub fn insert_attribute_mapping(
        &mut self,
        attribute_name: impl Into<String>,
        codelist_number: impl Into<String>,
    ) {
        self.attribute_codelist
            .insert(attribute_name.into(), codelist_number.into());
    }

    /// Direct lookup by `(codelist_number, codelist_index)` pair.
    /// Returns the display text, or `None` if no row matches.
    pub fn lookup(&self, codelist_number: &str, codelist_index: &str) -> Option<&str> {
        self.entries
            .get(&(codelist_number.to_string(), codelist_index.to_string()))
            .map(|s| s.as_str())
    }

    /// Resolve a codelisted attribute value by name:
    /// `attribute_name` → codelist_number → `(codelist_number, value)`
    /// → display text. Returns `None` when either the attribute is
    /// not codelisted or the value is not in the resolved codelist.
    pub fn lookup_by_attribute(&self, attribute_name: &str, value: &str) -> Option<&str> {
        let codelist_number = self.attribute_codelist.get(attribute_name)?;
        self.lookup(codelist_number, value)
    }

    /// True when no codelist entries and no attribute mappings are
    /// registered — useful for tests and for callers who want to
    /// skip the lookup fast-path when the fixture does not ship
    /// codelist metadata.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty() && self.attribute_codelist.is_empty()
    }

    /// Number of resolved codelist entries (not including the
    /// auxiliary attribute-name map).
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Number of registered `attribute_name → codelist_number`
    /// bindings.
    pub fn attribute_mapping_count(&self) -> usize {
        self.attribute_codelist.len()
    }
}

/// Error returned by any of the `publish::*` loaders. Kept string-
/// based so callers can bubble up context without defining an
/// enum for every SQL failure mode.
#[derive(Debug)]
pub enum PublishError {
    /// Generic SQLite / rusqlite failure.
    Sqlite(String),
    /// MDF parser failure from the Rust MDF reader.
    Mdf(String),
    /// No drawing row matched the requested UID.
    DrawingNotFound { uid: String },
}

impl fmt::Display for PublishError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(msg) => write!(f, "SQLite: {msg}"),
            Self::Mdf(msg) => write!(f, "MDF: {msg}"),
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

impl From<oxidized_mdf::error::Error> for PublishError {
    fn from(err: oxidized_mdf::error::Error) -> Self {
        Self::Mdf(err.to_string())
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
    /// still surface here as their loader-rendered string form.
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

/// SmartPlant export style — selects between the two
/// fixture-side variants observed on real Publish Data XML.
///
/// SmartPlant's Publish Data exporter emits structurally
/// identical XML across plants and projects, but two
/// fixture-side conventions differ in attribute naming on
/// `IObject`:
///
/// * **A01 style** — A01 plant export uses `ItemTag` on
///   PIDPipeline / PIDPipingConnector / PIDProcessVessel
///   IObjects; the business identifier is exposed under the
///   `ItemTag` attribute key. PIDProcessVessel always carries
///   the tag (UID + ItemTag + Description shape).
/// * **DWG style** — DWG plant export (DWG-0202GP06-01
///   reference fixture) uses `Name` instead of `ItemTag` on
///   the same IObjects. PIDProcessVessel omits the
///   identifier attribute entirely (UID + Description only).
///
/// Both styles publish the same data; the choice is a
/// per-plant SmartPlant project flavor, not a runtime
/// preference. The writer is shape-aware: it emits the
/// IObject according to the drawing's [`PublishDrawing::style`]
/// field. Default is [`PublishStyle::A01`] to preserve every
/// pre-A29 round-trip.
///
/// # Example
///
/// ```
/// use pid_parse::publish::{PublishDrawing, PublishStyle};
///
/// // Default style is A01 — every pre-A29 caller round-trips.
/// let mut drawing = PublishDrawing::new("UID-D", "DEMO");
/// assert_eq!(drawing.style, PublishStyle::A01);
///
/// // Opt into DWG-flavor IObject shape (drops ItemTag in favor
/// // of Name on PIDPipeline / PIDPipingConnector, omits the
/// // identifier on PIDProcessVessel).
/// drawing.style = PublishStyle::Dwg;
/// assert_eq!(drawing.style, PublishStyle::Dwg);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PublishStyle {
    /// A01-flavor fixture export — IObject uses `ItemTag` on
    /// pipe / connector / vessel; vessel always stamps a tag.
    /// This is the writer default.
    #[default]
    A01,
    /// DWG-flavor fixture export — IObject uses `Name`
    /// instead of `ItemTag` on pipe / connector; vessel omits
    /// the identifier attribute entirely.
    Dwg,
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
    /// emitted by the MDF loader's value rendering.
    pub date_created: Option<String>,
    /// All model items that show up on this drawing. Populated by
    /// [`crate::publish::load_drawing_graph`] after matching
    /// T_Representation → T_ModelItem.
    pub objects: Vec<PublishObject>,
    /// Every representation tied to this drawing.
    pub representations: Vec<PublishRepresentation>,
    /// Every relationship tied to this drawing.
    pub relationships: Vec<PublishRelationship>,
    /// Plant-wide SmartPlant codelist metadata. Populated once per
    /// load because the catalog is the same for every drawing; kept
    /// on the drawing DTO for convenience of the XML writer, which
    /// is already drawing-scoped.
    ///
    /// Empty when the fixture's SQLite mirror has not populated the
    /// `codelists` / `attributes` tables — the writer must fall
    /// through to its symbol-path or raw-value heuristics in that
    /// case.
    pub codelist: CodelistIndex,
    /// SmartPlant project-flavor selector — chooses between
    /// the A01 and DWG attribute-naming conventions on
    /// IObject. See [`PublishStyle`] for full semantics.
    /// Defaults to [`PublishStyle::A01`] so every pre-A29
    /// caller and round-trip stays byte-identical.
    pub style: PublishStyle,
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

#[cfg(test)]
mod codelist_tests {
    use super::*;

    #[test]
    fn empty_index_reports_empty() {
        let idx = CodelistIndex::default();
        assert!(idx.is_empty());
        assert_eq!(idx.entry_count(), 0);
        assert_eq!(idx.attribute_mapping_count(), 0);
        assert!(idx.lookup("28", "0").is_none());
        assert!(idx.lookup_by_attribute("EquipmentType", "0").is_none());
    }

    #[test]
    fn direct_lookup_returns_registered_text() {
        let mut idx = CodelistIndex::default();
        idx.insert_entry("28", "0", "Horizontal Drum");
        idx.insert_entry("28", "1", "Vertical Drum");
        assert_eq!(idx.lookup("28", "0"), Some("Horizontal Drum"));
        assert_eq!(idx.lookup("28", "1"), Some("Vertical Drum"));
        assert_eq!(idx.lookup("28", "99"), None);
        assert_eq!(idx.lookup("31", "0"), None);
    }

    #[test]
    fn lookup_by_attribute_resolves_via_attribute_mapping() {
        let mut idx = CodelistIndex::default();
        idx.insert_entry("28", "0", "Horizontal Drum");
        idx.insert_attribute_mapping("EquipmentType", "28");
        assert_eq!(
            idx.lookup_by_attribute("EquipmentType", "0"),
            Some("Horizontal Drum"),
        );
        // Known attribute but value missing from codelist → None.
        assert_eq!(idx.lookup_by_attribute("EquipmentType", "9999"), None);
        // Unknown attribute → None even if the value matches some
        // other codelist.
        assert_eq!(idx.lookup_by_attribute("NominalDiameter", "0"), None);
    }

    #[test]
    fn insert_entry_overwrites_duplicate_keys() {
        let mut idx = CodelistIndex::default();
        idx.insert_entry("28", "0", "Stale");
        idx.insert_entry("28", "0", "Fresh");
        assert_eq!(idx.lookup("28", "0"), Some("Fresh"));
        assert_eq!(idx.entry_count(), 1);
    }

    #[test]
    fn is_empty_becomes_false_with_any_registration() {
        let mut idx = CodelistIndex::default();
        idx.insert_attribute_mapping("EquipmentType", "28");
        assert!(!idx.is_empty());

        let mut idx2 = CodelistIndex::default();
        idx2.insert_entry("28", "0", "Horizontal Drum");
        assert!(!idx2.is_empty());
    }
}
