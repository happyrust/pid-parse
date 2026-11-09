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

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use pid_parse::{
    NormalizedPidGeometry, PidGeometryConfidence, PidGraphicEntity, PidGraphicKind,
    PidSymbolDefinitionRef,
};
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
    /// The family-level conservation remainder: decoded records that neither
    /// became an entity nor were skipped by their emitter. Has to be zero.
    pub unattributed: u64,
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
    /// How many of them produced at least one page primitive.
    ///
    /// Counted per **body primitive**, not per page primitive, so the
    /// conservation equation reads `total = produced + Σ dropped +
    /// unattributed` with every term in the same unit. One body primitive may
    /// put several page primitives on the page — a `Boundary` fills one ring
    /// per circle member — and it still counts once here.
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

/// What one entity's layout pass handed back, either shape.
///
/// `push_entity_with` answers with an [`EntityLayout`]; `push_symbol_instance`
/// answers with a [`PlacementLayout`]. The recorder's `finish` takes either, so
/// the two loops keep one exit rather than two.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntityOutcome {
    /// A drawing- or furniture-domain record.
    Entity(EntityLayout),
    /// A symbol placement.
    Placement(PlacementLayout),
}

impl From<EntityLayout> for EntityOutcome {
    fn from(layout: EntityLayout) -> Self {
        Self::Entity(layout)
    }
}

impl From<PlacementLayout> for EntityOutcome {
    fn from(layout: PlacementLayout) -> Self {
        Self::Placement(layout)
    }
}

/// The layout pass's bookkeeper, a local of `lay_out_with_symbol_roots`.
///
/// It owns the [`LayoutLedger`] the page ends up carrying and does the one
/// thing the layout code cannot do for itself: claim page primitives by
/// counting them. `finish` is called at every entity's exit with the current
/// `primitives.len()`, so everything appended since the previous claim belongs
/// to the entity that just finished — the complex-dash stamps, the second and
/// third lines of a label, and a symbol body's strokes all land on the entity
/// that produced them without a single `push_*` knowing about the ledger.
///
/// # Not to be confused with the write side
///
/// This half sees entities and page primitives. [`WriteLedger`] (in
/// `crate::write`) sees the same primitives again at the writer and answers
/// whether each reached the paper. [`ExportLedger`](super::assemble::ExportLedger)
/// joins the two.
///
/// # Determinism and safety
///
/// Nothing here reads a clock, a hash map or a path's absolute form: sources
/// are recorded by file name only (requirement 12.4), every count is
/// `saturating_add`, and an index that will not narrow to `u32` is dropped
/// rather than cast — an entity that cannot be named leaves its primitives
/// unclaimed, which shows up as an unattributed remainder instead of as a
/// wrong attribution.
///
/// [`WriteLedger`]: crate::write::WriteLedger
#[derive(Debug, Clone, Default)]
pub struct LedgerRecorder {
    ledger: LayoutLedger,
    /// How many page primitives an entity has already claimed.
    claimed: usize,
    /// The surviving primitives' entity indices, rebuilt by
    /// [`Self::note_retained`].
    kept: Vec<u32>,
    /// Whether the final `retain` has started, so `kept` is authoritative.
    retaining: bool,
    /// The fill keys the furniture domain inserted, so a drawing-domain
    /// duplicate can name who took the key first (requirement 7.4).
    furniture_fills: BTreeSet<u64>,
}

impl LedgerRecorder {
    /// A recorder that has seen nothing yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record the symbol and archive sources the search had.
    ///
    /// Names only: the corpus descriptor a baseline stores is built from these
    /// and requirement 12.4 keeps absolute machine paths out of a baseline, so
    /// a directory contributes its own name and an archive its file name.
    pub fn sources(&mut self, roots: &[PathBuf], archives: &[PathBuf]) {
        for root in roots {
            let Some(name) = root.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            self.ledger.sources.symbols.push(SourceName {
                kind: SourceKind::Directory,
                name: name.to_string(),
                archive: None,
            });
        }
        for archive in archives {
            let Some(name) = archive.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            self.ledger.sources.symbols.push(SourceName {
                kind: SourceKind::Archive,
                name: name.to_string(),
                archive: None,
            });
        }
    }

    /// Record one domain's parse-layer counts (A.1) and its entity total.
    ///
    /// The three census tables give the three per-type-code reasons, the sheet
    /// streams give the ones with no geometry at all, the nested `JSite`
    /// storages give the body cache, the definition cache gives the unplaced
    /// definitions, and the emitter accounts give the last three. The family
    /// conservation equation is checked here rather than in the assembly
    /// because only this pass holds both halves of it: the decoded count (from
    /// the family registry) and the emitted count (from the entity list).
    pub fn parse(
        &mut self,
        domain: LedgerDomain,
        sheet_streams: &[pid_parse::SheetStream],
        jsites: &[pid_parse::JSite],
        geometry: &NormalizedPidGeometry,
    ) {
        *self.ledger.entity_totals.get_mut(domain) =
            u64::try_from(geometry.entities.len()).unwrap_or(u64::MAX);
        let counts = self.ledger.parse.get_mut(domain);

        for row in &geometry.dropped_graphic_records {
            counts.push_record(
                ParseRecord::new(
                    DropReason::RecordNoDecoder,
                    row.stream_path.clone(),
                    u64::try_from(row.count).unwrap_or(u64::MAX),
                )
                .typed(row.type_code),
            );
        }
        for row in &geometry.refused_graphic_records {
            counts.push_record(
                ParseRecord::new(
                    DropReason::RecordRefused,
                    row.stream_path.clone(),
                    u64::try_from(row.count).unwrap_or(u64::MAX),
                )
                .typed(row.type_code),
            );
        }
        for row in &geometry.skipped_graphic_records {
            counts.push_record(
                ParseRecord::new(
                    DropReason::RecordNativeSkip,
                    row.stream_path.clone(),
                    u64::try_from(row.count).unwrap_or(u64::MAX),
                )
                .typed(row.type_code),
            );
        }
        for stream in sheet_streams {
            if stream.geometry.is_some() {
                continue;
            }
            counts.push_record(
                ParseRecord::new(
                    DropReason::SheetStreamWithoutGeometry,
                    stream.path.clone(),
                    1,
                )
                .of_bytes(stream.size),
            );
        }
        for site in jsites {
            let Some(nested) = site.nested_geometry.as_ref() else {
                continue;
            };
            let records = u64::try_from(nested.len()).unwrap_or(u64::MAX);
            if records == 0 {
                continue;
            }
            counts.push_record(ParseRecord::new(
                DropReason::RecordSymbolBodyCache,
                site.path.clone(),
                records,
            ));
        }
        let mut referenced: BTreeSet<PidSymbolDefinitionRef> = BTreeSet::new();
        for entity in &geometry.entities {
            if let PidGraphicKind::SymbolInstance {
                definition: Some(reference),
                ..
            } = &entity.kind
            {
                referenced.insert(*reference);
            }
        }
        for definition in &geometry.symbol_definitions {
            if referenced.contains(&definition.reference) {
                continue;
            }
            let storage = jsites
                .iter()
                .find(|site| site_id(&site.path) == Some(definition.reference.site))
                .map_or_else(
                    || format!("/JSite{}", definition.reference.site),
                    |site| site.path.clone(),
                );
            counts.push_record(
                ParseRecord::new(DropReason::CacheDefinitionUnplaced, storage, 1)
                    .with_body_primitives(
                        u64::try_from(definition.primitives.len()).unwrap_or(u64::MAX),
                    ),
            );
        }
        for row in &geometry.emitter_accounts {
            let stream = row.stream_path.clone();
            let family = row.family.clone();
            if row.audit_only {
                if row.decoded > 0 {
                    counts.push_record(
                        ParseRecord::new(
                            DropReason::RecordAuditOnlyFamily,
                            stream,
                            u64::try_from(row.decoded).unwrap_or(u64::MAX),
                        )
                        .in_family(family),
                    );
                }
                continue;
            }
            if row.out_of_range > 0 {
                counts.push_record(
                    ParseRecord::new(
                        DropReason::RecordEmitterOutOfRange,
                        stream.clone(),
                        u64::try_from(row.out_of_range).unwrap_or(u64::MAX),
                    )
                    .in_family(family.clone()),
                );
            }
            if row.shape_rejected > 0 {
                counts.push_record(
                    ParseRecord::new(
                        DropReason::RecordEmitterShapeRejected,
                        stream,
                        u64::try_from(row.shape_rejected).unwrap_or(u64::MAX),
                    )
                    .in_family(family),
                );
            }
            let accounted = row
                .emitted
                .saturating_add(row.out_of_range)
                .saturating_add(row.shape_rejected);
            if row.decoded > accounted {
                let remainder = u64::try_from(row.decoded - accounted).unwrap_or(u64::MAX);
                counts.unattributed = counts.unattributed.saturating_add(remainder);
            }
        }
    }

    /// Record what the template search saw and what it could not read (A.8).
    pub fn template_search(&mut self, trace: &TemplateTrace) {
        self.ledger
            .sources
            .templates
            .extend(trace.candidates.iter().cloned());
        self.ledger
            .template_unreadable
            .extend(trace.unreadable.iter().cloned());
    }

    /// Open a record for `entity` and return its index.
    #[must_use]
    pub fn begin(&mut self, domain: LedgerDomain, entity: &PidGraphicEntity) -> usize {
        self.ledger.entities.push(EntityRecord {
            domain,
            kind: entity_kind(&entity.kind),
            confidence: confidence_of(entity.confidence),
            dropped: None,
            fill_first_taker: None,
            placement: None,
            observed: Vec::new(),
            site: Site {
                stream: entity.source.stream_path.clone(),
                oid: entity.graphic_oid,
                bytes: entity
                    .source
                    .byte_range
                    .as_ref()
                    .map(|range| [range.start, range.end]),
                record_kind: entity.source.record_kind.map(|kind| format!("{kind:?}")),
            },
        });
        self.ledger.entities.len().saturating_sub(1)
    }

    /// Close the record opened by [`Self::begin`], claiming every primitive
    /// appended since the previous claim.
    ///
    /// `primitives_len` is the page's `primitives.len()` at the entity's exit,
    /// including the exits that drew nothing: an entity that produced no
    /// primitive claims an empty range, which is how the assembly can tell
    /// "drew nothing, and here is why" from "drew something".
    pub fn finish(&mut self, index: usize, outcome: EntityOutcome, primitives_len: usize) {
        let start = self.claimed.min(primitives_len);
        let end = primitives_len.max(start);
        if let Ok(entity) = u32::try_from(index) {
            for _ in start..end {
                self.ledger.primitive_origin.push(entity);
            }
        }
        self.claimed = end;
        let Some(record) = self.ledger.entities.get_mut(index) else {
            return;
        };
        let domain = record.domain;
        match outcome {
            EntityOutcome::Entity(layout) => {
                record.dropped = layout.dropped;
                if let Some(key) = layout.fill_key {
                    match domain {
                        LedgerDomain::Furniture => {
                            if layout.dropped.is_none() {
                                self.furniture_fills.insert(key);
                            }
                        }
                        LedgerDomain::Drawing => {
                            if layout.dropped == Some(DropReason::FillDuplicate)
                                && self.furniture_fills.contains(&key)
                            {
                                record.fill_first_taker = Some(LedgerDomain::Furniture);
                            }
                        }
                    }
                }
                let text = self.ledger.text.get_mut(domain);
                text.add(layout.text);
            }
            EntityOutcome::Placement(layout) => {
                record.dropped = layout.dropped;
                record.placement = Some(PlacementAccount {
                    source: layout.source,
                    body: layout.body,
                });
            }
        }
    }

    /// Record one furniture-domain asymmetry (A.7).
    pub fn observe(&mut self, index: usize, asymmetry: Asymmetry) {
        if let Some(record) = self.ledger.entities.get_mut(index) {
            record.observed.push(asymmetry);
        }
    }

    /// Record that the final `retain` visited primitive `index`.
    ///
    /// `Vec::retain` visits every element exactly once, in order, so the
    /// survivors can be rebuilt as they are seen: the kept ones become the new
    /// `primitive_origin`, and the removed ones go back to the entity that
    /// produced them as `off_sheet` (A.4).
    pub fn note_retained(&mut self, index: usize, keep: bool) {
        if !self.retaining {
            self.retaining = true;
            self.kept.clear();
        }
        let Some(origin) = self.ledger.primitive_origin.get(index).copied() else {
            return;
        };
        if keep {
            self.kept.push(origin);
        } else {
            self.ledger.off_sheet.push(origin);
        }
    }

    /// The page's finished ledger.
    #[must_use]
    pub fn into_ledger(mut self) -> LayoutLedger {
        if self.retaining {
            self.ledger.primitive_origin = self.kept;
        }
        self.ledger
    }
}

impl ParseCounts {
    /// Add one count, merging with an identical `(reason, stream, type code,
    /// family)` row when the pass reaches the same site twice.
    fn push_record(&mut self, record: ParseRecord) {
        if record.count == 0 {
            return;
        }
        if let Some(existing) = self.items.iter_mut().find(|item| {
            item.reason == record.reason
                && item.stream == record.stream
                && item.type_code == record.type_code
                && item.family == record.family
        }) {
            existing.count = existing.count.saturating_add(record.count);
            existing.bytes = match (existing.bytes, record.bytes) {
                (Some(a), Some(b)) => Some(a.saturating_add(b)),
                (None, other) | (other, None) => other,
            };
            existing.body_primitives = match (existing.body_primitives, record.body_primitives) {
                (Some(a), Some(b)) => Some(a.saturating_add(b)),
                (None, other) | (other, None) => other,
            };
            return;
        }
        self.items.push(record);
    }
}

impl ParseRecord {
    /// One count at one site, before its type code, family and sizes are said.
    fn new(reason: DropReason, stream: String, count: u64) -> Self {
        Self {
            reason,
            stream,
            type_code: None,
            family: None,
            count,
            bytes: None,
            body_primitives: None,
        }
    }

    /// The PSM type code this count is per.
    fn typed(mut self, type_code: u16) -> Self {
        self.type_code = Some(type_code);
        self
    }

    /// The record family's registry name this count is per.
    fn in_family(mut self, family: String) -> Self {
        self.family = Some(family);
        self
    }

    /// The stream's byte size, for `sheet_stream_without_geometry`.
    fn of_bytes(mut self, bytes: u64) -> Self {
        self.bytes = Some(bytes);
        self
    }

    /// How many body primitives an unplaced cache definition carries.
    fn with_body_primitives(mut self, primitives: u64) -> Self {
        self.body_primitives = Some(primitives);
        self
    }
}

/// The storage id a `/JSite<id>` path names, if it is one.
fn site_id(path: &str) -> Option<u32> {
    path.rsplit('/')
        .next()
        .and_then(|name| name.strip_prefix("JSite"))
        .and_then(|id| id.parse().ok())
}

/// The ledger's name for one projection kind.
fn entity_kind(kind: &PidGraphicKind) -> EntityKind {
    match kind {
        PidGraphicKind::Line { .. } => EntityKind::Line,
        PidGraphicKind::Polyline { .. } => EntityKind::Polyline,
        PidGraphicKind::Arc { .. } => EntityKind::Arc,
        PidGraphicKind::Circle { .. } => EntityKind::Circle,
        PidGraphicKind::Point { .. } => EntityKind::Point,
        PidGraphicKind::Text { .. } => EntityKind::Text,
        PidGraphicKind::SymbolInstance { .. } => EntityKind::SymbolInstance,
        PidGraphicKind::Annotation { .. } => EntityKind::Annotation,
        PidGraphicKind::Unknown { .. } => EntityKind::Unknown,
    }
}

/// The ledger's name for one projection confidence.
fn confidence_of(confidence: PidGeometryConfidence) -> Confidence {
    match confidence {
        PidGeometryConfidence::Decoded => Confidence::Decoded,
        PidGeometryConfidence::Inferred => Confidence::Inferred,
        PidGeometryConfidence::ProbeOnly => Confidence::ProbeOnly,
    }
}

#[cfg(test)]
// A test's job is to fail loudly on a bad value, so the four production bans
// are lifted here and only here (design: "测试模块按需 #[allow]"). The fifth is
// a construction style: a fixture states the one or two fields it is about and
// leaves the rest default, which reads better than a 30-field initializer.
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::field_reassign_with_default
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
            unattributed: 0,
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

    // ------------------------------------------------------------ A.1 cases
    //
    // One constructed geometry per reason code. The point is not that the
    // recorder can be handed a value — it is that each code's *criterion* is
    // wired to the table it is documented against, so a census that stops
    // being read fails here rather than silently counting nothing.

    use pid_parse::{
        JSite, NormalizedPidGeometry, PidDroppedGraphicRecords, PidEmitterAccount,
        PidGraphicEntity, PidGraphicKind, PidGraphicProvenance, PidPoint, PidRefusedGraphicRecords,
        PidSkippedGraphicRecords, PidSymbolDefinition, PidSymbolDefinitionRef, SheetStream,
    };

    fn a_stream(path: &str, size: u64, with_geometry: bool) -> SheetStream {
        let mut stream: SheetStream =
            serde_json::from_str(r#"{"name":"x","path":"/x","size":0,"extracted_texts":[]}"#)
                .expect("a minimal stream reads");
        stream.path = path.to_string();
        stream.size = size;
        stream.geometry = with_geometry.then(Default::default);
        stream
    }

    fn a_site(path: &str, records: usize) -> JSite {
        let mut site = JSite {
            name: path.trim_start_matches('/').to_string(),
            path: path.to_string(),
            ..JSite::default()
        };
        if records > 0 {
            let mut nested = pid_parse::JSiteNestedGeometry::default();
            nested.circles = (0..records)
                .map(|index| {
                    serde_json::from_str(&format!(
                        r#"{{"byte_start":0,"byte_end":1,"oid":{index},"parent_ref":0,
                            "sheet_layer_ref":0,"index":0,"center_x":0.0,"center_y":0.0,
                            "radius":1.0}}"#
                    ))
                    .expect("a circle reads")
                })
                .collect();
            site.nested_geometry = Some(nested);
        }
        site
    }

    fn a_definition(site: u32, sheet: u32, primitives: usize) -> PidSymbolDefinition {
        serde_json::from_str(&format!(
            r#"{{"reference":{{"site":{site},"sheet":{sheet}}},"layers":[],
                "sheet_layers":[],"primitives":{},"primitive_layers":[]}}"#,
            serde_json::json!(
                (0..primitives)
                    .map(|_| serde_json::json!({"Line":{"start":[0.0,0.0],"end":[1.0,1.0]}}))
                    .collect::<Vec<_>>()
            )
        ))
        .expect("a definition reads")
    }

    fn a_symbol_entity(reference: Option<PidSymbolDefinitionRef>) -> PidGraphicEntity {
        PidGraphicEntity {
            id: "s".to_string(),
            drawing_id: None,
            graphic_oid: None,
            source_layer: None,
            kind: PidGraphicKind::SymbolInstance {
                insertion: PidPoint { x: 0.0, y: 0.0 },
                symbol_path: None,
                rotation: 0.0,
                scale: [1.0, 1.0],
                definition: reference,
            },
            coordinate_context: Default::default(),
            source: PidGraphicProvenance::default(),
            confidence: pid_parse::PidGeometryConfidence::Decoded,
        }
    }

    fn reasons(recorder: &LedgerRecorder, domain: LedgerDomain) -> Vec<DropReason> {
        recorder
            .ledger
            .parse
            .get(domain)
            .items
            .iter()
            .map(|item| item.reason)
            .collect()
    }

    #[test]
    fn a1_census_tables_are_read_into_their_reason_codes() {
        let mut geometry = NormalizedPidGeometry::default();
        geometry.dropped_graphic_records = vec![PidDroppedGraphicRecords {
            stream_path: "/Sheet6".to_string(),
            type_code: 0x0019,
            rad_class_name: None,
            count: 5,
        }];
        geometry.refused_graphic_records = vec![PidRefusedGraphicRecords {
            stream_path: "/Sheet6".to_string(),
            type_code: 0x00CE,
            rad_class_name: None,
            count: 1,
        }];
        geometry.skipped_graphic_records = vec![PidSkippedGraphicRecords {
            stream_path: "/Sheet6".to_string(),
            type_code: 0x0018,
            rad_class_name: None,
            count: 2,
        }];

        let mut recorder = LedgerRecorder::new();
        recorder.parse(LedgerDomain::Drawing, &[], &[], &geometry);
        let counts = recorder.ledger.parse.get(LedgerDomain::Drawing);
        assert_eq!(
            reasons(&recorder, LedgerDomain::Drawing),
            vec![
                DropReason::RecordNoDecoder,
                DropReason::RecordRefused,
                DropReason::RecordNativeSkip,
            ]
        );
        assert_eq!(counts.items.first().map(|item| item.count), Some(5));
        assert_eq!(counts.total(), 8);
        // The type code is kept, so the baseline says which eight records.
        assert_eq!(
            counts.items.first().and_then(|item| item.type_code),
            Some(0x0019)
        );
    }

    #[test]
    fn a1_a_stream_without_geometry_is_counted_with_its_size() {
        let streams = vec![
            a_stream("/Sheet1", 4096, false),
            a_stream("/Sheet6", 8192, true),
        ];
        let mut recorder = LedgerRecorder::new();
        recorder.parse(
            LedgerDomain::Drawing,
            &streams,
            &[],
            &NormalizedPidGeometry::default(),
        );
        let counts = recorder.ledger.parse.get(LedgerDomain::Drawing);
        assert_eq!(counts.items.len(), 1);
        assert_eq!(
            counts.items.first().map(|item| item.reason),
            Some(DropReason::SheetStreamWithoutGeometry)
        );
        assert_eq!(counts.items.first().and_then(|item| item.bytes), Some(4096));
    }

    #[test]
    fn a1_nested_storages_are_counted_as_body_cache_records() {
        let sites = vec![a_site("/JSite12", 3), a_site("/JSite13", 0)];
        let mut recorder = LedgerRecorder::new();
        recorder.parse(
            LedgerDomain::Drawing,
            &[],
            &sites,
            &NormalizedPidGeometry::default(),
        );
        let counts = recorder.ledger.parse.get(LedgerDomain::Drawing);
        assert_eq!(counts.items.len(), 1);
        assert_eq!(
            counts.items.first().map(|item| item.reason),
            Some(DropReason::RecordSymbolBodyCache)
        );
        assert_eq!(counts.items.first().map(|item| item.count), Some(3));
        assert_eq!(
            counts.items.first().map(|item| item.stream.as_str()),
            Some("/JSite12")
        );
    }

    #[test]
    fn a1_an_unreferenced_cache_definition_is_counted_with_its_body_size() {
        let mut geometry = NormalizedPidGeometry::default();
        geometry.symbol_definitions = vec![
            a_definition(12, 1, 4),
            // The second one a placement names, so it is not unplaced.
            a_definition(13, 2, 7),
        ];
        geometry.entities = vec![a_symbol_entity(Some(PidSymbolDefinitionRef {
            site: 13,
            sheet: 2,
        }))];
        let sites = vec![a_site("/JSite12", 0), a_site("/JSite13", 0)];

        let mut recorder = LedgerRecorder::new();
        recorder.parse(LedgerDomain::Drawing, &[], &sites, &geometry);
        let counts = recorder.ledger.parse.get(LedgerDomain::Drawing);
        assert_eq!(counts.items.len(), 1);
        let item = counts.items.first().expect("one unplaced definition");
        assert_eq!(item.reason, DropReason::CacheDefinitionUnplaced);
        assert_eq!(item.stream, "/JSite12");
        assert_eq!(item.body_primitives, Some(4));
        assert_eq!(recorder.ledger.entity_totals.drawing, 1);
    }

    #[test]
    fn a1_an_audit_only_family_is_counted_and_its_skips_are_not() {
        let mut geometry = NormalizedPidGeometry::default();
        geometry.emitter_accounts = vec![
            PidEmitterAccount {
                stream_path: "/Sheet6".to_string(),
                family: "igSmartFrame2d".to_string(),
                audit_only: true,
                decoded: 2,
                emitted: 0,
                out_of_range: 0,
                shape_rejected: 0,
            },
            PidEmitterAccount {
                stream_path: "/Sheet6".to_string(),
                family: "igLine2d".to_string(),
                audit_only: false,
                decoded: 4,
                emitted: 4,
                out_of_range: 0,
                shape_rejected: 0,
            },
        ];
        let mut recorder = LedgerRecorder::new();
        recorder.parse(LedgerDomain::Drawing, &[], &[], &geometry);
        let counts = recorder.ledger.parse.get(LedgerDomain::Drawing);
        assert_eq!(counts.items.len(), 1);
        assert_eq!(
            counts.items.first().map(|item| item.reason),
            Some(DropReason::RecordAuditOnlyFamily)
        );
        // The emitting family balanced, so it left no remainder.
        assert_eq!(counts.unattributed, 0);
    }

    #[test]
    fn a1_emitter_skips_are_counted_per_family_and_a_gap_is_a_remainder() {
        let mut geometry = NormalizedPidGeometry::default();
        geometry.emitter_accounts = vec![PidEmitterAccount {
            stream_path: "/Sheet6".to_string(),
            family: "igLineString2d".to_string(),
            audit_only: false,
            decoded: 10,
            emitted: 5,
            out_of_range: 2,
            shape_rejected: 1,
        }];
        let mut recorder = LedgerRecorder::new();
        recorder.parse(LedgerDomain::Drawing, &[], &[], &geometry);
        let counts = recorder.ledger.parse.get(LedgerDomain::Drawing);
        assert_eq!(
            reasons(&recorder, LedgerDomain::Drawing),
            vec![
                DropReason::RecordEmitterOutOfRange,
                DropReason::RecordEmitterShapeRejected,
            ]
        );
        // 10 decoded, 5 + 2 + 1 accounted: two records that nobody claims.
        assert_eq!(counts.unattributed, 2);
    }

    #[test]
    fn a1_the_family_equation_balances_when_every_record_is_claimed() {
        let mut geometry = NormalizedPidGeometry::default();
        geometry.emitter_accounts = vec![PidEmitterAccount {
            stream_path: "/Sheet6".to_string(),
            family: "igBoundary2d".to_string(),
            audit_only: false,
            decoded: 3,
            emitted: 0,
            out_of_range: 0,
            shape_rejected: 3,
        }];
        let mut recorder = LedgerRecorder::new();
        recorder.parse(LedgerDomain::Drawing, &[], &[], &geometry);
        assert_eq!(
            recorder
                .ledger
                .parse
                .get(LedgerDomain::Drawing)
                .unattributed,
            0
        );
    }

    #[test]
    fn a1_the_two_domains_keep_separate_accounts() {
        let mut drawing = NormalizedPidGeometry::default();
        drawing.dropped_graphic_records = vec![PidDroppedGraphicRecords {
            stream_path: "/Sheet6".to_string(),
            type_code: 0x0019,
            rad_class_name: None,
            count: 1,
        }];
        let mut furniture = NormalizedPidGeometry::default();
        furniture.refused_graphic_records = vec![PidRefusedGraphicRecords {
            stream_path: "/Sheet2".to_string(),
            type_code: 0x00CE,
            rad_class_name: None,
            count: 4,
        }];
        let mut recorder = LedgerRecorder::new();
        recorder.parse(LedgerDomain::Drawing, &[], &[], &drawing);
        recorder.parse(LedgerDomain::Furniture, &[], &[], &furniture);
        assert_eq!(
            reasons(&recorder, LedgerDomain::Drawing),
            vec![DropReason::RecordNoDecoder]
        );
        assert_eq!(
            reasons(&recorder, LedgerDomain::Furniture),
            vec![DropReason::RecordRefused]
        );
        assert_eq!(
            recorder.ledger.parse.get(LedgerDomain::Furniture).total(),
            4
        );
    }

    /// // Feature: pdf-export-ledger, Property 8: 解析层计数项等于几何表
    ///
    /// Every parse-layer count is read straight out of a geometry table, so the
    /// sum over the items has to equal the sum over the table it came from —
    /// for arbitrary row counts, in both domains, with the type codes kept.
    #[test]
    fn parse_counts_equal_the_geometry_table() {
        use proptest::prelude::*;

        let mut runner = proptest::test_runner::TestRunner::default();
        let strategy = (
            prop::collection::vec((0u16..0x4000, 0u64..50), 0..6),
            prop::collection::vec((0u16..0x4000, 0u64..50), 0..6),
            prop::collection::vec((0u16..0x4000, 0u64..50), 0..6),
        );
        runner
            .run(&strategy, |(dropped, refused, skipped)| {
                let mut geometry = NormalizedPidGeometry::default();
                geometry.dropped_graphic_records = dropped
                    .iter()
                    .enumerate()
                    .map(|(index, (code, count))| PidDroppedGraphicRecords {
                        stream_path: format!("/Sheet{index}"),
                        type_code: *code,
                        rad_class_name: None,
                        count: usize::try_from(*count).unwrap_or(0),
                    })
                    .collect();
                geometry.refused_graphic_records = refused
                    .iter()
                    .enumerate()
                    .map(|(index, (code, count))| PidRefusedGraphicRecords {
                        stream_path: format!("/Refused{index}"),
                        type_code: *code,
                        rad_class_name: None,
                        count: usize::try_from(*count).unwrap_or(0),
                    })
                    .collect();
                geometry.skipped_graphic_records = skipped
                    .iter()
                    .enumerate()
                    .map(|(index, (code, count))| PidSkippedGraphicRecords {
                        stream_path: format!("/Skipped{index}"),
                        type_code: *code,
                        rad_class_name: None,
                        count: usize::try_from(*count).unwrap_or(0),
                    })
                    .collect();
                let expected: u64 = dropped
                    .iter()
                    .chain(&refused)
                    .chain(&skipped)
                    .map(|(_, count)| *count)
                    .sum();

                let mut recorder = LedgerRecorder::new();
                recorder.parse(LedgerDomain::Drawing, &[], &[], &geometry);
                prop_assert_eq!(
                    recorder.ledger.parse.get(LedgerDomain::Drawing).total(),
                    expected
                );
                Ok(())
            })
            .expect("the property holds over the generated tables");
    }
}
