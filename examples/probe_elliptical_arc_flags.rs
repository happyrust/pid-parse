//! Every `igEllipticalArc2d` (`0x007E`) record a file holds, with the
//! trailing byte it carries, so the package's flag can be read against the
//! half-ellipse the reference plot actually draws.
//!
//! `SheetIgEllipticalArc2dDecoded::flag` is payload `+74`, undecoded since the
//! family landed. The diaphragm actuator is why it matters: its cached body
//! (`PidSymbolDefinitionRef { site: 37311, sheet: 52 }`) stores the dome as a
//! B-spline whose apex is at `y = 6.35 mm` under a chord at `8.89 mm`, and the
//! library `.sym`'s own `0x007E` decodes to the same side — yet the reference
//! plot draws the dome on the other side. If A01's four heads (whose bulge is
//! verified outward against the body rectangle) carry a different flag from
//! this dome, that byte is the side, and one bit decides both.
//!
//! ```text
//! cargo run -p pid-parse --example probe_elliptical_arc_flags -- "<drawing.pid|symbol.sym|symbol-dir>"
//! ```
//!
//! A directory is walked for `.sym` files, and only the files that hold a
//! record are listed, so a library-wide census is one command.
//!
//! Read-only.

use std::path::{Path, PathBuf};

use pid_parse::parsers::sheet_records::decode_igellipticalarcs;
use pid_parse::PidParser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).expect("usage: <file|dir>");
    let root = PathBuf::from(&path);
    if root.is_dir() {
        let mut files = Vec::new();
        collect(&root, &mut files)?;
        files.sort();
        let mut carriers = 0usize;
        let mut flags = std::collections::BTreeMap::<u8, usize>::new();
        for file in &files {
            let package = match PidParser::new().parse_package(file) {
                Ok(package) => package,
                Err(error) => {
                    eprintln!("{}: {error}", file.display());
                    continue;
                }
            };
            let mut here = 0usize;
            for raw in package.streams.values() {
                for arc in decode_igellipticalarcs(&raw.data) {
                    here += 1;
                    *flags.entry(arc.flag).or_default() += 1;
                }
            }
            if here > 0 {
                carriers += 1;
                println!("{}: {here} record(s)", file.display());
            }
        }
        println!(
            "-- {} file(s) walked, {carriers} carry 0x007E, flags {flags:?}",
            files.len()
        );
        return Ok(());
    }
    let package = PidParser::new().parse_package(Path::new(&path))?;
    let mut total = 0usize;
    for (stream_path, raw) in &package.streams {
        for arc in decode_igellipticalarcs(&raw.data) {
            total += 1;
            println!(
                "{stream_path}\toid {}\tflag {}\tlayer {}\tindex {}\tcenter ({:.6},{:.6})\t\
                 major ({:.6},{:.6})\tratio {:.6}\tsweep {:.6} -> {:.6}",
                arc.oid,
                arc.flag,
                arc.sheet_layer_ref,
                arc.index,
                arc.center.0,
                arc.center.1,
                arc.major_axis.0,
                arc.major_axis.1,
                arc.ratio,
                arc.sweep_start,
                arc.sweep_end
            );
        }
    }
    println!("-- {total} record(s) in {}", Path::new(&path).display());
    Ok(())
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect(&path, out)?;
        } else if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("sym"))
        {
            out.push(path);
        }
    }
    Ok(())
}
