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

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::layout::{Confidence, EntityKind, SourceName, TextNormalization};
use super::reason::{Asymmetry, DropReason, PartialMark, PlacementSource};

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

    #[test]
    fn the_ledger_carries_no_floating_point_field() {
        // Requirement 12.2: counts are integers. A float in this module would
        // make a baseline compare platform-dependent, which is the one thing
        // the ledger must never be. The check is textual because the property
        // is about the type declarations themselves.
        let source = include_str!("assemble.rs");
        let declarations = source
            .lines()
            .filter(|line| line.trim_start().starts_with("pub "))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !declarations.contains("f64") && !declarations.contains("f32"),
            "no public field of the assembled ledger may be a float"
        );
    }
}
