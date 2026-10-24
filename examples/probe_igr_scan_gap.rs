use std::io::Read;
fn main() {
    let arg = std::env::args().nth(1).unwrap();
    let mut cfb = cfb::CompoundFile::open(std::fs::File::open(&arg).unwrap()).unwrap();
    let mut data = Vec::new();
    cfb.open_stream("/Sheet6")
        .unwrap()
        .read_to_end(&mut data)
        .unwrap();
    // every offset whose type word is 0x004D with plausible btf
    let mut raw_offsets = Vec::new();
    for i in 0..data.len() - 6 {
        if u16::from_le_bytes([data[i], data[i + 1]]) == 0x004D {
            let btf = u32::from_le_bytes(data[i + 2..i + 6].try_into().unwrap()) as usize;
            if (60..=4096).contains(&btf) && i + 6 + btf <= data.len() {
                raw_offsets.push((i, i + 6 + btf));
            }
        }
    }
    println!("{} raw 0x004D candidates", raw_offsets.len());
    let decoded = pid_parse::parsers::sheet_records::decode_igtextboxes(&data);
    let ranges: Vec<_> = decoded.iter().map(|r| r.byte_range.clone()).collect();
    for (s, e) in &raw_offsets {
        let hit = ranges.iter().any(|r| r.start == *s);
        if !hit {
            // which decoded record overlaps it?
            let swallower: Vec<_> = ranges
                .iter()
                .filter(|r| r.start < *s && r.end > *s)
                .collect();
            println!("MISSING @{s:#06x}..{e:#06x}  swallowed_by={swallower:?}");
        }
    }
}
