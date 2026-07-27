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
//! Four record types carry drawable geometry; the rest are connect points,
//! text, styles and structure. See [`SymbolPrimitive`].
//!
//! ```no_run
//! use pid_parse::symbol_library::SymbolLibrary;
//!
//! let mut library = SymbolLibrary::new(r"C:\Plant\Ref\Symbols");
//! if let Some(body) = library.resolve(r"\\WIN-SPID\p\Ref\Symbols\Piping\Valves\Angle\2-Way Angle Globe Valve.sym") {
//!     println!("{} primitives", body.primitives.len());
//! }
//! ```

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::api::PidParser;
use crate::error::PidError;

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

const TYPE_LINE: u16 = 0x0018;
const TYPE_CIRCLE: u16 = 0x0059;
const TYPE_ARC: u16 = 0x0061;
const TYPE_POLYLINE: u16 = 0x0084;

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
/// The library uses four geometry records out of the 27 PSM type codes that
/// appear in it; the others carry connect points, text, line styles and
/// object structure, none of which draw a shape.
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
}

/// A symbol's drawable body, as read from one `.sym` file.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SymbolGeometry {
    /// Drawable primitives in the symbol's own coordinate space.
    pub primitives: Vec<SymbolPrimitive>,
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
        for primitive in &self.primitives {
            match primitive {
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
        _ => None,
    }
}

/// Walk one symbol `Sheet*` stream's record chain into `out`.
///
/// The chain is only read when it accounts for the stream exactly: a walk
/// that stops early or overruns means the bytes are not the layout this
/// reader knows, and half a symbol drawn from a misread chain is worse than
/// no symbol at all.
fn read_sheet_chain(bytes: &[u8], out: &mut SymbolGeometry) {
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
        match decode_primitive(type_code, &bytes[body..end]) {
            Some(primitive) => primitives.push(primitive),
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
        read_sheet_chain(&raw.data, &mut sheet);

        // A symbol often keeps more than one sheet holding the same body:
        // the angle globe valve's `Sheet63` is nine primitives that are
        // byte-identical to nine of `Sheet6`'s eleven. Drawing both doubles
        // every stroke. Duplicates are dropped across sheets but kept within
        // one, where a repeated stroke is the symbol's own doing.
        for primitive in sheet.primitives {
            if !geometry.primitives.contains(&primitive) {
                geometry.primitives.push(primitive);
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
        let SymbolPrimitive::Circle { center, radius } = body.primitives[0] else {
            panic!("expected a circle, got {:?}", body.primitives[0]);
        };
        assert!((radius - 0.003_810).abs() < 1e-9, "radius {radius}");
        assert!((center.0 - 0.101_600).abs() < 1e-9, "centre x {}", center.0);
        assert!((center.1 - 0.127_000).abs() < 1e-9, "centre y {}", center.1);
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
            .filter(|p| matches!(p, SymbolPrimitive::Line { .. }))
            .count();
        let circles = body
            .primitives
            .iter()
            .filter(|p| matches!(p, SymbolPrimitive::Circle { .. }))
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
