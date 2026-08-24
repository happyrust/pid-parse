//! Symbol bodies read from the `SmartPlant` `.sym` reference library.
//!
//! An `igSymbol2d` placement in a drawing says *where* a symbol goes and
//! which library file it comes from, but not what it looks like — the body
//! lives in a separate `.sym` file on the project's reference-data share.
//! Without it, a valve can only be marked, not drawn.
//!
//! A `.sym` turns out to be the same compound file as a `.pid`, down to the
//! cluster magic and the 6-byte PSM record envelope, with one structural
//! simplification: a symbol's `Sheet*` stream is a plain record chain
//! starting at [`CHAIN_START`], so the records can be walked rather than
//! searched for. Across the 618-file reference library that walk lands
//! exactly on the last byte of all 1134 sheet streams, which is what makes
//! it safe to read the chain instead of scanning for candidates the way the
//! drawing-side decoders must.
//!
//! Five record types carry something to draw — four geometry families and
//! the symbol's own lettering; the rest are connect points, styles and
//! structure. See [`SymbolPrimitive`].
//!
//! ```no_run
//! use pid_parse::symbol_library::SymbolLibrary;
//!
//! let mut library = SymbolLibrary::new(r"C:\Plant\Ref\Symbols");
//! if let Some(body) = library.resolve(r"\\WIN-SPID\p\Ref\Symbols\Piping\Valves\Angle\2-Way Angle Globe Valve.sym") {
//!     println!("{} primitives", body.primitives.len());
//! }
//! ```

use std::collections::{BTreeMap, HashMap};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::api::PidParser;
use crate::error::PidError;
use crate::style_link::DocumentStyleTable;

/// `u32` LE signature shared by every cluster-family stream, including a
/// symbol's `Sheet*` streams.
const CLUSTER_MAGIC: u32 = 0x6C90_F544;

/// Offset of the first record in a symbol `Sheet*` stream.
///
/// The first 8 bytes are the cluster magic and a record count. The bytes a
/// cluster-header reader would take as `stream_type` / `body_len` / `flags`
/// at 8..16 are in fact the first record's own PSM envelope, which is why
/// the chain starts here and not at 16.
pub const CHAIN_START: usize = 8;

/// Length of the PSM record envelope: `u16` type code + `u32` payload length.
const PSM_ENVELOPE_LEN: usize = 6;

/// Payload bytes ahead of the geometry: oid, parent reference, a `u32`, a
/// `u16` sub-type, and an index. Read positionally — the drawing-side
/// decoders name the same span but validate values a `.sym` does not share
/// (`igLine2d` at `+8` holds 12 in a `.pid` and 8 here).
const PAYLOAD_HEADER_LEN: usize = 18;

/// Offset of the style index inside the payload header, four bytes wide.
const STYLE_INDEX_OFFSET: usize = 14;

const TYPE_LINE: u16 = 0x0018;
const TYPE_CIRCLE: u16 = 0x0059;
const TYPE_ARC: u16 = 0x0061;
const TYPE_POLYLINE: u16 = 0x0084;
const TYPE_TEXT: u16 = 0x004D;

/// Bytes of geometry and style following a text record's characters. The
/// first two `f64`s of it are the insertion point, and the run's constant
/// width is what makes the character count identifiable — see
/// [`TEXT_COUNT_OFFSETS`].
const TEXT_TRAILING_LEN: usize = 36;

/// Payload offsets at which a text record can put its `u16` character count,
/// most common first.
///
/// The drawing-side decoder reads the count at `+30`, and 908 of the
/// library's 1490 text records agree. The rest carry a wider sub-header and
/// put it at `+22`. The two are told apart by the payload size rather than
/// by a flag: with the trailing run fixed at [`TEXT_TRAILING_LEN`], a count
/// is only believed when `payload_len` comes out exactly right, and across
/// the corpus no record is explained by both offsets — 248 fit `+22` alone,
/// none fit both, and every string either recovers is readable text.
const TEXT_COUNT_OFFSETS: [usize; 2] = [30, 22];

/// Longest run of characters a symbol label can plausibly hold. The corpus
/// tops out at 117 (an embedded XML field reference); this only has to
/// reject a count that would reach past the payload anyway.
const TEXT_MAX_CHARS: usize = 1024;

/// Payload length of a line record: header plus four `f64`s.
const LINE_PAYLOAD_LEN: usize = PAYLOAD_HEADER_LEN + 32;
/// Payload length of a circle record: header, three `f64`s, one trailing byte.
const CIRCLE_PAYLOAD_LEN: usize = PAYLOAD_HEADER_LEN + 24 + 1;
/// Payload length of an arc record: header, five `f64`s, one trailing byte.
const ARC_PAYLOAD_LEN: usize = PAYLOAD_HEADER_LEN + 40 + 1;
/// Offset of a polyline's first vertex, past the vertex count and the
/// form/scope bytes.
const POLYLINE_VERTEX_START: usize = 24;
/// Guard against a corrupt vertex count claiming an implausible polyline.
const POLYLINE_MAX_VERTICES: usize = 4096;

/// Widest coordinate a symbol body can plausibly use, in source units
/// (metres). The reference library's own extent is under 0.6; an order of
/// magnitude of headroom rejects garbage without rejecting real geometry.
const COORDINATE_LIMIT: f64 = 10.0;

/// The path separator `SmartPlant` reference shares are addressed by, and the
/// marker that splits the site-specific UNC prefix from the library-relative
/// path. Placements name the same symbol through several different shares
/// (`\\WIN-SPID\...`, `\\SPID\...`, `\\MM-128\...`), and the part after this
/// marker is what they agree on.
const LIBRARY_MARKER: &str = r"\symbols\";

/// One drawable element of a symbol body, in the symbol's own coordinate
/// space (source units, i.e. metres — the same unit the drawing uses).
///
/// The library draws with five of the 27 PSM type codes that appear in it —
/// four geometry families and one text family; the others carry connect
/// points, line styles and object structure, none of which put anything on
/// the sheet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SymbolPrimitive {
    /// Straight segment (PSM `0x0018` `igLine2d`) — 43% of all library
    /// records and the bulk of every symbol.
    Line {
        /// Segment start.
        start: (f64, f64),
        /// Segment end.
        end: (f64, f64),
    },
    /// Full circle (PSM `0x0059` `igCircle2d`).
    Circle {
        /// Circle centre.
        center: (f64, f64),
        /// Radius; always positive across the library.
        radius: f64,
    },
    /// Circular arc (PSM `0x0061`), swept counter-clockwise from
    /// [`Self::Arc::start_angle`] to [`Self::Arc::end_angle`].
    ///
    /// Both angles are absolute, not a start plus a sweep: the two arcs of
    /// `ElecTraceLine` run `2.944 -> 0.197` and `6.086 -> 3.339` radians,
    /// which are different numbers but the same 202.6-degree sweep at the
    /// same radius, as a repeating wave symbol requires.
    Arc {
        /// Arc centre.
        center: (f64, f64),
        /// Radius; always positive across the library.
        radius: f64,
        /// Start angle in radians, counter-clockwise from +X.
        start_angle: f64,
        /// End angle in radians, counter-clockwise from +X.
        end_angle: f64,
    },
    /// Connected vertex run (PSM `0x0084` `igLineString2d`).
    Polyline {
        /// Vertices in source order.
        vertices: Vec<(f64, f64)>,
    },
    /// Lettering the symbol carries itself (PSM `0x004D` `igTextBox`) — the
    /// `设备位号` header on an equipment table, the `HH=` on an alarm, the
    /// `XXX` a note symbol shows as its own sample.
    ///
    /// The library is a set of templates, so a run is often the placeholder
    /// `NULL`, standing for a value the drawing supplies at placement time
    /// rather than text anyone means to read. It is decoded as it appears;
    /// deciding whether to draw it belongs to whoever is drawing.
    ///
    /// Carries no height or font: the record does not hold one, and the
    /// style table it points at is unread.
    Text {
        /// Text content, decoded from UTF-16LE.
        text: String,
        /// Insertion point in the symbol's own coordinate space.
        at: (f64, f64),
    },
}

/// The colour and width one primitive draws with.
///
/// A symbol states its own symbology and the drawing does not state it for
/// them: the placement record carries no style link at all, because the
/// payload slot its `igLine2d` siblings use for the style `index` holds the
/// `JSite` id instead. Every `.sym` carries a `StyleCluster` stream of the
/// same shape a `.pid` does, and each of the symbol's own records indexes
/// into it exactly as a drawing's line work indexes into the drawing's.
///
/// The dash a style may also name is not carried: nothing draws it yet, and
/// a field nobody reads is a field nobody maintains.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PrimitiveStyle {
    /// Stroke colour as `[r, g, b]`.
    pub rgb: [u8; 3],
    /// Stroke width in millimetres.
    pub width_mm: f64,
}

/// One drawable primitive together with the symbology its own `.sym` states
/// for it.
///
/// `style` is `None` where the record's index names nothing this reader can
/// follow — a fill or text style, or a `.sym` with no readable style table.
/// That is "draw it the way you would have anyway", not a failure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StyledPrimitive {
    /// What to draw, in the symbol's own coordinate space.
    pub primitive: SymbolPrimitive,
    /// What to draw it with.
    pub style: Option<PrimitiveStyle>,
}

/// A symbol's drawable body, as read from one `.sym` file.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SymbolGeometry {
    /// Drawable primitives in the symbol's own coordinate space.
    pub primitives: Vec<StyledPrimitive>,
    /// Records the walk stepped over because they carry no shape, keyed by
    /// PSM type code. Reported rather than dropped silently so a symbol that
    /// comes out looking too sparse can be told apart from one that is.
    pub skipped_records: BTreeMap<u16, usize>,
}

impl SymbolGeometry {
    /// Axis-aligned bounds of the body as `(min_x, min_y, max_x, max_y)`,
    /// or `None` when there is nothing to draw.
    ///
    /// Arcs contribute their endpoints and centre rather than a swept
    /// extent, which can under-report a bulge; that is enough for the
    /// placement-level framing this feeds and avoids a quadrant walk.
    pub fn bounds(&self) -> Option<(f64, f64, f64, f64)> {
        let mut acc: Option<(f64, f64, f64, f64)> = None;
        let mut add = |x: f64, y: f64| {
            acc = Some(match acc {
                None => (x, y, x, y),
                Some((min_x, min_y, max_x, max_y)) => {
                    (min_x.min(x), min_y.min(y), max_x.max(x), max_y.max(y))
                }
            });
        };
        for styled in &self.primitives {
            match &styled.primitive {
                SymbolPrimitive::Line { start, end } => {
                    add(start.0, start.1);
                    add(end.0, end.1);
                }
                SymbolPrimitive::Circle { center, radius } => {
                    add(center.0 - radius, center.1 - radius);
                    add(center.0 + radius, center.1 + radius);
                }
                SymbolPrimitive::Arc {
                    center,
                    radius,
                    start_angle,
                    end_angle,
                } => {
                    add(center.0, center.1);
                    for angle in [*start_angle, *end_angle] {
                        add(
                            center.0 + radius * angle.cos(),
                            center.1 + radius * angle.sin(),
                        );
                    }
                }
                SymbolPrimitive::Polyline { vertices } => {
                    for (x, y) in vertices {
                        add(*x, *y);
                    }
                }
                // The insertion point only. How far the lettering actually
                // reaches needs a height and a font, and the record carries
                // neither.
                SymbolPrimitive::Text { at, .. } => add(at.0, at.1),
            }
        }
        acc
    }
}

fn u16_le(bytes: &[u8], at: usize) -> Option<u16> {
    bytes
        .get(at..at + 2)
        .and_then(|s| <[u8; 2]>::try_from(s).ok())
        .map(u16::from_le_bytes)
}

fn u32_le(bytes: &[u8], at: usize) -> Option<u32> {
    bytes
        .get(at..at + 4)
        .and_then(|s| <[u8; 4]>::try_from(s).ok())
        .map(u32::from_le_bytes)
}

fn f64_le(bytes: &[u8], at: usize) -> Option<f64> {
    bytes
        .get(at..at + 8)
        .and_then(|s| <[u8; 8]>::try_from(s).ok())
        .map(f64::from_le_bytes)
}

/// A finite coordinate inside the plausible symbol extent.
fn coordinate(bytes: &[u8], at: usize) -> Option<f64> {
    let value = f64_le(bytes, at)?;
    (value.is_finite() && value.abs() <= COORDINATE_LIMIT).then_some(value)
}

/// Read a text record's characters and insertion point.
///
/// The character count sits at one of [`TEXT_COUNT_OFFSETS`], and which one
/// is settled by arithmetic rather than by a flag: the count is believed
/// only when the characters it claims, plus the fixed
/// [`TEXT_TRAILING_LEN`]-byte trailing run, account for the payload exactly.
/// A wrong offset almost never adds up, and across the reference library no
/// record adds up at both.
fn decode_text(payload: &[u8]) -> Option<SymbolPrimitive> {
    let (text_end, text) = TEXT_COUNT_OFFSETS
        .iter()
        .find_map(|&count_at| read_text_run(payload, count_at))?;
    let at = (
        coordinate(payload, text_end)?,
        coordinate(payload, text_end + 8)?,
    );
    Some(SymbolPrimitive::Text { text, at })
}

/// The run introduced by the `u16` at `count_at`, if it accounts for the
/// payload exactly and decodes to readable text.
fn read_text_run(payload: &[u8], count_at: usize) -> Option<(usize, String)> {
    let count = u16_le(payload, count_at)? as usize;
    if count == 0 || count > TEXT_MAX_CHARS {
        return None;
    }
    let start = count_at + 2;
    let end = start.checked_add(count * 2)?;
    if payload.len() != end.checked_add(TEXT_TRAILING_LEN)? {
        return None;
    }
    let mut chars = Vec::with_capacity(count);
    for index in 0..count {
        chars.push(u16_le(payload, start + index * 2)?);
    }
    let text = String::from_utf16(&chars).ok()?;
    // An unpaired surrogate is caught above; a NUL means the run is not
    // really characters, whatever the arithmetic says.
    (!text.contains('\0')).then_some((end, text))
}

/// Turn one record payload into a primitive, or `None` when the record
/// carries no shape or fails a domain check.
fn decode_primitive(type_code: u16, payload: &[u8]) -> Option<SymbolPrimitive> {
    match type_code {
        TYPE_LINE if payload.len() >= LINE_PAYLOAD_LEN => {
            let start = (
                coordinate(payload, PAYLOAD_HEADER_LEN)?,
                coordinate(payload, PAYLOAD_HEADER_LEN + 8)?,
            );
            let end = (
                coordinate(payload, PAYLOAD_HEADER_LEN + 16)?,
                coordinate(payload, PAYLOAD_HEADER_LEN + 24)?,
            );
            // A zero-length segment draws nothing and only widens bounds.
            (start != end).then_some(SymbolPrimitive::Line { start, end })
        }
        TYPE_CIRCLE if payload.len() >= CIRCLE_PAYLOAD_LEN => {
            let center = (
                coordinate(payload, PAYLOAD_HEADER_LEN)?,
                coordinate(payload, PAYLOAD_HEADER_LEN + 8)?,
            );
            let radius = coordinate(payload, PAYLOAD_HEADER_LEN + 16)?;
            (radius > 0.0).then_some(SymbolPrimitive::Circle { center, radius })
        }
        TYPE_ARC if payload.len() >= ARC_PAYLOAD_LEN => {
            let center = (
                coordinate(payload, PAYLOAD_HEADER_LEN)?,
                coordinate(payload, PAYLOAD_HEADER_LEN + 8)?,
            );
            let radius = coordinate(payload, PAYLOAD_HEADER_LEN + 16)?;
            let start_angle = f64_le(payload, PAYLOAD_HEADER_LEN + 24)?;
            let end_angle = f64_le(payload, PAYLOAD_HEADER_LEN + 32)?;
            if radius <= 0.0 || !start_angle.is_finite() || !end_angle.is_finite() {
                return None;
            }
            Some(SymbolPrimitive::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            })
        }
        TYPE_POLYLINE if payload.len() > POLYLINE_VERTEX_START => {
            let span = payload.len() - POLYLINE_VERTEX_START;
            if !span.is_multiple_of(16) {
                return None;
            }
            let declared = u32_le(payload, PAYLOAD_HEADER_LEN)? as usize;
            let count = span / 16;
            if count != declared || !(2..=POLYLINE_MAX_VERTICES).contains(&count) {
                return None;
            }
            let mut vertices = Vec::with_capacity(count);
            for index in 0..count {
                let at = POLYLINE_VERTEX_START + index * 16;
                vertices.push((coordinate(payload, at)?, coordinate(payload, at + 8)?));
            }
            Some(SymbolPrimitive::Polyline { vertices })
        }
        TYPE_TEXT => decode_text(payload),
        _ => None,
    }
}

/// Walk one symbol `Sheet*` stream's record chain into `out`.
///
/// The chain is only read when it accounts for the stream exactly: a walk
/// that stops early or overruns means the bytes are not the layout this
/// reader knows, and half a symbol drawn from a misread chain is worse than
/// no symbol at all.
fn read_sheet_chain(bytes: &[u8], styles: Option<&DocumentStyleTable>, out: &mut SymbolGeometry) {
    if u32_le(bytes, 0) != Some(CLUSTER_MAGIC) {
        return;
    }
    let mut primitives = Vec::new();
    let mut skipped: BTreeMap<u16, usize> = BTreeMap::new();
    let mut at = CHAIN_START;
    while at + PSM_ENVELOPE_LEN <= bytes.len() {
        let (Some(raw_type), Some(length)) = (u16_le(bytes, at), u32_le(bytes, at + 2)) else {
            return;
        };
        if raw_type == 0 {
            return;
        }
        let body = at + PSM_ENVELOPE_LEN;
        let Some(end) = body.checked_add(length as usize) else {
            return;
        };
        if end > bytes.len() {
            return;
        }
        let type_code = raw_type & 0x0FFF;
        let payload = &bytes[body..end];
        match decode_primitive(type_code, payload) {
            Some(primitive) => primitives.push(StyledPrimitive {
                primitive,
                style: style_of(payload, styles),
            }),
            None => *skipped.entry(type_code).or_default() += 1,
        }
        at = end;
    }
    if at != bytes.len() {
        return;
    }
    out.primitives.append(&mut primitives);
    for (code, count) in skipped {
        *out.skipped_records.entry(code).or_default() += count;
    }
}

/// The symbology a record's own style index names, if the symbol's style
/// table defines it as a line style.
///
/// The index sits at [`STYLE_INDEX_OFFSET`] of the payload, the same slot the
/// drawing-side families carry it in — settled for those in
/// `docs/analysis/2026-08-05-geometry-index-is-the-style-link.md`.
fn style_of(payload: &[u8], styles: Option<&DocumentStyleTable>) -> Option<PrimitiveStyle> {
    let table = styles?;
    let index = u32_le(payload, STYLE_INDEX_OFFSET)?;
    let resolved = table.resolve_line_style(index)?;
    Some(PrimitiveStyle {
        rgb: resolved.symbology.rgb(),
        width_mm: resolved.symbology.width_mm(),
    })
}

/// The storage a stream lives in, which is what a `Sheet*` and the
/// `StyleCluster` that styles it have in common.
fn parent_storage(stream_path: &str) -> &str {
    stream_path.rfind('/').map_or("", |cut| &stream_path[..cut])
}

/// Read a `.sym` file's drawable body.
///
/// # Errors
///
/// Returns the underlying [`PidError`] when the file cannot be opened or is
/// not a readable compound file. A `.sym` that opens but holds no geometry
/// yields an empty [`SymbolGeometry`] rather than an error — an empty symbol
/// is a real thing in the library.
pub fn read_symbol_geometry(path: &Path) -> Result<SymbolGeometry, PidError> {
    let package = PidParser::new().parse_package(path)?;
    // Styles are scoped to the storage they sit in, the same rule
    // `style_link` applies to a drawing: a sheet is resolved against its own
    // document's table and never against another's.
    let mut tables: HashMap<&str, DocumentStyleTable> = HashMap::new();
    for (stream_path, raw) in &package.streams {
        if stream_path.rsplit('/').next() == Some("StyleCluster") {
            tables.insert(
                parent_storage(stream_path),
                DocumentStyleTable::from_stylecluster_bytes(&raw.data),
            );
        }
    }

    let mut geometry = SymbolGeometry::default();
    for (stream_path, raw) in &package.streams {
        let is_sheet = stream_path
            .rsplit('/')
            .next()
            .is_some_and(|name| name.starts_with("Sheet"));
        if !is_sheet || raw.data.len() <= CHAIN_START {
            continue;
        }
        let mut sheet = SymbolGeometry::default();
        read_sheet_chain(
            &raw.data,
            tables.get(parent_storage(stream_path)),
            &mut sheet,
        );

        // A symbol often keeps more than one sheet holding the same body:
        // the angle globe valve's `Sheet63` is nine primitives that are
        // byte-identical to nine of `Sheet6`'s eleven. Drawing both doubles
        // every stroke. Duplicates are dropped across sheets but kept within
        // one, where a repeated stroke is the symbol's own doing. The shape
        // decides that, not the symbology: the same stroke twice in two
        // colours is still the same stroke twice.
        for styled in sheet.primitives {
            if !geometry
                .primitives
                .iter()
                .any(|kept| kept.primitive == styled.primitive)
            {
                geometry.primitives.push(styled);
            }
        }
        for (code, count) in sheet.skipped_records {
            *geometry.skipped_records.entry(code).or_default() += count;
        }
    }
    Ok(geometry)
}

/// The library-relative part of a placement's symbol path.
///
/// Placements carry a UNC path into whichever reference share the project
/// was drawn against — `\\WIN-SPID\qsmcqtaz13\Plant\Ref\Symbols\...`,
/// `\\SPID\XaLNG\P&ID Reference Data\Symbols\...`, `\\MM-128\...` — and the
/// same symbol is named through different shares in different drawings. The
/// part after the `Symbols` directory is the part they agree on, and it is
/// what a local copy of the library is laid out by.
///
/// Returns `None` when the path has no `Symbols` component to split on.
#[must_use]
pub fn library_relative_path(symbol_path: &str) -> Option<&str> {
    let normalized = symbol_path.replace('/', r"\").to_ascii_lowercase();
    let at = normalized.find(LIBRARY_MARKER)?;
    let tail = at + LIBRARY_MARKER.len();
    (tail < symbol_path.len()).then(|| symbol_path[tail..].trim())
}

/// A local copy of a `SmartPlant` symbol library, resolving the UNC paths
/// carried by placements to the bodies on disk.
///
/// More than one root is allowed, searched in order. Drawings from different
/// projects name different reference shares — the fixtures here cite five —
/// and a local machine usually holds several partial copies rather than one
/// merged tree, so a single root leaves whichever project it did not come
/// from undrawn.
///
/// Lookups are cached, including misses: a drawing places the same handful
/// of symbols dozens of times, and a project that lacks a symbol lacks it
/// for every placement.
#[derive(Debug)]
pub struct SymbolLibrary {
    roots: Vec<PathBuf>,
    cache: BTreeMap<String, Option<SymbolGeometry>>,
}

impl SymbolLibrary {
    /// Point the library at the directory that holds the `Design`,
    /// `Piping`, `Equipment` … trees.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            roots: vec![root.into()],
            cache: BTreeMap::new(),
        }
    }

    /// Point the library at several such directories, searched in order.
    pub fn with_roots<P: Into<PathBuf>>(roots: impl IntoIterator<Item = P>) -> Self {
        Self {
            roots: roots.into_iter().map(Into::into).collect(),
            cache: BTreeMap::new(),
        }
    }

    /// Build a library from a `PATH`-style list of roots — `;` separated on
    /// Windows, `:` elsewhere — in search order.
    pub fn from_path_list(list: &OsStr) -> Self {
        Self::with_roots(std::env::split_paths(list))
    }

    /// The directories this library reads from, in search order.
    #[must_use]
    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }

    /// Number of distinct symbol paths looked up so far, resolved or not.
    #[must_use]
    pub fn lookups(&self) -> usize {
        self.cache.len()
    }

    /// Number of distinct symbol paths that resolved to a body.
    #[must_use]
    pub fn resolved(&self) -> usize {
        self.cache.values().filter(|v| v.is_some()).count()
    }

    /// The symbol paths that had no body in this library, in sorted order.
    ///
    /// A miss is normally a symbol from a reference share the local copy was
    /// not taken from, not a decode failure — worth reporting so the gap is
    /// visible.
    #[must_use]
    pub fn missing(&self) -> Vec<&str> {
        self.cache
            .iter()
            .filter(|(_, body)| body.is_none())
            .map(|(path, _)| path.as_str())
            .collect()
    }

    /// Resolve a placement's symbol path to its body.
    ///
    /// Returns `None` when the path is not library-relative, or when no root
    /// holds a readable copy. A symbol that reads but holds no geometry
    /// resolves to an empty body, which callers can tell from a miss.
    pub fn resolve(&mut self, symbol_path: &str) -> Option<&SymbolGeometry> {
        if !self.cache.contains_key(symbol_path) {
            let body = self.read_from_roots(symbol_path);
            self.cache.insert(symbol_path.to_owned(), body);
        }
        self.cache.get(symbol_path)?.as_ref()
    }

    /// Read a symbol from the first root that draws it.
    ///
    /// A root can hold a symbol as a text-only or connect-point body with
    /// nothing to draw while another root has the drawn version, so an empty
    /// body is kept only as a fallback rather than ending the search.
    fn read_from_roots(&self, symbol_path: &str) -> Option<SymbolGeometry> {
        let relative = library_relative_path(symbol_path)?.replace('\\', "/");
        let mut empty = None;
        for root in &self.roots {
            let path = root.join(&relative);
            if !path.is_file() {
                continue;
            }
            let Ok(body) = read_symbol_geometry(&path) else {
                continue;
            };
            if !body.primitives.is_empty() {
                return Some(body);
            }
            empty.get_or_insert(body);
        }
        empty
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn symbols_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("test-file")
            .join("symbols")
    }

    fn fixture(relative: &str) -> Option<PathBuf> {
        let path = symbols_root().join(relative);
        path.is_file().then_some(path)
    }

    fn scratch_root(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos());
        let path = std::env::temp_dir().join(format!(
            "pid-parse-symlib-{tag}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("scratch root");
        path
    }

    const CIRCLE_UNC: &str = r"\\WIN-SPID\p\Ref\Symbols\Design\Annotation\Graphics\Circle.sym";

    #[test]
    fn library_relative_path_strips_every_site_prefix() {
        let cases = [
            (
                r"\\WIN-SPID\qsmcqtaz13\Plant\Ref\Symbols\Piping\Valves\Angle\Globe.sym",
                r"Piping\Valves\Angle\Globe.sym",
            ),
            (
                r"\\SPID\XaLNG\P&ID Reference Data\Symbols\Equipment Components\Nozzles\Flanged Nozzle.sym",
                r"Equipment Components\Nozzles\Flanged Nozzle.sym",
            ),
            (
                r"\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Design\Annotation\Graphics\Line.sym",
                r"Design\Annotation\Graphics\Line.sym",
            ),
        ];
        for (unc, expected) in cases {
            assert_eq!(library_relative_path(unc), Some(expected), "for {unc}");
        }
    }

    #[test]
    fn library_relative_path_rejects_a_path_with_no_library_root() {
        assert_eq!(library_relative_path(r"C:\scratch\thing.sym"), None);
        assert_eq!(library_relative_path(""), None);
    }

    #[test]
    fn circle_symbol_reads_as_a_single_circle() {
        let Some(path) = fixture(r"Design/Annotation/Graphics/Circle.sym") else {
            return;
        };
        let body = read_symbol_geometry(&path).expect("Circle.sym is a readable compound file");
        assert_eq!(
            body.primitives.len(),
            1,
            "the annotation Circle symbol is one circle: {:?}",
            body.primitives
        );
        let SymbolPrimitive::Circle { center, radius } = body.primitives[0].primitive else {
            panic!("expected a circle, got {:?}", body.primitives[0]);
        };
        assert!((radius - 0.003_810).abs() < 1e-9, "radius {radius}");
        assert!((center.0 - 0.101_600).abs() < 1e-9, "centre x {}", center.0);
        assert!((center.1 - 0.127_000).abs() < 1e-9, "centre y {}", center.1);
    }

    /// A symbol states its own colour and width, and the reader hands them
    /// over with the stroke they belong to.
    ///
    /// The colour cannot come from the drawing: an `igSymbol2d` placement
    /// carries the `JSite` id where its siblings carry the style index, so
    /// nothing on the placement names a style at all. It comes from the
    /// `StyleCluster` the `.sym` carries itself. `ElecTraceLine` is the
    /// smallest symbol that proves the join: its table defines exactly one
    /// line style, `#0000FF` at 0.5mm, and every one of its ten strokes
    /// indexes it -- which is why the trace-heating row draws blue.
    #[test]
    fn a_symbol_carries_the_colour_and_width_its_own_style_table_states() {
        let Some(path) = fixture(r"Design/Annotation/Graphics/ElecTraceLine.sym") else {
            return;
        };
        let body =
            read_symbol_geometry(&path).expect("ElecTraceLine.sym is a readable compound file");
        assert_eq!(body.primitives.len(), 10, "8 lines and 2 arcs");
        for styled in &body.primitives {
            let style = styled
                .style
                .unwrap_or_else(|| panic!("every stroke names a style: {styled:?}"));
            assert_eq!(style.rgb, [0x00, 0x00, 0xFF], "{styled:?}");
            assert!(
                (style.width_mm - 0.5).abs() < 1e-9,
                "width {}",
                style.width_mm
            );
        }
    }

    #[test]
    fn valve_symbol_reads_as_lines_and_circles_around_its_own_origin() {
        let Some(path) = fixture(r"Piping/Valves/Angle/2-Way Angle Globe Valve.sym") else {
            return;
        };
        let body = read_symbol_geometry(&path).expect("valve .sym is a readable compound file");
        let lines = body
            .primitives
            .iter()
            .filter(|p| matches!(p.primitive, SymbolPrimitive::Line { .. }))
            .count();
        let circles = body
            .primitives
            .iter()
            .filter(|p| matches!(p.primitive, SymbolPrimitive::Circle { .. }))
            .count();
        assert!(lines >= 8, "a globe valve is drawn with lines: {lines}");
        assert!(circles >= 2, "and its seat circles: {circles}");

        let (min_x, min_y, max_x, max_y) = body.bounds().expect("a drawn symbol has bounds");
        // The body is authored around the placement origin at roughly the
        // size of a P&ID valve: a handful of millimetres, not metres.
        assert!(min_x >= -0.02 && min_y >= -0.02, "bounds {min_x} {min_y}");
        assert!(max_x <= 0.02 && max_y <= 0.02, "bounds {max_x} {max_y}");
        assert!(max_x - min_x > 0.001, "width {}", max_x - min_x);
        assert!(max_y - min_y > 0.001, "height {}", max_y - min_y);
    }

    #[test]
    fn library_resolves_through_a_unc_path_and_caches_the_miss() {
        let root = symbols_root();
        if !root.is_dir() {
            return;
        }
        let mut library = SymbolLibrary::new(&root);
        let hit = library
            .resolve(CIRCLE_UNC)
            .map(|body| body.primitives.len());
        assert_eq!(hit, Some(1));

        assert!(library
            .resolve(r"\\WIN-SPID\p\Ref\Symbols\Piping\Nope\Not Here.sym")
            .is_none());
        assert_eq!(library.lookups(), 2);
        assert_eq!(library.resolved(), 1);
        assert_eq!(library.missing().len(), 1);
    }

    #[test]
    fn library_searches_past_a_root_that_lacks_the_symbol() {
        // The fixtures cite five different reference shares, and a machine
        // normally holds a partial copy of each rather than one merged tree.
        if !symbols_root().is_dir() {
            return;
        }
        let absent = scratch_root("absent");
        let mut library = SymbolLibrary::with_roots([absent.clone(), symbols_root()]);
        let hit = library
            .resolve(CIRCLE_UNC)
            .map(|body| body.primitives.len());
        assert_eq!(hit, Some(1), "the second root holds the symbol");
        std::fs::remove_dir_all(&absent).ok();
    }

    #[test]
    fn library_searches_past_a_root_whose_copy_will_not_read() {
        // A partial copy can leave a placeholder or a truncated file where
        // another copy has the real symbol. That must not end the search.
        if !symbols_root().is_dir() {
            return;
        }
        let broken = scratch_root("broken");
        let dir = broken.join("Design").join("Annotation").join("Graphics");
        std::fs::create_dir_all(&dir).expect("scratch tree");
        std::fs::write(dir.join("Circle.sym"), b"not a compound file").expect("placeholder");

        let mut library = SymbolLibrary::with_roots([broken.clone(), symbols_root()]);
        let hit = library
            .resolve(CIRCLE_UNC)
            .map(|body| body.primitives.len());
        assert_eq!(hit, Some(1), "the readable copy wins");
        std::fs::remove_dir_all(&broken).ok();
    }

    /// A text record laid out the way the library writes them: `count_at`
    /// bytes of header, the `u16` count, the characters, then the fixed
    /// trailing run whose first two `f64`s are the insertion point.
    fn text_payload(count_at: usize, text: &str, at: (f64, f64)) -> Vec<u8> {
        let chars: Vec<u16> = text.encode_utf16().collect();
        let mut payload = vec![0u8; count_at];
        payload.extend_from_slice(&u16::try_from(chars.len()).unwrap().to_le_bytes());
        for c in &chars {
            payload.extend_from_slice(&c.to_le_bytes());
        }
        payload.extend_from_slice(&at.0.to_le_bytes());
        payload.extend_from_slice(&at.1.to_le_bytes());
        payload.extend_from_slice(&[0u8; TEXT_TRAILING_LEN - 16]);
        payload
    }

    #[test]
    fn text_record_decodes_at_the_drawing_side_offset() {
        let payload = text_payload(30, "HH=NULL ", (0.012, -0.034));
        assert_eq!(
            decode_primitive(TYPE_TEXT, &payload),
            Some(SymbolPrimitive::Text {
                text: "HH=NULL ".to_owned(),
                at: (0.012, -0.034),
            })
        );
    }

    #[test]
    fn text_record_decodes_past_a_wider_sub_header() {
        // 248 of the library's records put the count at +22 instead of +30;
        // the payload size is what tells the two apart.
        let payload = text_payload(22, "设备位号", (0.05, 0.06));
        assert_eq!(
            decode_primitive(TYPE_TEXT, &payload),
            Some(SymbolPrimitive::Text {
                text: "设备位号".to_owned(),
                at: (0.05, 0.06),
            })
        );
    }

    #[test]
    fn text_record_is_skipped_when_the_count_does_not_account_for_the_payload() {
        // Neither offset adds up once the payload grows by a byte, and a
        // count that only nearly fits is exactly the reading that would put
        // reinterpreted binary on the sheet.
        let mut payload = text_payload(30, "XXX", (0.0, 0.0));
        payload.push(0);
        assert_eq!(decode_primitive(TYPE_TEXT, &payload), None);
    }

    #[test]
    fn text_record_with_an_absurd_insertion_point_is_skipped() {
        let payload = text_payload(30, "XXX", (1.0e9, 0.0));
        assert_eq!(decode_primitive(TYPE_TEXT, &payload), None);
    }

    #[test]
    fn bounds_take_in_the_text_insertion_point() {
        let unstyled = |primitive| StyledPrimitive {
            primitive,
            style: None,
        };
        let body = SymbolGeometry {
            primitives: vec![
                unstyled(SymbolPrimitive::Line {
                    start: (0.0, 0.0),
                    end: (0.1, 0.1),
                }),
                unstyled(SymbolPrimitive::Text {
                    text: "设备名称".to_owned(),
                    at: (-0.2, 0.3),
                }),
            ],
            skipped_records: BTreeMap::new(),
        };
        assert_eq!(body.bounds(), Some((-0.2, 0.0, 0.1, 0.3)));
    }

    #[test]
    fn from_path_list_keeps_the_roots_in_order() {
        let joined =
            std::env::join_paths([Path::new("/first"), Path::new("/second")]).expect("plain paths");
        let library = SymbolLibrary::from_path_list(&joined);
        assert_eq!(library.roots().len(), 2);
        assert!(library.roots()[0].ends_with("first"));
        assert!(library.roots()[1].ends_with("second"));
    }
}
