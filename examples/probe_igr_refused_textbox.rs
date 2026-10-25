use std::io::Read;
fn main() {
    let arg = std::env::args().nth(1).unwrap();
    let mut cfb = cfb::CompoundFile::open(std::fs::File::open(&arg).unwrap()).unwrap();
    let mut data = Vec::new();
    cfb.open_stream("/Sheet6")
        .unwrap()
        .read_to_end(&mut data)
        .unwrap();
    match pid_parse::parsers::sheet_records::decode_igtextbox_at(&data, 13235) {
        Some(r) => println!(
            "DECODED: {:?} x={} y={} h={} anchor={:#04x}",
            r.text, r.trailing_double_1, r.trailing_double_2, r.letter_height_m, r.anchor_code
        ),
        None => {
            println!("REFUSED — walking sub-checks by hand:");
            let rec = &data[13235..13307];
            let p = &rec[6..]; // payload 66B
            println!(
                "btf={} payload={}",
                u32::from_le_bytes(rec[2..6].try_into().unwrap()),
                p.len()
            );
            let st = u16::from_le_bytes([p[18], p[19]]);
            println!("text_sub_type={st}");
            let cnt = u16::from_le_bytes([p[22], p[23]]);
            println!(
                "count={cnt} text={:?}",
                String::from_utf16_lossy(&[
                    u16::from_le_bytes([p[24], p[25]]),
                    u16::from_le_bytes([p[26], p[27]]),
                    u16::from_le_bytes([p[28], p[29]])
                ])
            );
            let tail = 22 + 2 + 2 * cnt as usize;
            let f = |o: usize| f64::from_le_bytes(p[tail + o..tail + o + 8].try_into().unwrap());
            println!("x={} y={} cos={} sin={}", f(0), f(8), f(16), f(24));
            println!("anchor={:#04x}", p[tail + 35]);
        }
    }
}
