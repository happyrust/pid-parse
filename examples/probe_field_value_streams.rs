//! Which streams of a .pid hold the title-block field values, and are they
//! already-decoded igTextBox records?

use cfb::CompoundFile;
use std::io::Read;

const NEEDLES: &[&str] = &[
    "集注站",
    "凝析油闪蒸单元",
    "一级闪蒸分离器B",
    "DWG-0000",
    "DD11008",
    "施工图",
    "华北油田",
    "010065",
    "A111001641",
    "CPENC",
    "1 版",
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for arg in std::env::args().skip(1) {
        let mut cfb = CompoundFile::open(std::fs::File::open(&arg)?)?;
        let streams: Vec<_> = cfb
            .walk()
            .filter(cfb::Entry::is_stream)
            .map(|e| e.path().to_path_buf())
            .collect();
        println!("{arg}: {} streams", streams.len());
        for sp in streams {
            let mut data = Vec::new();
            cfb.open_stream(&sp)?.read_to_end(&mut data)?;
            let mut hits: Vec<(usize, &str)> = Vec::new();
            for needle in NEEDLES {
                let pat: Vec<u8> = needle
                    .encode_utf16()
                    .flat_map(|u| u.to_le_bytes())
                    .collect();
                for i in 0..data.len().saturating_sub(pat.len()) {
                    if data[i..i + pat.len()] == pat[..] {
                        hits.push((i, needle));
                    }
                }
            }
            if hits.is_empty() {
                continue;
            }
            hits.sort();
            // Do any hits fall inside decoded igTextBox byte ranges?
            let texts: Vec<(usize, usize, String)> =
                pid_parse::parsers::sheet_records::decode_igtextboxes(&data)
                    .iter()
                    .map(|r| (r.byte_range.start, r.byte_range.end, r.text.clone()))
                    .collect();
            println!(
                "  {} ({}b, {} textboxes):",
                sp.display(),
                data.len(),
                texts.len()
            );
            for (off, needle) in hits {
                let owner = texts
                    .iter()
                    .find(|(s, e, _)| off >= *s && off < *e)
                    .map(|(_, _, t)| t.replace('\n', "\\n"));
                println!("    @{off:#06x} {needle}  igTextBox={owner:?}");
            }
        }
    }
    Ok(())
}
