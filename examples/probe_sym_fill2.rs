//! For one .sym: walk each Sheet chain the way `read_sheet_chain` does and
//! report, per record, the type code, its style-index word, and what that
//! index resolves to via `resolve_line_style` and `resolve_fill`.

use pid_parse::style_link::DocumentStyleTable;
use pid_parse::PidParser;
use std::path::Path;

fn u32le(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}
fn u16le(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}

fn main() {
    for arg in std::env::args().skip(1) {
        let path = Path::new(&arg);
        let package = PidParser::new().parse_package(path).expect("package");
        println!("=== {}", path.file_name().unwrap().to_string_lossy());
        // style tables per storage
        let mut tables = std::collections::HashMap::new();
        for (stream_path, raw) in &package.streams {
            if stream_path.rsplit('/').next() == Some("StyleCluster") {
                let parent = stream_path
                    .rfind('/')
                    .map_or("", |c| &stream_path[..c])
                    .to_string();
                tables.insert(
                    parent,
                    DocumentStyleTable::from_stylecluster_bytes(&raw.data),
                );
            }
        }
        for (stream_path, raw) in &package.streams {
            let name = stream_path.rsplit('/').next().unwrap_or("");
            if !name.starts_with("Sheet") {
                continue;
            }
            let parent = stream_path
                .rfind('/')
                .map_or("", |c| &stream_path[..c])
                .to_string();
            let table = tables.get(&parent);
            let bytes = &raw.data;
            if bytes.len() < 8 || u32le(bytes, 0) != 0x6C90_F544 {
                continue;
            }
            let mut at = 8usize;
            while at + 6 <= bytes.len() {
                let raw_type = u16le(bytes, at);
                let length = u32le(bytes, at + 2) as usize;
                if raw_type == 0 {
                    break;
                }
                let body = at + 6;
                let end = body + length;
                if end > bytes.len() {
                    break;
                }
                let tc = raw_type & 0x0FFF;
                let p = &bytes[body..end];
                if p.len() >= 18 {
                    let idx = u32le(p, 14);
                    let line = table.and_then(|t| t.resolve_line_style(idx));
                    let fill = table.and_then(|t| t.resolve_fill(idx));
                    if tc == 0x13 || tc == 0x59 || fill.is_some() {
                        println!(
                            "  {name}@{at} tc=0x{tc:03X} oid={} idx={idx} line={} fill={:?}",
                            u32le(p, 0),
                            line.is_some(),
                            fill.map(|f| (f.style_id, f.type_code, f.colour))
                        );
                    }
                }
                at = end;
            }
        }
    }
}
