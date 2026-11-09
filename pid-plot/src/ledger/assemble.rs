//! The assembled ledger: the per-page account a baseline stores and the gate
//! compares.
//!
//! This module owns the serializable half of the account. Everything here is
//! counts, codes and names — no floats, no text content, no machine paths
//! (requirement 12.4) — so a baseline file can be committed and read as the
//! table it is.
//!
//! # Five layers, five remainders
//!
//! Each domain's account is a parse layer, an entity layer, a body layer, a
//! primitive layer and a segment layer, and every layer carries an
//! `unattributed` count. Zero is the only acceptable value: a non-zero
//! remainder is a batch of things that neither reached the page nor earned a
//! reason code, which is precisely the gap this ledger exists to make visible.
//! The remainder is stored even when it is zero, so a reader can tell "we
//! checked and it balanced" from "we did not check".
//!
//! # Determinism
//!
//! Every map here is a [`BTreeMap`] and every list is sorted by its own key, so
//! two runs over the same page serialize to the same bytes. That is what makes
//! a byte-for-byte baseline comparison meaningful (requirement 10.1).

use std::collections::{BTreeMap, BTreeSet};

use pid_parse::NormalizedPidGeometry;
use serde::{Deserialize, Serialize};

use super::layout::{
    Confidence, EntityKind, EntityRecord, LayoutLedger, ParseCounts, Site, SourceName,
    TextNormalization,
};
use super::reason::{Asymmetry, DropReason, LedgerDomain, PartialMark, PlacementSource, Tier};
use crate::model::{BorderTemplate, PaperModel};
use crate::write::{PlotAudit, PrimitiveOutcome, WriteLedger};

/// How far past the page edge ink must reach before it counts as
/// `past_the_edge`, millimetres.
///
/// Requirement 5.4 / 8.7 state 0.5 mm, and the partial mark is the ledger's own
/// reading — **not** `export_plant_backup`'s `EDGE_SLACK_MM` of 0.1, which is a
/// gate's tolerance for a different question. Two constants, two purposes; the
/// `past_the_edge_uses_half_mm` test holds them apart.
const PAST_THE_EDGE_SLACK_MM: f64 = 0.5;

/// One page's whole account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportLedger {
    /// Page-level facts: size source, template, page-level drops.
    pub page: PageAccount,
    /// Whether the run measured the two font-dependent partial marks.
    ///
    /// A ledger taken without faces (`write_ledger`) cannot measure condensing
    /// or clipping, so it carries neither mark (design deviation I1) and says
    /// so here rather than reporting a zero that would look measured.
    pub fonts_measured: bool,
    /// The `.pid`'s own records and entities.
    pub drawing: DomainAccount,
    /// The border template's records and entities.
    pub furniture: DomainAccount,
}

/// Page-level facts (requirement 7.5, appendix A.8).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PageAccount {
    /// Where the page size came from.
    pub size_source: PageSizeSource,
    /// The template the page wears, when it wears one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<TemplateFact>,
    /// Template stems the page names that no file answered.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing_templates: Vec<String>,
    /// Page-level drops by reason: [`DropReason::TemplateCandidateUnreadable`]
    /// and [`DropReason::TemplateMissing`].
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dropped: BTreeMap<DropReason, u64>,
    /// Where the page-level suspect drops came from.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suspect_sites: Vec<SuspectSite>,
}

/// Where the page's size came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageSizeSource {
    /// The drawing states its own page (`PaperModel::page_mm`).
    #[default]
    Declared,
    /// The page was measured (`PaperModel::resolved_page_mm` fell back to a
    /// standard sheet).
    Measured,
    /// Neither: the model has no page at all.
    None,
}

impl PageSizeSource {
    /// The source's code, which is also its serde name.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::Declared => "declared",
            Self::Measured => "measured",
            Self::None => "none",
        }
    }
}

/// The template a page wears (requirement 7.5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateFact {
    /// The template's file name.
    pub name: String,
    /// Where the file was read from.
    pub source: TemplateSourceKind,
    /// The archive's file name, when the template came out of one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archive: Option<String>,
}

/// Where a page's template file was read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateSourceKind {
    /// A `.igr` on disk.
    DirectoryFile,
    /// A `.igr` inside a `RefData~*.zip`.
    ArchiveMember,
}

impl TemplateSourceKind {
    /// The kind's code, which is also its serde name.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::DirectoryFile => "directory_file",
            Self::ArchiveMember => "archive_member",
        }
    }
}

/// One domain's account: five layers, five remainders.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DomainAccount {
    /// Records that never became entities (A.1).
    pub parse: ParseAccount,
    /// Entities and their outcomes (A.2, requirement 1).
    pub entities: EntityAccount,
    /// Symbol bodies (A.3, requirement 4).
    pub bodies: BodyAccount,
    /// What the text normalizer removed (A.4).
    pub text: TextNormalization,
    /// Page primitives and their outcomes (A.5, requirement 5).
    pub primitives: PrimitiveAccount,
    /// Strokes and their outcomes (A.5's segment reasons, requirement 6.3).
    pub segments: SegmentAccount,
    /// The furniture-domain asymmetries (A.7).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub asymmetry: BTreeMap<Asymmetry, u64>,
    /// Where this domain's suspect drops came from.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suspect_sites: Vec<SuspectSite>,
}

/// The parse layer: records that never became entities (A.1).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ParseAccount {
    /// One entry per `(reason, stream, type code, family)`, sorted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<RecordItem>,
    /// The family-level conservation remainder: decoded records that neither
    /// became entities nor were skipped by their emitter. Has to be zero.
    pub unattributed: u64,
}

impl ParseAccount {
    /// How many records this domain dropped at parse time.
    #[must_use]
    pub fn dropped_total(&self) -> u64 {
        self.items
            .iter()
            .fold(0u64, |sum, item| sum.saturating_add(item.count))
    }
}

/// One parse-layer count.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordItem {
    /// Which drop this is.
    pub reason: DropReason,
    /// The Sheet stream's path, or the storage path for a body record.
    pub stream: String,
    /// The PSM type code, written and read as `0xNNNN`.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "type_code_hex"
    )]
    pub type_code: Option<u16>,
    /// The record family's registry name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    /// How many records.
    pub count: u64,
    /// The stream's byte size, for
    /// [`DropReason::SheetStreamWithoutGeometry`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
    /// How many body primitives an unplaced cache definition carries, for
    /// [`DropReason::CacheDefinitionUnplaced`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_primitives: Option<u64>,
}

/// `type_code` as a `0xNNNN` string.
///
/// A bare `24` in a baseline does not say which base it is in, and the whole
/// reason catalog is written in hex (`0x0018 igLine2d`). Storing the spelling
/// the reader recognizes keeps a baseline readable next to the table it was
/// checked against.
mod type_code_hex {
    use serde::{Deserialize, Deserializer, Serializer};

    pub(super) fn serialize<S: Serializer>(
        value: &Option<u16>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(code) => serializer.serialize_some(&format!("0x{code:04X}")),
            None => serializer.serialize_none(),
        }
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<u16>, D::Error> {
        let raw = Option::<String>::deserialize(deserializer)?;
        let Some(raw) = raw else {
            return Ok(None);
        };
        let digits = raw
            .strip_prefix("0x")
            .or_else(|| raw.strip_prefix("0X"))
            .ok_or_else(|| serde::de::Error::custom("a type code is written 0xNNNN"))?;
        u16::from_str_radix(digits, 16)
            .map(Some)
            .map_err(serde::de::Error::custom)
    }
}

/// The entity layer (requirement 1).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct EntityAccount {
    /// Entities the projection produced.
    pub total: u64,
    /// Entities at least one of whose primitives reached the page.
    pub on_page: u64,
    /// Of those, the ones that reached it incompletely (design deviation I2).
    pub partial: u64,
    /// The rest, by reason.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dropped: BTreeMap<DropReason, u64>,
    /// Drawing-domain `fill_duplicate` entities whose fill key the furniture
    /// domain took first (requirement 7.4).
    pub fill_duplicate_first_taken_by_furniture: u64,
    /// Requirement 1.6's cross-count: kind → confidence → outcome.
    ///
    /// The innermost key is `on_page`, `unattributed`, or a reason code.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub cross: BTreeMap<EntityKind, BTreeMap<Confidence, BTreeMap<String, u64>>>,
    /// Entities the layout said it produced a primitive for, that no primitive
    /// claims. Has to be zero.
    pub unattributed: u64,
}

/// The body layer (requirement 4).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BodyAccount {
    /// Placements by where their body came from.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub placements: BTreeMap<PlacementSource, u64>,
    /// Body primitives in the bodies the placements resolved.
    pub total: u64,
    /// Those that produced a page primitive.
    pub produced: u64,
    /// The rest, by A.3 reason.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dropped: BTreeMap<DropReason, u64>,
    /// The remainder no reason accounts for — an empty-member `Boundary`, per
    /// design deviation I3. Has to be zero.
    pub unattributed: u64,
}

/// The page-primitive layer (requirement 5, A.4's `off_sheet` and A.5).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PrimitiveAccount {
    /// Page primitives the layout produced, before the final `retain`.
    pub laid_out: u64,
    /// Those the final `retain` removed as off the sheet.
    pub off_sheet: u64,
    /// Those the writer drew.
    pub on_page: u64,
    /// The rest, by reason.
    ///
    /// A.5's write-layer codes live here, and so does A.4's `off_sheet`: a
    /// primitive the final `retain` removed is a primitive-layer drop as much
    /// as one the writer refused, so it is filed under
    /// [`DropReason::OffSheet`] as well as counted in [`Self::off_sheet`].
    /// The two fields answer different questions — one is the reason table,
    /// the other the remainder equation's input — and the primitive equation
    /// reads this map once, never both:
    /// `laid_out = on_page + Σ dropped + unattributed`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dropped: BTreeMap<DropReason, u64>,
    /// Primitives that reached the page but not whole (A.6).
    pub partial: u64,
    /// The partial marks, by kind. A ledger taken without faces carries
    /// neither `text_condensed` nor `text_clipped_at_floor`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub marks: BTreeMap<PartialMark, u64>,
    /// The remainder. Has to be zero.
    pub unattributed: u64,
}

/// The segment layer (requirement 6.3).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SegmentAccount {
    /// Strokes the primitives' point chains describe.
    ///
    /// Written for a reader, not compared: how many segments an arc flattens
    /// to depends on `kurbo`'s floating-point arithmetic, which can differ in
    /// the last place between platforms (design deviation I4).
    pub total: u64,
    /// Strokes written. Also written for a reader rather than compared.
    pub written: u64,
    /// The rest, by A.5's segment reasons.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dropped: BTreeMap<DropReason, u64>,
    /// The remainder. Has to be zero.
    pub unattributed: u64,
}

/// Where one suspect drop came from, so a gate can name it when the count
/// grows (requirement 13.4).
///
/// Only identifiers: a stream path, an oid, a byte range, a type code, a family
/// name, a template name. Never text the page carries (requirement 12.4).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SuspectSite {
    /// Which drop this is.
    pub reason: DropReason,
    /// The stream it was read from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<String>,
    /// The graphic oid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oid: Option<u32>,
    /// The record's byte range.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<[usize; 2]>,
    /// The PSM type code, written and read as `0xNNNN`.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "type_code_hex"
    )]
    pub type_code: Option<u16>,
    /// The record family's registry name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    /// The template's file name or stem.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// How many drops this site accounts for.
    pub count: u64,
}

/// The corpus a baseline was taken from: sources by name, never by path.
///
/// Requirement 12.3 stores this in the baseline's header, and requirement 13.7
/// compares it: a different symbol or template part means the counts are not
/// comparable. The font part is separate (design deviation D1) because only
/// two partial marks depend on a face.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CorpusDescriptor {
    /// The symbol sources, in search order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub symbol_sources: Vec<SourceName>,
    /// Every template candidate the pages saw, sorted and deduplicated.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub template_sources: Vec<SourceName>,
    /// The typeface keys the run embedded, sorted and deduplicated. Empty for
    /// a run that measured no fonts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub font_faces: Vec<String>,
}

impl CorpusDescriptor {
    /// Whether the symbol and template parts agree — the part that decides
    /// whether counts are comparable at all.
    #[must_use]
    pub fn same_corpus(&self, other: &Self) -> bool {
        self.symbol_sources == other.symbol_sources
            && self.template_sources == other.template_sources
    }
}

/// The two-domain total a CLI prints and a reader reads first (requirement
/// 14.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LedgerSummary {
    /// Entities both domains visited.
    pub entities: u64,
    /// Of those, the ones that reached the page.
    pub on_page: u64,
    /// Intentional drops over every layer of both domains.
    pub intentional: u64,
    /// Suspect drops over every layer of both domains.
    pub suspect: u64,
    /// Remainders over every layer of both domains. Has to be zero.
    pub unattributed: u64,
}

impl ExportLedger {
    /// Join the layout half and the write half into the page's account.
    ///
    /// The five layers and their remainders are computed here, not while the
    /// page is being laid out or written, because only this pass holds both
    /// halves: which entity claimed which page primitive (the layout ledger's
    /// `primitive_origin`) and what the writer did with each of those
    /// primitives (`WriteLedger::fates`). Requirement 6.1–6.4's equations are
    /// the shape of the code below: each layer's input count is taken
    /// independently of the outcome records, so a drop nobody registered shows
    /// up as a remainder instead of being absorbed by the arithmetic.
    ///
    /// # Entity outcomes (requirement 1)
    ///
    /// An entity's outcome is decided in the order the requirement states:
    ///
    /// 1. the layout layer already dropped it → that reason;
    /// 2. at least one of its primitives reached the page → `on_page`, and a
    ///    partial-on-page count when the entity reached it incompletely —
    ///    either one of its drawn primitives carries an A.6 mark, or one of its
    ///    primitives did not reach the page at all (design deviation I2);
    /// 3. it has primitives but none reached the page → the appendix's first
    ///    reason among theirs, `off_sheet` included;
    /// 4. it has no primitive at all → a remainder, because the layout layer
    ///    said it produced one.
    ///
    /// # `past_the_edge`
    ///
    /// Requirement 5.4 and 8.7 measure it at [`PAST_THE_EDGE_SLACK_MM`] (0.5
    /// mm), by pointer, through
    /// [`PaperModel::primitives_past_the_edge`] — the same function the gate
    /// reads, so the mark and the cross-check can never disagree.
    #[must_use]
    pub fn assemble(model: &PaperModel, write: &WriteLedger) -> Self {
        let ledger = &model.ledger;
        // Requirement 5.4 / 8.7: the page model answers which primitives reach
        // past the edge, and the answer is turned back into indices by pointer
        // — the only identity a `&PaperPrimitive` carries. One call, so the
        // mark and the cross-check read the same list.
        let past: Vec<*const _> = model
            .primitives_past_the_edge(PAST_THE_EDGE_SLACK_MM)
            .into_iter()
            .map(std::ptr::from_ref)
            .collect();
        let past_the_edge: BTreeSet<usize> = model
            .primitives
            .iter()
            .enumerate()
            .filter(|(_, primitive)| {
                let address = std::ptr::from_ref(*primitive);
                past.contains(&address)
            })
            .map(|(index, _)| index)
            .collect();

        let mut accounts = DomainPairBuilder::default();
        for domain in LedgerDomain::all() {
            accounts.set(
                *domain,
                assemble_domain(*domain, ledger, model, write, &past_the_edge),
            );
        }

        let mut page = PageAccount {
            size_source: page_size_source(model),
            template: model.template.as_ref().map(template_fact),
            missing_templates: model.missing_templates.clone(),
            dropped: BTreeMap::new(),
            suspect_sites: Vec::new(),
        };
        // A page that named a template and got none is a page-level suspect
        // drop (A.8), and the name that went unanswered is its site.
        if !page.missing_templates.is_empty() {
            let count = u64::try_from(page.missing_templates.len()).unwrap_or(u64::MAX);
            page.dropped.insert(DropReason::TemplateMissing, count);
            for name in &page.missing_templates {
                page.suspect_sites.push(SuspectSite {
                    reason: DropReason::TemplateMissing,
                    stream: None,
                    oid: None,
                    bytes: None,
                    type_code: None,
                    family: None,
                    name: Some(name.clone()),
                    count: 1,
                });
            }
        }
        // A template candidate that could not be read or parsed is the other
        // page-level suspect drop; its site is the candidate's own name.
        let unreadable = u64::try_from(ledger.template_unreadable.len()).unwrap_or(u64::MAX);
        if unreadable > 0 {
            page.dropped
                .insert(DropReason::TemplateCandidateUnreadable, unreadable);
            for name in &ledger.template_unreadable {
                page.suspect_sites.push(SuspectSite {
                    reason: DropReason::TemplateCandidateUnreadable,
                    stream: None,
                    oid: None,
                    bytes: None,
                    type_code: None,
                    family: None,
                    name: Some(name.name.clone()),
                    count: 1,
                });
            }
        }
        page.suspect_sites.sort();

        Self {
            page,
            fonts_measured: write.fonts_measured,
            drawing: accounts.drawing,
            furniture: accounts.furniture,
        }
    }

    /// Every non-zero remainder, one line each (requirement 6.5).
    ///
    /// A gate fails on any line here. Empty means every layer of every domain
    /// balanced: nothing was dropped without a name.
    #[must_use]
    pub fn conservation_breaches(&self) -> Vec<String> {
        let mut breaches = Vec::new();
        for (name, domain) in [
            (LedgerDomain::Drawing.code(), &self.drawing),
            (LedgerDomain::Furniture.code(), &self.furniture),
        ] {
            for (layer, remainder) in [
                ("parse", domain.parse.unattributed),
                ("entity", domain.entities.unattributed),
                ("body", domain.bodies.unattributed),
                ("primitive", domain.primitives.unattributed),
                ("segment", domain.segments.unattributed),
            ] {
                if remainder > 0 {
                    breaches.push(format!(
                        "{name}: {layer} layer leaves {remainder} unattributed"
                    ));
                }
            }
        }
        breaches
    }

    /// Requirement 8's seven cross-checks against counts the code already
    /// keeps, one line per mismatch.
    ///
    /// Empty means the ledger agrees with the page model, and — when a caller
    /// passes them — with the geometry tables and the writer's own audit. A
    /// mismatch is a ledger bug or a code change the ledger has not followed;
    /// either way it has to be seen, so every check reports rather than
    /// short-circuiting.
    #[must_use]
    pub fn cross_check(
        &self,
        model: &PaperModel,
        audit: Option<&PlotAudit>,
        geometry: Option<&NormalizedPidGeometry>,
    ) -> Vec<String> {
        let mut mismatches = Vec::new();
        // 8.1
        let endpoint_pairs = entity_dropped(&self.drawing, DropReason::EndpointPairGuess);
        let expected = u64::try_from(model.skipped_endpoint_pairs).unwrap_or(u64::MAX);
        if endpoint_pairs != expected {
            mismatches.push(format!(
                "8.1 drawing endpoint_pair_guess {endpoint_pairs} != skipped_endpoint_pairs {expected}"
            ));
        }
        // 8.2
        let off_sheet = self
            .drawing
            .primitives
            .off_sheet
            .saturating_add(self.furniture.primitives.off_sheet);
        let expected = u64::try_from(model.off_sheet_primitives).unwrap_or(u64::MAX);
        if off_sheet != expected {
            mismatches.push(format!(
                "8.2 off_sheet {off_sheet} != off_sheet_primitives {expected}"
            ));
        }
        // 8.3
        let missing = entity_dropped(&self.drawing, DropReason::SymbolMissing);
        let expected = u64::try_from(model.missing_boxes.len()).unwrap_or(u64::MAX);
        if missing != expected {
            mismatches.push(format!(
                "8.3 drawing symbol_missing {missing} != missing_boxes {expected}"
            ));
        }
        // 8.4
        let expected = model.template.as_ref().map_or(0, |template| {
            u64::try_from(template.primitives).unwrap_or(u64::MAX)
        });
        if self.furniture.primitives.laid_out != expected {
            mismatches.push(format!(
                "8.4 furniture laid_out {} != template primitives {expected}",
                self.furniture.primitives.laid_out
            ));
        }
        // 8.5
        if let Some(geometry) = geometry {
            mismatches.extend(check_parse_tables(&self.drawing.parse, geometry));
        }
        // 8.6
        if let Some(audit) = audit {
            let condensed = self
                .drawing
                .primitives
                .marks
                .get(&PartialMark::TextCondensed)
                .copied()
                .unwrap_or(0)
                .saturating_add(
                    self.furniture
                        .primitives
                        .marks
                        .get(&PartialMark::TextCondensed)
                        .copied()
                        .unwrap_or(0),
                );
            let expected = u64::try_from(audit.runs_condensed).unwrap_or(u64::MAX);
            if condensed != expected {
                mismatches.push(format!(
                    "8.6 text_condensed {condensed} != runs_condensed {expected}"
                ));
            }
            let clipped = self
                .drawing
                .primitives
                .marks
                .get(&PartialMark::TextClippedAtFloor)
                .copied()
                .unwrap_or(0)
                .saturating_add(
                    self.furniture
                        .primitives
                        .marks
                        .get(&PartialMark::TextClippedAtFloor)
                        .copied()
                        .unwrap_or(0),
                );
            let expected =
                u64::try_from(audit.runs_clipped_at_the_condensation_floor).unwrap_or(u64::MAX);
            if clipped != expected {
                mismatches.push(format!(
                    "8.6 text_clipped_at_floor {clipped} != runs_clipped_at_the_condensation_floor {expected}"
                ));
            }
        }
        // 8.7
        let past = self
            .drawing
            .primitives
            .marks
            .get(&PartialMark::PastTheEdge)
            .copied()
            .unwrap_or(0)
            .saturating_add(
                self.furniture
                    .primitives
                    .marks
                    .get(&PartialMark::PastTheEdge)
                    .copied()
                    .unwrap_or(0),
            );
        let expected = u64::try_from(model.primitives_past_the_edge(PAST_THE_EDGE_SLACK_MM).len())
            .unwrap_or(u64::MAX);
        if past != expected {
            mismatches.push(format!(
                "8.7 past_the_edge {past} != primitives_past_the_edge(0.5) {expected}"
            ));
        }
        mismatches
    }

    /// The two-domain totals the CLI prints (requirement 14.3).
    ///
    /// The page-level drops are counted too: a template that no file answered
    /// is a suspect drop of the page's own account, and a summary that left it
    /// out would say a page was clean when its furniture never arrived.
    #[must_use]
    pub fn summary(&self) -> LedgerSummary {
        let mut summary = LedgerSummary::default();
        for (reason, count) in &self.page.dropped {
            match reason.tier() {
                Tier::Suspect => summary.suspect = summary.suspect.saturating_add(*count),
                Tier::Intentional => {
                    summary.intentional = summary.intentional.saturating_add(*count);
                }
            }
        }
        for domain in [&self.drawing, &self.furniture] {
            summary.entities = summary.entities.saturating_add(domain.entities.total);
            summary.on_page = summary.on_page.saturating_add(domain.entities.on_page);
            summary.unattributed = summary
                .unattributed
                .saturating_add(domain.parse.unattributed)
                .saturating_add(domain.entities.unattributed)
                .saturating_add(domain.bodies.unattributed)
                .saturating_add(domain.primitives.unattributed)
                .saturating_add(domain.segments.unattributed);
            for (reason, count) in domain_counts(domain) {
                match reason.tier() {
                    Tier::Suspect => summary.suspect = summary.suspect.saturating_add(count),
                    Tier::Intentional => {
                        summary.intentional = summary.intentional.saturating_add(count);
                    }
                }
            }
        }
        summary
    }
}

/// The summary line's own rendering, so the CLI and a test print one thing.
impl std::fmt::Display for LedgerSummary {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "ledger entities {} on_page {} intentional {} suspect {} unattributed {}",
            self.entities, self.on_page, self.intentional, self.suspect, self.unattributed
        )
    }
}

/// One domain's whole account.
fn assemble_domain(
    domain: LedgerDomain,
    ledger: &LayoutLedger,
    model: &PaperModel,
    write: &WriteLedger,
    past_the_edge: &BTreeSet<usize>,
) -> DomainAccount {
    let mut account = DomainAccount {
        parse: parse_account(ledger.parse.get(domain)),
        ..DomainAccount::default()
    };
    // The parse layer's suspect rows are sites of their own.
    for item in &account.parse.items {
        if item.reason.tier() == Tier::Suspect {
            account.suspect_sites.push(SuspectSite {
                reason: item.reason,
                stream: Some(item.stream.clone()),
                oid: None,
                bytes: None,
                type_code: item.type_code,
                family: item.family.clone(),
                name: None,
                count: item.count,
            });
        }
    }

    // Which page primitive each entity claimed, in model order.
    let mut claimed: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (index, origin) in ledger.primitive_origin.iter().enumerate() {
        if let Ok(origin) = usize::try_from(*origin) {
            claimed.entry(origin).or_default().push(index);
        }
    }
    // The `retain` removals, per entity, are primitives that reached no page.
    // Only this domain's entities count here: the list is page-wide, and a
    // furniture removal must not show up in the drawing domain's account.
    let mut removed: BTreeMap<usize, u64> = BTreeMap::new();
    for origin in &ledger.off_sheet {
        if let Ok(origin) = usize::try_from(*origin) {
            let belongs = ledger
                .entities
                .get(origin)
                .is_some_and(|record| record.domain == domain);
            if !belongs {
                continue;
            }
            let entry = removed.entry(origin).or_insert(0);
            *entry = entry.saturating_add(1);
        }
    }

    let mut cross: BTreeMap<EntityKind, BTreeMap<Confidence, BTreeMap<String, u64>>> =
        BTreeMap::new();
    for (index, record) in ledger.entities.iter().enumerate() {
        if record.domain != domain {
            continue;
        }
        let outcome = entity_outcome(
            index,
            record,
            write,
            &claimed,
            &removed,
            past_the_edge,
            &mut account,
        );
        let entry = cross
            .entry(record.kind.clone())
            .or_default()
            .entry(record.confidence)
            .or_default();
        let key = outcome.code();
        let count = entry.entry(key).or_insert(0);
        *count = count.saturating_add(1);
        if let Some(reason) = outcome.reason()
            && reason.tier() == Tier::Suspect
        {
            account.suspect_sites.push(suspect_site(reason, record));
        }
        if let Some(placement) = &record.placement {
            account.bodies.total = account.bodies.total.saturating_add(placement.body.total);
            account.bodies.produced = account
                .bodies
                .produced
                .saturating_add(placement.body.produced);
            account.bodies.unattributed = account
                .bodies
                .unattributed
                .saturating_add(placement.body.unattributed);
            for (reason, count) in &placement.body.dropped {
                let entry = account.bodies.dropped.entry(*reason).or_insert(0);
                *entry = entry.saturating_add(*count);
            }
            let entry = account
                .bodies
                .placements
                .entry(placement.source)
                .or_insert(0);
            *entry = entry.saturating_add(1);
        }
        for asymmetry in &record.observed {
            let entry = account.asymmetry.entry(*asymmetry).or_insert(0);
            *entry = entry.saturating_add(1);
        }
    }
    account.entities.cross = cross;

    // The primitive layer: every primitive of this domain, kept or removed.
    let mut laid_out = 0u64;
    for (index, primitive) in model.primitives.iter().enumerate() {
        if primitive.furniture != (domain == LedgerDomain::Furniture) {
            continue;
        }
        laid_out = laid_out.saturating_add(1);
        let Some(fate) = write.fate(index) else {
            continue;
        };
        match fate.outcome {
            PrimitiveOutcome::OnPage => {
                account.primitives.on_page = account.primitives.on_page.saturating_add(1);
                let mut partial = false;
                if fate.segments_dropped {
                    mark(&mut account.primitives, PartialMark::SegmentsDropped);
                    partial = true;
                }
                if past_the_edge.contains(&index) {
                    mark(&mut account.primitives, PartialMark::PastTheEdge);
                    partial = true;
                }
                if let Some(text) = fate.text {
                    if text.condensed {
                        mark(&mut account.primitives, PartialMark::TextCondensed);
                        partial = true;
                    }
                    if text.clipped_at_floor {
                        mark(&mut account.primitives, PartialMark::TextClippedAtFloor);
                        partial = true;
                    }
                }
                if partial {
                    account.primitives.partial = account.primitives.partial.saturating_add(1);
                }
            }
            PrimitiveOutcome::Dropped(reason) => {
                let entry = account.primitives.dropped.entry(reason).or_insert(0);
                *entry = entry.saturating_add(1);
                if reason.tier() == Tier::Suspect {
                    account.suspect_sites.push(SuspectSite {
                        reason,
                        stream: primitive.stream_path.clone(),
                        oid: None,
                        bytes: primitive.byte_range,
                        type_code: None,
                        family: None,
                        name: primitive.symbol.clone(),
                        count: 1,
                    });
                }
            }
        }
        // The segment layer, one stroke at a time.
        account.segments.total = account.segments.total.saturating_add(fate.segments.total);
        account.segments.written = account
            .segments
            .written
            .saturating_add(fate.segments.written);
        for (reason, count) in [
            (DropReason::SegmentNonFinite, fate.segments.non_finite),
            (DropReason::SegmentTooShort, fate.segments.too_short),
            (DropReason::SegmentOnSheetEdge, fate.segments.on_sheet_edge),
        ] {
            if count > 0 {
                let entry = account.segments.dropped.entry(reason).or_insert(0);
                *entry = entry.saturating_add(count);
            }
        }
    }
    for count in removed.values() {
        laid_out = laid_out.saturating_add(*count);
    }
    account.primitives.laid_out = laid_out;
    account.primitives.off_sheet = removed.values().copied().fold(0u64, u64::saturating_add);
    if account.primitives.off_sheet > 0 {
        // `off_sheet` is a primitive-layer drop as well as a remainder input:
        // A.4 files it under the same layer as A.5's codes.
        let entry = account
            .primitives
            .dropped
            .entry(DropReason::OffSheet)
            .or_insert(0);
        *entry = entry.saturating_add(account.primitives.off_sheet);
    }

    // Entity conservation (6.1): the input is the geometry's own count.
    account.entities.total = *ledger.entity_totals.get(domain);
    let entity_dropped_total = account
        .entities
        .dropped
        .values()
        .copied()
        .fold(0u64, u64::saturating_add);
    account.entities.unattributed = account
        .entities
        .total
        .saturating_sub(account.entities.on_page)
        .saturating_sub(entity_dropped_total);

    // Primitive conservation (6.2).
    let primitive_dropped_total = account
        .primitives
        .dropped
        .values()
        .copied()
        .fold(0u64, u64::saturating_add);
    account.primitives.unattributed = account
        .primitives
        .laid_out
        .saturating_sub(account.primitives.on_page)
        .saturating_sub(primitive_dropped_total);

    // Segment conservation (6.3).
    let segment_dropped_total = account
        .segments
        .dropped
        .values()
        .copied()
        .fold(0u64, u64::saturating_add);
    account.segments.unattributed = account
        .segments
        .total
        .saturating_sub(account.segments.written)
        .saturating_sub(segment_dropped_total);

    account.text = *ledger.text.get(domain);
    account.suspect_sites.sort();
    account
}

/// One entity's outcome, counted into `account` (requirement 1).
///
/// `past_the_edge` is the primitive-level A.6 mark, already resolved by
/// [`assemble_domain`]'s caller; the entity-level partial count reads the same
/// set so the two layers can never disagree about a primitive that reaches
/// past the sheet.
fn entity_outcome(
    index: usize,
    record: &EntityRecord,
    write: &WriteLedger,
    claimed: &BTreeMap<usize, Vec<usize>>,
    removed: &BTreeMap<usize, u64>,
    past_the_edge: &BTreeSet<usize>,
    account: &mut DomainAccount,
) -> EntityVerdict {
    // 1. the layout layer already answered.
    if let Some(reason) = record.dropped {
        count_entity_drop(account, reason);
        if reason == DropReason::FillDuplicate
            && record.fill_first_taker == Some(LedgerDomain::Furniture)
        {
            account.entities.fill_duplicate_first_taken_by_furniture = account
                .entities
                .fill_duplicate_first_taken_by_furniture
                .saturating_add(1);
        }
        return EntityVerdict::Dropped(reason);
    }
    // 2. at least one primitive reached the page.
    let indices = claimed.get(&index).map_or(&[][..], Vec::as_slice);
    let mut any_on_page = false;
    let mut partial = false;
    let mut first_reason: Option<DropReason> = None;
    for primitive_index in indices {
        let Some(fate) = write.fate(*primitive_index) else {
            continue;
        };
        if fate.is_on_page() {
            any_on_page = true;
            // I2's first clause: one of its page primitives carries an appendix
            // A.6 mark. A.6 has four rows — `TextCondensed`,
            // `TextClippedAtFloor`, `PastTheEdge`, `SegmentsDropped` — and all
            // four must be read here, not only the two text ones. Reading just
            // the text pair left an entity whose drawn primitive ran past the
            // sheet at `partial 0` while that primitive was marked
            // `PastTheEdge` at the primitive layer.
            if fate.segments_dropped
                || fate.text.is_some_and(partially_marked)
                || past_the_edge.contains(primitive_index)
            {
                partial = true;
            }
        } else if let Some(reason) = fate.reason() {
            // Design deviation I2: an entity is partially on the page when one
            // of its page primitives carries an A.6 mark *or one of them did
            // not reach the page at all*. This is the second half of that
            // sentence — the writer dropped a primitive the entity claimed
            // while another of its primitives was drawn. The reason is still
            // kept for step 4 below, which files an entity that reached the
            // page nowhere under the first of its primitives' reasons.
            partial = true;
            first_reason = Some(match first_reason {
                Some(held) => held.min(reason),
                None => reason,
            });
        }
    }
    // 3. primitives that the final `retain` removed reached no page at all.
    if removed.get(&index).is_some_and(|count| *count > 0) {
        partial = true;
        first_reason = Some(match first_reason {
            Some(held) => held.min(DropReason::OffSheet),
            None => DropReason::OffSheet,
        });
    }
    if any_on_page {
        account.entities.on_page = account.entities.on_page.saturating_add(1);
        if partial {
            account.entities.partial = account.entities.partial.saturating_add(1);
        }
        return EntityVerdict::OnPage;
    }
    // 4. it has primitives but none reached the page.
    if let Some(reason) = first_reason {
        count_entity_drop(account, reason);
        return EntityVerdict::Dropped(reason);
    }
    // 5. nothing claims it: a remainder.
    EntityVerdict::Unattributed
}

/// Whether a written run's marks make it partial.
fn partially_marked(marks: crate::write::TextMarks) -> bool {
    marks.condensed || marks.clipped_at_floor
}

/// One entity's outcome, as the cross-count's innermost key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntityVerdict {
    /// At least one primitive reached the page.
    OnPage,
    /// The layout layer or every primitive dropped it.
    Dropped(DropReason),
    /// Nothing accounts for it.
    Unattributed,
}

impl EntityVerdict {
    /// The cross-count's key: `on_page`, a reason code, or `unattributed`.
    fn code(self) -> String {
        match self {
            Self::OnPage => "on_page".to_string(),
            Self::Dropped(reason) => reason.code().to_string(),
            Self::Unattributed => "unattributed".to_string(),
        }
    }

    /// The reason, when it was dropped.
    fn reason(self) -> Option<DropReason> {
        match self {
            Self::Dropped(reason) => Some(reason),
            _ => None,
        }
    }
}

/// Count one entity's drop under its reason.
fn count_entity_drop(account: &mut DomainAccount, reason: DropReason) {
    let entry = account.entities.dropped.entry(reason).or_insert(0);
    *entry = entry.saturating_add(1);
}

/// Record one partial mark on the primitive layer.
fn mark(account: &mut PrimitiveAccount, mark: PartialMark) {
    let entry = account.marks.entry(mark).or_insert(0);
    *entry = entry.saturating_add(1);
}

/// A suspect drop's site, from an entity record.
fn suspect_site(reason: DropReason, record: &EntityRecord) -> SuspectSite {
    let Site {
        stream,
        oid,
        bytes,
        record_kind: _,
    } = &record.site;
    SuspectSite {
        reason,
        stream: stream.clone(),
        oid: *oid,
        bytes: *bytes,
        type_code: None,
        family: None,
        name: None,
        count: 1,
    }
}

/// How many entities one domain filed under `reason`.
fn entity_dropped(domain: &DomainAccount, reason: DropReason) -> u64 {
    domain.entities.dropped.get(&reason).copied().unwrap_or(0)
}

/// Every reason-count pair a domain carries, over all its layers.
fn domain_counts(domain: &DomainAccount) -> Vec<(DropReason, u64)> {
    let mut counts: BTreeMap<DropReason, u64> = BTreeMap::new();
    let mut add = |reason: DropReason, count: u64| {
        if count > 0 {
            let entry = counts.entry(reason).or_insert(0);
            *entry = entry.saturating_add(count);
        }
    };
    for (reason, count) in &domain.entities.dropped {
        add(*reason, *count);
    }
    for (reason, count) in &domain.primitives.dropped {
        add(*reason, *count);
    }
    for (reason, count) in &domain.bodies.dropped {
        add(*reason, *count);
    }
    for (reason, count) in &domain.segments.dropped {
        add(*reason, *count);
    }
    for item in &domain.parse.items {
        add(item.reason, item.count);
    }
    counts.into_iter().collect()
}

/// The layout ledger's parse counts, as the baseline's own records.
fn parse_account(counts: &ParseCounts) -> ParseAccount {
    let mut items: Vec<RecordItem> = counts
        .items
        .iter()
        .map(|record| RecordItem {
            reason: record.reason,
            stream: record.stream.clone(),
            type_code: record.type_code,
            family: record.family.clone(),
            count: record.count,
            bytes: record.bytes,
            body_primitives: record.body_primitives,
        })
        .collect();
    items.sort_by(|left, right| {
        (left.reason, &left.stream, left.type_code, &left.family).cmp(&(
            right.reason,
            &right.stream,
            right.type_code,
            &right.family,
        ))
    });
    ParseAccount {
        items,
        unattributed: counts.unattributed,
    }
}

/// Where a page's size came from.
fn page_size_source(model: &PaperModel) -> PageSizeSource {
    if model.page_mm.is_some() {
        PageSizeSource::Declared
    } else if model.resolved_page_mm().is_some() {
        PageSizeSource::Measured
    } else {
        PageSizeSource::None
    }
}

/// The template a page wears, as the baseline names it.
fn template_fact(template: &BorderTemplate) -> TemplateFact {
    // The path is either a `.igr` on disk or an archive's path with the
    // member's own name on the end. The file name is the first, and the
    // archive's file name is read off the path only when there is one — a
    // baseline stores names, never paths (requirement 12.4).
    let path = std::path::Path::new(&template.path);
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(&template.path)
        .to_string();
    let archive = path
        .ancestors()
        .find_map(|ancestor| {
            let is_archive = ancestor
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("RefData") && name.ends_with(".zip"));
            is_archive.then(|| {
                ancestor
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default()
                    .to_string()
            })
        })
        .filter(|archive| *archive != name);
    TemplateFact {
        name,
        source: if archive.is_some() {
            TemplateSourceKind::ArchiveMember
        } else {
            TemplateSourceKind::DirectoryFile
        },
        archive,
    }
}

/// One census table's rows, as `(stream, type code, count)`.
type CensusRows = Vec<(String, u16, u64)>;

/// Requirement 8.5: the three census tables, item for item.
fn check_parse_tables(parse: &ParseAccount, geometry: &NormalizedPidGeometry) -> Vec<String> {
    let mut mismatches = Vec::new();
    let tables: [(&str, DropReason, CensusRows); 3] = [
        (
            "dropped_graphic_records",
            DropReason::RecordNoDecoder,
            geometry
                .dropped_graphic_records
                .iter()
                .map(|row| {
                    (
                        row.stream_path.clone(),
                        row.type_code,
                        u64::try_from(row.count).unwrap_or(u64::MAX),
                    )
                })
                .collect(),
        ),
        (
            "refused_graphic_records",
            DropReason::RecordRefused,
            geometry
                .refused_graphic_records
                .iter()
                .map(|row| {
                    (
                        row.stream_path.clone(),
                        row.type_code,
                        u64::try_from(row.count).unwrap_or(u64::MAX),
                    )
                })
                .collect(),
        ),
        (
            "skipped_graphic_records",
            DropReason::RecordNativeSkip,
            geometry
                .skipped_graphic_records
                .iter()
                .map(|row| {
                    (
                        row.stream_path.clone(),
                        row.type_code,
                        u64::try_from(row.count).unwrap_or(u64::MAX),
                    )
                })
                .collect(),
        ),
    ];
    for (table, reason, rows) in tables {
        for (stream, type_code, count) in rows {
            let found = parse
                .items
                .iter()
                .find(|item| {
                    item.reason == reason
                        && item.stream == stream
                        && item.type_code == Some(type_code)
                })
                .map_or(0, |item| item.count);
            if found != count {
                mismatches.push(format!(
                    "8.5 {table} {stream} 0x{type_code:04X}: ledger {found} != table {count}"
                ));
            }
        }
    }
    mismatches
}

/// The two domains' accounts while the assembly fills them.
#[derive(Debug, Default)]
struct DomainPairBuilder {
    drawing: DomainAccount,
    furniture: DomainAccount,
}

impl DomainPairBuilder {
    fn set(&mut self, domain: LedgerDomain, account: DomainAccount) {
        match domain {
            LedgerDomain::Drawing => self.drawing = account,
            LedgerDomain::Furniture => self.furniture = account,
        }
    }
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

    fn a_record_item(type_code: Option<u16>) -> RecordItem {
        RecordItem {
            reason: DropReason::RecordRefused,
            stream: "/Sheet6".to_string(),
            type_code,
            family: None,
            count: 1,
            bytes: None,
            body_primitives: None,
        }
    }

    #[test]
    fn a_type_code_is_written_as_hex_and_read_back() {
        let item = a_record_item(Some(0x00CE));
        let json = serde_json::to_string(&item).expect("a record item serializes");
        assert!(
            json.contains("\"type_code\":\"0x00CE\""),
            "the type code must be spelled the way the catalog spells it: {json}"
        );
        let back: RecordItem = serde_json::from_str(&json).expect("and reads back");
        assert_eq!(back, item);
    }

    #[test]
    fn a_missing_type_code_is_absent_rather_than_null() {
        let item = a_record_item(None);
        let json = serde_json::to_string(&item).expect("a record item serializes");
        assert!(!json.contains("type_code"), "{json}");
    }

    #[test]
    fn a_type_code_without_the_prefix_is_refused() {
        let json = r#"{"reason":"record_refused","stream":"/Sheet6","type_code":"00CE","count":1}"#;
        assert!(serde_json::from_str::<RecordItem>(json).is_err());
    }

    #[test]
    fn an_export_ledger_round_trips_and_carries_every_remainder() {
        let ledger = ExportLedger {
            page: PageAccount {
                size_source: PageSizeSource::Declared,
                template: Some(TemplateFact {
                    name: "CPE-SQ-A1.igr".to_string(),
                    source: TemplateSourceKind::DirectoryFile,
                    archive: None,
                }),
                missing_templates: Vec::new(),
                dropped: BTreeMap::new(),
                suspect_sites: Vec::new(),
            },
            fonts_measured: false,
            drawing: DomainAccount {
                parse: ParseAccount {
                    items: vec![a_record_item(Some(0x0019))],
                    unattributed: 0,
                },
                entities: EntityAccount {
                    total: 3,
                    on_page: 2,
                    partial: 0,
                    dropped: BTreeMap::from([(DropReason::KindUnknown, 1)]),
                    fill_duplicate_first_taken_by_furniture: 0,
                    cross: BTreeMap::from([(
                        EntityKind::Unknown,
                        BTreeMap::from([(
                            Confidence::ProbeOnly,
                            BTreeMap::from([("kind_unknown".to_string(), 1)]),
                        )]),
                    )]),
                    unattributed: 0,
                },
                bodies: BodyAccount {
                    placements: BTreeMap::from([(PlacementSource::Cached, 1)]),
                    total: 4,
                    produced: 3,
                    dropped: BTreeMap::from([(DropReason::BodyLayerHidden, 1)]),
                    unattributed: 0,
                },
                text: TextNormalization::default(),
                primitives: PrimitiveAccount {
                    laid_out: 2,
                    off_sheet: 0,
                    on_page: 2,
                    dropped: BTreeMap::new(),
                    partial: 0,
                    marks: BTreeMap::from([(PartialMark::PastTheEdge, 1)]),
                    unattributed: 0,
                },
                segments: SegmentAccount {
                    total: 10,
                    written: 10,
                    dropped: BTreeMap::new(),
                    unattributed: 0,
                },
                asymmetry: BTreeMap::new(),
                suspect_sites: Vec::new(),
            },
            furniture: DomainAccount::default(),
        };

        let json = serde_json::to_string_pretty(&ledger).expect("an export ledger serializes");
        let back: ExportLedger = serde_json::from_str(&json).expect("and reads back");
        assert_eq!(back, ledger);

        // Requirement 6.4: the remainders are present in the JSON even at zero.
        // Two domains carry five layers each, so ten keys.
        let count = json.matches("\"unattributed\"").count();
        assert_eq!(
            count, 10,
            "each of the two domains carries five unattributed keys: {json}"
        );
    }

    #[test]
    fn an_empty_domain_account_still_reports_its_remainders() {
        let json = serde_json::to_string(&DomainAccount::default()).expect("it serializes");
        assert!(json.contains("\"unattributed\":0"), "{json}");
    }

    #[test]
    fn a_corpus_descriptor_compares_only_the_symbol_and_template_parts() {
        let left = CorpusDescriptor {
            symbol_sources: vec![SourceName {
                kind: super::super::layout::SourceKind::Directory,
                name: "symbols".to_string(),
                archive: None,
            }],
            template_sources: Vec::new(),
            font_faces: vec!["SimSun".to_string()],
        };
        let right = CorpusDescriptor {
            font_faces: vec!["FangSong".to_string()],
            ..left.clone()
        };
        assert!(
            left.same_corpus(&right),
            "a font change is not a corpus change"
        );
        let other = CorpusDescriptor {
            template_sources: vec![SourceName {
                kind: super::super::layout::SourceKind::ArchiveMember,
                name: "CPE-SQ-A1.igr".to_string(),
                archive: Some("RefData~4~682.zip".to_string()),
            }],
            ..left.clone()
        };
        assert!(!left.same_corpus(&other));
    }

    /// The only kinds a baseline field may be built from: a count, a flag, a
    /// name, a page coordinate pair, one of the payload-free enums this module
    /// and `ledger` declare, or a list/map/option of those.
    ///
    /// This is the whole point of [`the_ledger_carries_no_floating_point_field`]:
    /// the trait has no `f64` / `f32` impl, so a float field fails to compile.
    /// A map's key needs no impl — `f64` is not `Ord`, so it cannot be one.
    trait BaselineValue {}
    impl BaselineValue for u16 {}
    impl BaselineValue for u32 {}
    impl BaselineValue for u64 {}
    impl BaselineValue for usize {}
    impl BaselineValue for bool {}
    impl BaselineValue for String {}
    impl BaselineValue for [usize; 2] {}
    impl BaselineValue for DropReason {}
    impl BaselineValue for PartialMark {}
    impl BaselineValue for Asymmetry {}
    impl BaselineValue for EntityKind {}
    impl BaselineValue for Confidence {}
    impl BaselineValue for PlacementSource {}
    impl BaselineValue for SourceKind {}
    impl BaselineValue for PageSizeSource {}
    impl BaselineValue for TemplateSourceKind {}
    impl BaselineValue for TextNormalization {}
    impl BaselineValue for SourceName {}
    impl BaselineValue for TemplateFact {}
    impl BaselineValue for RecordItem {}
    impl BaselineValue for SuspectSite {}
    impl<T: BaselineValue> BaselineValue for Option<T> {}
    impl<T: BaselineValue> BaselineValue for Vec<T> {}
    impl<K, V: BaselineValue> BaselineValue for BTreeMap<K, V> {}

    /// Accepts a field the baseline may hold; refuses every other type.
    fn baseline_value<T: BaselineValue>(_: &T) {}

    #[test]
    fn the_ledger_carries_no_floating_point_field() {
        // Requirement 12.2: counts are integers. A float anywhere in this
        // module would make a baseline compare platform-dependent, which is
        // the one thing the ledger must never be.
        //
        // Asserted at the type level, not by scanning source text: each struct
        // is destructured field by field (so a new field has to be declared
        // here before it compiles at all) and every field is handed to
        // `baseline_value`, whose bound admits only the kinds listed on
        // `BaselineValue`. A `f64` field is a compile error. The earlier form
        // filtered `pub ` lines out of `include_str!("assemble.rs")`, which a
        // comment or a reflow could defeat in either direction.
        let ledger = ExportLedger {
            page: PageAccount::default(),
            fonts_measured: false,
            drawing: DomainAccount::default(),
            furniture: DomainAccount::default(),
        };
        let ExportLedger {
            page,
            fonts_measured,
            drawing,
            furniture,
        } = &ledger;
        baseline_value(fonts_measured);

        let PageAccount {
            size_source,
            template,
            missing_templates,
            dropped,
            suspect_sites,
        } = page;
        baseline_value(size_source);
        baseline_value(template);
        baseline_value(missing_templates);
        baseline_value(dropped);
        baseline_value(suspect_sites);

        for domain in [drawing, furniture] {
            let DomainAccount {
                parse,
                entities,
                bodies,
                text,
                primitives,
                segments,
                asymmetry,
                suspect_sites,
            } = domain;
            let ParseAccount {
                items,
                unattributed,
            } = parse;
            baseline_value(items);
            baseline_value(unattributed);
            let EntityAccount {
                total,
                on_page,
                partial,
                dropped,
                fill_duplicate_first_taken_by_furniture,
                cross,
                unattributed,
            } = entities;
            for count in [
                total,
                on_page,
                partial,
                fill_duplicate_first_taken_by_furniture,
                unattributed,
            ] {
                baseline_value(count);
            }
            baseline_value(dropped);
            baseline_value(cross);
            let BodyAccount {
                placements,
                total,
                produced,
                dropped,
                unattributed,
            } = bodies;
            baseline_value(placements);
            for count in [total, produced, unattributed] {
                baseline_value(count);
            }
            baseline_value(dropped);
            let PrimitiveAccount {
                laid_out,
                off_sheet,
                on_page,
                dropped,
                partial,
                marks,
                unattributed,
            } = primitives;
            for count in [laid_out, off_sheet, on_page, partial, unattributed] {
                baseline_value(count);
            }
            baseline_value(dropped);
            baseline_value(marks);
            let SegmentAccount {
                total,
                written,
                dropped,
                unattributed,
            } = segments;
            for count in [total, written, unattributed] {
                baseline_value(count);
            }
            baseline_value(dropped);
            baseline_value(asymmetry);
            baseline_value(suspect_sites);
            let TextNormalization {
                control_chars,
                trailing_empty_lines,
            } = text;
            baseline_value(control_chars);
            baseline_value(trailing_empty_lines);
        }

        let item = a_record_item(None);
        let RecordItem {
            reason,
            stream,
            type_code,
            family,
            count,
            bytes,
            body_primitives,
        } = &item;
        baseline_value(reason);
        baseline_value(stream);
        baseline_value(type_code);
        baseline_value(family);
        baseline_value(count);
        baseline_value(bytes);
        baseline_value(body_primitives);

        let fact = TemplateFact {
            name: "CPE-SQ-A1.igr".to_string(),
            source: TemplateSourceKind::DirectoryFile,
            archive: None,
        };
        let TemplateFact {
            name,
            source,
            archive,
        } = &fact;
        baseline_value(name);
        baseline_value(source);
        baseline_value(archive);

        let source = SourceName {
            kind: SourceKind::Directory,
            name: "Symbols".to_string(),
            archive: None,
        };
        let SourceName {
            kind,
            name,
            archive,
        } = &source;
        baseline_value(kind);
        baseline_value(name);
        baseline_value(archive);

        let site = SuspectSite {
            reason: DropReason::TemplateMissing,
            stream: Some("/Sheet6".to_string()),
            oid: Some(7),
            bytes: Some([10, 42]),
            type_code: Some(0x0018),
            family: Some("igLine2d".to_string()),
            name: Some("CPE-SQ-A1.igr".to_string()),
            count: 1,
        };
        let SuspectSite {
            reason,
            stream,
            oid,
            bytes,
            type_code,
            family,
            name,
            count,
        } = &site;
        for field in [stream, family, name] {
            baseline_value(field);
        }
        baseline_value(oid);
        baseline_value(bytes);
        baseline_value(type_code);
        baseline_value(count);
        baseline_value(reason);

        let descriptor = CorpusDescriptor::default();
        let CorpusDescriptor {
            symbol_sources,
            template_sources,
            font_faces,
        } = &descriptor;
        baseline_value(symbol_sources);
        baseline_value(template_sources);
        baseline_value(font_faces);

        let summary = LedgerSummary::default();
        let LedgerSummary {
            entities,
            on_page,
            intentional,
            suspect,
            unattributed,
        } = &summary;
        for count in [entities, on_page, intentional, suspect, unattributed] {
            baseline_value(count);
        }
    }

    // ---------------------------------------------------------------- fixtures

    use super::super::layout::{EntityRecord, Site, SourceKind};
    use super::super::reason::LedgerDomain;
    use crate::model::{PaperModel, PaperPrimitive, PaperShape, PaperStroke};
    use crate::write::{PrimitiveFate, PrimitiveOutcome, SegmentTally, TextMarks, WriteLedger};

    /// A page primitive; the caller decides whether it runs past the sheet.
    fn a_line(start: [f64; 2], end: [f64; 2]) -> PaperPrimitive {
        PaperPrimitive {
            confidence: "decoded",
            stream_path: Some("/Sheet6".to_string()),
            byte_range: Some([10, 42]),
            symbol: None,
            stroke: Some(PaperStroke {
                rgb: [0, 0, 0],
                width_mm: 0.35,
                dash_mm: Vec::new(),
                dash_fit: None,
            }),
            shape: PaperShape::Line {
                start_mm: start,
                end_mm: end,
            },
            furniture: false,
        }
    }

    /// A model that carries `ledger` and `primitives`, on the stated page.
    fn a_model(
        ledger: LayoutLedger,
        primitives: Vec<PaperPrimitive>,
        page_mm: Option<[f64; 2]>,
    ) -> PaperModel {
        PaperModel {
            library: Vec::new(),
            page_mm,
            template: None,
            primitives,
            off_sheet_primitives: 0,
            skipped_endpoint_pairs: 0,
            missing_symbols: Vec::new(),
            missing_templates: Vec::new(),
            missing_boxes: Vec::new(),
            symbols_found: 0,
            symbols_referenced: 0,
            ledger,
        }
    }

    /// One verdict per primitive: on the page, or dropped for `reason`.
    fn a_fate(outcome: PrimitiveOutcome) -> PrimitiveFate {
        PrimitiveFate {
            outcome,
            segments: SegmentTally::default(),
            segments_dropped: false,
            text: None,
        }
    }

    fn a_write_ledger(fates: Vec<PrimitiveFate>) -> WriteLedger {
        WriteLedger {
            fonts_measured: false,
            fates,
            mismatches: Vec::new(),
        }
    }

    /// One entity record, drawing domain, `Line` at `Decoded`.
    fn an_entity(dropped: Option<DropReason>) -> EntityRecord {
        EntityRecord {
            domain: LedgerDomain::Drawing,
            kind: EntityKind::Line,
            confidence: Confidence::Decoded,
            dropped,
            fill_first_taker: None,
            placement: None,
            observed: Vec::new(),
            site: Site {
                stream: Some("/Sheet6".to_string()),
                oid: Some(7),
                bytes: Some([10, 42]),
                record_kind: None,
            },
        }
    }

    /// The sum of one domain's entity reasons.
    fn dropped_total(account: &EntityAccount) -> u64 {
        account
            .dropped
            .values()
            .copied()
            .fold(0u64, u64::saturating_add)
    }

    // ------------------------------------------------------- property tests

    /// // Feature: pdf-export-ledger, Property 1: 实体结局唯一且端到端合成
    ///
    /// *For any* set of layout records (each with a layout-layer verdict and
    /// the primitives it claimed) and a write verdict for every primitive,
    /// `assemble` gives each entity exactly one outcome: the layout reason when
    /// it dropped the entity; `on_page` when at least one primitive reached the
    /// page; otherwise the first appendix reason among its primitives, or a
    /// remainder when nothing claims it. Every domain balances:
    /// `total = on_page + Σ reasons + unattributed`.
    #[test]
    fn entity_fate_is_unique_and_assembled() {
        use proptest::prelude::*;

        // The layout-layer reasons a generated entity may carry.
        static LAYOUT: [DropReason; 4] = [
            DropReason::LayerHidden,
            DropReason::EndpointPairGuess,
            DropReason::KindUnknown,
            DropReason::SymbolBodyEmpty,
        ];
        // The write-layer reasons a generated primitive may carry.
        static WRITE: [DropReason; 3] = [
            DropReason::ProbeOnly,
            DropReason::ChainEmpty,
            DropReason::TextEmpty,
        ];

        let strategy = prop::collection::vec(
            (
                prop::option::of(prop::sample::select(&LAYOUT[..])),
                0usize..3,
            ),
            0..8,
        );
        let fate_strategy = prop::collection::vec(
            prop_oneof![Just(None), prop::sample::select(&WRITE[..]).prop_map(Some),],
            0..16,
        );
        let mut runner = proptest::test_runner::TestRunner::new(proptest::test_runner::Config {
            cases: 256,
            ..proptest::test_runner::Config::default()
        });
        runner
            .run(&(strategy, fate_strategy), |(entities, fates)| {
                let mut ledger = LayoutLedger::default();
                ledger.entity_totals.drawing = u64::try_from(entities.len()).unwrap_or(u64::MAX);
                for (dropped, claimed) in &entities {
                    ledger.entities.push(an_entity(*dropped));
                    for _ in 0..*claimed {
                        let origin = u32::try_from(ledger.entities.len() - 1).unwrap_or(u32::MAX);
                        ledger.primitive_origin.push(origin);
                    }
                }
                let primitives: Vec<PaperPrimitive> = ledger
                    .primitive_origin
                    .iter()
                    .map(|_| a_line([10.0, 10.0], [20.0, 10.0]))
                    .collect();
                let fates: Vec<PrimitiveFate> = primitives
                    .iter()
                    .enumerate()
                    .map(|(index, _)| {
                        let choice = fates.get(index).copied().flatten();
                        a_fate(match choice {
                            None => PrimitiveOutcome::OnPage,
                            Some(reason) => PrimitiveOutcome::Dropped(reason),
                        })
                    })
                    .collect();
                let model = a_model(ledger, primitives, Some([841.0, 594.0]));
                let write = a_write_ledger(fates);

                let assembled = ExportLedger::assemble(&model, &write);
                let account = &assembled.drawing;

                // Every entity got exactly one outcome.
                let cross_total: u64 = account
                    .entities
                    .cross
                    .values()
                    .flat_map(|by_confidence| by_confidence.values())
                    .flat_map(|by_outcome| by_outcome.values())
                    .copied()
                    .fold(0u64, u64::saturating_add);
                prop_assert_eq!(
                    cross_total,
                    account.entities.total,
                    "one outcome per entity"
                );
                // The equation (requirement 6.1).
                prop_assert_eq!(
                    account.entities.total,
                    account
                        .entities
                        .on_page
                        .saturating_add(dropped_total(&account.entities))
                        .saturating_add(account.entities.unattributed)
                );
                // A remainder is exactly the entity the layout pass said it
                // produced and no primitive claimed.
                let unclaimed = entities
                    .iter()
                    .filter(|(dropped, claimed)| dropped.is_none() && *claimed == 0)
                    .count() as u64;
                prop_assert_eq!(account.entities.unattributed, unclaimed);
                Ok(())
            })
            .expect("the property holds over the generated pages");
    }

    /// // Feature: pdf-export-ledger, Property 2: 交叉计数与部分上页计数自洽
    ///
    /// *For any* assembled account, summing `cross[kind][confidence][outcome]`
    /// over kinds and confidences gives that outcome's entity count, all of
    /// them together give `total`, `partial ≤ on_page`, and `partial` equals
    /// the number of entities design deviation I2 calls partially on the page:
    ///
    /// > 实体已上页，且它的页面图元里至少一个带附录 A.6 的标记，或至少一个
    /// > 没有上页（含被 `off_sheet` 移除的）。
    ///
    /// The generator therefore produces every shape that sentence ranges over,
    /// written as a fixed table of eight claim codes so no shape can go missing:
    ///
    /// | code | shape |
    /// |---|---|
    /// | 0 | drawn whole, no mark |
    /// | 1 | drawn, `TextCondensed` |
    /// | 2 | drawn, `TextClippedAtFloor` |
    /// | 3 | the writer dropped this primitive |
    /// | 4 | the final `retain` removed it (`off_sheet`) |
    /// | 5 | drawn 0.9 mm past the sheet (`PastTheEdge`) |
    /// | 6 | drawn, `SegmentsDropped` |
    /// | 7 | the entity claimed no primitive at all (a remainder) |
    ///
    /// Codes 0–6 are claims, code 7 is an entity shape. One fixed entity claims
    /// a primitive of every claim shape 0–6 and one fixed entity claims
    /// nothing, so every code appears in every case; the random tail adds
    /// mixtures. Before this table the generator produced only codes 0–5 and
    /// never set `segments_dropped`, so A.6's fourth row had no test that could
    /// see it at the entity layer.
    ///
    /// The expected count below is written from those inputs — the claim codes
    /// themselves — never read back out of the assembly.
    #[test]
    fn cross_counts_agree_with_the_outcome_totals() {
        use proptest::prelude::*;

        static KINDS: [EntityKind; 4] = [
            EntityKind::Line,
            EntityKind::Text,
            EntityKind::SymbolInstance,
            EntityKind::Unknown,
        ];
        static CONFIDENCES: [Confidence; 3] = [
            Confidence::Decoded,
            Confidence::Inferred,
            Confidence::ProbeOnly,
        ];
        static REASONS: [DropReason; 2] = [DropReason::LayerHidden, DropReason::SymbolMissing];

        // One claim's shape, the fixed table this property ranges over:
        // 0 drawn whole, 1 `TextCondensed`, 2 `TextClippedAtFloor`, 3 a
        // primitive the writer dropped, 4 a primitive `retain` removed, 5 drawn
        // past the sheet (`PastTheEdge`), 6 `SegmentsDropped`, 7 an entity that
        // claimed no primitive at all (a remainder).
        //
        // 6 and 7 were the two the generator never produced: without 6, A.6's
        // `SegmentsDropped` row had no entity-layer test at all — deleting the
        // `fate.segments_dropped` half of `entity_outcome`'s partial test left
        // every test green.
        const CLAIM_CODES: u8 = 8;
        let strategy = prop::collection::vec(
            (
                prop::sample::select(&KINDS[..]),
                prop::sample::select(&CONFIDENCES[..]),
                prop::option::of(prop::sample::select(&REASONS[..])),
                prop::collection::vec(0u8..7, 0..4),
            ),
            0..8,
        );
        let mut runner = proptest::test_runner::TestRunner::new(proptest::test_runner::Config {
            cases: 256,
            ..proptest::test_runner::Config::default()
        });
        runner
            .run(&strategy, |random| {
                // Every claim code in every case, so coverage never depends on
                // a random draw: one entity claims one primitive of each shape
                // 0–6, and one claims nothing (7).
                let mut entities = vec![
                    (
                        EntityKind::Line,
                        Confidence::Decoded,
                        None,
                        vec![0u8, 1, 2, 3, 4, 5, 6],
                    ),
                    (EntityKind::Line, Confidence::Decoded, None, Vec::new()),
                ];
                entities.extend(random);

                let mut ledger = LayoutLedger::default();
                ledger.entity_totals.drawing = u64::try_from(entities.len()).unwrap_or(u64::MAX);
                let mut fates: Vec<PrimitiveFate> = Vec::new();
                // Whether each fate's primitive is drawn past the sheet edge —
                // A.6's `PastTheEdge`, which lives in the page model rather
                // than in the writer's verdict.
                let mut past_flags: Vec<bool> = Vec::new();
                // I2's own count, taken from these inputs.
                let mut expected_on_page = 0u64;
                let mut expected_partial = 0u64;
                // Which claim codes this case actually produced.
                let mut seen = [false; CLAIM_CODES as usize];
                for (kind, confidence, dropped, claims) in &entities {
                    let mut record = an_entity(*dropped);
                    record.kind = kind.clone();
                    record.confidence = *confidence;
                    ledger.entities.push(record);
                    let origin = u32::try_from(ledger.entities.len() - 1).unwrap_or(u32::MAX);
                    if claims.is_empty() {
                        // Code 7: the entity claimed nothing, so no primitive
                        // can reach the page and it ends up a remainder.
                        seen[7] = true;
                    }
                    let mut any_on_page = false;
                    let mut partial = false;
                    for claim in claims {
                        seen[usize::from(*claim)] = true;
                        if *claim == 4 {
                            ledger.off_sheet.push(origin);
                            partial = true;
                            continue;
                        }
                        let fate = match claim {
                            1 => PrimitiveFate {
                                outcome: PrimitiveOutcome::OnPage,
                                segments: SegmentTally::default(),
                                segments_dropped: false,
                                text: Some(TextMarks {
                                    condensed: true,
                                    clipped_at_floor: false,
                                }),
                            },
                            2 => PrimitiveFate {
                                outcome: PrimitiveOutcome::OnPage,
                                segments: SegmentTally::default(),
                                segments_dropped: false,
                                text: Some(TextMarks {
                                    condensed: false,
                                    clipped_at_floor: true,
                                }),
                            },
                            3 => a_fate(PrimitiveOutcome::Dropped(DropReason::ChainEmpty)),
                            6 => PrimitiveFate {
                                outcome: PrimitiveOutcome::OnPage,
                                segments: SegmentTally {
                                    total: 2,
                                    written: 1,
                                    non_finite: 0,
                                    too_short: 0,
                                    on_sheet_edge: 1,
                                },
                                segments_dropped: true,
                                text: None,
                            },
                            _ => a_fate(PrimitiveOutcome::OnPage),
                        };
                        ledger.primitive_origin.push(origin);
                        past_flags.push(*claim == 5);
                        if fate.is_on_page() {
                            any_on_page = true;
                            // A.6's four rows, read straight off the claim
                            // code: the text pair, `PastTheEdge` (5) and
                            // `SegmentsDropped` (6).
                            if matches!(*claim, 1 | 2 | 5 | 6) {
                                partial = true;
                            }
                        } else {
                            // A primitive of an entity that may still have
                            // reached the page: I2's second clause.
                            partial = true;
                        }
                        fates.push(fate);
                    }
                    if dropped.is_none() && any_on_page {
                        expected_on_page = expected_on_page.saturating_add(1);
                        if partial {
                            expected_partial = expected_partial.saturating_add(1);
                        }
                    }
                }
                // The table is a contract: a shape that stops being generated
                // is a shape this property stopped checking.
                for (code, produced) in seen.iter().enumerate() {
                    prop_assert!(*produced, "claim code {} was not generated", code);
                }
                let primitives: Vec<PaperPrimitive> = past_flags
                    .iter()
                    .map(|past| {
                        if *past {
                            // 0.9 mm past the 841 mm edge: more than the
                            // ledger's 0.5 mm slack, so `primitives_past_the_edge`
                            // counts it.
                            a_line([800.0, 20.0], [841.9, 20.0])
                        } else {
                            a_line([10.0, 10.0], [20.0, 10.0])
                        }
                    })
                    .collect();
                let model = a_model(ledger, primitives, Some([841.0, 594.0]));
                let assembled = ExportLedger::assemble(&model, &a_write_ledger(fates));
                let account = &assembled.drawing;

                let mut by_outcome: BTreeMap<String, u64> = BTreeMap::new();
                for by_confidence in account.entities.cross.values() {
                    for by_outcome_inner in by_confidence.values() {
                        for (outcome, count) in by_outcome_inner {
                            let entry = by_outcome.entry(outcome.clone()).or_insert(0);
                            *entry = entry.saturating_add(*count);
                        }
                    }
                }
                prop_assert_eq!(
                    by_outcome.get("on_page").copied().unwrap_or(0),
                    account.entities.on_page
                );
                for (reason, count) in &account.entities.dropped {
                    prop_assert_eq!(by_outcome.get(reason.code()).copied().unwrap_or(0), *count);
                }
                prop_assert_eq!(
                    by_outcome.get("unattributed").copied().unwrap_or(0),
                    account.entities.unattributed
                );
                let cross_total: u64 = by_outcome.values().copied().fold(0u64, u64::saturating_add);
                prop_assert_eq!(cross_total, account.entities.total);
                prop_assert_eq!(account.entities.on_page, expected_on_page);
                prop_assert!(
                    account.entities.partial <= account.entities.on_page,
                    "partial {} exceeds on_page {}",
                    account.entities.partial,
                    account.entities.on_page
                );
                prop_assert_eq!(
                    account.entities.partial,
                    expected_partial,
                    "I2's partial count, taken from the generated fates"
                );
                Ok(())
            })
            .expect("the property holds over the generated accounts");
    }

    /// // Feature: pdf-export-ledger, Property 4: 写出层守恒
    ///
    /// *For any* page model and write verdicts, each domain balances its
    /// primitive layer and its segment layer, both remainders are zero, and a
    /// primitive carries `segments_dropped` exactly when it is on the page with
    /// at least one dropped window.
    ///
    /// The primitive equation is the one `assemble` actually computes:
    ///
    /// ```text
    /// laid_out = on_page + Σ primitives.dropped + unattributed
    /// ```
    ///
    /// Two facts make it what it is:
    ///
    /// - `laid_out` is what the layout produced *before* the final `retain`:
    ///   the survivors `PaperModel::primitives` holds plus the primitives the
    ///   `retain` removed, which `assemble` adds back from `ledger.off_sheet`;
    /// - A.4's `off_sheet` is a primitive-layer drop as well as a remainder
    ///   input, so it is already inside `primitives.dropped`. Adding
    ///   `off_sheet` again on the right-hand side counts those primitives
    ///   twice — which is what this test used to do while its generator never
    ///   produced one, so the double count could never show.
    #[test]
    fn write_layer_is_conserved() {
        use proptest::prelude::*;

        static OUTCOMES: [PrimitiveOutcome; 4] = [
            PrimitiveOutcome::OnPage,
            PrimitiveOutcome::Dropped(DropReason::ProbeOnly),
            PrimitiveOutcome::Dropped(DropReason::ChainEmpty),
            PrimitiveOutcome::Dropped(DropReason::PenWidthInvalid),
        ];
        // `outcome`, the three segment buckets, and whether the final `retain`
        // removed this primitive before the page was written.
        let strategy = prop::collection::vec(
            (
                prop::sample::select(&OUTCOMES[..]),
                0u64..6,
                0u64..6,
                0u64..3,
                0u64..3,
                0u64..3,
                prop::bool::ANY,
            ),
            0..12,
        );
        let mut runner = proptest::test_runner::TestRunner::new(proptest::test_runner::Config {
            cases: 256,
            ..proptest::test_runner::Config::default()
        });
        runner
            .run(&strategy, |rows| {
                let mut ledger = LayoutLedger::default();
                let mut primitives = Vec::new();
                let mut fates = Vec::new();
                let mut expected_off_sheet = 0u64;
                for (outcome, total, written, non_finite, too_short, _on_edge, removed) in &rows {
                    if *removed {
                        // A primitive the `retain` took reaches no page and has
                        // no fate: `primitive_origin` is rebuilt from the
                        // survivors, so this index never appears in the model.
                        ledger.off_sheet.push(0);
                        expected_off_sheet = expected_off_sheet.saturating_add(1);
                        continue;
                    }
                    // A verdict's own numbers always add up — a chain of `n`
                    // points walks `n - 1` windows and each is either written or
                    // dropped for one of the three segment reasons. The
                    // generator's numbers are clamped into that shape so the
                    // property is about the assembly, not about arithmetic the
                    // writer never does.
                    let walked = *total;
                    let written = (*written).min(walked);
                    let rest = walked.saturating_sub(written);
                    let non_finite = (*non_finite).min(rest);
                    let rest = rest.saturating_sub(non_finite);
                    let too_short = (*too_short).min(rest);
                    // Whatever is left is the sheet-edge bucket: every walked
                    // window is either written or dropped for one of the three
                    // reasons, which is the equation under test.
                    let on_edge = rest.saturating_sub(too_short);
                    let segments_dropped = *outcome == PrimitiveOutcome::OnPage && written < walked;
                    fates.push(PrimitiveFate {
                        outcome: *outcome,
                        segments: SegmentTally {
                            total: walked,
                            written,
                            non_finite,
                            too_short,
                            on_sheet_edge: on_edge,
                        },
                        segments_dropped,
                        text: None,
                    });
                    primitives.push(a_line([10.0, 10.0], [20.0, 10.0]));
                }
                ledger.primitive_origin = vec![0; primitives.len()];
                ledger.entities.push(an_entity(None));
                ledger.entity_totals.drawing = 1;
                let survivors = u64::try_from(primitives.len()).unwrap_or(u64::MAX);
                let model = a_model(ledger, primitives, Some([841.0, 594.0]));
                let assembled = ExportLedger::assemble(&model, &a_write_ledger(fates));
                let account = &assembled.drawing;

                let dropped: u64 = account
                    .primitives
                    .dropped
                    .values()
                    .copied()
                    .fold(0u64, u64::saturating_add);
                prop_assert_eq!(
                    account.primitives.laid_out,
                    survivors.saturating_add(expected_off_sheet),
                    "`laid_out` is what the layout produced before the retain"
                );
                prop_assert_eq!(
                    account.primitives.off_sheet,
                    expected_off_sheet,
                    "one `off_sheet` record per primitive the retain removed"
                );
                prop_assert_eq!(
                    account
                        .primitives
                        .dropped
                        .get(&DropReason::OffSheet)
                        .copied()
                        .unwrap_or(0),
                    expected_off_sheet,
                    "A.4's off_sheet is filed under the primitive layer's drops"
                );
                // The equation, with `off_sheet` counted once: inside `dropped`.
                prop_assert_eq!(
                    account.primitives.laid_out,
                    account
                        .primitives
                        .on_page
                        .saturating_add(dropped)
                        .saturating_add(account.primitives.unattributed)
                );
                prop_assert_eq!(account.primitives.unattributed, 0);
                let segment_dropped: u64 = account
                    .segments
                    .dropped
                    .values()
                    .copied()
                    .fold(0u64, u64::saturating_add);
                prop_assert_eq!(
                    account.segments.total,
                    account
                        .segments
                        .written
                        .saturating_add(segment_dropped)
                        .saturating_add(account.segments.unattributed)
                );
                prop_assert_eq!(account.segments.unattributed, 0);
                prop_assert_eq!(
                    account
                        .primitives
                        .marks
                        .get(&PartialMark::SegmentsDropped)
                        .copied()
                        .unwrap_or(0),
                    account.primitives.partial,
                    "every partial primitive here is partial for its segments"
                );
                Ok(())
            })
            .expect("the property holds over the generated writes");
    }

    // ------------------------------------------------------------- unit tests

    #[test]
    fn past_the_edge_uses_half_mm_not_the_gates_tenth() {
        // Two primitives: one 0.3 mm past the right edge, one 0.7 mm past.
        // At the ledger's 0.5 mm both are inside the page except the second;
        // at the gate's 0.1 mm slack both would count. The two constants are
        // different questions and the mark follows the ledger's.
        let mut ledger = LayoutLedger::default();
        ledger.entities.push(an_entity(None));
        ledger.entity_totals.drawing = 1;
        ledger.primitive_origin = vec![0, 0];
        let primitives = vec![
            a_line([800.0, 10.0], [841.3, 10.0]),
            a_line([800.0, 20.0], [841.7, 20.0]),
        ];
        let model = a_model(ledger, primitives, Some([841.0, 594.0]));
        assert_eq!(
            model.primitives_past_the_edge(0.1).len(),
            2,
            "the gate's slack counts both"
        );
        assert_eq!(
            model.primitives_past_the_edge(0.5).len(),
            1,
            "the ledger's slack counts only the one past half a millimetre"
        );
        let fates = vec![
            a_fate(PrimitiveOutcome::OnPage),
            a_fate(PrimitiveOutcome::OnPage),
        ];
        let assembled = ExportLedger::assemble(&model, &a_write_ledger(fates));
        assert_eq!(
            assembled
                .drawing
                .primitives
                .marks
                .get(&PartialMark::PastTheEdge)
                .copied(),
            Some(1)
        );
        assert!(assembled.cross_check(&model, None, None).is_empty());
    }

    #[test]
    fn a_dropped_segment_marks_the_primitive_partial() {
        let mut ledger = LayoutLedger::default();
        ledger.entities.push(an_entity(None));
        ledger.entity_totals.drawing = 1;
        ledger.primitive_origin = vec![0];
        let model = a_model(
            ledger,
            vec![a_line([10.0, 10.0], [20.0, 10.0])],
            Some([841.0, 594.0]),
        );
        let fate = PrimitiveFate {
            outcome: PrimitiveOutcome::OnPage,
            segments: SegmentTally {
                total: 3,
                written: 2,
                non_finite: 0,
                too_short: 0,
                on_sheet_edge: 1,
            },
            segments_dropped: true,
            text: None,
        };
        let assembled = ExportLedger::assemble(&model, &a_write_ledger(vec![fate]));
        assert_eq!(assembled.drawing.primitives.partial, 1);
        assert_eq!(
            assembled
                .drawing
                .primitives
                .marks
                .get(&PartialMark::SegmentsDropped)
                .copied(),
            Some(1)
        );
        assert_eq!(
            assembled
                .drawing
                .segments
                .dropped
                .get(&DropReason::SegmentOnSheetEdge)
                .copied(),
            Some(1)
        );
        assert_eq!(assembled.drawing.segments.total, 3);
        assert_eq!(assembled.drawing.segments.written, 2);
        assert!(assembled.conservation_breaches().is_empty());
    }

    #[test]
    fn an_entity_with_a_dropped_segment_is_partial() {
        // A.6's fourth row at the entity layer, smallest form: one entity, one
        // primitive, drawn, but the writer dropped one of its strokes. The
        // entity is on the page (its primitive reached it) and partial (that
        // primitive carries `SegmentsDropped`).
        //
        // Before this test the entity layer read `partial 0` here — the
        // primitive layer said `partial 1, marks {SegmentsDropped: 1}` — and
        // deleting the `fate.segments_dropped` half of `entity_outcome` left
        // the whole suite green, because the property generator never set it.
        let mut ledger = LayoutLedger::default();
        ledger.entities.push(an_entity(None));
        ledger.entity_totals.drawing = 1;
        ledger.primitive_origin = vec![0];
        let model = a_model(
            ledger,
            vec![a_line([10.0, 10.0], [20.0, 10.0])],
            Some([841.0, 594.0]),
        );
        let fate = PrimitiveFate {
            outcome: PrimitiveOutcome::OnPage,
            segments: SegmentTally {
                total: 2,
                written: 1,
                non_finite: 0,
                too_short: 0,
                on_sheet_edge: 1,
            },
            segments_dropped: true,
            text: None,
        };
        let assembled = ExportLedger::assemble(&model, &a_write_ledger(vec![fate]));
        assert_eq!(assembled.drawing.entities.on_page, 1);
        assert_eq!(
            assembled.drawing.entities.partial, 1,
            "a drawn primitive carrying A.6's SegmentsDropped mark makes the entity partial"
        );
        assert_eq!(
            assembled
                .drawing
                .primitives
                .marks
                .get(&PartialMark::SegmentsDropped)
                .copied(),
            Some(1),
            "the primitive layer agrees"
        );
        assert!(assembled.conservation_breaches().is_empty());
    }

    #[test]
    fn an_entity_whose_second_primitive_was_dropped_is_partial() {
        // Design deviation I2's second clause, at its smallest: one entity, two
        // primitives, the first drawn and the second refused by the writer. The
        // entity is on the page (one primitive reached it) and partial (one did
        // not). Before the fix this read `on_page 1 / partial 0`.
        let mut ledger = LayoutLedger::default();
        ledger.entities.push(an_entity(None));
        ledger.entity_totals.drawing = 1;
        ledger.primitive_origin = vec![0, 0];
        let model = a_model(
            ledger,
            vec![
                a_line([10.0, 10.0], [20.0, 10.0]),
                a_line([30.0, 10.0], [40.0, 10.0]),
            ],
            Some([841.0, 594.0]),
        );
        let fates = vec![
            a_fate(PrimitiveOutcome::OnPage),
            a_fate(PrimitiveOutcome::Dropped(DropReason::ChainEmpty)),
        ];
        let assembled = ExportLedger::assemble(&model, &a_write_ledger(fates));
        assert_eq!(assembled.drawing.entities.on_page, 1);
        assert_eq!(
            assembled.drawing.entities.partial, 1,
            "one primitive reached the page and one did not"
        );
        // The entity's outcome is still `on_page`: the drop is not its fate.
        assert!(
            !assembled
                .drawing
                .entities
                .dropped
                .contains_key(&DropReason::ChainEmpty)
        );
        assert!(assembled.conservation_breaches().is_empty());
    }

    #[test]
    fn an_entity_whose_second_primitive_left_the_sheet_is_partial() {
        // The same clause's other half, which already held: a primitive the
        // final `retain` removed is one that did not reach the page either.
        let mut ledger = LayoutLedger::default();
        ledger.entities.push(an_entity(None));
        ledger.entity_totals.drawing = 1;
        // One survivor on the page, one removed by the retain.
        ledger.primitive_origin = vec![0];
        ledger.off_sheet = vec![0];
        let model = a_model(
            ledger,
            vec![a_line([10.0, 10.0], [20.0, 10.0])],
            Some([841.0, 594.0]),
        );
        let assembled = ExportLedger::assemble(
            &model,
            &a_write_ledger(vec![a_fate(PrimitiveOutcome::OnPage)]),
        );
        assert_eq!(assembled.drawing.entities.on_page, 1);
        assert_eq!(assembled.drawing.entities.partial, 1);
        assert!(assembled.conservation_breaches().is_empty());
    }

    #[test]
    fn an_entity_whose_primitive_went_past_the_edge_is_partial() {
        // I2's first clause, smallest form: one entity, two primitives, both
        // drawn, the second reaching 0.9 mm past the 841 mm edge. That
        // primitive carries A.6's `PastTheEdge` mark, so the entity reached the
        // page incompletely and must count as partial. Before the fix this read
        // `on_page 1 / partial 0` against a primitive layer that already said
        // `partial 1, marks {PastTheEdge: 1}` — and no gate reported it.
        let mut ledger = LayoutLedger::default();
        ledger.entities.push(an_entity(None));
        ledger.entity_totals.drawing = 1;
        ledger.primitive_origin = vec![0, 0];
        let model = a_model(
            ledger,
            vec![
                a_line([10.0, 10.0], [20.0, 10.0]),
                a_line([800.0, 20.0], [841.9, 20.0]),
            ],
            Some([841.0, 594.0]),
        );
        let fates = vec![
            a_fate(PrimitiveOutcome::OnPage),
            a_fate(PrimitiveOutcome::OnPage),
        ];
        let assembled = ExportLedger::assemble(&model, &a_write_ledger(fates));

        // The primitive layer said so first; the entity layer must agree.
        assert_eq!(
            assembled
                .drawing
                .primitives
                .marks
                .get(&PartialMark::PastTheEdge)
                .copied(),
            Some(1),
            "one primitive ran past the sheet by more than the 0.5 mm slack"
        );
        assert_eq!(assembled.drawing.primitives.partial, 1);
        assert_eq!(assembled.drawing.entities.on_page, 1);
        assert_eq!(
            assembled.drawing.entities.partial, 1,
            "a drawn primitive carrying an A.6 mark makes the entity partial"
        );
        // Neither the conservation check nor the cross-check could see this.
        assert!(assembled.conservation_breaches().is_empty());
        assert!(assembled.cross_check(&model, None, None).is_empty());
    }

    #[test]
    fn an_entity_whose_every_primitive_dropped_is_not_partial() {
        // `partial` counts entities *on the page* that reached it incompletely.
        // An entity none of whose primitives reached the page has a drop
        // reason instead, and must not be counted twice.
        let mut ledger = LayoutLedger::default();
        ledger.entities.push(an_entity(None));
        ledger.entity_totals.drawing = 1;
        ledger.primitive_origin = vec![0, 0];
        let model = a_model(
            ledger,
            vec![
                a_line([10.0, 10.0], [20.0, 10.0]),
                a_line([30.0, 10.0], [40.0, 10.0]),
            ],
            Some([841.0, 594.0]),
        );
        let fates = vec![
            a_fate(PrimitiveOutcome::Dropped(DropReason::ChainEmpty)),
            a_fate(PrimitiveOutcome::Dropped(DropReason::PenWidthInvalid)),
        ];
        let assembled = ExportLedger::assemble(&model, &a_write_ledger(fates));
        assert_eq!(assembled.drawing.entities.on_page, 0);
        assert_eq!(assembled.drawing.entities.partial, 0);
        assert_eq!(
            assembled
                .drawing
                .entities
                .dropped
                .get(&DropReason::PenWidthInvalid)
                .copied(),
            Some(1),
            "the appendix's first reason among the entity's primitives: pen \
             before chain, because that is A.5's row order"
        );
    }

    #[test]
    fn both_domains_serialize_every_remainder_even_at_zero() {
        let model = a_model(LayoutLedger::default(), Vec::new(), Some([841.0, 594.0]));
        let assembled = ExportLedger::assemble(&model, &a_write_ledger(Vec::new()));
        let json = serde_json::to_string(&assembled).expect("it serializes");
        assert_eq!(
            json.matches("\"unattributed\"").count(),
            10,
            "five layers in each of the two domains: {json}"
        );
        assert!(json.contains("\"unattributed\":0"), "{json}");
        // Requirement 12.8's round trip.
        let back: ExportLedger = serde_json::from_str(&json).expect("and reads back");
        assert_eq!(back, assembled);
    }

    #[test]
    fn the_summary_line_names_all_five_numbers() {
        let mut ledger = LayoutLedger::default();
        ledger.entities.push(an_entity(None));
        ledger
            .entities
            .push(an_entity(Some(DropReason::LayerHidden)));
        ledger
            .entities
            .push(an_entity(Some(DropReason::SymbolMissing)));
        ledger.entity_totals.drawing = 3;
        ledger.primitive_origin = vec![0];
        let model = a_model(
            ledger,
            vec![a_line([10.0, 10.0], [20.0, 10.0])],
            Some([841.0, 594.0]),
        );
        let assembled = ExportLedger::assemble(
            &model,
            &a_write_ledger(vec![a_fate(PrimitiveOutcome::OnPage)]),
        );
        let summary = assembled.summary();
        assert_eq!(summary.entities, 3);
        assert_eq!(summary.on_page, 1);
        assert_eq!(summary.intentional, 1, "layer_hidden");
        assert_eq!(summary.suspect, 1, "symbol_missing");
        assert_eq!(summary.unattributed, 0);
        assert_eq!(
            summary.to_string(),
            "ledger entities 3 on_page 1 intentional 1 suspect 1 unattributed 0"
        );
    }

    #[test]
    fn cross_check_reports_every_mismatch_it_finds() {
        let mut ledger = LayoutLedger::default();
        ledger
            .entities
            .push(an_entity(Some(DropReason::EndpointPairGuess)));
        ledger.entity_totals.drawing = 1;
        let mut model = a_model(ledger, Vec::new(), Some([841.0, 594.0]));
        // The model's own count has to agree with the ledger before any check
        // can be called clean.
        model.skipped_endpoint_pairs = 1;
        let assembled = ExportLedger::assemble(&model, &a_write_ledger(Vec::new()));
        assert!(
            assembled.cross_check(&model, None, None).is_empty(),
            "the page agrees with the ledger as built"
        );

        // 8.1: the model claims a skipped pair the ledger never counted.
        model.skipped_endpoint_pairs = 2;
        let mismatches = assembled.cross_check(&model, None, None);
        assert_eq!(mismatches.len(), 1, "{mismatches:?}");
        assert!(mismatches[0].starts_with("8.1"), "{mismatches:?}");

        // 8.2: an off-sheet primitive the ledger did not see.
        model.skipped_endpoint_pairs = 1;
        model.off_sheet_primitives = 4;
        let mismatches = assembled.cross_check(&model, None, None);
        assert_eq!(mismatches.len(), 1, "{mismatches:?}");
        assert!(mismatches[0].starts_with("8.2"), "{mismatches:?}");

        // 8.4: a template that contributed primitives the ledger has no record
        // of. A template with no furniture is exactly that.
        model.off_sheet_primitives = 0;
        model.template = Some(crate::model::BorderTemplate {
            path: "CPE-SQ-A1.igr".to_string(),
            extent_mm: [841.0, 594.0],
            offset_mm: [0.0, 0.0],
            scale: 1.0,
            primitives: 9,
            symbols: Vec::new(),
        });
        let mismatches = assembled.cross_check(&model, None, None);
        assert_eq!(mismatches.len(), 1, "{mismatches:?}");
        assert!(mismatches[0].starts_with("8.4"), "{mismatches:?}");

        // 8.6: the audit says a run was condensed and the ledger never marked
        // one. The audit is the writer's own account, so a mismatch is a bug.
        model.template = None;
        let audit = PlotAudit {
            runs_condensed: 3,
            ..PlotAudit::default()
        };
        let mismatches = assembled.cross_check(&model, Some(&audit), None);
        assert_eq!(mismatches.len(), 1, "{mismatches:?}");
        assert!(mismatches[0].starts_with("8.6"), "{mismatches:?}");

        // 8.7: the page has ink past the edge and the ledger never marked it.
        model.primitives = vec![a_line([10.0, 10.0], [841.9, 10.0])];
        let mismatches = assembled.cross_check(&model, None, None);
        assert_eq!(mismatches.len(), 1, "{mismatches:?}");
        assert!(mismatches[0].starts_with("8.7"), "{mismatches:?}");
    }

    #[test]
    fn cross_check_8_3_counts_the_symbol_missing_entities_against_the_boxes() {
        // 8.3 is the one cross-check whose two sides come from different
        // producers: the ledger counts entities the layout layer refused as
        // `symbol_missing`, the model carries one 30 mm box per symbol the
        // library did not draw. They agree only when every refusal left a box.
        let mut ledger = LayoutLedger::default();
        ledger
            .entities
            .push(an_entity(Some(DropReason::SymbolMissing)));
        ledger.entity_totals.drawing = 1;
        let mut model = a_model(ledger, Vec::new(), Some([841.0, 594.0]));
        model.missing_boxes = vec![crate::model::MissingSymbolBox {
            path: r"piping\valve.sym".to_string(),
            min_mm: [10.0, 10.0],
            max_mm: [40.0, 40.0],
        }];
        let assembled = ExportLedger::assemble(&model, &a_write_ledger(Vec::new()));
        assert!(
            assembled.cross_check(&model, None, None).is_empty(),
            "one refusal, one box: the check agrees"
        );

        // A refusal with no box: the ledger counted a symbol the page never
        // marked, so a reader looking at the page cannot find it.
        model.missing_boxes.clear();
        let mismatches = assembled.cross_check(&model, None, None);
        assert_eq!(mismatches.len(), 1, "{mismatches:?}");
        assert!(mismatches[0].starts_with("8.3"), "{mismatches:?}");
        assert!(
            mismatches[0].contains("symbol_missing 1") && mismatches[0].contains("missing_boxes 0"),
            "the mismatch names both sides: {mismatches:?}"
        );

        // And the other way round: a box for a symbol the ledger never refused
        // as missing.
        model.missing_boxes = vec![
            crate::model::MissingSymbolBox {
                path: r"piping\valve.sym".to_string(),
                min_mm: [10.0, 10.0],
                max_mm: [40.0, 40.0],
            },
            crate::model::MissingSymbolBox {
                path: r"piping\pump.sym".to_string(),
                min_mm: [50.0, 10.0],
                max_mm: [80.0, 40.0],
            },
        ];
        let mismatches = assembled.cross_check(&model, None, None);
        assert_eq!(mismatches.len(), 1, "{mismatches:?}");
        assert!(mismatches[0].starts_with("8.3"), "{mismatches:?}");
        assert!(
            mismatches[0].contains("symbol_missing 1") && mismatches[0].contains("missing_boxes 2"),
            "the mismatch names both sides: {mismatches:?}"
        );
    }

    #[test]
    fn cross_check_8_5_compares_the_three_census_tables() {
        let mut ledger = LayoutLedger::default();
        ledger
            .parse
            .drawing
            .items
            .push(super::super::layout::ParseRecord {
                reason: DropReason::RecordRefused,
                stream: "/Sheet6".to_string(),
                type_code: Some(0x0019),
                family: None,
                count: 5,
                bytes: None,
                body_primitives: None,
            });
        let model = a_model(ledger, Vec::new(), Some([841.0, 594.0]));
        let assembled = ExportLedger::assemble(&model, &a_write_ledger(Vec::new()));

        let mut geometry = pid_parse::NormalizedPidGeometry {
            refused_graphic_records: vec![pid_parse::PidRefusedGraphicRecords {
                stream_path: "/Sheet6".to_string(),
                type_code: 0x0019,
                rad_class_name: None,
                count: 5,
            }],
            ..pid_parse::NormalizedPidGeometry::default()
        };
        assert!(
            assembled
                .cross_check(&model, None, Some(&geometry))
                .is_empty(),
            "the table and the ledger agree"
        );

        geometry.refused_graphic_records[0].count = 6;
        let mismatches = assembled.cross_check(&model, None, Some(&geometry));
        assert_eq!(mismatches.len(), 1, "{mismatches:?}");
        assert!(mismatches[0].starts_with("8.5"), "{mismatches:?}");
        assert!(
            mismatches[0].contains("0x0019"),
            "the mismatch names the type code: {mismatches:?}"
        );
    }

    #[test]
    fn a_page_that_names_a_template_it_does_not_wear_reports_it() {
        let mut model = a_model(LayoutLedger::default(), Vec::new(), Some([841.0, 594.0]));
        model.missing_templates = vec!["CPE-SQ-A1".to_string()];
        let assembled = ExportLedger::assemble(&model, &a_write_ledger(Vec::new()));
        assert_eq!(
            assembled
                .page
                .dropped
                .get(&DropReason::TemplateMissing)
                .copied(),
            Some(1)
        );
        assert_eq!(assembled.page.suspect_sites.len(), 1);
        assert_eq!(
            assembled.page.suspect_sites[0].name.as_deref(),
            Some("CPE-SQ-A1")
        );
        assert_eq!(assembled.summary().suspect, 1);
    }

    #[test]
    fn an_unreadable_template_candidate_is_a_page_level_suspect_drop() {
        let mut ledger = LayoutLedger::default();
        ledger.template_unreadable.push(SourceName {
            kind: super::super::layout::SourceKind::DirectoryFile,
            name: "CPE-SQ-A1.igr".to_string(),
            archive: None,
        });
        let model = a_model(ledger, Vec::new(), Some([841.0, 594.0]));
        let assembled = ExportLedger::assemble(&model, &a_write_ledger(Vec::new()));
        assert_eq!(
            assembled
                .page
                .dropped
                .get(&DropReason::TemplateCandidateUnreadable)
                .copied(),
            Some(1)
        );
        assert_eq!(assembled.summary().suspect, 1);
    }

    #[test]
    fn a_template_read_from_an_archive_is_named_by_its_member() {
        let fact = template_fact(&crate::model::BorderTemplate {
            path: r"D:\corpus\RefData~4~682.zip/CPE-SQ-A1.igr".to_string(),
            extent_mm: [841.0, 594.0],
            offset_mm: [0.0, 0.0],
            scale: 1.0,
            primitives: 3,
            symbols: Vec::new(),
        });
        assert_eq!(fact.name, "CPE-SQ-A1.igr");
        assert_eq!(fact.source, TemplateSourceKind::ArchiveMember);
        assert_eq!(fact.archive.as_deref(), Some("RefData~4~682.zip"));
        assert!(
            !fact.name.contains('\\') && !fact.name.contains("D:"),
            "a baseline stores names, never paths"
        );
    }

    #[test]
    fn a_page_size_is_declared_measured_or_absent() {
        assert_eq!(
            page_size_source(&a_model(
                LayoutLedger::default(),
                Vec::new(),
                Some([10.0, 10.0])
            )),
            PageSizeSource::Declared
        );
        assert_eq!(
            page_size_source(&a_model(
                LayoutLedger::default(),
                vec![a_line([10.0, 10.0], [20.0, 20.0])],
                None
            )),
            PageSizeSource::Measured
        );
        assert_eq!(
            page_size_source(&a_model(LayoutLedger::default(), Vec::new(), None)),
            PageSizeSource::None
        );
    }

    #[test]
    fn a_text_mark_from_the_writer_reaches_the_primitive_layer() {
        let mut ledger = LayoutLedger::default();
        ledger.entities.push(an_entity(None));
        ledger.entity_totals.drawing = 1;
        ledger.primitive_origin = vec![0];
        let model = a_model(
            ledger,
            vec![a_line([10.0, 10.0], [20.0, 10.0])],
            Some([841.0, 594.0]),
        );
        let fate = PrimitiveFate {
            outcome: PrimitiveOutcome::OnPage,
            segments: SegmentTally::default(),
            segments_dropped: false,
            text: Some(TextMarks {
                condensed: true,
                clipped_at_floor: true,
            }),
        };
        let assembled = ExportLedger::assemble(&model, &a_write_ledger(vec![fate]));
        assert_eq!(
            assembled
                .drawing
                .primitives
                .marks
                .get(&PartialMark::TextCondensed)
                .copied(),
            Some(1)
        );
        assert_eq!(
            assembled
                .drawing
                .primitives
                .marks
                .get(&PartialMark::TextClippedAtFloor)
                .copied(),
            Some(1)
        );
        assert_eq!(assembled.drawing.primitives.partial, 1);
    }

    #[test]
    fn a_ledger_without_faces_carries_neither_text_mark() {
        let mut ledger = LayoutLedger::default();
        ledger.entities.push(an_entity(None));
        ledger.entity_totals.drawing = 1;
        ledger.primitive_origin = vec![0];
        let model = a_model(
            ledger,
            vec![a_line([10.0, 10.0], [20.0, 10.0])],
            Some([841.0, 594.0]),
        );
        let assembled = ExportLedger::assemble(
            &model,
            &a_write_ledger(vec![a_fate(PrimitiveOutcome::OnPage)]),
        );
        assert!(!assembled.fonts_measured);
        assert!(
            !assembled
                .drawing
                .primitives
                .marks
                .contains_key(&PartialMark::TextCondensed)
        );
        assert!(
            !assembled
                .drawing
                .primitives
                .marks
                .contains_key(&PartialMark::TextClippedAtFloor)
        );
    }

    #[test]
    fn a_breach_names_the_domain_and_the_layer() {
        let mut ledger = LayoutLedger::default();
        ledger.entities.push(an_entity(None));
        ledger.entity_totals.drawing = 3;
        ledger.primitive_origin = vec![0];
        let model = a_model(
            ledger,
            vec![a_line([10.0, 10.0], [20.0, 10.0])],
            Some([841.0, 594.0]),
        );
        let assembled = ExportLedger::assemble(
            &model,
            &a_write_ledger(vec![a_fate(PrimitiveOutcome::OnPage)]),
        );
        let breaches = assembled.conservation_breaches();
        assert_eq!(breaches.len(), 1, "{breaches:?}");
        assert!(breaches[0].contains("drawing"), "{breaches:?}");
        assert!(breaches[0].contains("entity layer"), "{breaches:?}");
        assert!(
            breaches[0].contains('2'),
            "two entities are unaccounted: {breaches:?}"
        );
    }

    #[test]
    fn the_drawing_domain_does_not_absorb_a_furniture_removal() {
        // The `retain` list is page-wide. A furniture primitive removed there
        // must land in the furniture account only, or the drawing domain's
        // primitive equation would count an off-sheet drop it never had.
        let mut ledger = LayoutLedger::default();
        let mut furniture_entity = an_entity(None);
        furniture_entity.domain = LedgerDomain::Furniture;
        ledger.entities.push(an_entity(None));
        ledger.entities.push(furniture_entity);
        ledger.entity_totals.drawing = 1;
        ledger.entity_totals.furniture = 1;
        ledger.primitive_origin = vec![0, 1];
        ledger.off_sheet = vec![1];
        let model = a_model(
            ledger,
            vec![
                a_line([10.0, 10.0], [20.0, 10.0]),
                a_line([30.0, 10.0], [40.0, 10.0]),
            ],
            Some([841.0, 594.0]),
        );
        let fates = vec![
            a_fate(PrimitiveOutcome::OnPage),
            a_fate(PrimitiveOutcome::OnPage),
        ];
        let assembled = ExportLedger::assemble(&model, &a_write_ledger(fates));
        assert_eq!(assembled.drawing.primitives.off_sheet, 0);
        assert_eq!(assembled.furniture.primitives.off_sheet, 1);
        assert_eq!(assembled.furniture.primitives.laid_out, 1);
        assert_eq!(
            assembled
                .furniture
                .primitives
                .dropped
                .get(&DropReason::OffSheet)
                .copied(),
            Some(1)
        );
        assert!(assembled.conservation_breaches().is_empty());
    }
}
