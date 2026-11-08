//! The layout-side ledger: what the layout engine did with each entity, and
//! what the page's sources were.
//!
//! This is the in-memory half of the account — [`LayoutLedger`] rides on
//! `PaperModel::ledger` and is **not** serialized (`#[serde(skip)]`), so a page
//! model's JSON is byte-for-byte what it was before the ledger existed. The
//! serializable account the gate compares against a baseline is
//! [`ExportLedger`](super::assemble::ExportLedger), assembled from this one plus
//! the writer's own ledger.
//!
//! The return types at the bottom ([`EntityLayout`], [`PlacementLayout`],
//! [`BodyItemLayout`]) are what the layout engine's private functions hand back
//! to the recorder. They are plain structs rather than `Result` on purpose: the
//! existing unit tests call those functions as statements, and a `#[must_use]`
//! value under `RUSTFLAGS=-Dwarnings` would turn every one of them into an
//! error.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::reason::{Asymmetry, DomainPair, DropReason, LedgerDomain, PlacementSource};

/// What one page's layout pass recorded.
///
/// Not serialized onto a page model: it lives on
/// [`PaperModel::ledger`](crate::model::PaperModel) behind `#[serde(skip)]`, and
/// the model's own JSON stays exactly as it was. Everything a baseline needs is
/// copied out of here into
/// [`ExportLedger`](super::assemble::ExportLedger).
///
/// It still implements [`Serialize`] and [`Deserialize`] — the ledger types are
/// all round-trippable (requirement 12.8's往返 property) and a probe that wants
/// to inspect the layout half alone can serialize it without going through the
/// assembly.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LayoutLedger {
    /// The symbol and template sources the search actually had, in order.
    pub sources: PageSources,
    /// Template candidates that could not be read or parsed (A.8).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub template_unreadable: Vec<SourceName>,
    /// The parse layer's counts, one per domain (A.1).
    pub parse: DomainPair<ParseCounts>,
    /// How many entities the geometry projection produced, per domain.
    ///
    /// This is the input side of the entity conservation equation, taken
    /// independently of the outcome records so that a missing outcome shows up
    /// as a remainder instead of being absorbed.
    pub entity_totals: DomainPair<u64>,
    /// One record per entity the layout engine visited.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entities: Vec<EntityRecord>,
    /// What the text normalizer removed, per domain (A.4).
    pub text: DomainPair<TextNormalization>,
    /// Index into [`Self::entities`] for each of `PaperModel::primitives`, same
    /// order and same length.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub primitive_origin: Vec<u32>,
    /// The entity records whose primitives the final `retain` removed, in the
    /// order the `retain` visited them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub off_sheet: Vec<u32>,
}

/// One domain's parse-layer counts (A.1).
///
/// Kept as records rather than as a flat map so that the site of a drop — the
/// stream it was read from, the type code, the family — survives into the
/// baseline: a count alone says a decoder refused eight records, the record
/// says which eight.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ParseCounts {
    /// One entry per `(reason, stream, type code, family)`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<ParseRecord>,
}

impl ParseCounts {
    /// The number of records this domain dropped at parse time, summed over
    /// every reason.
    #[must_use]
    pub fn total(&self) -> u64 {
        self.items
            .iter()
            .fold(0u64, |sum, item| sum.saturating_add(item.count))
    }
}

/// One parse-layer count: how many records one reason took from one site.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParseRecord {
    /// Which drop this is.
    pub reason: DropReason,
    /// The Sheet stream's path, or the storage path for a body record.
    pub stream: String,
    /// The PSM type code, when the reason is per type code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_code: Option<u16>,
    /// The record family's registry name, when the reason is per family.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    /// How many records.
    pub count: u64,
    /// The stream's byte size, for [`DropReason::SheetStreamWithoutGeometry`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
    /// How many body primitives an unplaced cache definition carries, for
    /// [`DropReason::CacheDefinitionUnplaced`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_primitives: Option<u64>,
}

/// The symbol and template sources a page's layout pass had.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PageSources {
    /// The symbol library search order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub symbols: Vec<SourceName>,
    /// The template candidates the search saw.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub templates: Vec<SourceName>,
}

/// One source a page was laid out from: a directory, an archive, or a file
/// inside one of them.
///
/// Names only. The corpus descriptor a baseline stores is built from these, and
/// requirement 12.4 keeps absolute machine paths out of a baseline — so a
/// [`SourceName`] holds the file or member name, never the path it was found
/// under.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SourceName {
    /// What kind of source this is.
    pub kind: SourceKind,
    /// The directory name, the archive's file name, the template's file name,
    /// or the archive member's name.
    pub name: String,
    /// The archive's file name, when this source is a member of one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archive: Option<String>,
}

/// Where a source lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    /// A directory searched for symbol bodies.
    Directory,
    /// A `RefData~*.zip` searched for symbol bodies or templates.
    Archive,
    /// A template file on disk.
    DirectoryFile,
    /// A template read out of an archive.
    ArchiveMember,
}

impl SourceKind {
    /// The kind's code, which is also its serde name.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::Directory => "directory",
            Self::Archive => "archive",
            Self::DirectoryFile => "directory_file",
            Self::ArchiveMember => "archive_member",
        }
    }
}

/// The template search's own account (A.8).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TemplateTrace {
    /// Every candidate the search considered, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<SourceName>,
    /// The candidates that could not be read or parsed, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unreadable: Vec<SourceName>,
}

/// One entity the layout engine visited.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityRecord {
    /// Which account it belongs to.
    pub domain: LedgerDomain,
    /// Its graphic kind.
    pub kind: EntityKind,
    /// How sure the projection was.
    pub confidence: Confidence,
    /// The layout layer's verdict, when it produced no page primitive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dropped: Option<DropReason>,
    /// For a drawing-domain `fill_duplicate`: who took the key first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill_first_taker: Option<LedgerDomain>,
    /// Where the body came from and what it produced, for a `SymbolInstance`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placement: Option<PlacementAccount>,
    /// What the furniture domain drew that the drawing domain would have
    /// dropped (A.7).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub observed: Vec<Asymmetry>,
    /// The entity's site in the file.
    pub site: Site,
}

/// Where an entity came from: enough to find it again in the drawing.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Site {
    /// The Sheet stream's path, or the storage path for a body entity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<String>,
    /// The graphic oid, when the record carries one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oid: Option<u32>,
    /// The record's byte range in its stream.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<[usize; 2]>,
    /// The record kind the projection named.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record_kind: Option<String>,
}

/// One symbol placement's account: where the body came from and what it drew.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlacementAccount {
    /// Where the body came from.
    pub source: PlacementSource,
    /// The body primitives, counted.
    pub body: BodyTally,
}

/// A symbol body's primitive tally (requirement 4.4).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BodyTally {
    /// How many body primitives the body carried.
    pub total: u64,
    /// How many of them produced a page primitive.
    pub produced: u64,
    /// The rest, by A.3 reason.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dropped: BTreeMap<DropReason, u64>,
    /// The remainder that no reason accounts for. Has to be zero.
    pub unattributed: u64,
}

/// An entity's graphic kind, by variant name.
///
/// A name rather than a copy of `PidGraphicKind`: the ledger counts kinds, and
/// the variant name is what a reader — and a baseline file — needs. Adding a
/// kind to the projection adds a name here rather than a new field.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    /// `PidGraphicKind::Line`.
    Line,
    /// `PidGraphicKind::Polyline`.
    Polyline,
    /// `PidGraphicKind::Arc`.
    Arc,
    /// `PidGraphicKind::Circle`.
    Circle,
    /// `PidGraphicKind::Point`.
    Point,
    /// `PidGraphicKind::Text`.
    Text,
    /// `PidGraphicKind::SymbolInstance`.
    SymbolInstance,
    /// `PidGraphicKind::Annotation`.
    Annotation,
    /// `PidGraphicKind::Unknown`.
    Unknown,
}

impl EntityKind {
    /// The kind's code, which is also its serde name.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::Line => "line",
            Self::Polyline => "polyline",
            Self::Arc => "arc",
            Self::Circle => "circle",
            Self::Point => "point",
            Self::Text => "text",
            Self::SymbolInstance => "symbol_instance",
            Self::Annotation => "annotation",
            Self::Unknown => "unknown",
        }
    }

    /// The kind a projection confidence name maps from, if it is a kind.
    ///
    /// Used by the cross-count: requirement 1.6 counts entities by kind, by
    /// confidence and by outcome, and both halves of that key come from names.
    #[must_use]
    pub fn from_projection_name(name: &str) -> Option<Self> {
        match name {
            "Line" => Some(Self::Line),
            "Polyline" => Some(Self::Polyline),
            "Arc" => Some(Self::Arc),
            "Circle" => Some(Self::Circle),
            "Point" => Some(Self::Point),
            "Text" => Some(Self::Text),
            "SymbolInstance" => Some(Self::SymbolInstance),
            "Annotation" => Some(Self::Annotation),
            "Unknown" => Some(Self::Unknown),
            _ => None,
        }
    }
}

/// How sure the geometry projection was about an entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    /// Read from bytes whose layout is decoded.
    Decoded,
    /// Inferred from a record whose reading is a guess.
    Inferred,
    /// Byte provenance without a reading: evidence, not geometry.
    ProbeOnly,
}

impl Confidence {
    /// Every confidence, in the projection's own order.
    #[must_use]
    pub fn all() -> &'static [Confidence] {
        &[Self::Decoded, Self::Inferred, Self::ProbeOnly]
    }

    /// The confidence's code, which is also its serde name.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::Decoded => "decoded",
            Self::Inferred => "inferred",
            Self::ProbeOnly => "probe_only",
        }
    }

    /// Whether the writer plots this layer (`is_plotted`).
    ///
    /// Probe-only evidence stays in the model and off the page, which is
    /// [`DropReason::ProbeOnly`].
    #[must_use]
    pub fn is_plotted(self) -> bool {
        matches!(self, Self::Decoded | Self::Inferred)
    }
}

/// What the text normalizer removed from one domain's labels (A.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TextNormalization {
    /// Control characters removed, counted per character. A tab replaced by a
    /// space is a substitution and is not counted.
    pub control_chars: u64,
    /// Trailing empty lines dropped, counted per line.
    pub trailing_empty_lines: u64,
}

impl TextNormalization {
    /// Add another label's removals to this total.
    pub fn add(&mut self, other: Self) {
        self.control_chars = self.control_chars.saturating_add(other.control_chars);
        self.trailing_empty_lines = self
            .trailing_empty_lines
            .saturating_add(other.trailing_empty_lines);
    }
}

/// What the layout engine did with one entity (A.2), handed back to the
/// recorder.
///
/// Returned by value from `push_entity_with`, `push_entity` and
/// `push_furniture_entity`. Not `#[must_use]`: the existing unit tests call
/// those functions as statements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct EntityLayout {
    /// The layout layer's verdict, when it produced no page primitive.
    pub dropped: Option<DropReason>,
    /// An `igBoundary2d` fill's deduplication key — set both when the fill is
    /// produced and when it is refused as a duplicate, so the recorder can name
    /// who took the key first.
    pub fill_key: Option<u64>,
    /// What this entity's labels lost to normalization.
    pub text: TextNormalization,
}

/// What one symbol placement did (requirements 4.1–4.3), handed back to the
/// recorder.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PlacementLayout {
    /// The layout layer's verdict, when the placement drew nothing.
    pub dropped: Option<DropReason>,
    /// Where the body came from.
    pub source: PlacementSource,
    /// The body primitives, counted.
    pub body: BodyTally,
}

/// What one body primitive did (A.3), handed back to the recorder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BodyItemLayout {
    /// How many page primitives this body primitive produced.
    pub produced: usize,
    /// When `produced` is zero, the A.3 reason. `None` with a zero `produced`
    /// is a body-layer remainder — an empty-member `Boundary`, per design
    /// deviation I3 — and makes the gate fail rather than pass quietly.
    pub dropped: Option<DropReason>,
}

#[cfg(test)]
// A test's job is to fail loudly on a bad value, so the four production bans
// are lifted here and only here (design: "测试模块按需 #[allow]").
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_ledger_is_empty_and_equal_to_itself() {
        let ledger = LayoutLedger::default();
        assert!(ledger.entities.is_empty());
        assert!(ledger.primitive_origin.is_empty());
        assert_eq!(ledger, LayoutLedger::default());
        assert_eq!(ledger.entity_totals.drawing, 0);
        assert_eq!(ledger.entity_totals.furniture, 0);
    }

    #[test]
    fn text_normalization_accumulates_without_overflow() {
        let mut total = TextNormalization::default();
        total.add(TextNormalization {
            control_chars: 2,
            trailing_empty_lines: 1,
        });
        total.add(TextNormalization {
            control_chars: 3,
            trailing_empty_lines: 0,
        });
        assert_eq!(total.control_chars, 5);
        assert_eq!(total.trailing_empty_lines, 1);
    }

    #[test]
    fn a_parse_account_sums_its_records() {
        let counts = ParseCounts {
            items: vec![
                ParseRecord {
                    reason: DropReason::RecordNoDecoder,
                    stream: "/Sheet6".to_string(),
                    type_code: Some(0x0019),
                    family: None,
                    count: 5,
                    bytes: None,
                    body_primitives: None,
                },
                ParseRecord {
                    reason: DropReason::RecordRefused,
                    stream: "/Sheet6".to_string(),
                    type_code: Some(0x00CE),
                    family: None,
                    count: 1,
                    bytes: None,
                    body_primitives: None,
                },
            ],
        };
        assert_eq!(counts.total(), 6);
    }

    #[test]
    fn entity_kind_names_round_trip_through_the_projection() {
        for kind in [
            EntityKind::Line,
            EntityKind::Polyline,
            EntityKind::Arc,
            EntityKind::Circle,
            EntityKind::Point,
            EntityKind::Text,
            EntityKind::SymbolInstance,
            EntityKind::Annotation,
            EntityKind::Unknown,
        ] {
            let json = serde_json::to_string(&kind).expect("a unit variant serializes");
            assert_eq!(json, format!("\"{}\"", kind.code()));
        }
        assert_eq!(
            EntityKind::from_projection_name("SymbolInstance"),
            Some(EntityKind::SymbolInstance)
        );
        assert_eq!(EntityKind::from_projection_name("Nope"), None);
    }

    #[test]
    fn only_decoded_and_inferred_are_plotted() {
        assert!(Confidence::Decoded.is_plotted());
        assert!(Confidence::Inferred.is_plotted());
        assert!(!Confidence::ProbeOnly.is_plotted());
    }

    #[test]
    fn source_kinds_name_their_serde_form() {
        for kind in [
            SourceKind::Directory,
            SourceKind::Archive,
            SourceKind::DirectoryFile,
            SourceKind::ArchiveMember,
        ] {
            let json = serde_json::to_string(&kind).expect("a unit variant serializes");
            assert_eq!(json, format!("\"{}\"", kind.code()));
        }
    }

    #[test]
    fn a_layout_ledger_round_trips() {
        // Requirement 12.8's round-trip is stated for `ExportLedger`, but the
        // layout half is the input it is assembled from: a type that loses
        // information on the way through JSON would make the assembled account
        // unreproducible for a reason no test on `ExportLedger` alone would
        // show.
        let mut ledger = LayoutLedger::default();
        ledger.entity_totals.drawing = 3;
        ledger.entity_totals.furniture = 2;
        ledger.entities.push(EntityRecord {
            domain: LedgerDomain::Drawing,
            kind: EntityKind::Line,
            confidence: Confidence::Decoded,
            dropped: None,
            fill_first_taker: None,
            placement: Some(PlacementAccount {
                source: PlacementSource::Cached,
                body: BodyTally {
                    total: 4,
                    produced: 3,
                    dropped: BTreeMap::from([(DropReason::BodyLayerHidden, 1)]),
                    unattributed: 0,
                },
            }),
            observed: vec![Asymmetry::FurnitureEndpointPairDrawn],
            site: Site {
                stream: Some("/Sheet6".to_string()),
                oid: Some(0x1234),
                bytes: Some([100, 160]),
                record_kind: Some("PrimitiveLine".to_string()),
            },
        });
        ledger.parse.drawing.items.push(ParseRecord {
            reason: DropReason::RecordNoDecoder,
            stream: "/Sheet6".to_string(),
            type_code: Some(0x0019),
            family: None,
            count: 5,
            bytes: None,
            body_primitives: None,
        });
        ledger.text.drawing.control_chars = 2;
        ledger.primitive_origin.push(0);
        ledger.off_sheet.push(0);

        let json = serde_json::to_string(&ledger).expect("a layout ledger serializes");
        let back: LayoutLedger = serde_json::from_str(&json).expect("and reads back");
        assert_eq!(back, ledger);
    }
}
