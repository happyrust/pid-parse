use std::io::Read;
fn main() {
    let arg = std::env::args().nth(1).unwrap();
    let f = std::fs::File::open(&arg).unwrap();
    let mut cfb = cfb::CompoundFile::open(f).unwrap();
    let paths: Vec<_> = cfb
        .walk()
        .filter(|e| e.is_stream())
        .map(|e| e.path().to_path_buf())
        .collect();
    let pats: Vec<(Vec<u8>, &str)> = vec![
        (
            "igr".encode_utf16().flat_map(|u| u.to_le_bytes()).collect(),
            "u16-igr",
        ),
        (
            "IGR".encode_utf16().flat_map(|u| u.to_le_bytes()).collect(),
            "u16-IGR",
        ),
        (
            "CPE".encode_utf16().flat_map(|u| u.to_le_bytes()).collect(),
            "u16-CPE",
        ),
        (b"igr".to_vec(), "a-igr"),
        (b"IGR".to_vec(), "a-IGR"),
        (b"CPE".to_vec(), "a-CPE"),
        (
            "模板"
                .encode_utf16()
                .flat_map(|u| u.to_le_bytes())
                .collect(),
            "u16-tpl-cn",
        ),
    ];
    for p in &paths {
        let ps = p.to_string_lossy().into_owned();
        let mut d = Vec::new();
        cfb.open_stream(p).unwrap().read_to_end(&mut d).unwrap();
        for (pat, name) in &pats {
            for i in 0..d.len().saturating_sub(pat.len()) {
                if d[i..i + pat.len()] == pat[..] {
                    let s0 = i.saturating_sub(30);
                    let s1 = (i + pat.len() + 30).min(d.len());
                    let ctx = String::from_utf16_lossy(
                        &d[s0..s1]
                            .as_chunks::<2>()
                            .0
                            .iter()
                            .map(|c| u16::from_le_bytes(*c))
                            .collect::<Vec<_>>(),
                    );
                    println!("{ps} {name}@{i:#x} ctx={}", ctx.replace('\0', "·"));
                    break;
                }
            }
        }
    }
}
