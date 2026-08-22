//! How a geometry record reaches the line width and colour it draws with.
//!
//! `igLine2d` and its siblings carry a `u32` at payload `+14` that the
//! decoders call `index`. It is a **style reference**: it names a style id in
//! the `StyleCluster` of the document that geometry lives in, and that record
//! either is the line style or points at it.
//!
//! ```text
//! geometry payload +14 (u32)
//!    └─ look the id up in the SAME document's StyleCluster
//!         ├─ lands on 0x002E JStyleSimpleLine → width +34 (f64, metres)
//!         │                                     colour +50 (Win32 COLORREF)
//!         ├─ lands on 0x002A JStyleSimpleFill → colour +30 (Win32 COLORREF)
//!         └─ lands on 0x0030 JStyleOverride   → its +22 names a
//!                                               JStyleSimpleLine or a
//!                                               JStyleSimpleFill → as above
//! ```
//!
//! # Evidence
//!
//! **The two offsets this module reads inside a style record are
//! native-reader; the geometry side of the link is corpus.**
//!
//! `JStyleBase__ReadCommonFields` in `style.dll` reads the base-class block as
//! four contiguous fields — `DoIO(2)`, `DoIO(4)`, `DoIO(4)`, `DoIO(4)` — and
//! `JStyleBase__LoadV3Block` shows what the last one is for: it compares the
//! incoming value with the stored id, and on a change releases the cached
//! object pointer and stores the new id. That is a lazily-resolved reference
//! to another object, held by `JStyleBase` itself, so every style family has
//! exactly one.
//!
//! Sliding that four-field block over the corpus places it uniquely at payload
//! `+12`: it is the only offset where the reference field's non-zero values
//! name a *different* record, and it does so on 48 of 48 (every other
//! placement scores 0). So
//!
//! ```text
//! +0   12 bytes  prologue, ahead of the class Load
//! +12  u16       dash index, via (w & 7) != 0 ? (w & 7) + 10 : 0
//! +14  u32       the record's own identity          <- STYLE_ID_OFFSET
//! +18  u32       zero on all 718 corpus records
//! +22  u32       JStyleBase's object reference      <- BASE_OBJECT_REFERENCE_OFFSET
//! +26            class-specific fields begin
//! ```
//!
//! which also settles a number `docs/pid-format-guide.md` §6 flags as
//! unverified: the base block is 14 bytes and the class block starts at 26, so
//! **26 = 12 + 14**. The guide's `B = 26`, the native `DoIO` count of 14, and
//! the fixture-measured 12 were all correct and were measuring different
//! things.
//!
//! Two consequences worth stating plainly. The id at `+14` is unique per
//! document *across every style family* because it is a base-class field, not
//! a per-family one. And `+22` is not an override-specific slot — it is
//! `JStyleBase`'s single reference, which only `JStyleOverride` populates.
//! The resolver is base-class machinery, and this module is walking it.
//!
//! What remains corpus-level is the geometry side: that a geometry record's
//! `index` names one of these ids. Over the four `test-file/*.pid` fixtures:
//!
//! * Every drawable record resolves to a concrete width and colour, with
//!   nothing left over — all 562 that `decode_iglines` / `decode_igpoints` /
//!   `decode_iglinestrings` accept across the four fixtures. (A raw chain walk
//!   finds 574; the twelve polylines it finds and they refuse fail on their
//!   own validation rules, which is a decoder coverage question, not a link
//!   question. The four `igLine2d` in `DWG-0202/Sheet6615` that used to sit
//!   beside them are in the 562 now — and all four resolve, which is one more
//!   reading that refusing them was the decoder's error and not the file's.)
//!   The ratchet in `tests/style_link_ratchet.rs` pins the counts.
//! * Resolution alone is weak evidence: style ids form a 79–97% dense `1..N`
//!   run per document, so any small integer resolves. The evidence is in
//!   **which kind** the id lands on. Across 98 distinct index values not one
//!   lands on a wrong kind — text on `JStyleTextPara`, points and polylines on
//!   `JStyleSimpleLine`, lines on `JStyleSimpleLine` or `JStyleOverride`.
//!   Under the null model (the id names a record drawn from the document's own
//!   population of style kinds) that runs to `5.2e-12` on the strongest
//!   fixture.
//! * The override's line slot at `+22` is **zero on all 718 non-override style
//!   records** and populated on all 48 overrides, every one of them naming a
//!   real local style. A coincidence does not zero itself out elsewhere.
//! * The palette that falls out is nine entries wide and sits on the ISO 128
//!   ladder apart from the 0.100 mm point ticks.
//!
//! # The scoping rule, which is the part that is easy to get wrong
//!
//! A `.pid` is not one document. The root storage has its own `Sheet*` and
//! `StyleCluster`, and every `JSite<n>/` storage is a nested document with its
//! own pair. **Style ids restart from 1 in each.** Resolving geometry against
//! a pooled id set both invents matches and hides real ones — that is why an
//! earlier probe scored 0/24 on one drawing and 212/218 on another and
//! concluded the field was noise. [`stylecluster_path_for_sheet`] exists so
//! callers cannot repeat it.
//!
//! This module deliberately stays out of the model layer: it reads
//! `StyleCluster` bytes and answers questions about them, and emits no
//! `PidGraphicEntity`. A renderer joins the two indices below on
//! `(stream path, graphic oid)` and decides for itself what to do with a
//! record that does not resolve — which is a decision this crate should not
//! be making on its behalf.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::ops::Range;
use std::path::Path;

use crate::error::PidError;
use crate::parsers::sheet_records::{
    decode_igboundaries, decode_iglines, decode_iglinestrings, decode_igpoints, decode_igtextboxes,
    PSM_TYPE_CODE_JSTYLE_OVERRIDE,
};

/// Magic word every cluster-family stream opens with, `StyleCluster`
/// included.
const CLUSTER_MAGIC: u32 = 0x6C90_F544;

/// Bytes of stream header (`magic` + `record_count`) before the record chain.
const STREAM_HEADER_LEN: usize = 8;

/// Bytes of PSM envelope (`type_word` + `bytes_to_follow`) before a payload.
const PSM_ENVELOPE_LEN: usize = 6;

/// PSM type code of `JStyleSimpleLine` (RAD `style.dll`), the record that
/// carries a concrete line width and colour.
pub const PSM_TYPE_CODE_JSTYLE_SIMPLE_LINE: u16 = 0x002E;

/// The `StyleCluster` families whose `+14` is a style id, per
/// `docs/pid-format-guide.md` §4.
///
/// The chain also carries `0x005A JStyleLibrarian` — the type directory at
/// the head of every cluster — and it is deliberately excluded: its `+14` is
/// not an id in this space, and since the librarian comes first in the chain
/// a permissive table would let it shadow a real style. `0x0029`
/// `JStyleMultiplexer` is listed for completeness though it has zero hits
/// across the corpus.
const STYLE_FAMILY_TYPE_CODES: [u16; 10] = [
    0x0029, 0x002A, 0x002B, 0x002C, 0x002D, 0x002E, 0x002F, 0x0030, 0x0032, 0x0033,
];

/// Offset of a style record's own identity within its payload.
///
/// Level: native-reader. The second of the four `DoIO` reads
/// `JStyleBase__ReadCommonFields` performs, which is why the value is unique
/// per document across every style family rather than per family. The corpus
/// agrees independently: this is the only offset whose value is unique per
/// record (uniqueness 1.00; the next best candidate reaches 0.33).
pub const STYLE_ID_OFFSET: usize = 14;

/// Offset of `JStyleBase`'s lazily-resolved object reference.
///
/// Level: native-reader. The fourth `DoIO` read of the base block.
/// `JStyleBase__LoadV3Block` keys a cached object pointer on it and releases
/// that cache when the id changes, which is what makes it a reference rather
/// than a value.
///
/// Every style family carries the field; in this corpus only `JStyleOverride`
/// populates it — 48 of 48, against zero on all 718 other style records — and
/// the value always names a different record.
pub const BASE_OBJECT_REFERENCE_OFFSET: usize = 22;

/// Offset of the line width, an `f64` in metres, within a
/// `JStyleSimpleLine` payload. Level: native-reader.
pub const SIMPLE_LINE_WIDTH_OFFSET: usize = 34;

/// Offset of the colour, a Win32 `COLORREF`, within a `JStyleSimpleLine`
/// payload. Level: native-reader.
pub const SIMPLE_LINE_COLOUR_OFFSET: usize = 50;

/// PSM type code of `JStyleSimpleDashType`, which carries the dash pattern a
/// line is drawn with. CLSID `{47FCC336-2D0F-11D0-A1FF-080036A1CF02}`.
pub const PSM_TYPE_CODE_JSTYLE_SIMPLE_DASH_TYPE: u16 = 0x002F;

/// Offset at which a `JStyleSimpleLine` names its `JStyleSimpleDashType`.
///
/// Level: native-reader for the field's existence, corpus for the offset.
/// `style.dll` holds the dash type as an **id** with a lazily resolved object
/// pointer beside it — the getter resolves the id on first use and the setter
/// drops the cached pointer when the id changes — so the line record has to
/// write an id, and 0 means "draw solid".
///
/// The corpus places it: across all ten `StyleCluster` streams that define
/// any dash style, the number of line records carrying a dash id here is
/// exactly the number of dash styles defined in that stream — four for four
/// in the richest, one for one in six others. No other offset comes close.
pub const SIMPLE_LINE_DASH_REFERENCE_OFFSET: usize = 54;

/// Offset of the segment count within a `JStyleSimpleDashType` payload.
/// Level: native-reader for the shape, corpus for the width.
///
/// The serializer writes a count and then that many `f64`; on disk the count
/// is a `u16` and the record stops after the pattern, so the payload is
/// exactly `50 + 8N` bytes.
pub const DASH_SEGMENT_COUNT_OFFSET: usize = 48;

/// Most dash segments a pattern may declare before the record is refused.
///
/// The widest in the corpus is six. Sixteen leaves generous headroom while
/// still rejecting a misframed count, which reads as an enormous number.
pub const MAX_DASH_SEGMENTS: usize = 16;

/// PSM type code of `JStyleTextChar`, which carries the character height.
pub const PSM_TYPE_CODE_JSTYLE_TEXT_CHAR: u16 = 0x002C;

/// PSM type code of `JStyleTextPara`, which is what a text record names.
pub const PSM_TYPE_CODE_JSTYLE_TEXT_PARA: u16 = 0x002D;

/// Offset at which a `JStyleTextPara` names its `JStyleTextChar`.
///
/// Level: corpus, and a strong one — 237 of 237 `JStyleTextPara` records
/// across every document in the corpus name a locally defined
/// `JStyleTextChar` here, and no other offset comes close. Text needs this
/// hop because `igTextBox` names the paragraph style, while the height lives
/// on the character style.
pub const TEXT_PARA_CHAR_REFERENCE_OFFSET: usize = 38;

/// Offset of the character height, an `f64` in metres, within a
/// `JStyleTextChar` payload. Level: native-reader.
pub const TEXT_CHAR_HEIGHT_OFFSET: usize = 42;

/// Offset of the character colour, a Win32 `COLORREF` (`0x00BBGGRR`), within
/// a `JStyleTextChar` payload.
///
/// Level: **native-reader**. `style.dll`'s serialiser (`sub_10030A20`) reads
/// the record in order and brackets this word between two offsets already at
/// that level — `+30`, the language id with its `GetKeyboardLayout(0)`
/// fallback, and [`TEXT_CHAR_HEIGHT_OFFSET`] — with `30 + 2 + 2 + 4 + 4 = 42`
/// closing exactly and leaving no slack. The colour can only be here.
///
/// The corpus said the same before the read order was found, and it is what
/// makes the guide's earlier hypothesis ("holds a `-1` sentinel; the shape
/// fits a text colour") half right:
///
/// * **381 of 381** records read here with a **zero high byte**. An
///   unconstrained `u32` clears its high byte about once in 256; getting it on
///   every record means the field is 24-bit by construction, which is what a
///   `COLORREF` is.
/// * **No record holds `-1` on disk.** The sentinel is real but lives only in
///   memory: the serialiser normalises `-1` to `0` on the way through, in both
///   directions. So reading `0` as black is what the native reader does, not a
///   convenience of ours.
/// * The values are a drafting palette — 367 black, then `#FF0000`, `#404040`,
///   `#FE0060`, `#00FEA0` — and `#FE0060` is a colour the *same drawing* uses
///   for line work, pinned in `OpenCADStudio`'s import test. An unrelated
///   field would not land on that.
/// * Both sibling style families store colour the same way, at
///   [`SIMPLE_LINE_COLOUR_OFFSET`] and [`SIMPLE_FILL_COLOUR_OFFSET`].
///
/// See `docs/analysis/2026-08-13-text-colour-is-002c-plus-34.md`.
pub const TEXT_CHAR_COLOUR_OFFSET: usize = 34;

/// Smallest and largest character height accepted as real, in metres.
///
/// The corpus lands on recognisable drafting sizes — ISO 3098's 1.5 / 2.5 /
/// 3.5 mm, the imperial 1/16-1/8-1/4 inch steps, 7 to 12 point — all inside
/// 0.5..20 mm. Outside that window is the known unexplained case: 21 records
/// reference a `JStyleTextChar` whose height reads 0.254 mm, which
/// `docs/pid-format-guide.md` §8.2 has flagged as unexplained since 08-04.
/// Text that small cannot be what the drawing means, so it is refused and the
/// caller keeps its own default rather than drawing something invisible.
const MIN_PLAUSIBLE_HEIGHT_M: f64 = 0.0005;
const MAX_PLAUSIBLE_HEIGHT_M: f64 = 0.02;

/// Largest line width accepted as real, in metres.
///
/// Widths are stored in metres and the widest in the corpus is 10 mm, so
/// 100 mm leaves generous headroom while still catching a misframed `f64` —
/// which reads either enormous or subnormal, both of which are refused. Two
/// bytes of framing slip is exactly how this crate has been misled before.
const MAX_PLAUSIBLE_WIDTH_M: f64 = 0.1;

fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    let end = at.checked_add(2)?;
    let s = data.get(at..end)?;
    Some(u16::from_le_bytes([s[0], s[1]]))
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    let end = at.checked_add(4)?;
    let s = data.get(at..end)?;
    Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn f64_at(data: &[u8], at: usize) -> Option<f64> {
    let end = at.checked_add(8)?;
    let s = data.get(at..end)?;
    Some(f64::from_le_bytes([
        s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
    ]))
}

/// The width and colour a `JStyleSimpleLine` record declares.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineSymbology {
    /// Line width in metres, exactly as stored at
    /// [`SIMPLE_LINE_WIDTH_OFFSET`].
    pub width_m: f64,
    /// Colour as the raw Win32 `COLORREF` word from
    /// [`SIMPLE_LINE_COLOUR_OFFSET`], i.e. `0x00BBGGRR`.
    pub colour: u32,
}

impl LineSymbology {
    /// Line width in millimetres, which is the unit drawing standards use.
    #[must_use]
    pub fn width_mm(&self) -> f64 {
        self.width_m * 1000.0
    }

    /// Colour split into `[R, G, B]`.
    #[must_use]
    pub fn rgb(&self) -> [u8; 3] {
        [
            (self.colour & 0xFF) as u8,
            ((self.colour >> 8) & 0xFF) as u8,
            ((self.colour >> 16) & 0xFF) as u8,
        ]
    }
}

/// The dash pattern a `JStyleSimpleDashType` record declares.
///
/// Segment lengths alternate along the line the way every drafting linetype
/// does. The sign is the format's own and is preserved rather than
/// interpreted here: the corpus shows a leading negative run for the plain
/// dashed patterns and mixed signs for the chain ones, and nothing in
/// `style.dll` has yet said which sign draws and which skips. A renderer that
/// only needs the repeat can use the magnitudes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DashPattern {
    segments: [f64; MAX_DASH_SEGMENTS],
    len: u8,
}

impl DashPattern {
    /// Segment lengths in metres, exactly as stored.
    #[must_use]
    pub fn segments_m(&self) -> &[f64] {
        &self.segments[..usize::from(self.len)]
    }

    /// Segment lengths in millimetres, which is the unit drafting standards
    /// and renderers use.
    #[must_use]
    pub fn segments_mm(&self) -> Vec<f64> {
        self.segments_m().iter().map(|v| v * 1000.0).collect()
    }

    /// How many segments the pattern declares.
    #[must_use]
    pub fn len(&self) -> usize {
        usize::from(self.len)
    }

    /// Whether the pattern declares no segments at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Total length of one repeat, in metres.
    #[must_use]
    pub fn period_m(&self) -> f64 {
        self.segments_m().iter().map(|v| v.abs()).sum()
    }
}

/// Which route a geometry record took to its `JStyleSimpleLine`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleHop {
    /// The style id named a `JStyleSimpleLine` outright.
    Direct,
    /// The style id named a `JStyleOverride`, whose line slot named the
    /// `JStyleSimpleLine`.
    ViaOverride {
        /// Style id of the `JStyleOverride` that was traversed.
        override_id: u32,
    },
}

/// A geometry record's line style, plus the route taken to reach it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedLineStyle {
    /// Style id of the `JStyleSimpleLine` finally reached.
    pub style_id: u32,
    /// Width and colour that record declares.
    pub symbology: LineSymbology,
    /// The dash pattern that line style names, when it names one. `None`
    /// means the line draws solid — which is what most of the corpus says.
    pub dash: Option<DashPattern>,
    /// Whether an override was traversed on the way.
    pub hop: StyleHop,
}

/// One `StyleCluster` record, reduced to what the link needs.
#[derive(Debug, Clone, PartialEq)]
pub struct StyleRecord {
    /// PSM type code, low 14 bits of the envelope's type word.
    pub type_code: u16,
    /// The record's own style id, read at [`STYLE_ID_OFFSET`]. Unique within
    /// its document across every style family.
    pub style_id: u32,
    /// Byte range of the whole record — envelope included — within the
    /// stream, for provenance.
    pub byte_range: Range<usize>,
    /// Width and colour, when this is a `JStyleSimpleLine` whose fields read
    /// plausibly.
    pub symbology: Option<LineSymbology>,
    /// Style id named by [`BASE_OBJECT_REFERENCE_OFFSET`], when this record
    /// populates it. Only `JStyleOverride` does, in this corpus.
    pub base_reference: Option<u32>,
    /// Character height in metres, when this is a `JStyleTextChar` whose
    /// height reads plausibly.
    pub char_height_m: Option<f64>,
    /// `JStyleTextChar` this record names, when it is a `JStyleTextPara`.
    pub text_char_reference: Option<u32>,
    /// `JStyleSimpleDashType` this record names, when it is a
    /// `JStyleSimpleLine` that draws dashed.
    pub dash_reference: Option<u32>,
    /// The dash pattern, when this is a `JStyleSimpleDashType` whose own
    /// length agrees with its segment count.
    pub dash: Option<DashPattern>,
    /// Fill colour as a raw Win32 `COLORREF`, when this is a
    /// `JStyleSimpleFill` that states one rather than the unset sentinel.
    pub fill_colour: Option<u32>,
    /// Character colour as a raw Win32 `COLORREF`, when this is a
    /// `JStyleTextChar` whose [`TEXT_CHAR_COLOUR_OFFSET`] word reads as one.
    pub text_colour: Option<u32>,
}

/// PSM type code of `JStyleSimpleFill`, a flat colour fill.
pub const PSM_TYPE_CODE_JSTYLE_SIMPLE_FILL: u16 = 0x002A;

/// PSM type code of `JStyleHatchFill`, a patterned fill. Defined in every
/// corpus document and referenced by nothing in any of them.
pub const PSM_TYPE_CODE_JSTYLE_HATCH_FILL: u16 = 0x002B;

/// Offset of the fill colour, a Win32 `COLORREF`, within a `JStyleSimpleFill`
/// payload. Level: native-reader for the field, corpus for the offset.
///
/// `style.dll`'s version-2 `JStyleSimpleFill` worker (`sub_1001D610`, CLSID
/// `{47FCC331-…}`) reads a 30-byte base block and then three `u32` fields into
/// object offsets +104, +112, +120. The first, +104, is the very slot
/// `JStyleSimpleLine` keeps its colour in, and the one the object's
/// `IJStyleSolidFillImp` interface exposes through its single get/put colour
/// pair — so it is the solid fill's colour, in the same `0x00BBGGRR` encoding
/// as [`SIMPLE_LINE_COLOUR_OFFSET`]. The base block is the same helper
/// `JStyleSimpleLine` uses, so on disk the field lands at +30.
///
/// The corpus pins it: across the five documents every `JStyleSimpleFill`
/// reads a colour here whose high byte is zero and whose value is one the same
/// file already uses for a line — the flow-arrow fills on DWG-0202 and the
/// gongyi drawing both read `#0000FF`. See
/// `docs/analysis/2026-08-10-fill-colour-is-002a-plus-30.md`.
pub const SIMPLE_FILL_COLOUR_OFFSET: usize = 30;

/// The `JStyleSimpleFill` "no colour set" sentinel.
///
/// `IJStyleSimpleFillImp`'s put method writes `-2` into the colour slot when
/// asked to clear it, and every corpus document defines one such template fill
/// (id 3) that nothing references. A record reading this draws in its layer's
/// colour rather than one the drawing states.
const SIMPLE_FILL_COLOUR_UNSET: u32 = 0xFFFF_FFFE;

/// That a record draws filled, which style says so, and in what colour.
///
/// The colour is a `JStyleSimpleFill`'s stated one, read at
/// [`SIMPLE_FILL_COLOUR_OFFSET`]; it is `None` for a patterned
/// [`PSM_TYPE_CODE_JSTYLE_HATCH_FILL`], which carries no single colour, and
/// for the "unset" template fill every document defines. A caller keeps its
/// own default in that case — drawing in the layer's colour — which is what a
/// fill with no stated colour asks for anyway.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedFill {
    /// Style id of the fill record finally reached.
    pub style_id: u32,
    /// Which fill family it is — [`PSM_TYPE_CODE_JSTYLE_SIMPLE_FILL`] or
    /// [`PSM_TYPE_CODE_JSTYLE_HATCH_FILL`].
    pub type_code: u16,
    /// The fill colour as a raw Win32 `COLORREF` (`0x00BBGGRR`), when this is
    /// a solid fill that states one. `None` means "draw in the layer's
    /// colour" — a hatch, or the unset sentinel.
    pub colour: Option<u32>,
    /// Whether an override was traversed on the way. Every corpus fill is
    /// reached through one.
    pub hop: StyleHop,
}

impl ResolvedFill {
    /// Whether this is a flat colour fill rather than a pattern.
    #[must_use]
    pub fn is_solid(&self) -> bool {
        self.type_code == PSM_TYPE_CODE_JSTYLE_SIMPLE_FILL
    }

    /// The stated fill colour split into `[R, G, B]`, when it states one.
    #[must_use]
    pub fn rgb(&self) -> Option<[u8; 3]> {
        self.colour.map(|word| {
            [
                (word & 0xFF) as u8,
                ((word >> 8) & 0xFF) as u8,
                ((word >> 16) & 0xFF) as u8,
            ]
        })
    }
}

/// A text record's character style: the height it letters at, the colour it
/// letters in, and which style record they came from.
///
/// Both properties ride the same two hops (`igTextBox` names a
/// `JStyleTextPara`, which names a `JStyleTextChar`), so they are resolved
/// together rather than through two indexes over the same walk.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedTextHeight {
    /// Style id of the `JStyleTextChar` finally reached.
    pub style_id: u32,
    /// Character height in metres, as stored at [`TEXT_CHAR_HEIGHT_OFFSET`].
    pub height_m: f64,
    /// Character colour as a raw Win32 `COLORREF` (`0x00BBGGRR`), read at
    /// [`TEXT_CHAR_COLOUR_OFFSET`]. `None` when the word does not read as one
    /// — a caller keeps its own default then, the same way an unstated fill
    /// colour is handled.
    pub colour: Option<u32>,
}

impl ResolvedTextHeight {
    /// Character height in millimetres, which is the unit drafting standards
    /// and renderers use.
    #[must_use]
    pub fn height_mm(&self) -> f64 {
        self.height_m * 1000.0
    }

    /// The stated character colour split into `[R, G, B]`, when it states one.
    #[must_use]
    pub fn rgb(&self) -> Option<[u8; 3]> {
        self.colour.map(|word| {
            [
                (word & 0xFF) as u8,
                ((word >> 8) & 0xFF) as u8,
                ((word >> 16) & 0xFF) as u8,
            ]
        })
    }
}

/// Every style record of one document's `StyleCluster` stream.
///
/// Build one per document — see the scoping rule in the module docs — and
/// resolve only that document's geometry against it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DocumentStyleTable {
    records: Vec<StyleRecord>,
    by_id: BTreeMap<u32, usize>,
}

impl DocumentStyleTable {
    /// Walk a `StyleCluster` stream's bytes into a table.
    ///
    /// The stream is a pure record chain — `u32` magic, `u32` count, then
    /// records nose to tail — so it is walked exactly rather than scanned.
    /// Anything that is not such a chain yields an empty table; nothing here
    /// panics or allocates unboundedly on hostile input.
    #[must_use]
    pub fn from_stylecluster_bytes(data: &[u8]) -> Self {
        let mut table = Self::default();
        if u32_at(data, 0) != Some(CLUSTER_MAGIC) {
            return table;
        }
        let mut at = STREAM_HEADER_LEN;
        while let (Some(type_word), Some(bytes_to_follow)) =
            (u16_at(data, at), u32_at(data, at + 2))
        {
            let Some(payload_start) = at.checked_add(PSM_ENVELOPE_LEN) else {
                break;
            };
            let Some(end) = payload_start.checked_add(bytes_to_follow as usize) else {
                break;
            };
            // A zero-length record would not advance the cursor.
            if bytes_to_follow == 0 || end > data.len() {
                break;
            }
            let payload = &data[payload_start..end];
            let type_code = type_word & 0x3FFF;
            if !STYLE_FAMILY_TYPE_CODES.contains(&type_code) {
                at = end;
                continue;
            }
            if let Some(style_id) = u32_at(payload, STYLE_ID_OFFSET) {
                let record = StyleRecord {
                    type_code,
                    style_id,
                    byte_range: at..end,
                    symbology: read_symbology(type_code, payload),
                    base_reference: read_base_reference(payload),
                    char_height_m: read_char_height(type_code, payload),
                    text_char_reference: read_text_char_reference(type_code, payload),
                    dash_reference: read_dash_reference(type_code, payload),
                    dash: read_dash_pattern(type_code, payload),
                    fill_colour: read_fill_colour(type_code, payload),
                    text_colour: read_text_colour(type_code, payload),
                };
                // First writer wins, matching how a reader walking the chain
                // in order would bind the id.
                let slot = table.records.len();
                table.by_id.entry(style_id).or_insert(slot);
                table.records.push(record);
            }
            at = end;
        }
        table
    }

    /// Every record in chain order.
    #[must_use]
    pub fn records(&self) -> &[StyleRecord] {
        &self.records
    }

    /// The record a style id names, if this document defines it.
    #[must_use]
    pub fn get(&self, style_id: u32) -> Option<&StyleRecord> {
        self.by_id
            .get(&style_id)
            .and_then(|at| self.records.get(*at))
    }

    /// How many style records the document defines.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether the document defines no style records at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Resolve a geometry record's `index` to the line style it draws with.
    ///
    /// Returns `None` when the id is not defined here, when it names a family
    /// that carries no line symbology — a fill or a text style — or when an
    /// override's reference leads nowhere. Callers should treat `None` as
    /// "keep the current default", not as a parse failure: an override
    /// referencing a `JStyleSimpleFill` is a well-formed record describing a
    /// filled object, and 4 of the corpus's 48 overrides are exactly that.
    #[must_use]
    pub fn resolve_line_style(&self, style_id: u32) -> Option<ResolvedLineStyle> {
        let record = self.get(style_id)?;
        if record.type_code == PSM_TYPE_CODE_JSTYLE_OVERRIDE {
            let referenced = record.base_reference?;
            let target = self.get(referenced)?;
            return Some(ResolvedLineStyle {
                style_id: target.style_id,
                symbology: target.symbology?,
                dash: self.dash_of(target),
                hop: StyleHop::ViaOverride {
                    override_id: record.style_id,
                },
            });
        }
        Some(ResolvedLineStyle {
            style_id: record.style_id,
            symbology: record.symbology?,
            dash: self.dash_of(record),
            hop: StyleHop::Direct,
        })
    }

    /// The dash pattern a `JStyleSimpleLine` names, if it names one that this
    /// document actually defines.
    ///
    /// A dangling reference yields `None` — the line then draws solid, which
    /// is the same thing an absent reference means, so nothing is lost by not
    /// distinguishing them here.
    fn dash_of(&self, line: &StyleRecord) -> Option<DashPattern> {
        self.get(line.dash_reference?)?.dash
    }

    /// Resolve a text record's `index` to the character height it draws at.
    ///
    /// A text record names a `JStyleTextPara`, and the height is on the
    /// `JStyleTextChar` that paragraph style points at, so this is two hops
    /// like the line case — just through a different field.
    ///
    /// Returns `None` when the id names something else, when the paragraph
    /// style names no character style, or when the height it reaches is
    /// outside the plausible window. Callers should keep their own default in
    /// that case: 21 corpus records legitimately reach a `JStyleTextChar`
    /// whose height reads 0.254 mm, which nobody has explained yet.
    #[must_use]
    pub fn resolve_text_height(&self, style_id: u32) -> Option<ResolvedTextHeight> {
        let para = self.get(style_id)?;
        let char_style = if para.type_code == PSM_TYPE_CODE_JSTYLE_TEXT_PARA {
            self.get(para.text_char_reference?)?
        } else {
            para
        };
        Some(ResolvedTextHeight {
            style_id: char_style.style_id,
            height_m: char_style.char_height_m?,
            colour: char_style.text_colour,
        })
    }

    /// Resolve a boundary record's `index` to the fill it draws with.
    ///
    /// Same two shapes as the line case — the id names a fill outright, or it
    /// names a `JStyleOverride` whose base object is one. In this corpus every
    /// hit takes the override route.
    ///
    /// Returns `None` when the id names anything else, which is what every
    /// line, point and text record does: fill is a property of areas, and the
    /// corpus separates cleanly on that.
    #[must_use]
    pub fn resolve_fill(&self, style_id: u32) -> Option<ResolvedFill> {
        let record = self.get(style_id)?;
        let is_fill = |type_code: u16| {
            type_code == PSM_TYPE_CODE_JSTYLE_SIMPLE_FILL
                || type_code == PSM_TYPE_CODE_JSTYLE_HATCH_FILL
        };
        if is_fill(record.type_code) {
            return Some(ResolvedFill {
                style_id: record.style_id,
                type_code: record.type_code,
                colour: record.fill_colour,
                hop: StyleHop::Direct,
            });
        }
        if record.type_code != PSM_TYPE_CODE_JSTYLE_OVERRIDE {
            return None;
        }
        let target = self.get(record.base_reference?)?;
        is_fill(target.type_code).then_some(ResolvedFill {
            style_id: target.style_id,
            type_code: target.type_code,
            colour: target.fill_colour,
            hop: StyleHop::ViaOverride {
                override_id: record.style_id,
            },
        })
    }
}

fn read_symbology(type_code: u16, payload: &[u8]) -> Option<LineSymbology> {
    if type_code != PSM_TYPE_CODE_JSTYLE_SIMPLE_LINE {
        return None;
    }
    let width_m = f64_at(payload, SIMPLE_LINE_WIDTH_OFFSET)?;
    let colour = u32_at(payload, SIMPLE_LINE_COLOUR_OFFSET)?;
    // Zero is a real stored value; anything else must be a normal float in
    // range, which rules out both the huge and the subnormal misframings.
    if width_m != 0.0 && (!width_m.is_normal() || width_m < 0.0 || width_m > MAX_PLAUSIBLE_WIDTH_M)
    {
        return None;
    }
    Some(LineSymbology { width_m, colour })
}

fn read_char_height(type_code: u16, payload: &[u8]) -> Option<f64> {
    if type_code != PSM_TYPE_CODE_JSTYLE_TEXT_CHAR {
        return None;
    }
    let height_m = f64_at(payload, TEXT_CHAR_HEIGHT_OFFSET)?;
    if !height_m.is_normal()
        || !(MIN_PLAUSIBLE_HEIGHT_M..=MAX_PLAUSIBLE_HEIGHT_M).contains(&height_m)
    {
        return None;
    }
    Some(height_m)
}

fn read_text_char_reference(type_code: u16, payload: &[u8]) -> Option<u32> {
    if type_code != PSM_TYPE_CODE_JSTYLE_TEXT_PARA {
        return None;
    }
    u32_at(payload, TEXT_PARA_CHAR_REFERENCE_OFFSET).filter(|referenced| *referenced != 0)
}

fn read_dash_reference(type_code: u16, payload: &[u8]) -> Option<u32> {
    if type_code != PSM_TYPE_CODE_JSTYLE_SIMPLE_LINE {
        return None;
    }
    // Zero is "solid", and style ids start at 1, so zero is unambiguous.
    u32_at(payload, SIMPLE_LINE_DASH_REFERENCE_OFFSET).filter(|referenced| *referenced != 0)
}

fn read_dash_pattern(type_code: u16, payload: &[u8]) -> Option<DashPattern> {
    if type_code != PSM_TYPE_CODE_JSTYLE_SIMPLE_DASH_TYPE {
        return None;
    }
    let count = usize::from(u16_at(payload, DASH_SEGMENT_COUNT_OFFSET)?);
    if count == 0 || count > MAX_DASH_SEGMENTS {
        return None;
    }
    let first = DASH_SEGMENT_COUNT_OFFSET + 2;
    // The record's own length has to agree with the count it declares. That
    // is what makes this reading falsifiable instead of merely plausible:
    // every one of the corpus's 20 records agrees, and a misread count or a
    // misplaced field would leave a remainder on some of them.
    if payload.len() != first + 8 * count {
        return None;
    }
    let mut segments = [0.0_f64; MAX_DASH_SEGMENTS];
    for (i, slot) in segments.iter_mut().enumerate().take(count) {
        let value = f64_at(payload, first + 8 * i)?;
        if !value.is_finite() {
            return None;
        }
        // A zero-length segment is a dot, not a decode failure.
        *slot = value;
    }
    Some(DashPattern {
        segments,
        len: u8::try_from(count).ok()?,
    })
}

fn read_base_reference(payload: &[u8]) -> Option<u32> {
    // Zero means "references nothing", which is what 670 of the corpus's 718
    // style records say. Style ids start at 1, so zero is unambiguous.
    u32_at(payload, BASE_OBJECT_REFERENCE_OFFSET).filter(|referenced| *referenced != 0)
}

fn read_fill_colour(type_code: u16, payload: &[u8]) -> Option<u32> {
    // Only a solid fill carries a single colour; a hatch fill is a pattern.
    if type_code != PSM_TYPE_CODE_JSTYLE_SIMPLE_FILL {
        return None;
    }
    let word = u32_at(payload, SIMPLE_FILL_COLOUR_OFFSET)?;
    // The unset sentinel means "no colour stated"; and a real COLORREF has a
    // zero high byte, so anything else is either a sentinel or a misframing —
    // either way not a plain RGB value to be trusted. Both refuse.
    if word == SIMPLE_FILL_COLOUR_UNSET || word >> 24 != 0 {
        return None;
    }
    Some(word)
}

fn read_text_colour(type_code: u16, payload: &[u8]) -> Option<u32> {
    if type_code != PSM_TYPE_CODE_JSTYLE_TEXT_CHAR {
        return None;
    }
    let word = u32_at(payload, TEXT_CHAR_COLOUR_OFFSET)?;
    // Same gate the other two colour readers use: a real COLORREF has a zero
    // high byte, so a word that sets it is a sentinel or a misframing and is
    // refused rather than rendered. Every corpus record passes -- 381 of 381
    // -- which is the measurement that made this field worth reading at all.
    if word >> 24 != 0 {
        return None;
    }
    Some(word)
}

/// Every drawable record's line style, keyed by the stream it lives in and
/// the record's `oid`.
///
/// `oid` is what [`crate::geometry::PidGraphicEntity::graphic_oid`] carries
/// and the stream is what its provenance names, so a renderer can join on the
/// pair without re-deriving anything.
pub type LineStyleIndex = BTreeMap<(String, u32), ResolvedLineStyle>;

/// Every text record's character height, keyed the same way as
/// [`LineStyleIndex`].
pub type TextHeightIndex = BTreeMap<(String, u32), ResolvedTextHeight>;

/// Every boundary record's fill, keyed the same way as [`LineStyleIndex`].
pub type FillIndex = BTreeMap<(String, u32), ResolvedFill>;

/// Resolve the fill of every boundary record in one `.pid`.
///
/// `igBoundary2d` is the only family in this corpus that reaches a fill, and
/// all 20 of its records do — see
/// `docs/analysis/2026-08-10-fill-has-a-consumer-after-all.md`. Keyed like
/// [`line_styles_for_file`] so a renderer joins all three the same way.
///
/// # Errors
///
/// Returns [`PidError`] when the file cannot be opened or read as a compound
/// file.
pub fn fill_styles_for_file(path: &Path) -> Result<FillIndex, PidError> {
    let mut out = FillIndex::new();
    for_each_document(path, &mut |stream, sheet, table| {
        for record in decode_igboundaries(sheet) {
            if let Some(resolved) = table.resolve_fill(record.index) {
                out.insert((stream.to_string(), record.oid), resolved);
            }
        }
    })?;
    Ok(out)
}

/// Resolve the character height of every text record in one `.pid`.
///
/// Keyed like [`line_styles_for_file`], so a renderer joins both the same
/// way. Records whose height does not resolve are absent rather than
/// defaulted — see [`DocumentStyleTable::resolve_text_height`] for when that
/// happens and why a caller should keep its own default.
///
/// # Errors
///
/// Returns [`PidError`] when the file cannot be opened or read as a compound
/// file.
pub fn text_heights_for_file(path: &Path) -> Result<TextHeightIndex, PidError> {
    let mut out = TextHeightIndex::new();
    for_each_document(path, &mut |stream, sheet, table| {
        for record in decode_igtextboxes(sheet) {
            if let Some(resolved) = table.resolve_text_height(record.index) {
                out.insert((stream.to_string(), record.oid), resolved);
            }
        }
    })?;
    Ok(out)
}

/// Resolve the line style of every drawable record in one `.pid`.
///
/// Walks each `Sheet*` stream, decodes the three families that carry an
/// `index` — `igLine2d`, `igPoint2d`, `igLineString2d` — and resolves each
/// against the `StyleCluster` of that sheet's own document.
///
/// Records whose index resolves to something with no line symbology are left
/// out rather than defaulted, so a caller can tell "this record asks for a
/// style I could not follow" from "this record asks for a fill".
///
/// # Errors
///
/// Returns [`PidError`] when the file cannot be opened or read as a compound
/// file. A stream that fails to read individually is skipped rather than
/// failing the whole index.
pub fn line_styles_for_file(path: &Path) -> Result<LineStyleIndex, PidError> {
    let mut out = LineStyleIndex::new();
    for_each_document(path, &mut |stream, sheet, table| {
        let indexed = decode_iglines(sheet)
            .into_iter()
            .map(|record| (record.oid, record.index))
            .chain(
                decode_igpoints(sheet)
                    .into_iter()
                    .map(|record| (record.oid, record.index)),
            )
            .chain(
                decode_iglinestrings(sheet)
                    .into_iter()
                    .map(|record| (record.oid, record.index)),
            );
        for (oid, index) in indexed {
            if let Some(resolved) = table.resolve_line_style(index) {
                out.insert((stream.to_string(), oid), resolved);
            }
        }
    })?;
    Ok(out)
}

/// Call `visit` once per `Sheet*` stream with that sheet's bytes and the
/// style table of the document it belongs to.
///
/// Sheets whose own `StyleCluster` is missing or empty are skipped rather
/// than resolved against someone else's — see the scoping rule in the module
/// docs.
fn for_each_document(
    path: &Path,
    visit: &mut dyn FnMut(&str, &[u8], &DocumentStyleTable),
) -> Result<(), PidError> {
    let file = File::open(path)?;
    let mut cfb = ::cfb::CompoundFile::open(file)?;
    let sheet_paths: Vec<String> = cfb
        .walk()
        .filter(::cfb::Entry::is_stream)
        .map(|entry| entry.path().to_string_lossy().into_owned())
        .filter(|name| {
            name.rsplit('/')
                .next()
                .unwrap_or_default()
                .starts_with("Sheet")
        })
        .collect();

    for sheet_path in &sheet_paths {
        let Some(sheet) = read_stream(&mut cfb, sheet_path) else {
            continue;
        };
        let table = read_stream(&mut cfb, &stylecluster_path_for_sheet(sheet_path))
            .map_or_else(DocumentStyleTable::default, |bytes| {
                DocumentStyleTable::from_stylecluster_bytes(&bytes)
            });
        if table.is_empty() {
            continue;
        }
        visit(sheet_path, &sheet, &table);
    }
    Ok(())
}

fn read_stream<R: std::io::Read + std::io::Seek>(
    cfb: &mut ::cfb::CompoundFile<R>,
    path: &str,
) -> Option<Vec<u8>> {
    let mut stream = cfb.open_stream(path).ok()?;
    let mut data = Vec::new();
    stream.read_to_end(&mut data).ok()?;
    Some(data)
}

/// The `StyleCluster` stream governing the geometry in `sheet_path`.
///
/// `/Sheet6` is governed by `/StyleCluster`; `/JSite329/Sheet6` by
/// `/JSite329/StyleCluster`. Resolving across that boundary is the mistake
/// that hid this link, so route every lookup through here rather than
/// searching for a stream whose name contains `Style`.
#[must_use]
pub fn stylecluster_path_for_sheet(sheet_path: &str) -> String {
    match sheet_path.rfind('/') {
        Some(0) | None => "/StyleCluster".to_string(),
        Some(at) => format!("{}/StyleCluster", &sheet_path[..at]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a `StyleCluster` stream out of `(type_code, payload)` pairs.
    fn stream(records: &[(u16, Vec<u8>)]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&CLUSTER_MAGIC.to_le_bytes());
        out.extend_from_slice(&(records.len() as u32).to_le_bytes());
        for (type_code, payload) in records {
            out.extend_from_slice(&type_code.to_le_bytes());
            out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            out.extend_from_slice(payload);
        }
        out
    }

    fn simple_line(style_id: u32, width_m: f64, colour: u32) -> (u16, Vec<u8>) {
        let mut payload = vec![0u8; 54];
        payload[STYLE_ID_OFFSET..STYLE_ID_OFFSET + 4].copy_from_slice(&style_id.to_le_bytes());
        payload[SIMPLE_LINE_WIDTH_OFFSET..SIMPLE_LINE_WIDTH_OFFSET + 8]
            .copy_from_slice(&width_m.to_le_bytes());
        payload[SIMPLE_LINE_COLOUR_OFFSET..SIMPLE_LINE_COLOUR_OFFSET + 4]
            .copy_from_slice(&colour.to_le_bytes());
        (PSM_TYPE_CODE_JSTYLE_SIMPLE_LINE, payload)
    }

    /// A `JStyleSimpleLine` of the longer of the two corpus shapes -- only
    /// the 58-byte one has room for a dash reference at +54, which is why a
    /// dashed line is always the long variant.
    fn simple_line_dashed(style_id: u32, dash_id: u32) -> (u16, Vec<u8>) {
        let mut payload = vec![0u8; 58];
        payload[STYLE_ID_OFFSET..STYLE_ID_OFFSET + 4].copy_from_slice(&style_id.to_le_bytes());
        payload[SIMPLE_LINE_WIDTH_OFFSET..SIMPLE_LINE_WIDTH_OFFSET + 8]
            .copy_from_slice(&0.000_35_f64.to_le_bytes());
        payload[SIMPLE_LINE_DASH_REFERENCE_OFFSET..SIMPLE_LINE_DASH_REFERENCE_OFFSET + 4]
            .copy_from_slice(&dash_id.to_le_bytes());
        (PSM_TYPE_CODE_JSTYLE_SIMPLE_LINE, payload)
    }

    fn dash_type(style_id: u32, segments: &[f64]) -> (u16, Vec<u8>) {
        let mut payload = vec![0u8; DASH_SEGMENT_COUNT_OFFSET + 2 + 8 * segments.len()];
        payload[STYLE_ID_OFFSET..STYLE_ID_OFFSET + 4].copy_from_slice(&style_id.to_le_bytes());
        let count = u16::try_from(segments.len()).expect("test patterns are short");
        payload[DASH_SEGMENT_COUNT_OFFSET..DASH_SEGMENT_COUNT_OFFSET + 2]
            .copy_from_slice(&count.to_le_bytes());
        for (i, value) in segments.iter().enumerate() {
            let at = DASH_SEGMENT_COUNT_OFFSET + 2 + 8 * i;
            payload[at..at + 8].copy_from_slice(&value.to_le_bytes());
        }
        (PSM_TYPE_CODE_JSTYLE_SIMPLE_DASH_TYPE, payload)
    }

    fn override_record(style_id: u32, references: u32) -> (u16, Vec<u8>) {
        let mut payload = vec![0u8; 90];
        payload[STYLE_ID_OFFSET..STYLE_ID_OFFSET + 4].copy_from_slice(&style_id.to_le_bytes());
        payload[BASE_OBJECT_REFERENCE_OFFSET..BASE_OBJECT_REFERENCE_OFFSET + 4]
            .copy_from_slice(&references.to_le_bytes());
        (PSM_TYPE_CODE_JSTYLE_OVERRIDE, payload)
    }

    fn simple_fill(style_id: u32, colour: u32) -> (u16, Vec<u8>) {
        let mut payload = vec![0u8; 46];
        payload[STYLE_ID_OFFSET..STYLE_ID_OFFSET + 4].copy_from_slice(&style_id.to_le_bytes());
        payload[SIMPLE_FILL_COLOUR_OFFSET..SIMPLE_FILL_COLOUR_OFFSET + 4]
            .copy_from_slice(&colour.to_le_bytes());
        (PSM_TYPE_CODE_JSTYLE_SIMPLE_FILL, payload)
    }

    #[test]
    fn a_boundary_reaches_its_fill_through_the_override_that_names_it() {
        // 0x00FF0000 is #0000FF, the blue the flow arrowheads read on both
        // DWG-0202 and the gongyi drawing.
        let data = stream(&[simple_fill(20, 0x00FF_0000), override_record(21, 20)]);
        let table = DocumentStyleTable::from_stylecluster_bytes(&data);

        let resolved = table.resolve_fill(21).expect("id 21 is defined");
        assert_eq!(
            resolved.style_id, 20,
            "reports the fill, not the override that named it"
        );
        assert!(resolved.is_solid());
        assert_eq!(
            resolved.rgb(),
            Some([0x00, 0x00, 0xFF]),
            "the fill's stated colour reaches the boundary through the override"
        );
        assert_eq!(resolved.hop, StyleHop::ViaOverride { override_id: 21 });
    }

    #[test]
    fn the_unset_sentinel_reads_as_no_colour() {
        // Every corpus document defines one template fill whose colour is the
        // -2 sentinel; it must read as "no colour" so the caller keeps its
        // layer default rather than drawing 0xFFFFFFFE as a colour.
        let data = stream(&[simple_fill(3, SIMPLE_FILL_COLOUR_UNSET)]);
        let table = DocumentStyleTable::from_stylecluster_bytes(&data);
        let resolved = table.resolve_fill(3).expect("id 3 is defined");
        assert!(resolved.is_solid());
        assert_eq!(
            resolved.colour, None,
            "the -2 sentinel is not a colour to be drawn"
        );
        assert_eq!(resolved.rgb(), None);
    }

    #[test]
    fn a_black_fill_is_a_stated_colour_not_the_unset_sentinel() {
        // #000000 is a real value the corpus states, distinct from "unset":
        // its high byte is zero, the sentinel's is not.
        let data = stream(&[simple_fill(13, 0x0000_0000)]);
        let table = DocumentStyleTable::from_stylecluster_bytes(&data);
        let resolved = table.resolve_fill(13).expect("id 13 is defined");
        assert_eq!(resolved.rgb(), Some([0, 0, 0]));
    }

    #[test]
    fn a_line_style_is_not_a_fill() {
        // The corpus separates cleanly on this: every line, point and text
        // record misses, and every boundary hits. A permissive resolve_fill
        // would erase that signal.
        let data = stream(&[
            simple_line(7, 0.000_35, 0),
            text_char(8),
            override_record(9, 7),
        ]);
        let table = DocumentStyleTable::from_stylecluster_bytes(&data);
        assert!(table.resolve_fill(7).is_none());
        assert!(table.resolve_fill(8).is_none());
        assert!(
            table.resolve_fill(9).is_none(),
            "an override onto a line style is not a fill"
        );
        assert!(table.resolve_fill(404).is_none(), "undefined id");
    }

    fn text_char(style_id: u32) -> (u16, Vec<u8>) {
        text_char_of_height(style_id, 0.0025)
    }

    fn text_char_of_height(style_id: u32, height_m: f64) -> (u16, Vec<u8>) {
        let mut payload = vec![0u8; 94];
        payload[STYLE_ID_OFFSET..STYLE_ID_OFFSET + 4].copy_from_slice(&style_id.to_le_bytes());
        payload[TEXT_CHAR_HEIGHT_OFFSET..TEXT_CHAR_HEIGHT_OFFSET + 8]
            .copy_from_slice(&height_m.to_le_bytes());
        (PSM_TYPE_CODE_JSTYLE_TEXT_CHAR, payload)
    }

    fn text_char_of_colour(style_id: u32, colour: u32) -> (u16, Vec<u8>) {
        let (type_code, mut payload) = text_char_of_height(style_id, 0.003_175);
        payload[TEXT_CHAR_COLOUR_OFFSET..TEXT_CHAR_COLOUR_OFFSET + 4]
            .copy_from_slice(&colour.to_le_bytes());
        (type_code, payload)
    }

    /// The colour rides the same two hops as the height, so a paragraph style
    /// resolves both at once.
    #[test]
    fn a_paragraph_style_resolves_to_the_colour_of_the_character_style_it_names() {
        let data = stream(&[text_char_of_colour(4, 0x0000_00FF), text_para(9, 4)]);
        let table = DocumentStyleTable::from_stylecluster_bytes(&data);

        let resolved = table.resolve_text_height(9).expect("id 9 is defined");
        assert_eq!(resolved.style_id, 4);
        assert_eq!(resolved.colour, Some(0x0000_00FF));
        assert_eq!(resolved.rgb(), Some([0xFF, 0x00, 0x00]), "COLORREF is BGR");
    }

    /// Black is a colour a drawing states, not an absent one. 367 of the
    /// corpus's 381 character styles letter in it, and a renderer that treated
    /// it as "unstated" would put every one of them back on its layer default.
    #[test]
    fn a_character_style_that_letters_in_black_states_a_colour() {
        let data = stream(&[text_char_of_colour(4, 0)]);
        let table = DocumentStyleTable::from_stylecluster_bytes(&data);

        let resolved = table.resolve_text_height(4).expect("defined");
        assert_eq!(resolved.colour, Some(0));
        assert_eq!(resolved.rgb(), Some([0, 0, 0]));
    }

    /// A word with its high byte set is not a `COLORREF`; the caller keeps its
    /// own default rather than rendering a misframed read.
    #[test]
    fn a_character_style_colour_with_a_high_byte_is_refused() {
        let data = stream(&[text_char_of_colour(4, 0xFFFF_FFFF)]);
        let table = DocumentStyleTable::from_stylecluster_bytes(&data);

        let resolved = table.resolve_text_height(4).expect("defined");
        assert_eq!(resolved.colour, None);
        assert_eq!(resolved.rgb(), None);
    }

    fn text_para(style_id: u32, names: u32) -> (u16, Vec<u8>) {
        let mut payload = vec![0u8; 90];
        payload[STYLE_ID_OFFSET..STYLE_ID_OFFSET + 4].copy_from_slice(&style_id.to_le_bytes());
        payload[TEXT_PARA_CHAR_REFERENCE_OFFSET..TEXT_PARA_CHAR_REFERENCE_OFFSET + 4]
            .copy_from_slice(&names.to_le_bytes());
        (PSM_TYPE_CODE_JSTYLE_TEXT_PARA, payload)
    }

    #[test]
    fn a_line_style_resolves_to_the_dash_pattern_it_names() {
        let data = stream(&[
            dash_type(17, &[-0.014, 0.001_75, 0.000_35, 0.001_75]),
            simple_line_dashed(30, 17),
        ]);
        let table = DocumentStyleTable::from_stylecluster_bytes(&data);

        let dash = table
            .resolve_line_style(30)
            .expect("id 30 is defined")
            .dash
            .expect("it names a dash type");
        assert_eq!(dash.len(), 4);
        let mm = dash.segments_mm();
        assert!((mm[0] + 14.0).abs() < 1e-9, "sign is preserved: {mm:?}");
        assert!((mm[1] - 1.75).abs() < 1e-9);
        assert!((dash.period_m() - 0.017_85).abs() < 1e-9);
    }

    #[test]
    fn a_line_style_with_no_room_for_a_dash_reference_draws_solid() {
        // The 54-byte shape stops before +54, so the field is absent rather
        // than zero -- and absent has to read as solid, not as a failure.
        let data = stream(&[
            dash_type(17, &[-0.0035, -0.001_75]),
            simple_line(30, 0.0035, 0),
        ]);
        let table = DocumentStyleTable::from_stylecluster_bytes(&data);
        let resolved = table.resolve_line_style(30).expect("id 30 is defined");
        assert!(resolved.dash.is_none());
        assert!((resolved.symbology.width_mm() - 3.5).abs() < 1e-9);
    }

    #[test]
    fn a_dash_record_whose_length_disagrees_with_its_count_is_refused() {
        let (type_code, mut payload) = dash_type(17, &[-0.0035, -0.001_75]);
        payload.push(0);
        let data = stream(&[(type_code, payload), simple_line_dashed(30, 17)]);
        let table = DocumentStyleTable::from_stylecluster_bytes(&data);
        assert!(
            table
                .resolve_line_style(30)
                .expect("defined")
                .dash
                .is_none(),
            "one stray byte has to break the fit, or the fit proves nothing"
        );
    }

    #[test]
    fn a_dangling_dash_reference_draws_solid() {
        let data = stream(&[simple_line_dashed(30, 404)]);
        let table = DocumentStyleTable::from_stylecluster_bytes(&data);
        assert!(table
            .resolve_line_style(30)
            .expect("defined")
            .dash
            .is_none());
    }

    #[test]
    fn a_paragraph_style_resolves_to_the_height_of_the_character_style_it_names() {
        let data = stream(&[text_char_of_height(4, 0.003_175), text_para(9, 4)]);
        let table = DocumentStyleTable::from_stylecluster_bytes(&data);

        let resolved = table.resolve_text_height(9).expect("id 9 is defined");
        assert_eq!(
            resolved.style_id, 4,
            "reports the character style, not the paragraph"
        );
        assert!((resolved.height_mm() - 3.175).abs() < 1e-9);
    }

    #[test]
    fn a_character_style_named_directly_still_resolves() {
        let data = stream(&[text_char_of_height(4, 0.0035)]);
        let table = DocumentStyleTable::from_stylecluster_bytes(&data);
        assert!((table.resolve_text_height(4).expect("defined").height_mm() - 3.5).abs() < 1e-9);
    }

    #[test]
    fn a_paragraph_style_naming_nothing_declines_to_guess() {
        let data = stream(&[text_para(9, 0), text_para(10, 404)]);
        let table = DocumentStyleTable::from_stylecluster_bytes(&data);
        assert!(table.resolve_text_height(9).is_none());
        assert!(table.resolve_text_height(10).is_none());
    }

    #[test]
    fn the_unexplained_sub_millimetre_height_is_refused() {
        // 21 corpus records reach a character style whose height reads
        // 0.254mm. Nobody has explained it and text that small cannot be
        // what the drawing means, so the caller keeps its own default.
        let data = stream(&[text_char_of_height(4, 0.000_254), text_para(9, 4)]);
        let table = DocumentStyleTable::from_stylecluster_bytes(&data);
        assert_eq!(table.get(4).expect("defined").char_height_m, None);
        assert!(table.resolve_text_height(9).is_none());
    }

    #[test]
    fn a_line_style_carries_no_character_height() {
        let data = stream(&[simple_line(7, 0.000_35, 0)]);
        let table = DocumentStyleTable::from_stylecluster_bytes(&data);
        assert!(table.resolve_text_height(7).is_none());
    }

    #[test]
    fn a_direct_line_style_resolves_to_its_own_width_and_colour() {
        let data = stream(&[simple_line(7, 0.000_35, 0x00FF_0000)]);
        let table = DocumentStyleTable::from_stylecluster_bytes(&data);

        let resolved = table.resolve_line_style(7).expect("id 7 is defined");
        assert_eq!(resolved.style_id, 7);
        assert_eq!(resolved.hop, StyleHop::Direct);
        assert!((resolved.symbology.width_mm() - 0.35).abs() < 1e-9);
        assert_eq!(resolved.symbology.rgb(), [0x00, 0x00, 0xFF]);
    }

    #[test]
    fn an_override_resolves_through_the_base_reference() {
        let data = stream(&[simple_line(3, 0.0007, 0x0000_8080), override_record(9, 3)]);
        let table = DocumentStyleTable::from_stylecluster_bytes(&data);

        let resolved = table.resolve_line_style(9).expect("id 9 is defined");
        assert_eq!(
            resolved.style_id, 3,
            "reports the line style, not the override"
        );
        assert_eq!(resolved.hop, StyleHop::ViaOverride { override_id: 9 });
        assert!((resolved.symbology.width_mm() - 0.7).abs() < 1e-9);
    }

    #[test]
    fn an_override_whose_reference_names_nothing_declines_to_guess() {
        let data = stream(&[override_record(9, 404)]);
        let table = DocumentStyleTable::from_stylecluster_bytes(&data);
        assert!(table.resolve_line_style(9).is_none());
    }

    #[test]
    fn an_override_pointing_at_a_non_line_style_declines_to_guess() {
        let data = stream(&[text_char(3), override_record(9, 3)]);
        let table = DocumentStyleTable::from_stylecluster_bytes(&data);
        assert!(
            table.resolve_line_style(9).is_none(),
            "a fill or text target carries no width; 4 of 48 corpus overrides are like this"
        );
    }

    #[test]
    fn an_override_with_a_zero_reference_references_nothing() {
        let data = stream(&[override_record(9, 0)]);
        let table = DocumentStyleTable::from_stylecluster_bytes(&data);
        assert_eq!(table.get(9).expect("defined").base_reference, None);
        assert!(table.resolve_line_style(9).is_none());
    }

    #[test]
    fn a_text_style_carries_no_line_symbology() {
        let data = stream(&[text_char(4)]);
        let table = DocumentStyleTable::from_stylecluster_bytes(&data);
        assert_eq!(table.len(), 1);
        assert_eq!(table.get(4).expect("defined").symbology, None);
        assert!(table.resolve_line_style(4).is_none());
    }

    #[test]
    fn an_implausible_width_is_rejected_rather_than_surfaced() {
        // 1e9 metres is what a two-byte framing slip turns a double into.
        let data = stream(&[simple_line(1, 1.0e9, 0)]);
        let table = DocumentStyleTable::from_stylecluster_bytes(&data);
        assert_eq!(table.get(1).expect("defined").symbology, None);
        assert!(table.resolve_line_style(1).is_none());
    }

    #[test]
    fn an_unknown_id_resolves_to_nothing() {
        let data = stream(&[simple_line(7, 0.000_35, 0)]);
        let table = DocumentStyleTable::from_stylecluster_bytes(&data);
        assert!(table.resolve_line_style(8).is_none());
    }

    #[test]
    fn a_stream_without_the_cluster_magic_yields_an_empty_table() {
        let mut data = stream(&[simple_line(7, 0.000_35, 0)]);
        data[0] ^= 0xFF;
        assert!(DocumentStyleTable::from_stylecluster_bytes(&data).is_empty());
    }

    #[test]
    fn the_walk_stops_rather_than_spinning_on_a_zero_length_record() {
        let mut data = stream(&[simple_line(7, 0.000_35, 0)]);
        // Blank the length word of the only record.
        let len_at = STREAM_HEADER_LEN + 2;
        data[len_at..len_at + 4].copy_from_slice(&0u32.to_le_bytes());
        assert!(DocumentStyleTable::from_stylecluster_bytes(&data).is_empty());
    }

    #[test]
    fn a_record_running_past_the_end_is_dropped_not_read() {
        let mut data = stream(&[simple_line(7, 0.000_35, 0)]);
        data.truncate(data.len() - 8);
        assert!(DocumentStyleTable::from_stylecluster_bytes(&data).is_empty());
    }

    #[test]
    fn a_payload_too_short_to_hold_an_id_is_skipped() {
        let data = stream(&[(PSM_TYPE_CODE_JSTYLE_SIMPLE_LINE, vec![0u8; 4])]);
        assert!(DocumentStyleTable::from_stylecluster_bytes(&data).is_empty());
    }

    #[test]
    fn the_table_is_panic_safe_on_adversarial_input() {
        let patterns: Vec<Vec<u8>> = vec![
            Vec::new(),
            vec![0u8; 3],
            vec![0u8; 4096],
            vec![0xFFu8; 4096],
            (0..4096).map(|i| (i & 0xFF) as u8).collect(),
        ];
        for pattern in &patterns {
            let _ = DocumentStyleTable::from_stylecluster_bytes(pattern);
        }
        // A well-formed header over hostile payload bytes is the nastier case.
        let mut framed = CLUSTER_MAGIC.to_le_bytes().to_vec();
        framed.extend_from_slice(&u32::MAX.to_le_bytes());
        framed.extend_from_slice(&vec![0xFFu8; 1024]);
        let _ = DocumentStyleTable::from_stylecluster_bytes(&framed);
    }

    #[test]
    fn a_sheet_resolves_against_the_style_cluster_of_its_own_document() {
        assert_eq!(stylecluster_path_for_sheet("/Sheet6"), "/StyleCluster");
        assert_eq!(
            stylecluster_path_for_sheet("/JSite329/Sheet6"),
            "/JSite329/StyleCluster"
        );
        assert_eq!(
            stylecluster_path_for_sheet("/JSite329/Nested/Sheet6615"),
            "/JSite329/Nested/StyleCluster"
        );
        assert_eq!(stylecluster_path_for_sheet("Sheet6"), "/StyleCluster");
    }
}
