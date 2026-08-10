//! Phase 40 S4 — do the records inside a `JSite*` Sheet belong on the page?
//!
//! 80 of the corpus's 88 refused `igLine2d` records sit in A01's
//! `/JSite204/Sheet6`. A `JSite` is `SmartPlant`'s symbol-instance container,
//! so the obvious worry is that its Sheet holds **symbol-local** geometry
//! that a placed `igSymbol2d` already stamps onto the page. Decoding those
//! lines would then draw them twice, or once in the wrong coordinate space.
//! Phase 39 S3's rule applies: prove the consumer before decoding.
//!
//! Four independent readings, each able to say no on its own:
//!
//! 1. **Is the stream already a page-content source?** Records of other
//!    families in the *same* stream are accepted today and drawn. If the
//!    drawing looks right with those on the page, the stream is page content.
//! 2. **One coordinate space or two?** The accepted records' coordinates and
//!    the refused lines' coordinates, in the same stream, against the page
//!    the border frame states. Symbol-local geometry is small and near its
//!    own origin; page content spans the sheet.
//! 3. **Is this `JSite` a symbol definition?** A `JSite` that names a
//!    `symbol_path` is a placed symbol; one that names none is a container
//!    of something else.
//! 4. **Would anything double-draw it?** Whether a top-level `igSymbol2d`
//!    resolves to this `JSite`.
//!
//! ```powershell
//! cargo run --example probe_phase40_jsite_sheet_is_page_content
//! ```

use std::io::Read;
use std::path::{Path, PathBuf};

use pid_parse::api::PidParser;
use pid_parse::parsers::sheet_records::{
    decode_iglinestrings, decode_igpoints, decode_igtextboxes,
};

const FIXTURE: &str = "export-test/publish-data/A01/A01.pid";

fn test_file_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("test-file")
}

fn stream_bytes(path: &Path, wanted: &str) -> Option<Vec<u8>> {
    let mut cfb = cfb::CompoundFile::open(std::fs::File::open(path).ok()?).ok()?;
    let found = cfb
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|entry| entry.path().to_path_buf())
        .find(|p| p.to_string_lossy().replace('\\', "/") == wanted)?;
    let mut stream = cfb.open_stream(&found).ok()?;
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

#[derive(Default)]
struct Extent {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
    count: usize,
}

impl Extent {
    fn new() -> Self {
        Self {
            min_x: f64::MAX,
            min_y: f64::MAX,
            max_x: f64::MIN,
            max_y: f64::MIN,
            count: 0,
        }
    }

    fn add(&mut self, x: f64, y: f64) {
        self.min_x = self.min_x.min(x);
        self.min_y = self.min_y.min(y);
        self.max_x = self.max_x.max(x);
        self.max_y = self.max_y.max(y);
        self.count += 1;
    }

    /// Metres in, millimetres out — the page is quoted in mm everywhere else.
    fn report(&self, label: &str) {
        if self.count == 0 {
            println!("    {label:<34} (no coordinates)");
            return;
        }
        println!(
            "    {label:<34} x {:8.1} .. {:8.1} mm   y {:8.1} .. {:8.1} mm   ({} pt)",
            self.min_x * 1000.0,
            self.max_x * 1000.0,
            self.min_y * 1000.0,
            self.max_y * 1000.0,
            self.count
        );
    }
}

/// Coordinates of the refused `igLine2d` records, read at the offsets the
/// accepted ones use. Whether that offset is right for this framing is S5's
/// question; here the only claim is which region of the sheet they fall in,
/// and being wrong about the offset would show up as nonsense rather than as
/// a tidy page-sized box.
/// Shape of the refused lines, not just their box. A misread coordinate
/// offset scatters; a border and its grid are axis-aligned and land on the
/// page's own corners, which no misframing produces by accident.
fn refused_line_survey(data: &[u8]) -> (Extent, LineShapes) {
    let mut extent = Extent::new();
    let mut shapes = LineShapes {
        shortest: f64::MAX,
        ..LineShapes::default()
    };
    let mut off = 0usize;
    while off + 56 <= data.len() {
        let type_word = u16::from_le_bytes([data[off], data[off + 1]]);
        let btf = u32::from_le_bytes([data[off + 2], data[off + 3], data[off + 4], data[off + 5]])
            as usize;
        if type_word & 0x3FFF != 0x0018 || btf != 50 {
            off += 1;
            continue;
        }
        let payload = &data[off + 6..off + 56];
        let remaining_header =
            u32::from_le_bytes([payload[8], payload[9], payload[10], payload[11]]);
        if remaining_header == 12 {
            off += 56;
            continue;
        }
        let read = |at: usize| {
            let s = &payload[at..at + 8];
            f64::from_le_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]])
        };
        let (sx, sy, ex, ey) = (read(18), read(26), read(34), read(42));
        if [sx, sy, ex, ey]
            .iter()
            .all(|v| v.is_finite() && v.abs() < 1e9)
        {
            extent.add(sx, sy);
            extent.add(ex, ey);
            shapes.total += 1;
            let horizontal = (ey - sy).abs() < 1e-9;
            let vertical = (ex - sx).abs() < 1e-9;
            if horizontal || vertical {
                shapes.axis_aligned += 1;
            }
            let length = ((ex - sx).powi(2) + (ey - sy).powi(2)).sqrt() * 1000.0;
            shapes.longest = shapes.longest.max(length);
            shapes.shortest = shapes.shortest.min(length);
        }
        off += 56;
    }
    (extent, shapes)
}

#[derive(Default)]
struct LineShapes {
    total: usize,
    axis_aligned: usize,
    longest: f64,
    shortest: f64,
}

fn accepted_extent(data: &[u8]) -> Extent {
    let mut extent = Extent::new();
    for text in decode_igtextboxes(data) {
        extent.add(text.trailing_double_1, text.trailing_double_2);
    }
    for point in decode_igpoints(data) {
        extent.add(point.point.0, point.point.1);
    }
    for polyline in decode_iglinestrings(data) {
        for (x, y) in &polyline.vertices {
            extent.add(*x, *y);
        }
    }
    extent
}

fn main() {
    let path = test_file_root().join(FIXTURE);
    if !path.exists() {
        eprintln!("skip: {FIXTURE} not present");
        return;
    }
    let doc = PidParser::new()
        .parse_file(&path)
        .expect("A01 should parse");

    println!("=== {FIXTURE} ===\n");

    // Reading 3: is JSite204 a placed symbol, or a container of something
    // else? Every JSite in the file, so the answer is comparative.
    println!("  JSite inventory ({} entries)", doc.jsites.len());
    for jsite in &doc.jsites {
        println!(
            "    {:<14} symbol_name={:?} symbol_path={:?} ole={}",
            jsite.name, jsite.symbol_name, jsite.symbol_path, jsite.has_ole_stream
        );
    }

    // Reading 1: which streams does this crate already treat as page
    // content, and how much does each contribute?
    println!("\n  Sheet streams this crate already decodes onto the page");
    for sheet in &doc.sheet_streams {
        let Some(geometry) = sheet.geometry.as_ref() else {
            println!("    {:<26} (no geometry)", sheet.path);
            continue;
        };
        let drawn = geometry.decoded_iglines.len()
            + geometry.decoded_iglinestrings.len()
            + geometry.decoded_igpoints.len()
            + geometry.decoded_igtextboxes.len()
            + geometry.decoded_igsymbols.len();
        let refused: usize = geometry.refused_records.iter().map(|r| r.count).sum();
        println!(
            "    {:<26} {drawn:3} record(s) drawn, {refused:3} refused",
            sheet.path
        );
    }

    // Reading 2: one coordinate space or two? Compare, inside the very same
    // stream, what is drawn today against what is refused.
    println!("\n  Coordinate extents (page is 594.0 x 420.0 mm per the border frame)");
    for wanted in ["/Sheet6", "/JSite204/Sheet6"] {
        let Some(data) = stream_bytes(&path, wanted) else {
            println!("    {wanted}: not found");
            continue;
        };
        println!("  {wanted}");
        accepted_extent(&data).report("accepted records (drawn today)");
        let (extent, shapes) = refused_line_survey(&data);
        extent.report("refused igLine2d records");
        if shapes.total > 0 {
            println!(
                "      shape: {}/{} axis-aligned, lengths {:.1} .. {:.1} mm",
                shapes.axis_aligned, shapes.total, shapes.shortest, shapes.longest
            );
            println!(
                "      corners: ({:.4}, {:.4}) .. ({:.4}, {:.4}) mm",
                extent.min_x * 1000.0,
                extent.min_y * 1000.0,
                extent.max_x * 1000.0,
                extent.max_y * 1000.0
            );
        }
    }

    // Reading 4: would decoding them double-draw anything? A symbol
    // instance on the top-level sheet that resolves to JSite204 would.
    println!("\n  Symbol placements on the top-level sheet");
    for sheet in &doc.sheet_streams {
        let Some(geometry) = sheet.geometry.as_ref() else {
            continue;
        };
        for symbol in &geometry.decoded_igsymbols {
            let resolves_here = if symbol.jsite_ref == 204 {
                "  <- RESOLVES TO JSite204: decoding its sheet would double-draw"
            } else {
                ""
            };
            println!(
                "    {} oid={} -> JSite{} at ({:.1}, {:.1}) mm{resolves_here}",
                sheet.path,
                symbol.oid,
                symbol.jsite_ref,
                symbol.insertion_x * 1000.0,
                symbol.insertion_y * 1000.0
            );
        }
    }
}
