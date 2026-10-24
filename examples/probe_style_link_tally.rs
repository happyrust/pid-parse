//! Per-fixture tally mirroring `tests/style_link_ratchet.rs`'s `tally()`,
//! so the EXPECTED table can be re-based after a decoder change.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;
use pid_parse::parsers::sheet_records::{
    decode_iglines, decode_iglinestrings, decode_igpoints, decode_igsymbols,
};
use pid_parse::style_link::{stylecluster_path_for_sheet, DocumentStyleTable, StyleHop};

fn read_stream(cfb: &mut CompoundFile<std::fs::File>, path: &str) -> Option<Vec<u8>> {
    let mut stream = cfb.open_stream(path).ok()?;
    let mut data = Vec::new();
    stream.read_to_end(&mut data).ok()?;
    Some(data)
}

fn main() {
    let mut corpus_palette: BTreeMap<String, usize> = BTreeMap::new();
    let mut total = 0usize;
    for fixture in [
        "test-file/D06.pid",
        "test-file/DWG-0201GP06-01.pid",
        "test-file/DWG-0202GP06-01.pid",
        "test-file/工艺管道及仪表流程-1.pid",
    ] {
        let path = Path::new(fixture);
        if !path.exists() {
            println!("{fixture}: ABSENT");
            continue;
        }
        let file = std::fs::File::open(path).unwrap();
        let mut cfb = CompoundFile::open(file).unwrap();
        let sheet_paths: Vec<String> = cfb
            .walk()
            .filter(cfb::Entry::is_stream)
            .map(|e| e.path().to_string_lossy().into_owned())
            .filter(|p| p.rsplit('/').next().unwrap_or("").starts_with("Sheet"))
            .collect();

        let (mut lines, mut points, mut linestrings, mut symbols) = (0, 0, 0, 0);
        let (mut direct, mut via_override) = (0usize, 0usize);
        let mut unresolved = Vec::new();
        for sheet_path in &sheet_paths {
            let Some(sheet) = read_stream(&mut cfb, sheet_path) else {
                continue;
            };
            let table = read_stream(&mut cfb, &stylecluster_path_for_sheet(sheet_path))
                .map_or_else(DocumentStyleTable::default, |bytes| {
                    DocumentStyleTable::from_stylecluster_bytes(&bytes)
                });
            let mut indices: Vec<(&'static str, u32)> = Vec::new();
            for record in decode_iglines(&sheet) {
                lines += 1;
                indices.push(("igLine2d", record.index));
            }
            for record in decode_igpoints(&sheet) {
                points += 1;
                indices.push(("igPoint2d", record.index));
            }
            for record in decode_iglinestrings(&sheet) {
                linestrings += 1;
                indices.push(("igLineString2d", record.index));
            }
            for record in decode_igsymbols(&sheet) {
                symbols += 1;
                indices.push(("igSymbol2d", record.style_ref));
            }
            for (family, index) in indices {
                let Some(resolved) = table.resolve_line_style(index) else {
                    unresolved.push(format!("{sheet_path} {family} index={index}"));
                    continue;
                };
                match resolved.hop {
                    StyleHop::Direct => direct += 1,
                    StyleHop::ViaOverride { .. } => via_override += 1,
                }
                let width_mm = resolved.symbology.width_mm();
                let [r, g, b] = resolved.symbology.rgb();
                *corpus_palette
                    .entry(format!("{width_mm:.3}mm #{r:02X}{g:02X}{b:02X}"))
                    .or_default() += 1;
            }
        }
        let resolved = direct + via_override;
        total += resolved;
        println!(
            "{fixture}: lines={lines} points={points} linestrings={linestrings} \
             symbols={symbols} direct={direct} via_override={via_override} \
             unresolved={unresolved:?}"
        );
    }
    println!("total resolved: {total}");
    println!("palette:");
    for (k, n) in &corpus_palette {
        println!("  (\"{k}\", {n}),");
    }
}
