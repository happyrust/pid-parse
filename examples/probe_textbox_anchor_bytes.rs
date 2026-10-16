//! Hex-dump the undecoded sub-field block (+18..30) and the 4-byte trailer
//! after the direction pair, for the labels whose reference anchor differs
//! (bottom-left "MPa" boxes vs centre-anchored bubble texts).

use std::io::Read;

fn hex(b: &[u8]) -> String {
    b.iter()
        .map(|x| format!("{x:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn main() {
    let file =
        r"D:\work\cad\PID自研软件验证\集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图.pid";
    let f = std::fs::File::open(file).unwrap();
    let mut cfb = cfb::CompoundFile::open(f).unwrap();
    let mut data = Vec::new();
    cfb.open_stream("/Sheet6")
        .unwrap()
        .read_to_end(&mut data)
        .unwrap();

    let records = pid_parse::parsers::sheet_records::decode_igtextboxes(&data);
    let want: &[&str] = &[
        "L=0.8MPa ",
        "H=2.4MPa ",
        " 0112101A",
        "PIA",
        "PT",
        "1/2\"",
        "NC",
        "2.45MPa",
        "SP:",
        "GLV 0112102A",
        "PSV0112101A",
    ];
    for r in &records {
        if !want.contains(&r.text.as_str()) {
            continue;
        }
        // Full record bytes.
        let raw = &data[r.byte_range.clone()];
        println!(
            "oid={} idx={} sub={} btf={} text={:?}",
            r.oid, r.index, r.text_sub_type, r.bytes_to_follow, r.text
        );
        println!("  raw: {}", hex(raw));
    }
}
