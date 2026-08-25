//! Two questions the first name probe raised.
//!
//! `probe_librarian_name_per_record` showed the librarian's names cut the
//! drawing where width and colour cannot — `0.350mm #800000` is both
//! `Equipment - New` and `Nozzle - New`, `0.350mm #808000` is three different
//! piping roles. Two things stand in the way of using that.
//!
//! **One: 188 of 669 resolved records reach a style with no name**, 182 of
//! them the single most numerous style in the corpus (`0.130mm #000000` in
//! 工艺管道). Either that document's librarian is genuinely silent about the
//! style, or the `+8` gap rule that binds a name to an `oid` misses it. Those
//! are very different situations and only one of them is our bug. Printed
//! here as: how many records the librarian holds, how many styles the document
//! defines, and whether the unnamed style's `oid` appears in the librarian
//! stream at all.
//!
//! **Two: nothing has corroborated the naming rule from outside itself.** The
//! §5 evidence was that names sort cleanly into families (`ps…` only ever on
//! `0x0032`, `ls…` only ever on `0x002E`). That is good, but it is still the
//! name table judging itself. Two names make falsifiable claims about facts
//! this crate decodes by a completely separate path:
//!
//! * `Dashed` should land on styles that reach a dash pattern, and `Normal`
//!   should not.
//! * `lsOk` / `lsWarning` / `lsApproved` should land on styles whose
//!   terminator glyph is blank / slash / tick respectively.
//!
//! If those agree the binding is confirmed by something other than itself. If
//! they disagree the gap rule is picking up text that is not the name.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;
use pid_parse::parsers::sheet_records::{
    decode_iglines, decode_iglinestrings, decode_igpoints, decode_igsymbols,
};
use pid_parse::style_link::{
    stylecluster_path_for_sheet, DocumentStyleTable, MarkerStatus, PSM_TYPE_CODE_JSTYLE_LIBRARIAN,
};

const FIXTURES: [&str; 5] = [
    "test-file/D06.pid",
    "test-file/DWG-0201GP06-01.pid",
    "test-file/DWG-0202GP06-01.pid",
    "test-file/工艺管道及仪表流程-1.pid",
    "test-file/A01.pid",
];

fn read_stream(cfb: &mut CompoundFile<std::fs::File>, path: &str) -> Option<Vec<u8>> {
    let mut stream = cfb.open_stream(path).ok()?;
    let mut data = Vec::new();
    stream.read_to_end(&mut data).ok()?;
    Some(data)
}

fn main() {
    let mut agree_dash = (0usize, 0usize, Vec::<String>::new());
    let mut agree_status = (0usize, 0usize, Vec::<String>::new());

    for fixture in FIXTURES {
        let path = Path::new(fixture);
        if !path.exists() {
            println!("skip: {fixture} is absent");
            continue;
        }
        let Ok(file) = std::fs::File::open(path) else {
            continue;
        };
        let Ok(mut cfb) = CompoundFile::open(file) else {
            continue;
        };
        let sheet_paths: Vec<String> = cfb
            .walk()
            .filter(cfb::Entry::is_stream)
            .map(|e| e.path().to_string_lossy().into_owned())
            .filter(|p| p.rsplit('/').next().unwrap_or("").starts_with("Sheet"))
            .collect();

        println!("\n=== {fixture} ===");
        for sheet_path in &sheet_paths {
            let Some(sheet) = read_stream(&mut cfb, sheet_path) else {
                continue;
            };
            let cluster_path = stylecluster_path_for_sheet(sheet_path);
            let Some(cluster) = read_stream(&mut cfb, &cluster_path) else {
                continue;
            };
            let table = DocumentStyleTable::from_stylecluster_bytes(&cluster);

            let librarians = table
                .records()
                .iter()
                .filter(|r| r.type_code == PSM_TYPE_CODE_JSTYLE_LIBRARIAN)
                .count();
            let named = table.records().iter().filter(|r| r.name.is_some()).count();
            println!(
                "  {cluster_path}: {} record(s), {librarians} librarian, {named} named",
                table.records().len()
            );

            // Which styles the geometry actually reaches, and whether named.
            let mut indices: Vec<u32> = Vec::new();
            for r in decode_iglines(&sheet) {
                indices.push(r.index);
            }
            for r in decode_igpoints(&sheet) {
                indices.push(r.index);
            }
            for r in decode_iglinestrings(&sheet) {
                indices.push(r.index);
            }
            for r in decode_igsymbols(&sheet) {
                indices.push(r.style_ref);
            }

            let mut unnamed_reached: BTreeMap<u32, usize> = BTreeMap::new();
            for index in indices {
                let Some(resolved) = table.resolve_line_style(index) else {
                    continue;
                };
                let name = table.name_of_style(resolved.style_id);
                if name.is_none() {
                    *unnamed_reached.entry(resolved.style_id).or_default() += 1;
                    continue;
                }
                let name = name.unwrap_or_default();

                // Claim one: `Dashed` reaches a dash pattern, `Normal` does not.
                if name == "Dashed" || name == "Normal" {
                    let has_dash = resolved.dash.is_some();
                    let want = name == "Dashed";
                    if has_dash == want {
                        agree_dash.0 += 1;
                    } else {
                        agree_dash.1 += 1;
                        agree_dash
                            .2
                            .push(format!("{name} dash={has_dash} id={}", resolved.style_id));
                    }
                }

                // Claim two: `ls*` agrees with the terminator glyph's status.
                let as_point_name = name.strip_prefix("ls").map(|rest| format!("ps{rest}"));
                if let Some(want) = as_point_name
                    .as_deref()
                    .and_then(MarkerStatus::from_style_name)
                {
                    let got = resolved.marker.and_then(|m| m.status);
                    if got == Some(want) {
                        agree_status.0 += 1;
                    } else {
                        agree_status.1 += 1;
                        agree_status
                            .2
                            .push(format!("{name} marker={got:?} id={}", resolved.style_id));
                    }
                }
            }

            for (style_id, count) in &unnamed_reached {
                let record = table.get(*style_id);
                let oid = record.map_or(0, |r| r.oid);
                let symbology = record.and_then(|r| r.symbology);
                println!(
                    "    unnamed style id={style_id} oid={oid} reached by {count} record(s), \
                     symbology={:?}",
                    symbology.map(|s| {
                        let [r, g, b] = s.rgb();
                        format!("{:.3}mm #{r:02X}{g:02X}{b:02X}", s.width_mm())
                    })
                );
            }

            // Every name this document holds, with the family it landed on.
            let mut names: Vec<String> = table
                .records()
                .iter()
                .filter_map(|r| {
                    r.name.as_deref().map(|n| {
                        format!(
                            "{n} [0x{:04X} id={} oid={}]",
                            r.type_code, r.style_id, r.oid
                        )
                    })
                })
                .collect();
            names.sort();
            if !names.is_empty() {
                println!("    names: {}", names.len());
                for name in &names {
                    println!("      {name}");
                }
            }
        }
    }

    println!("\n=== does the name agree with what we decode separately? ===");
    println!(
        "  Dashed/Normal vs dash pattern : {} agree, {} disagree {:?}",
        agree_dash.0, agree_dash.1, agree_dash.2
    );
    println!(
        "  ls* vs terminator glyph status: {} agree, {} disagree {:?}",
        agree_status.0, agree_status.1, agree_status.2
    );
}
