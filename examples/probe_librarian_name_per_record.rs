//! What does the librarian's name buy us that width and colour do not?
//!
//! `2026-08-25-a-point-draws-the-symbol-its-terminator-names.md` §5 decoded
//! `0x005A JStyleLibrarian` and found the drawing's own words for every style:
//! four marker states (`psOk` / `psWarning` / `psError` / `psApproved`) and,
//! riding along with them, a discipline vocabulary — `Primary Piping - New`,
//! `Equipment - New`, `Nozzle - New`, `Electric Signal`, … The marker states
//! have already landed. The discipline names have not, and the doc parks them
//! as "possibly worth more than the point symbols, take a separate round".
//!
//! This is that round's first question, and it is a subtraction question:
//!
//! * If every record carrying `Primary Piping - New` also carries
//!   `0.700mm #808000`, and no other name does, then the name is a **label
//!   for a palette entry** we already resolve. Nice for reading, no new
//!   classification.
//! * If one name spans several palette entries, or one palette entry carries
//!   several names, then the name **cuts the drawing somewhere width and
//!   colour cannot**, and it is a genuine semantic input.
//!
//! Prints both directions of that mapping plus per-family coverage, so the
//! answer is readable off the output rather than argued.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;
use pid_parse::parsers::sheet_records::{
    decode_iglines, decode_iglinestrings, decode_igpoints, decode_igsymbols,
};
use pid_parse::style_link::{stylecluster_path_for_sheet, DocumentStyleTable};

const FIXTURES: [&str; 5] = [
    "test-file/D06.pid",
    "test-file/DWG-0201GP06-01.pid",
    "test-file/DWG-0202GP06-01.pid",
    "test-file/工艺管道及仪表流程-1.pid",
    "test-file/A01.pid",
];

/// One resolved record, reduced to the two descriptions being compared.
struct Row {
    family: &'static str,
    palette: String,
    name: Option<String>,
}

fn read_stream(cfb: &mut CompoundFile<std::fs::File>, path: &str) -> Option<Vec<u8>> {
    let mut stream = cfb.open_stream(path).ok()?;
    let mut data = Vec::new();
    stream.read_to_end(&mut data).ok()?;
    Some(data)
}

fn rows(path: &Path) -> Vec<Row> {
    let Ok(file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let Ok(mut cfb) = CompoundFile::open(file) else {
        return Vec::new();
    };
    let sheet_paths: Vec<String> = cfb
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|e| e.path().to_string_lossy().into_owned())
        .filter(|p| p.rsplit('/').next().unwrap_or("").starts_with("Sheet"))
        .collect();

    let mut out = Vec::new();
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
            indices.push(("igLine2d", record.index));
        }
        for record in decode_igpoints(&sheet) {
            indices.push(("igPoint2d", record.index));
        }
        for record in decode_iglinestrings(&sheet) {
            indices.push(("igLineString2d", record.index));
        }
        for record in decode_igsymbols(&sheet) {
            indices.push(("igSymbol2d", record.style_ref));
        }

        for (family, index) in indices {
            let Some(resolved) = table.resolve_line_style(index) else {
                continue;
            };
            let width_mm = resolved.symbology.width_mm();
            let [r, g, b] = resolved.symbology.rgb();
            out.push(Row {
                family,
                palette: format!("{width_mm:.3}mm #{r:02X}{g:02X}{b:02X}"),
                name: table.name_of_style(resolved.style_id).map(str::to_string),
            });
        }
    }
    out
}

fn main() {
    let mut corpus: Vec<Row> = Vec::new();

    for fixture in FIXTURES {
        let path = Path::new(fixture);
        if !path.exists() {
            println!("skip: {fixture} is absent");
            continue;
        }
        let got = rows(path);
        let named = got.iter().filter(|r| r.name.is_some()).count();
        println!(
            "\n=== {fixture} === {} resolved, {named} named ({:.0}%)",
            got.len(),
            if got.is_empty() {
                0.0
            } else {
                100.0 * named as f64 / got.len() as f64
            }
        );

        let mut by_family: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
        for row in &got {
            let slot = by_family.entry(row.family).or_default();
            slot.0 += 1;
            if row.name.is_some() {
                slot.1 += 1;
            }
        }
        for (family, (total, named)) in &by_family {
            println!("  {family:<16} {named}/{total} named");
        }

        let mut names: BTreeMap<&str, usize> = BTreeMap::new();
        for row in &got {
            *names
                .entry(row.name.as_deref().unwrap_or("<unnamed>"))
                .or_default() += 1;
        }
        for (name, count) in &names {
            println!("    {count:>4}  {name}");
        }

        corpus.extend(got);
    }

    // The subtraction. Does either description refine the other?
    println!("\n=== corpus: name → palette entries it spans ===");
    let mut name_to_palette: BTreeMap<&str, BTreeMap<&str, usize>> = BTreeMap::new();
    let mut palette_to_name: BTreeMap<&str, BTreeMap<&str, usize>> = BTreeMap::new();
    for row in &corpus {
        let name = row.name.as_deref().unwrap_or("<unnamed>");
        *name_to_palette
            .entry(name)
            .or_default()
            .entry(row.palette.as_str())
            .or_default() += 1;
        *palette_to_name
            .entry(row.palette.as_str())
            .or_default()
            .entry(name)
            .or_default() += 1;
    }
    for (name, palettes) in &name_to_palette {
        let total: usize = palettes.values().sum();
        let spread: Vec<String> = palettes.iter().map(|(p, c)| format!("{p}×{c}")).collect();
        println!(
            "  {total:>4}  {name:<28} {} entry(ies): {}",
            palettes.len(),
            spread.join("  ")
        );
    }

    println!("\n=== corpus: palette entry → names it carries ===");
    for (palette, names) in &palette_to_name {
        let total: usize = names.values().sum();
        let spread: Vec<String> = names.iter().map(|(n, c)| format!("{n}×{c}")).collect();
        println!(
            "  {total:>4}  {palette:<18} {} name(s): {}",
            names.len(),
            spread.join("  ")
        );
    }

    let ambiguous_names: BTreeSet<&str> = name_to_palette
        .iter()
        .filter(|(n, p)| **n != "<unnamed>" && p.len() > 1)
        .map(|(n, _)| *n)
        .collect();
    let ambiguous_palettes: BTreeSet<&str> = palette_to_name
        .iter()
        .filter(|(_, n)| n.keys().filter(|k| **k != "<unnamed>").count() > 1)
        .map(|(p, _)| *p)
        .collect();
    println!(
        "\nnames spanning >1 palette entry: {ambiguous_names:?}\n\
         palette entries carrying >1 name: {ambiguous_palettes:?}"
    );
}
