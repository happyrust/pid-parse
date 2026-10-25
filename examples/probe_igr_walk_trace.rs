use std::io::Read;
fn main() {
    let arg = std::env::args().nth(1).unwrap();
    let mut cfb = cfb::CompoundFile::open(std::fs::File::open(&arg).unwrap()).unwrap();
    let mut data = Vec::new();
    cfb.open_stream("/Sheet6")
        .unwrap()
        .read_to_end(&mut data)
        .unwrap();
    // emulate scan, log everything within 13100..13400
    let mut off = 0usize;
    while off + 74 <= data.len() {
        if let Some(r) = pid_parse::parsers::sheet_records::decode_igtextbox_at(&data, off) {
            let end = r.byte_range.end;
            if off > 13100 && off < 13400 || end > 13100 && off <= 13100 {
                println!("accept @{off:#06x}..{end:#06x} text={:?}", r.text);
            }
            off = end;
        } else {
            off += 1;
        }
    }
    // what is around 13235: the record just before
}
