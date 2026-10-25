use cfb::CompoundFile;
use pid_parse::parsers::sheet_records::{decode_iglines, decode_iglinestrings, decode_igpoints};
use pid_parse::style_link::{stylecluster_path_for_sheet, DocumentStyleTable};
use std::io::Read;
use std::path::Path;

fn read_stream(cfb: &mut CompoundFile<std::fs::File>, path: &str) -> Option<Vec<u8>> {
    let mut s = cfb.open_stream(path).ok()?;
    let mut d = Vec::new();
    s.read_to_end(&mut d).ok()?;
    Some(d)
}

fn main() {
    let path = Path::new(r"D:\work\cad\PID自研软件验证\Ref\Template Files\CPE-SQ-A1.igr");
    let file = std::fs::File::open(path).unwrap();
    let mut cfb = CompoundFile::open(file).unwrap();
    let sheet_paths: Vec<String> = cfb
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|e| e.path().to_string_lossy().into_owned())
        .filter(|p| p.rsplit('/').next().unwrap_or("").starts_with("Sheet"))
        .collect();
    for sheet_path in &sheet_paths {
        let Some(sheet) = read_stream(&mut cfb, sheet_path) else {
            continue;
        };
        let table = read_stream(&mut cfb, &stylecluster_path_for_sheet(sheet_path))
            .map_or_else(DocumentStyleTable::default, |b| {
                DocumentStyleTable::from_stylecluster_bytes(&b)
            });
        for rec in decode_iglinestrings(&sheet) {
            match table.resolve_line_style(rec.index) {
                Some(rs) => {
                    let (x0, y0) = rec.vertices.first().copied().unwrap_or((0., 0.));
                    let (x1, y1) = rec.vertices.last().copied().unwrap_or((0., 0.));
                    println!(
                        "igLS2d idx={} ({:.1},{:.1})->({:.1},{:.1}) colour=0x{:06X} w={:.3}mm",
                        rec.index,
                        x0 * 1000.,
                        y0 * 1000.,
                        x1 * 1000.,
                        y1 * 1000.,
                        rs.symbology.colour,
                        rs.symbology.width_mm()
                    );
                }
                None => println!("igLS2d idx={} UNRESOLVED", rec.index),
            }
        }
        for rec in decode_iglines(&sheet) {
            match table.resolve_line_style(rec.index) {
                Some(rs) => println!(
                    "igLine2d idx={} colour=0x{:06X} w={:.3}mm",
                    rec.index,
                    rs.symbology.colour,
                    rs.symbology.width_mm()
                ),
                None => println!("igLine2d idx={} UNRESOLVED", rec.index),
            }
        }
        for rec in decode_igpoints(&sheet) {
            if let Some(rs) = table.resolve_line_style(rec.index) {
                println!(
                    "igPoint2d idx={} colour=0x{:06X}",
                    rec.index, rs.symbology.colour
                );
            }
        }
    }
}
