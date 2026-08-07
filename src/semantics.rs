//! Optional semantic layer: join a published `_Data.xml` onto the sheet
//! decode, so a drawn entity can answer "what am I?" (Phase 38 S3).
//!
//! `SmartPlant` publishes a drawing's semantic model as `<stem>_Data.xml`
//! next to the `.pid`. Each `<PIDRepresentation>` in it carries an
//! `IDrawingRepresentation GraphicOID="<n>"`, and Phase 38 S1 proved that
//! number lives in the same identifier space as the `oid` on every decoded
//! Sheet record (`docs/analysis/2026-08-07-graphic-oid-is-the-semantic-join.md`,
//! same-space null-hypothesis probability 9.8e-58 on `DWG-0202GP06-01`).
//!
//! The join follows S1's **two-hop rule**:
//!
//! 1. **Direct** — the queried oid is itself a published `GraphicOID`
//!    (symbols and points land here; the whole `igSymbol2d` family of the
//!    proving fixture is published).
//! 2. **Via dependency** — the published `GraphicOID` names a
//!    `DependencyObject` (PSM `0x00FA`) aggregate, and that record's tail
//!    references the queried oid (pipe runs land here: every one of the
//!    proving fixture's 12 aggregates references an `igLineString2d`).
//!    Tail references are read the way
//!    `docs/analysis/2026-08-04-graphicgroup-tail-property-block.md`
//!    established: aligned 4-byte windows filtered against the document's
//!    own decoded-oid pool.
//!
//! This layer is **strictly optional** (plan Stop clause): nothing here is
//! called during `.pid` parsing, a missing or unreadable `_Data.xml`
//! yields `None` from [`PidSemanticIndex::load_beside`], and no decode
//! behavior changes in either case. The `.pid` alone must keep working —
//! that is the importer's contract.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use crate::model::{PidDocument, SheetGeometry};

/// One published representation resolved to the model object that owns it.
///
/// `class` is the owning object's XML element name (`PIDPipeline`,
/// `PIDProcessVessel`, …). A representation no `DwgRepresentationComposition`
/// relationship claims keeps `class = "PIDRepresentation"` and its own UID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PidSemanticObject {
    /// Published `IDrawingRepresentation/@GraphicOID` — the join key.
    pub graphic_oid: u32,
    /// `IObject/@UID` of the `<PIDRepresentation>` element itself.
    pub representation_uid: String,
    /// XML element name of the owning model object, e.g. `PIDPipeline`.
    pub class: String,
    /// `IObject/@UID` of the owning model object, when one claims the
    /// representation.
    pub owner_uid: Option<String>,
    /// Owning object's `IObject/@ItemTag` (A01-flavor exports tag pipes,
    /// connectors and vessels here).
    pub item_tag: Option<String>,
    /// Owning object's `IObject/@Name` (DWG-flavor exports use `Name`
    /// where A01 uses `ItemTag`).
    pub name: Option<String>,
    /// Owning object's `IObject/@Description`.
    pub description: Option<String>,
}

impl PidSemanticObject {
    /// The label a consumer should show: `ItemTag` where present (A01
    /// flavor), else `Name` (DWG flavor), else nothing.
    pub fn label(&self) -> Option<&str> {
        self.item_tag
            .as_deref()
            .filter(|tag| !tag.is_empty())
            .or(self.name.as_deref().filter(|name| !name.is_empty()))
    }
}

/// How a queried oid reached a published object — the two hops of the S1
/// rule, kept distinct so consumers can present provenance honestly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PidSemanticHit<'a> {
    /// The queried oid is itself a published `GraphicOID`.
    Direct(&'a PidSemanticObject),
    /// The queried oid is referenced by the tail of a published
    /// `DependencyObject` aggregate (one hop).
    ViaDependency {
        /// The published object whose aggregate references the queried oid.
        object: &'a PidSemanticObject,
        /// The `DependencyObject` record's own oid (= the published
        /// `GraphicOID` that named the aggregate).
        dependency_oid: u32,
    },
}

impl PidSemanticHit<'_> {
    /// The resolved object, whichever hop found it.
    pub fn object(&self) -> &PidSemanticObject {
        match self {
            Self::Direct(object) => object,
            Self::ViaDependency { object, .. } => object,
        }
    }
}

/// Read-only oid → semantic-object index over one drawing's published
/// `_Data.xml`, plus the dependency edges of the drawing's own sheet
/// decode for the second hop.
#[derive(Debug, Clone, Default)]
pub struct PidSemanticIndex {
    source: Option<PathBuf>,
    by_oid: BTreeMap<u32, PidSemanticObject>,
    /// queried oid → `DependencyObject` oids whose tails reference it,
    /// sorted so resolution is deterministic.
    reverse_dependency: BTreeMap<u32, BTreeSet<u32>>,
}

impl PidSemanticIndex {
    /// Build the index from the `_Data.xml` sitting next to `pid_path`
    /// (`<stem>_Data.xml`), or `None` when there is no readable one.
    ///
    /// `None` is not an error: most drawings have no published
    /// counterpart, and the plan's Stop clause forbids treating the XML
    /// as a parse input. `doc` supplies the dependency edges and is never
    /// mutated.
    pub fn load_beside(pid_path: &Path, doc: &PidDocument) -> Option<Self> {
        let stem = pid_path.file_stem()?.to_string_lossy().into_owned();
        let xml_path = pid_path.with_file_name(format!("{stem}_Data.xml"));
        let xml = std::fs::read_to_string(&xml_path).ok()?;
        let mut index = Self::from_xml(&xml, doc);
        index.source = Some(xml_path);
        Some(index)
    }

    /// Build the index from XML text directly (the testable seam behind
    /// [`Self::load_beside`]).
    pub fn from_xml(xml: &str, doc: &PidDocument) -> Self {
        let scan = scan_published_xml(xml);

        let mut by_oid = BTreeMap::new();
        for representation in &scan.representations {
            let owner = scan
                .owner_of
                .get(&representation.uid)
                .and_then(|owner_uid| scan.objects.get(owner_uid));
            let object = PidSemanticObject {
                graphic_oid: representation.graphic_oid,
                representation_uid: representation.uid.clone(),
                class: owner.map_or_else(
                    || "PIDRepresentation".to_string(),
                    |owner| owner.class.clone(),
                ),
                owner_uid: scan.owner_of.get(&representation.uid).cloned(),
                item_tag: owner.and_then(|owner| owner.item_tag.clone()),
                name: owner.and_then(|owner| owner.name.clone()),
                description: owner.and_then(|owner| owner.description.clone()),
            };
            by_oid.insert(representation.graphic_oid, object);
        }

        Self {
            source: None,
            by_oid,
            reverse_dependency: reverse_dependency_edges(doc),
        }
    }

    /// Resolve a Sheet record oid (e.g. a drawn entity's `graphic_oid`)
    /// to its published object, applying the two-hop rule.
    ///
    /// Ambiguity policy: when several published aggregates reference the
    /// same oid, the smallest dependency oid wins — deterministic, and in
    /// the proving corpus the case does not arise.
    pub fn resolve(&self, graphic_oid: u32) -> Option<PidSemanticHit<'_>> {
        if let Some(object) = self.by_oid.get(&graphic_oid) {
            return Some(PidSemanticHit::Direct(object));
        }
        for dependency_oid in self.reverse_dependency.get(&graphic_oid)? {
            if let Some(object) = self.by_oid.get(dependency_oid) {
                return Some(PidSemanticHit::ViaDependency {
                    object,
                    dependency_oid: *dependency_oid,
                });
            }
        }
        None
    }

    /// Every published object, ordered by `GraphicOID`.
    pub fn objects(&self) -> impl Iterator<Item = &PidSemanticObject> {
        self.by_oid.values()
    }

    /// Number of published representations carrying a parseable
    /// `GraphicOID`.
    pub fn len(&self) -> usize {
        self.by_oid.len()
    }

    /// True when the XML published nothing joinable.
    pub fn is_empty(&self) -> bool {
        self.by_oid.is_empty()
    }

    /// The `_Data.xml` this index was loaded from, when it came from disk.
    pub fn source(&self) -> Option<&Path> {
        self.source.as_deref()
    }
}

/// Owning-object fields captured from one top-level XML block.
struct ScannedObject {
    class: String,
    item_tag: Option<String>,
    name: Option<String>,
    description: Option<String>,
}

/// One `<PIDRepresentation>` block's join fields.
struct ScannedRepresentation {
    uid: String,
    graphic_oid: u32,
}

/// Everything the index needs out of the XML, in one pass.
struct ScannedXml {
    /// Model-object UID → its captured fields.
    objects: BTreeMap<String, ScannedObject>,
    /// Every representation with a parseable `GraphicOID`.
    representations: Vec<ScannedRepresentation>,
    /// Representation UID → owning object UID
    /// (`Rel DefUID="DwgRepresentationComposition"`, UID1 owns UID2).
    owner_of: BTreeMap<String, String>,
}

fn attr(start: &BytesStart<'_>, name: &[u8]) -> Option<String> {
    start.attributes().flatten().find_map(|attribute| {
        (attribute.key.as_ref() == name)
            .then(|| String::from_utf8_lossy(&attribute.value).into_owned())
    })
}

/// One streaming pass over the published XML.
///
/// The vendor writes one attribute-bearing empty element per interface
/// (`<IObject …/>`, `<IRel …/>`), all nested exactly one level below a
/// top-level block element under `<Container>`. The scan tracks that
/// depth-1/depth-2 shape and nothing deeper.
fn scan_published_xml(xml: &str) -> ScannedXml {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut objects = BTreeMap::new();
    let mut representations = Vec::new();
    let mut owner_of = BTreeMap::new();

    // (block tag, IObject UID, block fields, GraphicOID, IRel triple)
    struct Block {
        tag: String,
        uid: Option<String>,
        item_tag: Option<String>,
        name: Option<String>,
        description: Option<String>,
        graphic_oid: Option<u32>,
        rel: Option<(String, String, String)>,
    }

    let mut depth = 0usize;
    let mut block: Option<Block> = None;

    let capture_child = |block: &mut Option<Block>, start: &BytesStart<'_>| {
        let Some(block) = block.as_mut() else {
            return;
        };
        match start.name().as_ref() {
            b"IObject" => {
                block.uid = attr(start, b"UID").or(block.uid.take());
                block.item_tag = attr(start, b"ItemTag").or(block.item_tag.take());
                block.name = attr(start, b"Name").or(block.name.take());
                block.description = attr(start, b"Description").or(block.description.take());
            }
            b"IDrawingRepresentation" => {
                block.graphic_oid = attr(start, b"GraphicOID")
                    .and_then(|value| value.parse::<u32>().ok())
                    .or(block.graphic_oid.take());
            }
            b"IRel" => {
                if let (Some(uid1), Some(uid2), Some(def_uid)) = (
                    attr(start, b"UID1"),
                    attr(start, b"UID2"),
                    attr(start, b"DefUID"),
                ) {
                    block.rel = Some((uid1, uid2, def_uid));
                }
            }
            _ => {}
        }
    };

    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) => {
                if depth == 1 && block.is_none() {
                    block = Some(Block {
                        tag: String::from_utf8_lossy(start.name().as_ref()).into_owned(),
                        uid: None,
                        item_tag: None,
                        name: None,
                        description: None,
                        graphic_oid: None,
                        rel: None,
                    });
                } else {
                    capture_child(&mut block, &start);
                }
                depth += 1;
            }
            Ok(Event::Empty(start)) => capture_child(&mut block, &start),
            Ok(Event::End(_)) => {
                depth = depth.saturating_sub(1);
                if depth == 1 {
                    if let Some(finished) = block.take() {
                        match finished.tag.as_str() {
                            "Rel" => {
                                if let Some((uid1, uid2, def_uid)) = finished.rel {
                                    if def_uid == "DwgRepresentationComposition" {
                                        owner_of.insert(uid2, uid1);
                                    }
                                }
                            }
                            "PIDRepresentation" => {
                                if let (Some(uid), Some(graphic_oid)) =
                                    (finished.uid, finished.graphic_oid)
                                {
                                    representations
                                        .push(ScannedRepresentation { uid, graphic_oid });
                                }
                            }
                            _ => {
                                if let Some(uid) = finished.uid {
                                    objects.insert(
                                        uid,
                                        ScannedObject {
                                            class: finished.tag,
                                            item_tag: finished.item_tag,
                                            name: finished.name,
                                            description: finished.description,
                                        },
                                    );
                                }
                            }
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            // The vendor XML is machine-generated; on a malformed byte the
            // scan keeps what it has rather than failing the whole join.
            Err(_) => break,
        }
    }

    ScannedXml {
        objects,
        representations,
        owner_of,
    }
}

/// Every oid the document's typed Sheet decode defines. This is the pool a
/// dependency-tail window must land in to count as a reference — the same
/// screen the S1 probe used, so the library join cannot claim more than
/// the evidence did.
fn sheet_oid_pool(geometry: &SheetGeometry) -> BTreeSet<u32> {
    let mut pool = BTreeSet::new();
    pool.extend(geometry.decoded_primitive_lines.iter().map(|r| r.oid));
    pool.extend(geometry.decoded_iglines.iter().map(|r| r.oid));
    pool.extend(geometry.decoded_iglinestrings.iter().map(|r| r.oid));
    pool.extend(geometry.decoded_igpoints.iter().map(|r| r.oid));
    pool.extend(geometry.decoded_igtextboxes.iter().map(|r| r.oid));
    pool.extend(geometry.decoded_igsymbols.iter().map(|r| r.oid));
    pool.extend(geometry.decoded_igboundaries.iter().map(|r| r.oid));
    pool.extend(geometry.decoded_igsmartframes.iter().map(|r| r.oid));
    pool.extend(geometry.decoded_dependency_objects.iter().map(|r| r.oid));
    pool.extend(geometry.decoded_jstyle_overrides.iter().map(|r| r.oid));
    pool
}

/// queried oid → the `DependencyObject` oids whose tails reference it.
///
/// Tail references are aligned 4-byte windows (payload `+18` onward, step
/// 4) filtered against the document's own oid pool — the reading proven by
/// the `0x00FA` tail-column analysis (`+22` / `+34` reference columns) and
/// exercised by the S1 probe on every published aggregate.
fn reverse_dependency_edges(doc: &PidDocument) -> BTreeMap<u32, BTreeSet<u32>> {
    let mut reverse: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
    for sheet in &doc.sheet_streams {
        let Some(geometry) = sheet.geometry.as_ref() else {
            continue;
        };
        let pool = sheet_oid_pool(geometry);
        for dependency in &geometry.decoded_dependency_objects {
            let tail = &dependency.raw_reference_payload;
            for at in (0..tail.len().saturating_sub(3)).step_by(4) {
                let value =
                    u32::from_le_bytes([tail[at], tail[at + 1], tail[at + 2], tail[at + 3]]);
                if value == 0 || value == dependency.oid || !pool.contains(&value) {
                    continue;
                }
                reverse.entry(value).or_default().insert(dependency.oid);
            }
        }
    }
    reverse
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DecodedDependencyObjectRecord, DecodedIgPoint2dRecord, SheetStream};

    const A01_LIKE_XML: &str = r#"<?xml version ="1.0" encoding="UTF-8"?>
<Container CompSchema="PIDComponent" DocName="A01">
   <PIDProcessVessel>
      <IObject UID="VESSEL-1" ItemTag="V 010121A" Description=""/>
      <IEquipment EqTypeDescription="Horizontal Drum"/>
   </PIDProcessVessel>
   <PIDPipeline>
      <IObject UID="PIPE-1" ItemTag="PH- 0102102-DN250"/>
   </PIDPipeline>
   <PIDRepresentation>
      <IObject UID="REP-VESSEL"/>
      <IDrawingRepresentation GraphicOID="24601"/>
   </PIDRepresentation>
   <PIDRepresentation>
      <IObject UID="REP-PIPE"/>
      <IDrawingRepresentation GraphicOID="24613"/>
   </PIDRepresentation>
   <PIDRepresentation>
      <IObject UID="REP-ORPHAN"/>
      <IDrawingRepresentation GraphicOID="24615"/>
   </PIDRepresentation>
   <Rel>
      <IObject UID="REL-1"/>
      <IRel UID1="VESSEL-1" UID2="REP-VESSEL" DefUID="DwgRepresentationComposition"/>
   </Rel>
   <Rel>
      <IObject UID="REL-2"/>
      <IRel UID1="PIPE-1" UID2="REP-PIPE" DefUID="DwgRepresentationComposition"/>
   </Rel>
   <Rel>
      <IObject UID="REL-3"/>
      <IRel UID1="PIPE-1" UID2="REP-ORPHAN" DefUID="DrawingItems"/>
   </Rel>
</Container>"#;

    fn empty_doc() -> PidDocument {
        PidDocument::default()
    }

    #[test]
    fn published_representations_resolve_to_their_owners() {
        let index = PidSemanticIndex::from_xml(A01_LIKE_XML, &empty_doc());

        assert_eq!(index.len(), 3);

        let Some(PidSemanticHit::Direct(vessel)) = index.resolve(24601) else {
            panic!("24601 must resolve directly");
        };
        assert_eq!(vessel.class, "PIDProcessVessel");
        assert_eq!(vessel.label(), Some("V 010121A"));
        assert_eq!(vessel.owner_uid.as_deref(), Some("VESSEL-1"));

        let pipe = index.resolve(24613).expect("pipe rep").object().clone();
        assert_eq!(pipe.class, "PIDPipeline");
        assert_eq!(pipe.item_tag.as_deref(), Some("PH- 0102102-DN250"));
    }

    #[test]
    fn a_representation_without_a_composition_rel_keeps_its_own_identity() {
        // REL-3 is a DrawingItems rel, not DwgRepresentationComposition, so
        // REP-ORPHAN has no owner and must not borrow one.
        let index = PidSemanticIndex::from_xml(A01_LIKE_XML, &empty_doc());

        let Some(PidSemanticHit::Direct(orphan)) = index.resolve(24615) else {
            panic!("24615 must resolve directly");
        };
        assert_eq!(orphan.class, "PIDRepresentation");
        assert_eq!(orphan.owner_uid, None);
        assert_eq!(orphan.label(), None);
    }

    /// A doc with one dependency aggregate (oid 103) whose tail references
    /// a decoded point (oid 139), matching the S1 fixture shape.
    fn doc_with_dependency_edge() -> PidDocument {
        let mut tail = vec![0u8; 4];
        tail.extend_from_slice(&139u32.to_le_bytes());
        let mut doc = PidDocument::default();
        doc.sheet_streams.push(SheetStream {
            name: "Sheet6".into(),
            path: "/Sheet6".into(),
            size: 0,
            extracted_texts: Vec::new(),
            magic_u32_le: None,
            magic_tag: None,
            header: None,
            attribute_records: Vec::new(),
            probe_summary: None,
            geometry: Some(SheetGeometry {
                decoded_igpoints: vec![DecodedIgPoint2dRecord {
                    byte_start: 0,
                    byte_end: 40,
                    type_code: 0x005E,
                    type_flags: 0,
                    bytes_to_follow: 34,
                    oid: 139,
                    parent_ref: 6,
                    sub_type_word: 0,
                    index: 0,
                    x: 0.5,
                    y: 0.5,
                }],
                decoded_dependency_objects: vec![DecodedDependencyObjectRecord {
                    byte_start: 40,
                    byte_end: 104,
                    type_code: 0x00FA,
                    type_flags: 0,
                    bytes_to_follow: 58,
                    oid: 103,
                    parent_ref: 6,
                    group_kind_word: 4,
                    sub_type_word: 1,
                    raw_reference_payload: tail,
                }],
                ..SheetGeometry::default()
            }),
            endpoint_records: Vec::new(),
            endpoint_decode_error: None,
        });
        doc
    }

    const AGGREGATE_XML: &str = r#"<?xml version ="1.0" encoding="UTF-8"?>
<Container>
   <PIDPipeline>
      <IObject UID="PIPE-1" ItemTag="PL-103"/>
   </PIDPipeline>
   <PIDRepresentation>
      <IObject UID="REP-1"/>
      <IDrawingRepresentation GraphicOID="103"/>
   </PIDRepresentation>
   <Rel>
      <IObject UID="REL-1"/>
      <IRel UID1="PIPE-1" UID2="REP-1" DefUID="DwgRepresentationComposition"/>
   </Rel>
</Container>"#;

    #[test]
    fn an_aggregate_edge_resolves_via_dependency() {
        let index = PidSemanticIndex::from_xml(AGGREGATE_XML, &doc_with_dependency_edge());

        // The aggregate itself resolves directly.
        assert!(matches!(
            index.resolve(103),
            Some(PidSemanticHit::Direct(object)) if object.class == "PIDPipeline"
        ));

        // The leaf the aggregate references resolves one hop out.
        let Some(PidSemanticHit::ViaDependency {
            object,
            dependency_oid,
        }) = index.resolve(139)
        else {
            panic!("139 must resolve via the 103 aggregate");
        };
        assert_eq!(dependency_oid, 103);
        assert_eq!(object.label(), Some("PL-103"));

        // An oid nothing references stays unresolved.
        assert!(index.resolve(555).is_none());
    }

    #[test]
    fn a_drawing_without_published_xml_yields_none() {
        let missing = Path::new("test-file/definitely-not-here.pid");
        assert!(PidSemanticIndex::load_beside(missing, &empty_doc()).is_none());
    }
}
