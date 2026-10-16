//! Which rule refuses the five real drawings' `igSymbol2d` records?
//! 5 records total (3 in I井场, 1 in 分离器B, 1 in 低温分离器A). Mirrors the
//! shipped `IgSymbol2dDecoder` rules in order.

use std::io::Read;
use std::path::{Path, PathBuf};

use pid_parse::parsers::sheet_records::{
    decode_igsymbol_at, parse_live_psm_header, sheet_record_starts, PSM_TYPE_CODE_IGSYMBOL2D,
};

const FILES: &[&str] = &[
    r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid",
    r"D:\work\cad\PID自研软件验证\集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图.pid",
    r"D:\work\cad\PID自研软件验证\集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图.pid",
];

const MIN_PAYLOAD: usize = 113;
const MAX_PAYLOAD: usize = 200;
const MATRIX_TAG: [u8; 4] = [0x02, 0x00, 0xA7, 0x50];
const TAG_SEARCH_END: usize = 64;

fn read_u16(p: &[u8], at: usize) -> Option<u16> {
    p.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]))
}
fn read_u32(p: &[u8], at: usize) -> Option<u32> {
    p.get(at..at + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}
fn read_f64(p: &[u8], at: usize) -> Option<f64> {
    p.get(at..at + 8)
        .map(|b| f64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]))
}

fn sheet_streams(path: &Path) -> Vec<(String, Vec<u8>)> {
    let Ok(file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let Ok(mut cfb) = cfb::CompoundFile::open(file) else {
        return Vec::new();
    };
    let names: Vec<PathBuf> = cfb
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|entry| entry.path().to_path_buf())
        .collect();
    let mut out = Vec::new();
    for name in names {
        let Ok(mut stream) = cfb.open_stream(&name) else {
            continue;
        };
        let mut bytes = Vec::new();
        if stream.read_to_end(&mut bytes).is_ok() {
            out.push((name.to_string_lossy().into_owned(), bytes));
        }
    }
    out
}

fn main() {
    for file in FILES {
        let path = Path::new(file);
        println!(
            "\n========== {}",
            path.file_name().unwrap().to_string_lossy()
        );
        for (name, bytes) in sheet_streams(path) {
            let starts = sheet_record_starts(&bytes);
            if starts.is_empty() {
                continue;
            }
            for off in starts {
                let Some(header) = parse_live_psm_header(&bytes, off) else {
                    continue;
                };
                if header.type_code != PSM_TYPE_CODE_IGSYMBOL2D {
                    continue;
                }
                if decode_igsymbol_at(&bytes, off).is_some() {
                    continue;
                }
                let btf = header.bytes_to_follow as usize;
                print!("  {name} @{off:#07x}: btf={btf}");
                let Some(end) = header.body_start.checked_add(btf) else {
                    println!("  -> overflow");
                    continue;
                };
                if end > bytes.len() {
                    println!("  -> truncated");
                    continue;
                }
                if !(MIN_PAYLOAD..=MAX_PAYLOAD).contains(&btf) {
                    println!("  -> btf-outside-[{MIN_PAYLOAD},{MAX_PAYLOAD}]");
                }
                let payload = &bytes[header.body_start..end];
                let sub_type_word = read_u16(payload, 12).unwrap_or_default();
                let tag_at = payload
                    .get(..TAG_SEARCH_END.min(payload.len()))
                    .and_then(|p| p.windows(MATRIX_TAG.len()).position(|w| w == MATRIX_TAG));
                match tag_at {
                    None => {
                        println!(
                            "  -> matrix-tag-missing sub_type_word=0x{sub_type_word:04X} head={:02X?}",
                            &payload[..payload.len().min(40)]
                        );
                        // Hunt for the tag anywhere in the payload.
                        if let Some(at) = payload
                            .windows(MATRIX_TAG.len())
                            .position(|w| w == MATRIX_TAG)
                        {
                            println!("     (tag found later at +{at})");
                        }
                    }
                    Some(t) => {
                        let matrix_at = t + 4;
                        let doubles: Vec<f64> = (0..6)
                            .map(|i| read_f64(payload, matrix_at + i * 8).unwrap_or(f64::NAN))
                            .collect();
                        println!(
                            "  -> tag@+{t} doubles={doubles:.4?} sub_type_word=0x{sub_type_word:04X} tail_u32s=[{:?},{:?}]",
                            read_u32(payload, payload.len() - 8),
                            read_u32(payload, payload.len() - 4)
                        );
                    }
                }
            }
        }
    }
}
