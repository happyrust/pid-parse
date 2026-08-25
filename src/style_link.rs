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
//!   nothing left over — all 669 that `decode_iglines` / `decode_igpoints` /
//!   `decode_iglinestrings` / `decode_igsymbols` accept across the four
//!   fixtures. (A raw chain walk finds more; the twelve polylines it finds
//!   and they refuse fail on their own validation rules, which is a decoder
//!   coverage question, not a link question. The four `igLine2d` in
//!   `DWG-0202/Sheet6615` that used to sit beside them are in the count now —
//!   and all four resolve, which is one more reading that refusing them was
//!   the decoder's error and not the file's.)
//!   The ratchet in `tests/style_link_ratchet.rs` pins the counts.
//! * A symbol placement's slot is `igSymbol2d +25` rather than `+14`, and the
//!   style it names covers the placed body's whole line work, overriding the
//!   per-stroke styles the `.sym` library states: `DWG-0201`'s vessel is
//!   authored black in `Parametric Manifold.sym` and shows on `SmartPlant`'s
//!   screen in its placement's `#800000`; `Off-Unit.sym` authors cyan strokes
//!   and the screen shows its placement's `#808000`. The palette the 107
//!   placements resolve to is item-class colouring — equipment `#800000`,
//!   piping `#808000`, instruments `#008000`, electric trace `#0000FF`,
//!   annotation black. See
//!   `docs/analysis/2026-08-24-placement-names-the-body-style.md`.
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
    decode_igboundaries, decode_iglines, decode_iglinestrings, decode_igpoints, decode_igsymbols,
    decode_igtextboxes, IGLINE2D_PAYLOAD_LEN, PSM_TYPE_CODE_IGLINE2D,
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

/// PSM type code of `JStylePointSymbol` — "JSL PointSymbol Style", CLSID
/// `{47FCC33B-2D0F-11D0-A1FF-080036A1CF02}`. Level: native-reader.
///
/// The class implements `IJGraphic` (`style.dll` carries the interface name
/// `JStylePointSymbol::IJGraphicImp`), which is the whole reason a style
/// record can own geometry: a point symbol *is* a graphic, and the shape it
/// draws lives in a group of line records it names at
/// [`POINT_SYMBOL_GROUP_REFERENCE_OFFSET`].
pub const PSM_TYPE_CODE_JSTYLE_POINT_SYMBOL: u16 = 0x0032;

/// PSM type code of `JStyleLineTerminator` — "JSL Line Terminator Style",
/// CLSID `{47FCC33C-2D0F-11D0-A1FF-080036A1CF02}`. Level: native-reader.
///
/// The style a line names for the mark drawn at its ends. It holds **two**
/// reference slots — `style.dll`'s copy helper walks object `+88` and `+92`,
/// each with its own cached object pointer, which is a start terminator and
/// an end terminator. Every corpus record populates only the second, at
/// [`LINE_TERMINATOR_POINT_SYMBOL_REFERENCE_OFFSET`].
pub const PSM_TYPE_CODE_JSTYLE_LINE_TERMINATOR: u16 = 0x0033;

/// PSM type code of `Group implementation` (`imagdex.dex`, CLSID
/// `{DA02A6D0-C991-11CD-B02F-08003601BE3A}`), the container a
/// `JStylePointSymbol` names to hold its glyph. Level: native-reader.
///
/// Not a style family: it is keyed by `oid` rather than by style id, and its
/// `+14` collides with the id of the line style beside it. That collision is
/// exactly why it is kept out of [`STYLE_FAMILY_TYPE_CODES`].
pub const PSM_TYPE_CODE_GROUP: u16 = 0x007B;

/// Offset at which a `JStyleLineTerminator` names its `JStylePointSymbol`.
///
/// Level: corpus, and exhaustive — every `JStyleLineTerminator` in all five
/// fixtures names a locally defined `JStylePointSymbol` here, and the pairing
/// is one-to-one. The guide has recorded since 2026-08-05 that the two
/// families "only ever appear in `StyleCluster`, in equal numbers, one pair
/// per drawing"; this is the reference that pairs them.
pub const LINE_TERMINATOR_POINT_SYMBOL_REFERENCE_OFFSET: usize = 46;

/// Offset at which a `JStylePointSymbol` names the `oid` of the group holding
/// its glyph.
///
/// Level: corpus. It is an `oid`, not a style id — the group is not a style
/// family and has no id in that space.
pub const POINT_SYMBOL_GROUP_REFERENCE_OFFSET: usize = 26;

/// Offset of a group's member count, a `u16`, within its payload.
pub const GROUP_MEMBER_COUNT_OFFSET: usize = 16;

/// Offset of a group's first member entry. Each entry is
/// [`GROUP_MEMBER_ENTRY_LEN`] bytes: the member's `oid`, the four `i16` of
/// its cached bounding box, and a two-byte kind word.
pub const GROUP_MEMBER_TABLE_OFFSET: usize = 28;

/// Bytes per group member entry.
pub const GROUP_MEMBER_ENTRY_LEN: usize = 14;

/// Most members a group may declare before the record is refused. Every
/// corpus group declares two.
const MAX_GROUP_MEMBERS: usize = 8;

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

/// Offset of the horizontal alignment, a `u8`, within a `JStyleTextPara`
/// payload.
///
/// Level: **native-reader**. `style.dll`'s serialiser (`sub_100337A0`) reads
/// the whole 90-byte record in order, and this byte is the fifth field —
/// immediately before the vertical alignment and four bytes before the
/// [`TEXT_PARA_CHAR_REFERENCE_OFFSET`] pointer this module already relies on,
/// which anchors the whole read. It is also one of only two bytes the
/// `IJStyleTextParaImp` interface exposes with a get/put pair.
pub const TEXT_PARA_HORIZONTAL_ALIGNMENT_OFFSET: usize = 35;

/// Offset of the line spacing multiple, an `f64`, within a `JStyleTextPara`
/// payload.
///
/// Level: **native-reader**. It is the fourth of the six doubles
/// `sub_100337A0` reads after the [`TEXT_PARA_CHAR_REFERENCE_OFFSET`] pointer
/// that anchors the record, and the only one of the six that varies: the other
/// five are `0.0` on every corpus record but six. Its values are `1.0` and
/// `1.5`, which is what a spacing *multiple* looks like rather than a length.
///
/// **This field was measured before it was wired, and the measurement is the
/// reason it is here.** Line spacing only moves a glyph when a label has a
/// second line, and nobody had checked whether this corpus has one. It does,
/// barely: 4 of 235 `igTextBox` labels carry a `U+000D` in their text, on one
/// drawing. What makes the field worth reading is not the 4 but the way they
/// line up with it — **every multi-line label states `1.5`, and not one of the
/// 228 labels stating `1.0` has a second line.** A field that only ever varies
/// where the thing it claims to govern also varies is not a coincidence.
/// See `docs/analysis/2026-08-22-four-labels-have-a-second-line.md`.
pub const TEXT_PARA_LINE_SPACING_OFFSET: usize = 66;

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

/// Offset of the font name's length, a `u16` count of UTF-16 code units,
/// within a `JStyleTextChar` payload.
///
/// Level: **native-reader**. The name is the last thing `sub_10030A20` reads,
/// so the count and the body together account for the rest of the record —
/// which is what [`TEXT_CHAR_FONT_NAME_OFFSET`] documents.
pub const TEXT_CHAR_FONT_NAME_COUNT_OFFSET: usize = 68;

/// Offset of the font name itself, UTF-16 code units with no terminator,
/// within a `JStyleTextChar` payload.
///
/// Level: **native-reader**, and the corpus agrees exactly: `payload.len() ==
/// 70 + 2 * count` holds for **381 of 381** records, and every one of them
/// carves a real typeface at `+70` — `Arial` 111, `Arial Narrow` 108,
/// `宋体` 79, `仿宋` 26, `仿宋_GB2312` 25, `SimSun-ExtB` 15, `Braggadocio` 4,
/// `Intergraph ANSI` 1. Nothing is unreadable. The guide used to read this
/// field by scanning the payload for its longest UTF-16 run, which produced
/// mojibake; the read order says where the name is instead of guessing.
///
/// Twelve of the names are damaged on the vendor's side rather than ours.
/// Eight read `ËÎÌå`, which is `宋体`'s GB2312 bytes (`CB CE CC E5`) widened
/// one byte per code unit — a narrow string written into a wide buffer without
/// transcoding. Four more read `匪_GB2312`, damaged some other way. They are
/// carried through exactly as stored: reconstructing a name is a guess, and a
/// name no installed font matches fails over to the consumer's default, which
/// is the same outcome a failed reconstruction would reach without writing an
/// invented typeface into the document.
pub const TEXT_CHAR_FONT_NAME_OFFSET: usize = 70;

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

/// Smallest and largest line spacing multiple accepted as real.
///
/// The window is not ours: `0.25..=4.0` is the range DXF allows MTEXT's line
/// spacing factor (group 44), which is the field a consumer will carry this
/// into. The corpus sits well inside it, at `1.0` and `1.5`.
///
/// The corpus's third value, `0.0`, falls outside by construction rather than
/// by taste — a zero multiple stacks every line of a paragraph onto the first.
/// It reads as "no spacing stated" and the caller keeps its own default, the
/// same way an unstated fill colour is handled. No label in the corpus reaches
/// one: all 8 such paragraph records sit in the style table unreferenced.
const MIN_PLAUSIBLE_LINE_SPACING: f64 = 0.25;
const MAX_PLAUSIBLE_LINE_SPACING: f64 = 4.0;

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

/// One stroke of a point symbol's glyph, in metres, exactly as the group's
/// `igLine2d` member states it.
///
/// The coordinates are relative to the symbol's own origin, not to the sheet.
/// Where the record draws is the consumer's business: this crate reports the
/// glyph, and how large it renders on a screen is **not** stated anywhere in
/// the file (see [`PointMarker`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarkerStroke {
    /// Start of the stroke, `(x, y)` in metres.
    pub start: (f64, f64),
    /// End of the stroke, `(x, y)` in metres.
    pub end: (f64, f64),
}

impl MarkerStroke {
    /// Whether the stroke has no length.
    ///
    /// A zero-length line is not a line — the same rule
    /// [`crate::parsers::sheet_records::decode_iglines`] applies to sheet
    /// geometry — and a glyph made only of these is a symbol with nothing to
    /// draw.
    #[must_use]
    pub fn is_degenerate(&self) -> bool {
        self.start == self.end
    }

    /// Length of the stroke in millimetres.
    #[must_use]
    pub fn length_mm(&self) -> f64 {
        ((self.end.0 - self.start.0).powi(2) + (self.end.1 - self.start.1).powi(2)).sqrt() * 1000.0
    }
}

/// The glyph a `JStylePointSymbol` draws, read out of the group it names.
///
/// # What the file does and does not state
///
/// The **shape** is the file's: each stroke is an `igLine2d` (PSM `0x0018`,
/// "Line Object") member of a `Group implementation` (PSM `0x007B`), read
/// with the same payload layout
/// [`crate::parsers::sheet_records::decode_iglines`] uses for sheet geometry.
/// Three distinct glyphs occur across the corpus — a slash, a five-millimetre
/// cross, and a bent two-stroke caret — and a drawing picks between them per
/// item class.
///
/// The **drawn size** is not. On the one drawing with screen truth
/// (`DWG-0201`) the slash renders about 1.7 times the stated length, anchored
/// so the point sits inside the long stroke rather than at its origin. No
/// `f64` anywhere in that `.pid` holds either number, so the magnification and
/// the anchoring convention belong to the viewer. `style.dll` hands its
/// rendering to `render.dll`, which is not in the reversed set, so the site
/// that applies them has not been read. Consumers get the glyph as stated;
/// scaling it to match a particular viewer is their decision, not this
/// crate's. See
/// `docs/analysis/2026-08-25-a-point-draws-the-symbol-its-terminator-names.md`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointMarker {
    /// Style id of the `JStylePointSymbol` reached.
    pub point_symbol_id: u32,
    /// Style id of the `JStyleLineTerminator` that named it.
    pub line_terminator_id: u32,
    strokes: [MarkerStroke; MAX_GROUP_MEMBERS],
    len: u8,
}

impl PointMarker {
    /// The glyph's strokes, in the order the group lists them.
    #[must_use]
    pub fn strokes(&self) -> &[MarkerStroke] {
        &self.strokes[..usize::from(self.len)]
    }

    /// Whether the glyph has anything to draw.
    ///
    /// **This is the file's own answer to "does this point show a mark".** A
    /// blank point symbol is stored as a full group whose members are
    /// zero-length lines, and the drawings agree: across all five fixtures
    /// every point whose symbol is blank is absent from `SmartPlant`'s screen
    /// and every point whose symbol draws is present, with no exceptions in
    /// either direction.
    #[must_use]
    pub fn draws(&self) -> bool {
        self.strokes().iter().any(|s| !s.is_degenerate())
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
    /// The point symbol that line style's terminator names, when it names
    /// one.
    ///
    /// Shares [`SIMPLE_LINE_DASH_REFERENCE_OFFSET`] with [`Self::dash`]: the
    /// slot is one reference and which of the two it is depends on the family
    /// it lands on, `JStyleSimpleDashType` or `JStyleLineTerminator`. That is
    /// the same "the evidence is in which kind the id lands on" rule the rest
    /// of this module resolves by, and the corpus separates cleanly — no line
    /// style reaches both.
    ///
    /// `None` means the style names no terminator. That is **not** the same
    /// as naming one whose glyph is blank; see [`PointMarker::draws`].
    pub marker: Option<PointMarker>,
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
    /// Horizontal alignment, when this is a `JStyleTextPara` stating one the
    /// vendor's enum names.
    pub text_alignment: Option<TextAlignment>,
    /// Line spacing multiple, when this is a `JStyleTextPara` stating a
    /// plausible one.
    pub line_spacing: Option<f64>,
    /// `JStyleSimpleDashType` this record names, when it is a
    /// `JStyleSimpleLine` that draws dashed.
    ///
    /// The slot is shared with the terminator reference — see
    /// [`ResolvedLineStyle::marker`] — so a populated value here does not by
    /// itself mean the line is dashed.
    pub dash_reference: Option<u32>,
    /// `JStylePointSymbol` this record names, when it is a
    /// `JStyleLineTerminator`.
    pub point_symbol_reference: Option<u32>,
    /// `oid` of the group holding the glyph, when this is a
    /// `JStylePointSymbol`.
    pub group_reference: Option<u32>,
    /// The dash pattern, when this is a `JStyleSimpleDashType` whose own
    /// length agrees with its segment count.
    pub dash: Option<DashPattern>,
    /// Fill colour as a raw Win32 `COLORREF`, when this is a
    /// `JStyleSimpleFill` that states one rather than the unset sentinel.
    pub fill_colour: Option<u32>,
    /// Character colour as a raw Win32 `COLORREF`, when this is a
    /// `JStyleTextChar` whose [`TEXT_CHAR_COLOUR_OFFSET`] word reads as one.
    pub text_colour: Option<u32>,
    /// Typeface name, when this is a `JStyleTextChar` whose tail reads as one.
    pub font_name: Option<String>,
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

/// Which side of its insertion point a paragraph letters from.
///
/// The values are Intergraph's own, not an interpretation of ours:
/// `Interop.RAD2D.dll` — the .NET interop assembly shipped beside the readers
/// — declares `TextHorizontalJustificationConstants` as
/// `igHorizontalTextLeft = 0`, `igHorizontalTextCenter = 1`,
/// `igHorizontalTextRight = 2`. The enum continues with three `Shape` variants
/// (3, 4, 5) that this corpus never uses, so they are refused rather than
/// guessed at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlignment {
    /// `igHorizontalTextLeft`: the insertion point is the left edge.
    Left,
    /// `igHorizontalTextCenter`: the insertion point is the centre.
    Center,
    /// `igHorizontalTextRight`: the insertion point is the right edge.
    Right,
}

impl TextAlignment {
    /// Read the stated byte, refusing anything the text range does not name.
    #[must_use]
    pub fn from_stated(byte: u8) -> Option<Self> {
        match byte {
            0 => Some(Self::Left),
            1 => Some(Self::Center),
            2 => Some(Self::Right),
            _ => None,
        }
    }
}

/// A text record's character style: the height it letters at, the colour and
/// typeface it letters in, the side it letters from, and which style record
/// they came from.
///
/// They all ride the same two hops (`igTextBox` names a `JStyleTextPara`,
/// which names a `JStyleTextChar`), so they are resolved together rather than
/// through several indexes over the same walk.
#[derive(Debug, Clone, PartialEq)]
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
    /// Which side of the insertion point the paragraph letters from, read at
    /// [`TEXT_PARA_HORIZONTAL_ALIGNMENT_OFFSET`].
    ///
    /// Unlike the other two, this comes off the **paragraph** style rather
    /// than the character style — alignment is a property of the run, not the
    /// glyphs. So it is `None` for the one-hop shape, where a text record
    /// names a `JStyleTextChar` outright and there is no paragraph to ask.
    pub alignment: Option<TextAlignment>,
    /// How far apart the lines of a multi-line label sit, as a multiple of the
    /// character height, read at [`TEXT_PARA_LINE_SPACING_OFFSET`].
    ///
    /// Comes off the **paragraph** style for the same reason the alignment
    /// does — spacing between lines is a property of the run — so it is `None`
    /// for the one-hop shape, and `None` again when the stated multiple is
    /// outside [`MIN_PLAUSIBLE_LINE_SPACING`]`..=`[`MAX_PLAUSIBLE_LINE_SPACING`].
    ///
    /// A caller only has anything to do with this when the label it belongs to
    /// has a line break in it. In this corpus that is 4 labels of 235, and
    /// every one of them states `1.5` while every single-line label states
    /// `1.0`.
    pub line_spacing: Option<f64>,
    /// Typeface the character style names, read at
    /// [`TEXT_CHAR_FONT_NAME_OFFSET`]. Carried verbatim, including the twelve
    /// corpus names the vendor damaged on the way out.
    pub font_name: Option<String>,
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
    /// Groups and line objects, keyed by `oid`.
    ///
    /// These two families ride in the same stream as the styles but are not
    /// styles: a point symbol's glyph is ordinary geometry parked in the
    /// style table. They need their own key because their `+14` is the id of
    /// the style they belong to, so several of them share one — indexing them
    /// by style id would let them shadow each other and the line style.
    groups: BTreeMap<u32, Vec<GroupMember>>,
    strokes: BTreeMap<u32, MarkerStroke>,
}

/// One entry of a group's member table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GroupMember {
    /// `oid` of the member record.
    oid: u32,
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
                // A point symbol's glyph rides in this stream as ordinary
                // geometry, so the two families carrying it are collected
                // here rather than skipped with the rest.
                match type_code {
                    PSM_TYPE_CODE_GROUP => {
                        if let (Some(oid), Some(members)) =
                            (u32_at(payload, 0), read_group_members(payload))
                        {
                            table.groups.insert(oid, members);
                        }
                    }
                    PSM_TYPE_CODE_IGLINE2D => {
                        if let (Some(oid), Some(stroke)) =
                            (u32_at(payload, 0), read_marker_stroke(payload))
                        {
                            table.strokes.insert(oid, stroke);
                        }
                    }
                    _ => {}
                }
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
                    text_alignment: read_text_alignment(type_code, payload),
                    line_spacing: read_line_spacing(type_code, payload),
                    dash_reference: read_dash_reference(type_code, payload),
                    point_symbol_reference: read_reference_of(
                        type_code,
                        PSM_TYPE_CODE_JSTYLE_LINE_TERMINATOR,
                        payload,
                        LINE_TERMINATOR_POINT_SYMBOL_REFERENCE_OFFSET,
                    ),
                    group_reference: read_reference_of(
                        type_code,
                        PSM_TYPE_CODE_JSTYLE_POINT_SYMBOL,
                        payload,
                        POINT_SYMBOL_GROUP_REFERENCE_OFFSET,
                    ),
                    dash: read_dash_pattern(type_code, payload),
                    fill_colour: read_fill_colour(type_code, payload),
                    text_colour: read_text_colour(type_code, payload),
                    font_name: read_font_name(type_code, payload),
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
                marker: self.marker_of(target),
                hop: StyleHop::ViaOverride {
                    override_id: record.style_id,
                },
            });
        }
        Some(ResolvedLineStyle {
            style_id: record.style_id,
            symbology: record.symbology?,
            dash: self.dash_of(record),
            marker: self.marker_of(record),
            hop: StyleHop::Direct,
        })
    }

    /// The point symbol a `JStyleSimpleLine`'s terminator names, when its
    /// reference slot lands on a `JStyleLineTerminator` rather than a dash
    /// type.
    ///
    /// Four hops, each one a reference the record states:
    ///
    /// ```text
    /// JStyleSimpleLine     +54  -> JStyleLineTerminator (style id)
    /// JStyleLineTerminator +46  -> JStylePointSymbol    (style id)
    /// JStylePointSymbol    +26  -> Group                (oid)
    /// Group                +28  -> igLine2d members     (oids)
    /// ```
    ///
    /// A chain that breaks anywhere yields `None`, which reads as "this style
    /// names no point symbol" — the same thing an absent reference means, so
    /// nothing is lost by not distinguishing them.
    fn marker_of(&self, line: &StyleRecord) -> Option<PointMarker> {
        let terminator = self.get(line.dash_reference?)?;
        if terminator.type_code != PSM_TYPE_CODE_JSTYLE_LINE_TERMINATOR {
            return None;
        }
        let symbol = self.get(terminator.point_symbol_reference?)?;
        if symbol.type_code != PSM_TYPE_CODE_JSTYLE_POINT_SYMBOL {
            return None;
        }
        let members = self.groups.get(&symbol.group_reference?)?;
        let mut strokes = [MarkerStroke {
            start: (0.0, 0.0),
            end: (0.0, 0.0),
        }; MAX_GROUP_MEMBERS];
        let mut len = 0_u8;
        for member in members {
            // A member this reader cannot read is not silently dropped: the
            // glyph would then be a different shape than the file states.
            let stroke = *self.strokes.get(&member.oid)?;
            *strokes.get_mut(usize::from(len))? = stroke;
            len += 1;
        }
        (len > 0).then_some(PointMarker {
            point_symbol_id: symbol.style_id,
            line_terminator_id: terminator.style_id,
            strokes,
            len,
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
            alignment: para.text_alignment,
            line_spacing: para.line_spacing,
            font_name: char_style.font_name.clone(),
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

/// A reference a record of one particular family states at `offset`.
///
/// Zero means "references nothing" throughout this format, and both id spaces
/// start at 1, so zero is unambiguous.
fn read_reference_of(type_code: u16, wanted: u16, payload: &[u8], offset: usize) -> Option<u32> {
    if type_code != wanted {
        return None;
    }
    u32_at(payload, offset).filter(|referenced| *referenced != 0)
}

/// The `oid` of every member a `Group implementation` record lists.
///
/// The count and the table have to agree with the record's own length, the
/// same closure test [`read_dash_pattern`] uses: a misread count would carve
/// members out of whatever follows. Every corpus group declares two members
/// and closes exactly.
fn read_group_members(payload: &[u8]) -> Option<Vec<GroupMember>> {
    let count = usize::from(u16_at(payload, GROUP_MEMBER_COUNT_OFFSET)?);
    if count == 0 || count > MAX_GROUP_MEMBERS {
        return None;
    }
    let table_end = GROUP_MEMBER_TABLE_OFFSET.checked_add(GROUP_MEMBER_ENTRY_LEN * count)?;
    if payload.len() < table_end {
        return None;
    }
    (0..count)
        .map(|i| {
            u32_at(
                payload,
                GROUP_MEMBER_TABLE_OFFSET + GROUP_MEMBER_ENTRY_LEN * i,
            )
            .map(|oid| GroupMember { oid })
        })
        .collect()
}

/// One `igLine2d` in a `StyleCluster`, read as a glyph stroke.
///
/// Offsets and length are `igLine2d`'s, not new ones: `+18`/`+26`/`+34`/`+42`
/// are `start.x`/`start.y`/`end.x`/`end.y` and the payload is
/// [`IGLINE2D_PAYLOAD_LEN`], exactly as
/// [`crate::parsers::sheet_records::decode_iglines`] reads them on a sheet.
/// A record in the style table that satisfies the sheet family's own layout,
/// and whose `+4` parent names the group listing it, is that family.
///
/// Unlike the sheet decoder this **keeps** a degenerate line: on a sheet a
/// zero-length line is noise, but in a point symbol it is the file's way of
/// saying the symbol is blank, which is the whole reason to read this.
fn read_marker_stroke(payload: &[u8]) -> Option<MarkerStroke> {
    if payload.len() != IGLINE2D_PAYLOAD_LEN {
        return None;
    }
    let coordinates = [18, 26, 34, 42].map(|at| f64_at(payload, at));
    let [start_x, start_y, end_x, end_y] = coordinates;
    let (start_x, start_y, end_x, end_y) = (start_x?, start_y?, end_x?, end_y?);
    if ![start_x, start_y, end_x, end_y]
        .iter()
        .all(|v| v.is_finite())
    {
        return None;
    }
    Some(MarkerStroke {
        start: (start_x, start_y),
        end: (end_x, end_y),
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

fn read_text_alignment(type_code: u16, payload: &[u8]) -> Option<TextAlignment> {
    if type_code != PSM_TYPE_CODE_JSTYLE_TEXT_PARA {
        return None;
    }
    // Refusing the `Shape` half of the vendor enum is not caution for its own
    // sake: those three values mean "align to the shape's box", which needs a
    // box we do not have. Reading one as a text alignment would place the run
    // confidently in the wrong place, which is worse than leaving it alone.
    TextAlignment::from_stated(*payload.get(TEXT_PARA_HORIZONTAL_ALIGNMENT_OFFSET)?)
}

fn read_line_spacing(type_code: u16, payload: &[u8]) -> Option<f64> {
    if type_code != PSM_TYPE_CODE_JSTYLE_TEXT_PARA {
        return None;
    }
    let multiple = f64_at(payload, TEXT_PARA_LINE_SPACING_OFFSET)?;
    // `is_normal` is what rejects the stored `0.0` along with the subnormal
    // that a misframed read produces; the window then rejects the enormous
    // one. Both refusals mean the same thing to a caller -- keep your own
    // default -- which is right, because a paragraph that states no usable
    // spacing is not asking for any particular one.
    if !multiple.is_normal()
        || !(MIN_PLAUSIBLE_LINE_SPACING..=MAX_PLAUSIBLE_LINE_SPACING).contains(&multiple)
    {
        return None;
    }
    Some(multiple)
}

fn read_font_name(type_code: u16, payload: &[u8]) -> Option<String> {
    if type_code != PSM_TYPE_CODE_JSTYLE_TEXT_CHAR {
        return None;
    }
    let count = u16_at(payload, TEXT_CHAR_FONT_NAME_COUNT_OFFSET)? as usize;
    // The name is the last thing the serialiser reads, so the payload has to
    // end exactly where the count says it does. A record that disagrees is
    // framed differently than this reader thinks, and a name carved out of it
    // would be bytes that happen to decode. All 381 corpus records agree.
    if count == 0 || payload.len() != TEXT_CHAR_FONT_NAME_OFFSET + 2 * count {
        return None;
    }
    let mut units = Vec::with_capacity(count);
    for i in 0..count {
        units.push(u16_at(payload, TEXT_CHAR_FONT_NAME_OFFSET + 2 * i)?);
    }
    // Strict rather than lossy: a lone surrogate is not a typeface, and a name
    // pieced together around a replacement character is the guess this reader
    // refuses to make. Damage the vendor did *before* encoding -- the twelve
    // GB2312-widened names -- decodes cleanly and is kept, because it is what
    // the file states.
    let name = String::from_utf16(&units).ok()?;
    if name.chars().any(char::is_control) {
        return None;
    }
    Some(name)
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
            )
            // A symbol placement names the style its whole body draws
            // with — `igSymbol2d +25` — and it wins over the styles the
            // `.sym` library states stroke by stroke: `DWG-0201`'s vessel
            // is authored black in `Parametric Manifold.sym` and shows on
            // `SmartPlant`'s screen in this reference's `#800000`. See
            // `docs/analysis/2026-08-24-placement-names-the-body-style.md`.
            .chain(
                decode_igsymbols(sheet)
                    .into_iter()
                    .map(|record| (record.oid, record.style_ref)),
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

    fn text_char_of_font(style_id: u32, font: &str) -> (u16, Vec<u8>) {
        let units: Vec<u16> = font.encode_utf16().collect();
        let mut payload = vec![0u8; TEXT_CHAR_FONT_NAME_OFFSET + 2 * units.len()];
        payload[STYLE_ID_OFFSET..STYLE_ID_OFFSET + 4].copy_from_slice(&style_id.to_le_bytes());
        payload[TEXT_CHAR_HEIGHT_OFFSET..TEXT_CHAR_HEIGHT_OFFSET + 8]
            .copy_from_slice(&0.003_175f64.to_le_bytes());
        let count = u16::try_from(units.len()).expect("test names are short");
        payload[TEXT_CHAR_FONT_NAME_COUNT_OFFSET..TEXT_CHAR_FONT_NAME_COUNT_OFFSET + 2]
            .copy_from_slice(&count.to_le_bytes());
        for (i, unit) in units.iter().enumerate() {
            let at = TEXT_CHAR_FONT_NAME_OFFSET + 2 * i;
            payload[at..at + 2].copy_from_slice(&unit.to_le_bytes());
        }
        (PSM_TYPE_CODE_JSTYLE_TEXT_CHAR, payload)
    }

    /// The typeface rides the same two hops as the height and the colour.
    #[test]
    fn a_paragraph_style_resolves_to_the_typeface_of_the_character_style_it_names() {
        let data = stream(&[text_char_of_font(4, "Arial Narrow"), text_para(9, 4)]);
        let table = DocumentStyleTable::from_stylecluster_bytes(&data);

        let resolved = table.resolve_text_height(9).expect("id 9 is defined");
        assert_eq!(resolved.style_id, 4);
        assert_eq!(resolved.font_name.as_deref(), Some("Arial Narrow"));
    }

    /// The name is the last thing the record holds, so its stated length has
    /// to reach the end of the payload exactly. A record where it does not is
    /// framed differently than this reader thinks, and the bytes at `+70`
    /// would be whatever else lives there.
    #[test]
    fn a_character_style_whose_name_overruns_its_payload_states_no_typeface() {
        let (type_code, mut payload) = text_char_of_font(4, "Arial");
        payload[TEXT_CHAR_FONT_NAME_COUNT_OFFSET..TEXT_CHAR_FONT_NAME_COUNT_OFFSET + 2]
            .copy_from_slice(&99u16.to_le_bytes());
        let data = stream(&[(type_code, payload)]);
        let table = DocumentStyleTable::from_stylecluster_bytes(&data);

        let resolved = table.resolve_text_height(4).expect("defined");
        assert_eq!(resolved.font_name, None);
    }

    /// Twelve corpus names are damaged before they were ever encoded -- the
    /// vendor widened `宋体`'s GB2312 bytes one per code unit. The reader
    /// hands back what the file says. Reconstructing the intended name is a
    /// guess, and a name no font matches falls back to the consumer's default
    /// anyway, which is where a wrong guess would land it too -- except a
    /// guess also writes an invented typeface into the document.
    #[test]
    fn a_character_style_states_the_damaged_name_the_vendor_wrote() {
        let data = stream(&[text_char_of_font(4, "ËÎÌå")]);
        let table = DocumentStyleTable::from_stylecluster_bytes(&data);

        let resolved = table.resolve_text_height(4).expect("defined");
        assert_eq!(resolved.font_name.as_deref(), Some("ËÎÌå"));
    }

    fn text_para(style_id: u32, names: u32) -> (u16, Vec<u8>) {
        let mut payload = vec![0u8; 90];
        payload[STYLE_ID_OFFSET..STYLE_ID_OFFSET + 4].copy_from_slice(&style_id.to_le_bytes());
        payload[TEXT_PARA_CHAR_REFERENCE_OFFSET..TEXT_PARA_CHAR_REFERENCE_OFFSET + 4]
            .copy_from_slice(&names.to_le_bytes());
        (PSM_TYPE_CODE_JSTYLE_TEXT_PARA, payload)
    }

    fn text_para_aligned(style_id: u32, names: u32, stated: u8) -> (u16, Vec<u8>) {
        let (type_code, mut payload) = text_para(style_id, names);
        payload[TEXT_PARA_HORIZONTAL_ALIGNMENT_OFFSET] = stated;
        (type_code, payload)
    }

    /// Alignment comes off the paragraph while height and colour come off the
    /// character style it names, so one walk has to carry both ends.
    #[test]
    fn a_paragraph_style_resolves_to_the_side_it_letters_from() {
        for (stated, expected) in [
            (0u8, TextAlignment::Left),
            (1, TextAlignment::Center),
            (2, TextAlignment::Right),
        ] {
            let data = stream(&[
                text_char_of_height(4, 0.003_175),
                text_para_aligned(9, 4, stated),
            ]);
            let table = DocumentStyleTable::from_stylecluster_bytes(&data);

            let resolved = table.resolve_text_height(9).expect("id 9 is defined");
            assert_eq!(resolved.alignment, Some(expected), "stated {stated}");
            // The hop still lands on the character style for the other two.
            assert_eq!(resolved.style_id, 4);
        }
    }

    /// The vendor enum continues past `Right` into three `Shape` variants that
    /// align to a box we do not have. Reading one as a text alignment would
    /// place the run confidently in the wrong place.
    #[test]
    fn a_paragraph_style_aligned_to_a_shape_is_refused() {
        for stated in 3u8..=5 {
            let data = stream(&[
                text_char_of_height(4, 0.003_175),
                text_para_aligned(9, 4, stated),
            ]);
            let table = DocumentStyleTable::from_stylecluster_bytes(&data);

            let resolved = table.resolve_text_height(9).expect("id 9 is defined");
            assert_eq!(resolved.alignment, None, "stated {stated}");
        }
    }

    /// A text record can name a character style outright, skipping the
    /// paragraph. There is nothing to ask about alignment then, and inventing
    /// a default would be indistinguishable from a stated left.
    #[test]
    fn a_character_style_reached_without_a_paragraph_states_no_alignment() {
        let data = stream(&[text_char_of_height(4, 0.003_175)]);
        let table = DocumentStyleTable::from_stylecluster_bytes(&data);

        let resolved = table.resolve_text_height(4).expect("defined");
        assert_eq!(resolved.alignment, None);
        assert_eq!(
            resolved.line_spacing, None,
            "spacing is a paragraph property too"
        );
    }

    fn text_para_spaced(style_id: u32, names: u32, multiple: f64) -> (u16, Vec<u8>) {
        let (type_code, mut payload) = text_para(style_id, names);
        payload[TEXT_PARA_LINE_SPACING_OFFSET..TEXT_PARA_LINE_SPACING_OFFSET + 8]
            .copy_from_slice(&multiple.to_le_bytes());
        (type_code, payload)
    }

    /// Spacing rides the same walk as the alignment, off the paragraph end of
    /// it, while the height still comes off the character style.
    #[test]
    fn a_paragraph_style_resolves_to_the_spacing_between_its_lines() {
        for multiple in [1.0_f64, 1.5] {
            let data = stream(&[
                text_char_of_height(4, 0.003_175),
                text_para_spaced(9, 4, multiple),
            ]);
            let table = DocumentStyleTable::from_stylecluster_bytes(&data);

            let resolved = table.resolve_text_height(9).expect("id 9 is defined");
            assert_eq!(resolved.line_spacing, Some(multiple));
            assert_eq!(resolved.style_id, 4, "the height still comes off the char");
        }
    }

    /// The corpus's third stored value. A zero multiple would stack every line
    /// of a paragraph onto the first, so it cannot be a spacing a drawing is
    /// asking for -- it is the absence of one, and the caller keeps its own.
    #[test]
    fn a_paragraph_style_stating_zero_spacing_states_none() {
        let data = stream(&[
            text_char_of_height(4, 0.003_175),
            text_para_spaced(9, 4, 0.0),
        ]);
        let table = DocumentStyleTable::from_stylecluster_bytes(&data);

        assert_eq!(table.get(9).expect("defined").line_spacing, None);
        assert_eq!(
            table
                .resolve_text_height(9)
                .expect("the height still resolves")
                .line_spacing,
            None,
            "an unusable spacing must not cost the label its height"
        );
    }

    /// Two bytes of framing slip turn a double into something enormous or
    /// subnormal. Either would be carried into a renderer as a line pitch, so
    /// both are refused rather than surfaced -- the same gate the width and
    /// the height already use.
    #[test]
    fn a_misframed_spacing_is_refused_rather_than_surfaced() {
        for multiple in [1.0e9_f64, 5.0e-324, -1.5, f64::NAN, f64::INFINITY] {
            let data = stream(&[
                text_char_of_height(4, 0.003_175),
                text_para_spaced(9, 4, multiple),
            ]);
            let table = DocumentStyleTable::from_stylecluster_bytes(&data);
            assert_eq!(
                table.get(9).expect("defined").line_spacing,
                None,
                "stated {multiple}"
            );
        }
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

    fn line_terminator(style_id: u32, point_symbol_id: u32) -> (u16, Vec<u8>) {
        let mut payload = vec![0u8; 50];
        payload[STYLE_ID_OFFSET..STYLE_ID_OFFSET + 4].copy_from_slice(&style_id.to_le_bytes());
        let at = LINE_TERMINATOR_POINT_SYMBOL_REFERENCE_OFFSET;
        payload[at..at + 4].copy_from_slice(&point_symbol_id.to_le_bytes());
        (PSM_TYPE_CODE_JSTYLE_LINE_TERMINATOR, payload)
    }

    fn point_symbol(style_id: u32, group_oid: u32) -> (u16, Vec<u8>) {
        let mut payload = vec![0u8; 30];
        payload[STYLE_ID_OFFSET..STYLE_ID_OFFSET + 4].copy_from_slice(&style_id.to_le_bytes());
        let at = POINT_SYMBOL_GROUP_REFERENCE_OFFSET;
        payload[at..at + 4].copy_from_slice(&group_oid.to_le_bytes());
        (PSM_TYPE_CODE_JSTYLE_POINT_SYMBOL, payload)
    }

    fn group(oid: u32, members: &[u32]) -> (u16, Vec<u8>) {
        let mut payload =
            vec![0u8; GROUP_MEMBER_TABLE_OFFSET + GROUP_MEMBER_ENTRY_LEN * members.len() + 4];
        payload[0..4].copy_from_slice(&oid.to_le_bytes());
        let count = u16::try_from(members.len()).expect("test groups are small");
        payload[GROUP_MEMBER_COUNT_OFFSET..GROUP_MEMBER_COUNT_OFFSET + 2]
            .copy_from_slice(&count.to_le_bytes());
        for (i, member) in members.iter().enumerate() {
            let at = GROUP_MEMBER_TABLE_OFFSET + GROUP_MEMBER_ENTRY_LEN * i;
            payload[at..at + 4].copy_from_slice(&member.to_le_bytes());
        }
        (PSM_TYPE_CODE_GROUP, payload)
    }

    /// One glyph stroke, written in the `igLine2d` layout the sheet decoder
    /// uses, with coordinates in metres.
    fn marker_line(oid: u32, start: (f64, f64), end: (f64, f64)) -> (u16, Vec<u8>) {
        let mut payload = vec![0u8; IGLINE2D_PAYLOAD_LEN];
        payload[0..4].copy_from_slice(&oid.to_le_bytes());
        for (at, value) in [(18, start.0), (26, start.1), (34, end.0), (42, end.1)] {
            payload[at..at + 8].copy_from_slice(&value.to_le_bytes());
        }
        (PSM_TYPE_CODE_IGLINE2D, payload)
    }

    /// `DWG-0201`'s slash, as the file states it: a 3×6mm stroke from the
    /// origin plus a short stub below-left of it.
    fn slash_glyph_stream(line_id: u32, blank: bool) -> Vec<u8> {
        let (a_end, b_start, b_end) = if blank {
            ((0.0, 0.0), (0.0, 0.0), (0.0, 0.0))
        } else {
            ((0.003, 0.006), (-0.001, -0.002), (-0.0007, -0.001))
        };
        stream(&[
            simple_line_dashed(line_id, 73),
            line_terminator(73, 72),
            point_symbol(72, 8305),
            group(8305, &[8306, 8308]),
            marker_line(8306, (0.0, 0.0), a_end),
            marker_line(8308, b_start, b_end),
        ])
    }

    /// The chain that decides whether a point shows a mark at all.
    #[test]
    fn a_line_style_resolves_to_the_point_symbol_its_terminator_names() {
        let table = DocumentStyleTable::from_stylecluster_bytes(&slash_glyph_stream(74, false));
        let marker = table
            .resolve_line_style(74)
            .expect("id 74 is defined")
            .marker
            .expect("its terminator names a point symbol");

        assert_eq!(marker.line_terminator_id, 73);
        assert_eq!(marker.point_symbol_id, 72);
        assert!(marker.draws());
        let strokes = marker.strokes();
        assert_eq!(strokes.len(), 2, "the group lists two lines");
        assert!((strokes[0].length_mm() - 6.708_203).abs() < 1e-5);
        assert!(!strokes[1].is_degenerate());
    }

    /// The discovery this whole chain exists to serve. A point that shows
    /// nothing on screen does **not** omit its symbol -- it names one whose
    /// group holds zero-length lines. Reading `Some(marker)` as "draws" puts
    /// a mark on all 53 of `DWG-0201`'s junction points.
    #[test]
    fn a_blank_point_symbol_is_a_group_of_zero_length_lines() {
        let table = DocumentStyleTable::from_stylecluster_bytes(&slash_glyph_stream(70, true));
        let marker = table
            .resolve_line_style(70)
            .expect("id 70 is defined")
            .marker
            .expect("a blank symbol is still a symbol, fully stored");

        assert_eq!(marker.strokes().len(), 2, "the group is complete");
        assert!(
            marker.strokes().iter().all(MarkerStroke::is_degenerate),
            "every stroke is zero-length"
        );
        assert!(!marker.draws(), "so there is nothing to draw");
    }

    /// `+54` is one slot holding either kind of reference, and which one it
    /// is comes off the family it lands on -- the same rule the rest of this
    /// module resolves by. Neither reading may leak into the other.
    #[test]
    fn the_shared_reference_slot_separates_a_dash_type_from_a_terminator() {
        let dashed = stream(&[
            dash_type(17, &[-0.014, 0.001_75]),
            simple_line_dashed(30, 17),
        ]);
        let table = DocumentStyleTable::from_stylecluster_bytes(&dashed);
        let resolved = table.resolve_line_style(30).expect("defined");
        assert!(resolved.dash.is_some(), "a dash type is still a dash");
        assert!(
            resolved.marker.is_none(),
            "and carries no point symbol: {:?}",
            resolved.marker
        );

        let table = DocumentStyleTable::from_stylecluster_bytes(&slash_glyph_stream(74, false));
        let resolved = table.resolve_line_style(74).expect("defined");
        assert!(resolved.marker.is_some());
        assert!(
            resolved.dash.is_none(),
            "a terminator is not a dash pattern: {:?}",
            resolved.dash
        );
    }

    /// A chain that breaks anywhere states no symbol, rather than half of
    /// one: a glyph missing a stroke is a different shape than the file says.
    #[test]
    fn a_broken_point_symbol_chain_states_no_marker() {
        let cases = [
            ("terminator missing", stream(&[simple_line_dashed(74, 404)])),
            (
                "point symbol missing",
                stream(&[simple_line_dashed(74, 73), line_terminator(73, 404)]),
            ),
            (
                "group missing",
                stream(&[
                    simple_line_dashed(74, 73),
                    line_terminator(73, 72),
                    point_symbol(72, 404),
                ]),
            ),
            (
                "a member line missing",
                stream(&[
                    simple_line_dashed(74, 73),
                    line_terminator(73, 72),
                    point_symbol(72, 8305),
                    group(8305, &[8306, 8308]),
                    marker_line(8306, (0.0, 0.0), (0.003, 0.006)),
                ]),
            ),
        ];
        for (what, data) in cases {
            let table = DocumentStyleTable::from_stylecluster_bytes(&data);
            assert!(
                table
                    .resolve_line_style(74)
                    .expect("defined")
                    .marker
                    .is_none(),
                "{what}: a broken chain must not yield a partial glyph"
            );
        }
    }

    /// The group and its line objects carry the **owning style's** id at
    /// `+14` -- in `DWG-0201` two `igLine2d` and two more records all read
    /// id 71 -- so they are keyed by `oid` instead. Admitting them to the
    /// style index would let them shadow the line style and each other.
    #[test]
    fn a_glyph_line_does_not_shadow_the_style_whose_id_it_carries() {
        let mut glyph = marker_line(8306, (0.0, 0.0), (0.003, 0.006));
        let mut twin = marker_line(8308, (-0.001, -0.002), (-0.0007, -0.001));
        // What the real records do: both name the line style they belong to.
        for record in [&mut glyph, &mut twin] {
            record.1[STYLE_ID_OFFSET..STYLE_ID_OFFSET + 4].copy_from_slice(&74u32.to_le_bytes());
        }
        let data = stream(&[
            simple_line_dashed(74, 73),
            line_terminator(73, 72),
            point_symbol(72, 8305),
            group(8305, &[8306, 8308]),
            glyph,
            twin,
        ]);
        let table = DocumentStyleTable::from_stylecluster_bytes(&data);

        assert_eq!(
            table.get(74).map(|record| record.type_code),
            Some(PSM_TYPE_CODE_JSTYLE_SIMPLE_LINE),
            "id 74 is the line style, not one of the two lines carrying that id"
        );
        let marker = table
            .resolve_line_style(74)
            .expect("defined")
            .marker
            .expect("and the glyph still resolves, by oid");
        assert_eq!(marker.strokes().len(), 2);
        assert!(marker.draws());
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
