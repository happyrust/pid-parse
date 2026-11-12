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
    /// The chain streams read only as far as their record chain holds, in
    /// the order the parse passes met them. Each is also one
    /// [`DropReason::SheetStreamWithoutGeometry`] item of [`Self::parse`];
    /// this is the whole of what the parser said about it, kept so that a
    /// caller can say it too ([`Self::chain_fault_warnings`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chain_faults: Vec<ChainFaultRecord>,
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
    /// The order the page is painted in: an index into
    /// `PaperModel::primitives` for every primitive, the first painted
    /// first.
    ///
    /// The model keeps its primitives in the order the layout visits the
    /// entities, which is the order of the record families, not of the
    /// sheet. What a sheet draws over what is the order its `JSheet` record
    /// lists its objects in (`pid_parse::parsers::sheet_display_order`), and
    /// this is that order carried onto the page: the border template where
    /// the sheet lists its frame, each record of the drawing where the sheet
    /// lists it or its owner. Empty when no sheet of the page states an
    /// order; the page is then painted in the model's.
    ///
    /// It rides here, beside the model rather than in it, so that the
    /// model's own JSON is what it was: see
    /// [`PaperModel::paint_sequence`](crate::model::PaperModel::paint_sequence).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paint_order: Vec<u32>,
    /// The records that draw on the page and take no place in the order
    /// its sheets draw in, by domain, stream and reason. Each stream of them
    /// is also one [`DropReason::RecordWithoutDisplayPlace`] item of
    /// [`Self::parse`]; this is what was found, kept so that a caller can
    /// say it ([`Self::display_order_warnings`]). Empty on every page of
    /// every corpus this crate is tested on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unplaced: Vec<UnplacedRecords>,
    /// The symbol files a placement of the page is drawn from that have a
    /// chain stream whose framing does not hold: the placement is drawn
    /// from whatever was read of such a file. Each is also one
    /// [`DropReason::SymbolFileChainFault`] item of [`Self::parse`]; this
    /// is what was found, kept so that a caller can say it
    /// ([`Self::symbol_chain_fault_warnings`]). Empty on every page of
    /// every corpus this crate is tested on, none of which draws a
    /// placement from a library body.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub symbol_chain_faults: Vec<SymbolChainFault>,
    /// The paper the drawing's sheet states, when it was read and not taken
    /// for the page because it is not the border frame's
    /// (`model::paper_is_the_frames`). The page is then the frame's
    /// rectangle; the page account files one suspect
    /// [`DropReason::PaperUnlikeTheFrame`] and `pid_plot` says it
    /// ([`Self::page_warnings`]). Only for a drawing a frame of which gave a
    /// page. `None` on every page of every corpus this crate is tested on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paper_refused: Option<PaperRefused>,
    /// The paper the drawing's sheet states and the size its template's
    /// name states, when no frame gave a page and the two are not one sheet
    /// (`model::paper_is_the_named_sheet`). The paper is the page all the
    /// same — a name overrules no reading — and the page account files one
    /// suspect [`DropReason::PaperUnlikeTheTemplateName`] and `pid_plot`
    /// says it ([`Self::page_warnings`]). `None` on every page of every
    /// corpus this crate is tested on, none of which states a paper beside
    /// a name alone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paper_unlike_the_name: Option<PaperBesideAName>,
    /// What the page is, and where that was read, when it is not a paper
    /// because none was read: the sheet's `JSheet` record, which states the
    /// paper behind its list of objects, was not found, is not the sheet's,
    /// or had no record of the sheet's to be held to. A sheet whose list is
    /// not read says so already ([`Self::display_order_warnings`]), and
    /// that sentence says this too.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_for_want_of_a_paper: Option<PageInstead>,
}

/// The rectangle a drawing has for its sheet beside the paper: what a page
/// is when no paper is taken, and what a paper is held to.
///
/// The two are not the same kind of thing, and a sentence about a page has
/// to say which it had. A border frame states its rectangle to the
/// nanometre, and every drawing on hand that states both keeps its paper
/// within `model::PAPER_FRAME_REACH_MM` of it. A template's name states a
/// size the way a label does — `A2` is 594 by 420 — and is what the parser
/// falls back to for a drawing no frame of which gives a page
/// (`pid_parse::framed_page_dimensions_mm`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SheetRectangle {
    /// The rectangle the drawing's border frame states.
    Frame,
    /// The size the name of the drawing's template stands for: no frame
    /// gave a page — the drawing carries none, its frames disagree, or its
    /// stream was not read as far as one.
    TemplateName,
}

impl SheetRectangle {
    /// The rectangle as a sentence names it: `the border frame's rectangle`
    /// or `the size its template's name states`.
    #[must_use]
    pub fn named(self) -> &'static str {
        match self {
            Self::Frame => "the border frame's rectangle",
            Self::TemplateName => "the size its template's name states",
        }
    }
}

/// The page of a sheet no paper was read for: which rectangle it is, and
/// its `[width, height]` in micrometres.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageInstead {
    /// Where the rectangle was read.
    pub from: SheetRectangle,
    /// Its sides.
    pub um: [u64; 2],
}

/// A paper that was read and refused: the paper and the border frame's
/// rectangle, each `[width, height]` in micrometres.
///
/// Whole micrometres rather than millimetres so that the ledger stays
/// comparable for equality; a side that is not a positive finite length is
/// kept as zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaperRefused {
    /// The paper the sheet's record states.
    pub paper_um: [u64; 2],
    /// The rectangle the border frame states.
    pub frame_um: [u64; 2],
}

/// A paper that is the page beside a template's name that states another
/// size: the two, each `[width, height]` in micrometres.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaperBesideAName {
    /// The paper the sheet's record states, which is the page.
    pub paper_um: [u64; 2],
    /// The size the template's name stands for.
    pub named_um: [u64; 2],
}

/// A length in millimetres as whole micrometres; zero for one that is not
/// a positive finite length.
#[must_use]
pub fn micrometres(millimetres: f64) -> u64 {
    if millimetres.is_finite() && millimetres > 0.0 {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        {
            (millimetres * 1000.0).round() as u64
        }
    } else {
        0
    }
}

/// `[width, height]` in micrometres, written `W x H mm` to the micrometre.
fn sides_mm(sides_um: [u64; 2]) -> String {
    #[allow(clippy::cast_precision_loss)]
    let millimetres = |side: u64| side as f64 / 1000.0;
    format!(
        "{:.3} x {:.3} mm",
        millimetres(sides_um[0]),
        millimetres(sides_um[1])
    )
}

impl PaperRefused {
    /// The two rectangles, for the page account's suspect site and the
    /// warning: `paper 297.000 x 420.000 mm, frame 594.328 x 420.566 mm`.
    #[must_use]
    pub fn sides(&self) -> String {
        format!(
            "paper {}, frame {}",
            sides_mm(self.paper_um),
            sides_mm(self.frame_um)
        )
    }
}

impl PaperBesideAName {
    /// The two sizes, for the page account's suspect site: `paper 297.000 x
    /// 420.000 mm, template name's size 594.000 x 420.000 mm`.
    #[must_use]
    pub fn sides(&self) -> String {
        format!(
            "paper {}, template name's size {}",
            sides_mm(self.paper_um),
            sides_mm(self.named_um)
        )
    }
}

/// Why a record that draws on a page takes no place in the order its sheet
/// draws in (`pid_parse::parsers::sheet_display_order`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnplacedWhy {
    /// The sheet the record is in states no list of its objects that could
    /// be read: no `JSheet` record of its storage's cluster names the
    /// stream's records, or the stream is one nothing reads a list for.
    NoList,
    /// The stream holds no record of the sheet's own, as far as it was
    /// read, so the list its sheet states — which is there — has nothing to
    /// be held to (`NoDisplayOrder::NoRecordOfTheSheet` of
    /// `pid_parse::parsers::sheet_display_order`). The records that draw
    /// hang under objects of the sheet's the stream does not hold.
    ///
    /// A chain that stops ahead of the sheet's first own record leaves a
    /// stream like that, and the stream's chain fault is then the word on
    /// it: [`LayoutLedger::display_order_warnings`] starts no sentence of
    /// its own for records a stopped chain already accounts for.
    NoRecordOfTheSheet,
    /// The sheet's list was read and does not reach the record: nothing it
    /// hangs under is listed.
    NotReached,
    /// The record is in a sheet nested in the drawing, which has a list of
    /// its own, and no frame the drawing lists was found to embed it.
    NoFrame,
    /// The primitive names no record.
    NoRecord,
}

/// The records of one stream that draw on the page and that no sheet's list
/// places, for one reason.
///
/// Such a record is painted all the same — after everything that has a
/// place, or, on a page no sheet of which states an order, strokes first and
/// lettering after — so the page lacks nothing; what it does not have is the
/// drawing's word on what covers what. The account says so as a suspect
/// item, and `pid_plot` on stderr.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnplacedRecords {
    /// The drawing's own, or the border template's.
    pub domain: LedgerDomain,
    /// The stream the records are in; empty when the primitives name none.
    pub stream: String,
    /// Why they take no place.
    pub why: UnplacedWhy,
    /// How many records.
    pub records: u64,
    /// How many primitives of the page they draw.
    pub primitives: u64,
}

/// One chain stream — a `Sheet*` stream or a symbol cache's `PSMcluster0` —
/// whose own framing does not hold, and the domain it was read for.
///
/// The parser reads such a stream as far as its record chain reads and no
/// further (`pid_parse::StreamChainFault`), so the page carries what the
/// records the chain read draw and nothing of what follows the stop. A page like
/// that is not a finished export of the drawing, and nothing on it says so:
/// the account does, as a suspect item, and `pid_plot` does, on stderr.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChainFaultRecord {
    /// The drawing's own stream, or the border template's.
    pub domain: LedgerDomain,
    /// What the parser found wrong with the stream.
    pub fault: pid_parse::StreamChainFault,
}

/// One chain stream of a symbol file whose framing does not hold, in a file
/// a placement of the page is drawn from, and the domain of that placement.
///
/// The placement is drawn from whatever the symbol reader took of the
/// file, and the reader takes a sheet only where its own walk of it ends
/// on the stream's last byte. On the committed pressure gauge, whose
/// `/Sheet6` is its normal view: a length of zero, stray bytes at the end
/// or a wrong magic leave that sheet out, the body is then the file's
/// other sheet, and the page draws the gauge's circle and a `PG` the sound
/// file does not letter; a type word with the `0x4000` flag or a header
/// count that is off is a fault that walk does not see, and the body is
/// the sound file's. The page cannot tell which: the account says the
/// file is damaged, as a suspect item, and `pid_plot` does, on stderr.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolChainFault {
    /// The drawing's own placement, or the border template's.
    pub domain: LedgerDomain,
    /// The symbol file, as the placement names it below the library's
    /// `Symbols` directory.
    pub symbol: String,
    /// How many placements of the page are drawn from the file.
    pub placements: u64,
    /// What the parser found wrong with the stream.
    pub fault: pid_parse::StreamChainFault,
}

impl LayoutLedger {
    /// File the symbol files with a chain stream that does not hold that
    /// `domain`'s placements are drawn from: one suspect
    /// [`DropReason::SymbolFileChainFault`] item a
    /// file and stream, with the bytes its chain left unread, and the
    /// finding itself in [`Self::symbol_chain_faults`]. `faults` is one
    /// entry a symbol file: its name, how many placements are drawn from
    /// it, and the faults its body carries.
    pub fn note_symbol_chain_faults(
        &mut self,
        domain: LedgerDomain,
        faults: impl IntoIterator<Item = (String, u64, Vec<pid_parse::StreamChainFault>)>,
    ) {
        for (symbol, placements, of_the_file) in faults {
            for fault in of_the_file {
                self.parse.get_mut(domain).push_record(
                    ParseRecord::new(
                        DropReason::SymbolFileChainFault,
                        format!("{symbol}:{}", fault.stream_path),
                        1,
                    )
                    .of_bytes(u64::try_from(fault.unread_bytes).unwrap_or(u64::MAX)),
                );
                self.symbol_chain_faults.push(SymbolChainFault {
                    domain,
                    symbol: symbol.clone(),
                    placements,
                    fault,
                });
            }
        }
    }

    /// One sentence per symbol file and stream whose chain does not hold
    /// and that a placement of the page is drawn from: the file, how many
    /// placements, and the reader's own sentence of the stream — its path,
    /// where its chain stops and what is left unread. One in the border
    /// template's placements says that it is the template's. Empty for a
    /// page whose symbol files are sound, and for one that draws every
    /// placement from the body its drawing caches — which is every page of
    /// every corpus this crate is tested on.
    #[must_use]
    pub fn symbol_chain_fault_warnings(&self) -> Vec<String> {
        self.symbol_chain_faults
            .iter()
            .map(|found| {
                let whose = match found.domain {
                    LedgerDomain::Drawing => "",
                    LedgerDomain::Furniture => "in the border template, ",
                };
                format!(
                    "{whose}the symbol file {}, which {} placement(s) on the page are drawn \
                     from, is damaged: {}",
                    found.symbol,
                    found.placements,
                    found.fault.describe()
                )
            })
            .collect()
    }

    /// File the records no sheet's list places: one suspect
    /// [`DropReason::RecordWithoutDisplayPlace`] item per domain and stream,
    /// counted in records, and the finding itself in [`Self::unplaced`].
    pub fn note_unplaced(&mut self, unplaced: Vec<UnplacedRecords>) {
        for item in &unplaced {
            self.parse
                .get_mut(item.domain)
                .push_record(ParseRecord::new(
                    DropReason::RecordWithoutDisplayPlace,
                    item.stream.clone(),
                    item.records,
                ));
        }
        self.unplaced = unplaced;
    }

    /// What is to be said of the page itself. One sentence when the paper
    /// the sheet states was refused for not being the border frame's, with
    /// the paper, the frame's rectangle and that the page is the frame's.
    /// One sentence when no frame was read and the paper is not the size
    /// the template's name states, with both sizes and that the page is the
    /// paper: a name refuses nothing. Empty for a page written on the paper
    /// its sheet states with nothing to say against it, which is every page
    /// of every corpus this crate is tested on.
    #[must_use]
    pub fn page_warnings(&self) -> Vec<String> {
        let limit_mm = crate::model::PAPER_FRAME_REACH_MM;
        let refused = self.paper_refused.iter().map(|refused| {
            format!(
                "the paper its sheet states, {}, is not its border frame's, {}: no \
                 drawing on hand has the two more than {limit_mm} mm apart on a side, so \
                 the paper is not taken for the page and the page is the frame's rectangle",
                sides_mm(refused.paper_um),
                sides_mm(refused.frame_um)
            )
        });
        let beside_a_name = self.paper_unlike_the_name.iter().map(|beside| {
            format!(
                "the paper its sheet states, {}, is not the size its template's name \
                 states, {}, and no border frame was read to say which the sheet is: a \
                 name is a label and the paper a reading, so the page is the paper",
                sides_mm(beside.paper_um),
                sides_mm(beside.named_um)
            )
        });
        refused.chain(beside_a_name).collect()
    }

    /// One sentence per stream and reason for the records that draw on the
    /// page and take no place in its display order: how many, how many
    /// primitives they draw, why, and where they are painted instead. One in
    /// the border template's file says that it is the template's. Empty for
    /// a page whose every record stands where a sheet lists it, which is
    /// every page of every corpus this crate is tested on.
    ///
    /// A drawing's own sheet whose list was not read has no paper either —
    /// the record that lists the objects states the paper behind them — so
    /// its sentence goes on to say what the page is instead, and where that
    /// was read ([`Self::page_for_want_of_a_paper`]).
    ///
    /// Records a stopped chain already accounts for get no sentence here:
    /// a stream that holds no record of the sheet's own
    /// ([`UnplacedWhy::NoRecordOfTheSheet`]) and whose chain is reported as
    /// stopping short ([`Self::chain_fault_warnings`]) has one cause, and
    /// that sentence names it. The account keeps both items. Any other
    /// reason is said beside a chain fault, being another matter; so is
    /// this one on a stream whose chain reads to its end.
    #[must_use]
    pub fn display_order_warnings(&self) -> Vec<String> {
        let painted = if self.paint_order.is_empty() {
            "no sheet of the page states an order, and it is painted strokes first and \
             lettering after"
        } else {
            "they are painted after everything that has a place"
        };
        self.unplaced
            .iter()
            .filter(|item| !self.a_stopped_chain_accounts_for(item))
            .map(|item| {
                let whose = match item.domain {
                    LedgerDomain::Drawing => "",
                    LedgerDomain::Furniture => "in the border template, ",
                };
                let of = if item.stream.is_empty() {
                    String::new()
                } else {
                    format!(" of {}", item.stream)
                };
                let why = match item.why {
                    UnplacedWhy::NoList => {
                        "the sheet states no list of its objects that could be read"
                    }
                    UnplacedWhy::NoRecordOfTheSheet => {
                        "the stream holds no record of the sheet's own, so the list its \
                         sheet states has nothing to be held to"
                    }
                    UnplacedWhy::NotReached => {
                        "the sheet's list of its objects does not reach them"
                    }
                    UnplacedWhy::NoFrame => {
                        "no frame the drawing lists was found to embed the sheet they are in"
                    }
                    UnplacedWhy::NoRecord => "they name no record",
                };
                // The drawing's own sheet: a stream of the document's root.
                let own_sheet = item.domain == LedgerDomain::Drawing
                    && matches!(
                        item.why,
                        UnplacedWhy::NoList | UnplacedWhy::NoRecordOfTheSheet
                    )
                    && item.stream.matches('/').count() == 1;
                let page = match self.page_for_want_of_a_paper {
                    Some(page) if own_sheet => format!(
                        "; the paper is stated in the same record and was not read either, so \
                         the page is {}, {}",
                        page.from.named(),
                        sides_mm(page.um)
                    ),
                    _ => String::new(),
                };
                format!(
                    "{whose}{} record(s){of}, {} primitive(s) on the page, take no place in \
                     the display order: {why}; {painted}{page}",
                    item.records, item.primitives
                )
            })
            .collect()
    }

    /// Whether a chain fault of the same stream is already the word on
    /// `item`: its records take no place because the stream holds no record
    /// of the sheet's own, and the stream's chain is one that stopped.
    fn a_stopped_chain_accounts_for(&self, item: &UnplacedRecords) -> bool {
        item.why == UnplacedWhy::NoRecordOfTheSheet
            && self.chain_faults.iter().any(|record| {
                record.domain == item.domain
                    && record.fault.stream_path == item.stream
                    && record.fault.stop.is_some()
            })
    }

    /// One sentence per chain stream that was not read whole, naming the
    /// stream, the offset its record walk stopped at and the bytes left
    /// unread, in the order the streams were met. A fault in the border
    /// template's file says that it is the template's. Empty for a page all
    /// of whose streams are sound, which is every page of every corpus this
    /// crate is tested on.
    ///
    /// Where the stop left the records it read without a place in the
    /// display order — none of them the sheet's own, so the sheet's list was
    /// not taken ([`UnplacedWhy::NoRecordOfTheSheet`]) — the sentence goes
    /// on to say that, and, for a drawing's own sheet, what the page is for
    /// want of the paper that list's record states
    /// ([`Self::page_for_want_of_a_paper`]). It is still one sentence:
    /// [`Self::display_order_warnings`] starts none for those records.
    #[must_use]
    pub fn chain_fault_warnings(&self) -> Vec<String> {
        self.chain_faults
            .iter()
            .map(|record| {
                let said = match record.domain {
                    LedgerDomain::Drawing => record.fault.describe(),
                    LedgerDomain::Furniture => {
                        format!("in the border template, {}", record.fault.describe())
                    }
                };
                match self.what_a_stop_left_without_a_place(record) {
                    Some(more) => format!("{said}{more}"),
                    None => said,
                }
            })
            .collect()
    }

    /// The rest of a stopped chain's sentence, where the stop is why records
    /// of its stream take no place: how many, and what the page is instead
    /// of the paper when the stream is the drawing's own sheet.
    fn what_a_stop_left_without_a_place(&self, record: &ChainFaultRecord) -> Option<String> {
        // Only a chain that stopped: one read to its end left nothing unread.
        record.fault.stop.as_ref()?;
        let item = self.unplaced.iter().find(|item| {
            item.why == UnplacedWhy::NoRecordOfTheSheet
                && item.domain == record.domain
                && item.stream == record.fault.stream_path
        })?;
        let own_sheet =
            item.domain == LedgerDomain::Drawing && item.stream.matches('/').count() == 1;
        let page = match self.page_for_want_of_a_paper {
            Some(page) if own_sheet => format!(
                ", and the paper stated with that list was not read either, so the page is \
                 {}, {}",
                page.from.named(),
                sides_mm(page.um)
            ),
            _ => String::new(),
        };
        Some(format!(
            "; none of the records read is the sheet's own, so the sheet's list has nothing \
             to be held to and {} record(s), {} primitive(s) on the page, take no place in \
             the display order{page}",
            item.records, item.primitives
        ))
    }
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
    /// The Sheet stream's path, or the storage path for a body record; for
    /// [`DropReason::SymbolFileChainFault`], the symbol file and the stream
    /// of it, `<file>:<stream>`.
    pub stream: String,
    /// The PSM type code, when the reason is per type code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_code: Option<u16>,
    /// The record family's registry name, when the reason is per family.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    /// How many records.
    pub count: u64,
    /// For [`DropReason::SheetStreamWithoutGeometry`]: the stream's byte
    /// size when the stream yielded nothing, and the bytes its record chain
    /// left unread when it was read up to a stop
    /// ([`LayoutLedger::chain_faults`] holds which, and where). For
    /// [`DropReason::SymbolFileChainFault`]: the bytes the symbol file's
    /// stream left unread ([`LayoutLedger::symbol_chain_faults`]).
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
        for fault in &geometry.chain_faults {
            self.ledger.chain_faults.push(ChainFaultRecord {
                domain,
                fault: fault.clone(),
            });
        }
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
            // A sheet stream is an 8-byte header — magic and record count —
            // and then its records. One no longer than the header holds no
            // record, so there is none it failed to turn into an entity and
            // no census it lost: it is an empty sheet, not a drop. The code
            // is for a stream that has records and yielded nothing.
            let header = u64::try_from(pid_parse::parsers::sheet_records::SHEET_STREAM_HEADER_LEN)
                .unwrap_or(u64::MAX);
            if stream.size <= header {
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
        // A chain stream whose framing does not hold was read as far as its
        // record chain reads: the records the chain read before the stop
        // are entities like any other, and whatever the bytes after it draw
        // is not on the page. Nothing above counts that — a census cannot
        // count records the walk never reached — so the stream itself is
        // the item, once, with the bytes left unread where a stream that
        // yielded nothing states its size. A stream already filed as
        // yielding nothing has said all of it. A header that disagrees with
        // a chain read to the end leaves no byte unread and is filed all
        // the same: the count it states is the only thing that says records
        // are missing, and the item's zero bytes say only that the walk
        // reached the end, not that nothing was lost on the way.
        for fault in &geometry.chain_faults {
            let filed = counts.items.iter().any(|item| {
                item.reason == DropReason::SheetStreamWithoutGeometry
                    && item.stream == fault.stream_path
            });
            if filed {
                continue;
            }
            counts.push_record(
                ParseRecord::new(
                    DropReason::SheetStreamWithoutGeometry,
                    fault.stream_path.clone(),
                    1,
                )
                .of_bytes(u64::try_from(fault.unread_bytes).unwrap_or(u64::MAX)),
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

    /// The stream's byte size, for `sheet_stream_without_geometry`; the
    /// bytes left unread, for it and for `symbol_file_chain_fault`.
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

    /// A sheet stream that is its 8-byte header alone holds no record, so it
    /// has dropped none: there is nothing for the three censuses to have
    /// lost. `N井场` has two such streams and `Q井场` one, each `magic +
    /// record count 0`, and `A01`'s embedded frame has four; they were seven
    /// of the sixty suspect items the two baselines carried. A stream one
    /// byte longer than its header can hold a record's first byte and is
    /// counted as before.
    #[test]
    fn a1_a_stream_that_is_its_header_alone_has_dropped_nothing() {
        let streams = vec![
            a_stream("/Sheet41276", 8, false),
            a_stream("/Sheet43032", 0, false),
            a_stream("/Sheet7", 9, false),
        ];
        let mut recorder = LedgerRecorder::new();
        recorder.parse(
            LedgerDomain::Drawing,
            &streams,
            &[],
            &NormalizedPidGeometry::default(),
        );
        let counts = recorder.ledger.parse.get(LedgerDomain::Drawing);
        assert_eq!(
            counts
                .items
                .iter()
                .map(|item| (item.reason, item.stream.as_str(), item.bytes))
                .collect::<Vec<_>>(),
            vec![(DropReason::SheetStreamWithoutGeometry, "/Sheet7", Some(9))]
        );
    }

    /// The fault the parser raises for a stream of `len` bytes whose record
    /// walk stops at `stop` on a record stating a length of zero.
    fn a_fault(path: &str, len: usize, stop: usize) -> pid_parse::StreamChainFault {
        pid_parse::StreamChainFault {
            stream_path: path.to_string(),
            stream_len: len,
            wrong_magic: None,
            stop: Some(pid_parse::StreamChainStop {
                offset: stop,
                reason: pid_parse::StreamChainStopReason::ZeroLength,
                record_len: Some(0),
                type_word: None,
                last_record_offset: None,
            }),
            unread_bytes: len - stop,
            records_read: 3,
            live_records_read: 3,
            stated_records: Some(7),
        }
    }

    /// A chain stream read only as far as its chain holds is a suspect item
    /// of its own: the records before the stop are on the page, so nothing
    /// else in the account would say the page is short. The item is the
    /// stream, once, with the bytes left unread; the fault rides along whole
    /// for whoever has to say it.
    #[test]
    fn a1_a_stream_whose_chain_stops_short_is_counted_with_its_unread_bytes() {
        // `/Sheet6` decoded its prefix, so it has geometry; `/Sheet7`
        // stopped on its first record and has none.
        let streams = vec![
            a_stream("/Sheet6", 30_000, true),
            a_stream("/Sheet7", 400, false),
        ];
        let mut geometry = NormalizedPidGeometry::default();
        geometry.chain_faults = vec![
            a_fault("/Sheet6", 30_000, 17_000),
            a_fault("/Sheet7", 400, 8),
            a_fault("/JSite12/PSMcluster0", 900, 500),
        ];
        let mut recorder = LedgerRecorder::new();
        recorder.parse(LedgerDomain::Drawing, &streams, &[], &geometry);
        let ledger = recorder.into_ledger();

        let counts = ledger.parse.get(LedgerDomain::Drawing);
        let mut items: Vec<_> = counts
            .items
            .iter()
            .map(|item| (item.reason, item.stream.as_str(), item.count, item.bytes))
            .collect();
        items.sort_by_key(|item| item.1);
        assert_eq!(
            items,
            vec![
                (
                    DropReason::SheetStreamWithoutGeometry,
                    "/JSite12/PSMcluster0",
                    1,
                    Some(400)
                ),
                (
                    DropReason::SheetStreamWithoutGeometry,
                    "/Sheet6",
                    1,
                    Some(13_000)
                ),
                // Filed as the stream that yielded nothing, with its size:
                // the fault adds no second item.
                (
                    DropReason::SheetStreamWithoutGeometry,
                    "/Sheet7",
                    1,
                    Some(400)
                ),
            ],
            "each stream filed once"
        );
        assert_eq!(
            DropReason::SheetStreamWithoutGeometry.tier(),
            super::super::reason::Tier::Suspect,
            "a stream not read whole has to reach the gate as a suspect"
        );
        assert!(ledger.parse.get(LedgerDomain::Furniture).items.is_empty());

        assert_eq!(
            ledger
                .chain_faults
                .iter()
                .map(|record| (record.domain, record.fault.stream_path.as_str()))
                .collect::<Vec<_>>(),
            vec![
                (LedgerDomain::Drawing, "/Sheet6"),
                (LedgerDomain::Drawing, "/Sheet7"),
                (LedgerDomain::Drawing, "/JSite12/PSMcluster0"),
            ],
            "each fault kept, in the order it was met"
        );
        let warnings = ledger.chain_fault_warnings();
        assert_eq!(warnings.len(), 3);
        assert!(
            warnings[0].contains("/Sheet6")
                && warnings[0].contains("stops at +17000 of 30000 bytes")
                && warnings[0].contains("13000 byte(s) from it on are not"),
            "{}",
            warnings[0]
        );
    }

    /// A fault in the border template is the template's, and says so; a
    /// header that disagrees with a chain read to the end leaves no byte
    /// unread and is filed all the same.
    #[test]
    fn a1_a_template_stream_cut_off_at_a_record_is_counted_in_the_furniture_domain() {
        let mut geometry = NormalizedPidGeometry::default();
        geometry.chain_faults = vec![pid_parse::StreamChainFault {
            stream_path: "/Sheet6".to_string(),
            stream_len: 4096,
            wrong_magic: None,
            stop: None,
            unread_bytes: 0,
            records_read: 40,
            live_records_read: 40,
            stated_records: Some(41),
        }];
        let mut recorder = LedgerRecorder::new();
        recorder.parse(
            LedgerDomain::Furniture,
            &[a_stream("/Sheet6", 4096, true)],
            &[],
            &geometry,
        );
        let ledger = recorder.into_ledger();
        assert!(ledger.parse.get(LedgerDomain::Drawing).items.is_empty());
        assert_eq!(
            ledger
                .parse
                .get(LedgerDomain::Furniture)
                .items
                .iter()
                .map(|item| (item.reason, item.stream.as_str(), item.count, item.bytes))
                .collect::<Vec<_>>(),
            vec![(
                DropReason::SheetStreamWithoutGeometry,
                "/Sheet6",
                1,
                Some(0)
            )]
        );
        let warnings = ledger.chain_fault_warnings();
        assert_eq!(warnings.len(), 1);
        assert!(
            warnings[0].starts_with("in the border template, the record chain of /Sheet6")
                && warnings[0].contains("40 live record(s) where the header states 41"),
            "{}",
            warnings[0]
        );

        // A page whose streams are all sound has nothing to say.
        assert!(LayoutLedger::default().chain_fault_warnings().is_empty());
    }

    fn unplaced(domain: LedgerDomain, stream: &str, why: UnplacedWhy) -> UnplacedRecords {
        UnplacedRecords {
            domain,
            stream: stream.to_string(),
            why,
            records: 1,
            primitives: 2,
        }
    }

    fn a_chain_fault(domain: LedgerDomain, stream: &str, stops: bool) -> ChainFaultRecord {
        let mut fault = a_fault(stream, 30_000, 143);
        if !stops {
            fault.stop = None;
            fault.unread_bytes = 0;
            fault.stated_records = Some(41);
        }
        ChainFaultRecord { domain, fault }
    }

    /// Records a stopped chain accounts for get no sentence of their own;
    /// everything else that takes no place does, beside a chain fault or
    /// without one.
    #[test]
    fn a_stopped_chain_is_the_word_on_the_records_it_left_without_a_place() {
        use LedgerDomain::{Drawing, Furniture};
        let page = |items: Vec<UnplacedRecords>, faults: Vec<ChainFaultRecord>| LayoutLedger {
            unplaced: items,
            chain_faults: faults,
            // Some sheet of the page states an order.
            paint_order: vec![0],
            ..LayoutLedger::default()
        };
        let no_own = || unplaced(Drawing, "/Sheet6", UnplacedWhy::NoRecordOfTheSheet);

        // The chain of the same stream stopped: its warning is the one.
        let stopped = page(
            vec![no_own()],
            vec![a_chain_fault(Drawing, "/Sheet6", true)],
        );
        assert!(stopped.display_order_warnings().is_empty());
        assert_eq!(stopped.chain_fault_warnings().len(), 1);
        assert_eq!(stopped.unplaced.len(), 1, "the finding is kept");
        // And its warning goes on to say what the stop left without a
        // place — and, where the stream is the drawing's own sheet and no
        // paper was read, what the page is.
        let goes_on = "; none of the records read is the sheet's own, so the sheet's list has \
                       nothing to be held to and 1 record(s), 2 primitive(s) on the page, take \
                       no place in the display order";
        let said = stopped.chain_fault_warnings();
        assert!(
            said[0].starts_with("the record chain of /Sheet6"),
            "{}",
            said[0]
        );
        assert!(said[0].ends_with(goes_on), "{}", said[0]);
        let mut with_a_page = page(
            vec![no_own()],
            vec![a_chain_fault(Drawing, "/Sheet6", true)],
        );
        with_a_page.page_for_want_of_a_paper = Some(PageInstead {
            from: SheetRectangle::TemplateName,
            um: [594_000, 420_000],
        });
        let said = with_a_page.chain_fault_warnings();
        assert!(
            said[0].ends_with(&format!(
                "{goes_on}, and the paper stated with that list was not read either, so the \
                 page is the size its template's name states, 594.000 x 420.000 mm"
            )),
            "{}",
            said[0]
        );

        // No chain fault at all: this sentence is the only word there is,
        // and it says what is so — the list is there, the stream is short.
        let sound = page(vec![no_own()], Vec::new());
        let said = sound.display_order_warnings();
        assert_eq!(said.len(), 1);
        assert_eq!(
            said[0],
            "1 record(s) of /Sheet6, 2 primitive(s) on the page, take no place in the \
             display order: the stream holds no record of the sheet's own, so the list its \
             sheet states has nothing to be held to; they are painted after everything that \
             has a place"
        );
        assert!(!said[0].contains("states no list"));

        // A chain that reads to its end and disagrees with its header's
        // count did not stop: both are said.
        let counted = page(
            vec![no_own()],
            vec![a_chain_fault(Drawing, "/Sheet6", false)],
        );
        assert_eq!(counted.display_order_warnings().len(), 1);
        assert!(!counted.chain_fault_warnings()[0].contains("take no place"));
        // Another stream's chain, and the template's chain of a stream of
        // the same name, are not this stream's.
        for fault in [
            a_chain_fault(Drawing, "/Sheet7", true),
            a_chain_fault(Furniture, "/Sheet6", true),
        ] {
            assert_eq!(
                page(vec![no_own()], vec![fault])
                    .display_order_warnings()
                    .len(),
                1
            );
        }
        // Any other reason is another matter than the stop, and is said
        // beside it.
        for why in [
            UnplacedWhy::NoList,
            UnplacedWhy::NotReached,
            UnplacedWhy::NoFrame,
            UnplacedWhy::NoRecord,
        ] {
            let both = page(
                vec![unplaced(Drawing, "/Sheet6", why)],
                vec![a_chain_fault(Drawing, "/Sheet6", true)],
            );
            assert_eq!(both.display_order_warnings().len(), 1, "{why:?}");
        }
        // The template's own, by its own chain.
        let template = page(
            vec![unplaced(
                Furniture,
                "/Sheet6",
                UnplacedWhy::NoRecordOfTheSheet,
            )],
            vec![a_chain_fault(Furniture, "/Sheet6", true)],
        );
        assert!(template.display_order_warnings().is_empty());
    }

    /// A page that is not a paper says which rectangle it is: a frame's,
    /// or the size a template's name states.
    #[test]
    fn a_page_that_is_not_a_paper_says_which_rectangle_it_is() {
        let frame = [594_328, 420_566];
        let named = [594_000, 420_000];
        // No paper read, and the list with it.
        for (from, um, says) in [
            (
                SheetRectangle::Frame,
                frame,
                "so the page is the border frame's rectangle, 594.328 x 420.566 mm",
            ),
            (
                SheetRectangle::TemplateName,
                named,
                "so the page is the size its template's name states, 594.000 x 420.000 mm",
            ),
        ] {
            for why in [UnplacedWhy::NoList, UnplacedWhy::NoRecordOfTheSheet] {
                let ledger = LayoutLedger {
                    unplaced: vec![unplaced(LedgerDomain::Drawing, "/Sheet6", why)],
                    page_for_want_of_a_paper: Some(PageInstead { from, um }),
                    ..LayoutLedger::default()
                };
                let said = ledger.display_order_warnings();
                assert_eq!(said.len(), 1);
                assert!(said[0].ends_with(says), "{from:?} {why:?}: {}", said[0]);
                assert_eq!(
                    said[0].contains("frame"),
                    from == SheetRectangle::Frame,
                    "{}",
                    said[0]
                );
            }
        }
        // A paper read and refused: only ever for a frame's rectangle.
        let by_a_frame = LayoutLedger {
            paper_refused: Some(PaperRefused {
                paper_um: [297_000, 420_000],
                frame_um: frame,
            }),
            ..LayoutLedger::default()
        };
        assert_eq!(
            by_a_frame.page_warnings(),
            [
                "the paper its sheet states, 297.000 x 420.000 mm, is not its border frame's, \
              594.328 x 420.566 mm: no drawing on hand has the two more than 2.707 mm apart on \
              a side, so the paper is not taken for the page and the page is the frame's \
              rectangle"
            ]
        );
        assert_eq!(
            by_a_frame.paper_refused.map(|refused| refused.sides()),
            Some("paper 297.000 x 420.000 mm, frame 594.328 x 420.566 mm".to_string())
        );
        // A paper that is the page, beside a name that states another size.
        let beside_a_name = LayoutLedger {
            paper_unlike_the_name: Some(PaperBesideAName {
                paper_um: [297_000, 420_000],
                named_um: named,
            }),
            ..LayoutLedger::default()
        };
        assert_eq!(
            beside_a_name.page_warnings(),
            [
                "the paper its sheet states, 297.000 x 420.000 mm, is not the size its \
              template's name states, 594.000 x 420.000 mm, and no border frame was read to \
              say which the sheet is: a name is a label and the paper a reading, so the page \
              is the paper"
            ]
        );
        assert_eq!(
            beside_a_name
                .paper_unlike_the_name
                .map(|beside| beside.sides()),
            Some(
                "paper 297.000 x 420.000 mm, template name's size 594.000 x 420.000 mm".to_string()
            )
        );
        let said = &beside_a_name.page_warnings()[0];
        assert!(
            !said.contains("frame's") && !said.contains("not taken"),
            "{said}"
        );
        // Nothing to say of a page that is its paper.
        assert!(LayoutLedger::default().page_warnings().is_empty());
    }

    /// A symbol file with a chain that does not hold, drawn from: one item
    /// a file and stream in the domain of the placement, and one sentence
    /// that names the file and says the reader's own of the stream.
    #[test]
    fn a_damaged_symbol_file_is_filed_and_said_once_a_stream() {
        let fault = a_fault("/Sheet6", 2_000, 600);
        let mut ledger = LayoutLedger::default();
        ledger.note_symbol_chain_faults(
            LedgerDomain::Drawing,
            [(
                r"Piping\Valves\Gate Valve.sym".to_string(),
                3,
                vec![fault.clone()],
            )],
        );
        ledger.note_symbol_chain_faults(
            LedgerDomain::Furniture,
            [("Logo.sym".to_string(), 1, vec![fault.clone()])],
        );
        // A file with no fault files nothing.
        ledger.note_symbol_chain_faults(
            LedgerDomain::Drawing,
            [("Sound.sym".to_string(), 9, Vec::new())],
        );
        let items = |domain| {
            ledger
                .parse
                .get(domain)
                .items
                .iter()
                .map(|item| (item.reason, item.stream.clone(), item.count, item.bytes))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            items(LedgerDomain::Drawing),
            [(
                DropReason::SymbolFileChainFault,
                r"Piping\Valves\Gate Valve.sym:/Sheet6".to_string(),
                1,
                Some(1_400)
            )]
        );
        assert_eq!(
            items(LedgerDomain::Furniture),
            [(
                DropReason::SymbolFileChainFault,
                "Logo.sym:/Sheet6".to_string(),
                1,
                Some(1_400)
            )]
        );
        let said = ledger.symbol_chain_fault_warnings();
        assert_eq!(
            said,
            [
                format!(
                    r"the symbol file Piping\Valves\Gate Valve.sym, which 3 placement(s) on the page are drawn from, is damaged: {}",
                    fault.describe()
                ),
                format!(
                    "in the border template, the symbol file Logo.sym, which 1 placement(s) \
                     on the page are drawn from, is damaged: {}",
                    fault.describe()
                ),
            ]
        );
        assert!(
            said[0].contains("/Sheet6") && said[0].contains("stops at +600 "),
            "{}",
            said[0]
        );
        assert!(
            LayoutLedger::default()
                .symbol_chain_fault_warnings()
                .is_empty()
        );
    }

    /// A symbol file with two streams that do not hold is two items and
    /// two sentences, one a stream: each stream is filed with what its own
    /// chain left unread and said in the reader's own sentence of it, of
    /// the one file and the one count of placements.
    #[test]
    fn a_symbol_file_with_two_damaged_streams_is_filed_and_said_once_for_each() {
        let sheet = a_fault("/Sheet68", 127, 78);
        let pens = a_fault("/StyleCluster", 3_584, 2_690);
        let mut ledger = LayoutLedger::default();
        ledger.note_symbol_chain_faults(
            LedgerDomain::Drawing,
            [(
                "Gauge.sym".to_string(),
                2,
                vec![sheet.clone(), pens.clone()],
            )],
        );
        let items: Vec<_> = ledger
            .parse
            .get(LedgerDomain::Drawing)
            .items
            .iter()
            .map(|item| (item.reason, item.stream.as_str(), item.count, item.bytes))
            .collect();
        assert_eq!(
            items,
            [
                (
                    DropReason::SymbolFileChainFault,
                    "Gauge.sym:/Sheet68",
                    1,
                    Some(49)
                ),
                (
                    DropReason::SymbolFileChainFault,
                    "Gauge.sym:/StyleCluster",
                    1,
                    Some(894)
                ),
            ]
        );
        assert!(ledger.parse.get(LedgerDomain::Furniture).items.is_empty());
        assert_eq!(
            ledger.symbol_chain_faults,
            [
                SymbolChainFault {
                    domain: LedgerDomain::Drawing,
                    symbol: "Gauge.sym".to_string(),
                    placements: 2,
                    fault: sheet.clone(),
                },
                SymbolChainFault {
                    domain: LedgerDomain::Drawing,
                    symbol: "Gauge.sym".to_string(),
                    placements: 2,
                    fault: pens.clone(),
                },
            ]
        );
        assert_eq!(
            ledger.symbol_chain_fault_warnings(),
            [
                format!(
                    "the symbol file Gauge.sym, which 2 placement(s) on the page are drawn \
                     from, is damaged: {}",
                    sheet.describe()
                ),
                format!(
                    "the symbol file Gauge.sym, which 2 placement(s) on the page are drawn \
                     from, is damaged: {}",
                    pens.describe()
                ),
            ]
        );
        assert_ne!(sheet.describe(), pens.describe());
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
